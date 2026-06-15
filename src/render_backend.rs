//! GPU and CPU render backends for LIME DE.
//!
//! This module provides a common `RenderBackend` trait with two implementations:
//! - `SoftbufferBackend`: CPU-based framebuffer rendering (fallback)
//! - `WgpuBackend`: GPU-accelerated rendering via wgpu (default)

use std::num::NonZeroU32;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::render::{RenderColor, RenderRect, RenderSceneFrame, RenderRoundedRect};

/// Common error type for render backends.
#[derive(Debug, Clone)]
pub struct RenderError(pub String);

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RenderError {}

impl From<String> for RenderError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A rectangle to be rendered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRectDesc {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub color: [f32; 4],
    pub radius: i32,
}

impl From<RenderRect> for RenderRectDesc {
    fn from(rect: RenderRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            color: [rect.color.red, rect.color.green, rect.color.blue, rect.color.alpha],
            radius: 0,
        }
    }
}

/// Common trait for render backends.
pub trait RenderBackendTrait {
    /// Resize the backend surface.
    fn resize(&mut self, width: u32, height: u32);

    /// Render a scene frame.
    fn render(&mut self, scene: &RenderSceneFrame) -> Result<(), RenderError>;
}

/// Backend kind selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Softbuffer,
    Wgpu,
}

// ============================================================================
// Softbuffer Backend (CPU fallback)
// ============================================================================

use softbuffer::{Context, Surface};

pub struct SoftbufferBackend {
    window: Arc<Window>,
    context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    framebuffer_size: Option<(u32, u32)>,
}

impl SoftbufferBackend {
    pub fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let context = Context::new(Arc::clone(&window))
            .map_err(|e| RenderError(format!("failed to create softbuffer context: {e}")))?;
        let surface = Surface::new(&context, Arc::clone(&window))
            .map_err(|e| RenderError(format!("failed to create softbuffer surface: {e}")))?;

        Ok(Self {
            window,
            context,
            surface,
            framebuffer_size: None,
        })
    }

    fn draw_commands(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        scene: &RenderSceneFrame,
    ) {
        // Draw rounded rectangles
        for rect in &scene.rounded_rectangles {
            Self::draw_rounded_rectangle(
                buffer,
                framebuffer_width,
                framebuffer_height,
                *rect,
            );
        }

        // Draw simple rectangles
        for rect in &scene.rectangles {
            Self::draw_rectangle(buffer, framebuffer_width, framebuffer_height, *rect);
        }

        // Draw circles
        for circle in &scene.circles {
            Self::draw_circle(buffer, framebuffer_width, framebuffer_height, *circle);
        }

        // Draw images
        for image in &scene.images {
            Self::draw_image(buffer, framebuffer_width, framebuffer_height, image);
        }

        // Draw text
        for text in &scene.text {
            Self::draw_text(buffer, framebuffer_width, framebuffer_height, text);
        }
    }

    fn draw_rectangle(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        rect: RenderRect,
    ) {
        let x0 = rect.x.max(0) as u32;
        let y0 = rect.y.max(0) as u32;
        let x1 = (rect.x + rect.width).max(0) as u32;
        let y1 = (rect.y + rect.height).max(0) as u32;

        let x0 = x0.min(framebuffer_width);
        let y0 = y0.min(framebuffer_height);
        let x1 = x1.min(framebuffer_width);
        let y1 = y1.min(framebuffer_height);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        let color = rect.color.to_argb_u32();
        let stride = framebuffer_width as usize;

        for y in y0 as usize..y1 as usize {
            let row_start = y * stride;
            for x in x0 as usize..x1 as usize {
                buffer[row_start + x] = color;
            }
        }
    }

    fn draw_rounded_rectangle(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        rect: RenderRoundedRect,
    ) {
        // Simplified: just draw as regular rectangle for now
        // Full rounded rect would need proper corner rendering
        let render_rect = RenderRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            color: rect.color,
        };
        Self::draw_rectangle(buffer, framebuffer_width, framebuffer_height, render_rect);
    }

    fn draw_circle(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        circle: crate::render::RenderCircle,
    ) {
        let cx = circle.x;
        let cy = circle.y;
        let r = circle.diameter / 2;
        let r_sq = r * r;
        let color = circle.color.to_argb_u32();
        let stride = framebuffer_width as usize;

        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r_sq {
                    let x = cx + dx;
                    let y = cy + dy;
                    if x >= 0 && y >= 0 && x < framebuffer_width as i32 && y < framebuffer_height as i32 {
                        let idx = y as usize * stride + x as usize;
                        buffer[idx] = color;
                    }
                }
            }
        }
    }

    fn draw_image(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        image: &crate::render::RenderImage,
    ) {
        let src_width = image.width;
        let src_height = image.height;
        let dst_width = image.draw_width;
        let dst_height = image.draw_height;

        if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
            return;
        }

        let stride = framebuffer_width as usize;

        for dy in 0..dst_height {
            for dx in 0..dst_width {
                let sx = (dx as f32 / dst_width as f32 * src_width as f32) as u32;
                let sy = (dy as f32 / dst_height as f32 * src_height as f32) as u32;
                let sx = sx.min(src_width - 1);
                let sy = sy.min(src_height - 1);

                let px = image.x + dx as i32;
                let py = image.y + dy as i32;

                if px < 0 || py < 0 || px >= framebuffer_width as i32 || py >= framebuffer_height as i32 {
                    continue;
                }

                let src_idx = sy as usize * src_width as usize + sx as usize;
                let dst_idx = py as usize * stride + px as usize;

                if src_idx < image.pixels_argb.len() {
                    buffer[dst_idx] = image.pixels_argb[src_idx];
                }
            }
        }
    }

    fn draw_text(
        buffer: &mut [u32],
        framebuffer_width: u32,
        framebuffer_height: u32,
        text: &crate::render::RenderText,
    ) {
        let color = text.color;
        let scale = 2;
        let stride = framebuffer_width as usize;

        for (row, ch) in text.text.chars().enumerate() {
            let glyph = glyph_rows(ch);
            let x = text.x + row as i32 * 6 * scale;
            let y = text.y;

            for (glyph_row, bits) in glyph.iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) == 0 {
                        continue;
                    }

                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = x + col * scale + dx;
                            let py = y + glyph_row as i32 * scale + dy;

                            if px < 0 || py < 0 {
                                continue;
                            }

                            let px = px as u32;
                            let py = py as u32;
                            if px >= framebuffer_width || py >= framebuffer_height {
                                continue;
                            }

                            buffer[py as usize * stride + px as usize] = color;
                        }
                    }
                }
            }
        }
    }
}

fn glyph_rows(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10000, 0b10011, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110],
        '6' => [0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '_' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111],
        ':' => [0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '@' => [0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110],
        '~' => [0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000],
        ' ' => [0; 7],
        _ => [0b11111, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100],
    }
}

impl RenderBackendTrait for SoftbufferBackend {
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        if let (Some(nw), Some(nh)) = (NonZeroU32::new(width), NonZeroU32::new(height)) {
            let _ = self.surface.resize(nw, nh);
            self.framebuffer_size = Some((width, height));
        }
    }

    fn render(&mut self, scene: &RenderSceneFrame) -> Result<(), RenderError> {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        let (width, height) = (size.width, size.height);
        self.resize(width, height);

        let mut buffer = self.surface.buffer_mut()
            .map_err(|e| RenderError(format!("failed to acquire buffer: {e}")))?;

        // Clear with background color
        let clear_color = scene.clear_color.to_argb_u32();
        buffer.fill(clear_color);

        // Draw all commands
        Self::draw_commands(&mut buffer, width, height, scene);

        // Draw cursor rects
        for cursor in &scene.cursor {
            Self::draw_rectangle(&mut buffer, width, height, *cursor);
        }

        buffer.present()
            .map_err(|e| RenderError(format!("failed to present buffer: {e}")))?;

        Ok(())
    }
}

// ============================================================================
// WGPU Backend (GPU accelerated)
// ============================================================================

#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct Uniforms {
    resolution: [f32; 2],
}

pub struct WgpuBackend {
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    depth_texture: Option<wgpu::TextureView>,
    width: u32,
    height: u32,
}

impl WgpuBackend {
    pub async fn new(window: Arc<Window>) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)
            .map_err(|e| RenderError(format!("failed to create wgpu surface: {e}")))?;

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.ok_or_else(|| RenderError("no suitable wgpu adapter found".to_string()))?;

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("lime-de device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
            },
            None,
        ).await.map_err(|e| RenderError(format!("failed to create wgpu device: {e}")))?;

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lime-de shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                resolution: [width as f32, height as f32],
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group layout and bind group
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::CreateBindGroupDescriptor {
            label: Some("bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create render pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2,
                        1 => Float32x4,
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buffer,
            bind_group,
            depth_texture: None,
            width,
            height,
        })
    }

    fn create_depth_texture(&mut self) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth texture"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        self.depth_texture = Some(texture.create_view(&wgpu::TextureViewDescriptor::default()));
    }

    fn generate_rect_vertices(&self, scene: &RenderSceneFrame) -> Vec<Vertex> {
        let mut vertices = Vec::new();

        // Background rectangle (full screen)
        let bg_color = scene.clear_color;
        vertices.extend_from_slice(&[
            Vertex { position: [-1.0, -1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
            Vertex { position: [1.0, -1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
            Vertex { position: [1.0, 1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
            Vertex { position: [-1.0, -1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
            Vertex { position: [1.0, 1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
            Vertex { position: [-1.0, 1.0], color: [bg_color.red, bg_color.green, bg_color.blue, bg_color.alpha] },
        ]);

        // Add rectangles from scene
        for rect in &scene.rounded_rectangles {
            let x0 = rect.x as f32 / (self.width as f32 / 2.0) - 1.0;
            let y0 = -(rect.y as f32 / (self.height as f32 / 2.0) - 1.0);
            let x1 = (rect.x + rect.width) as f32 / (self.width as f32 / 2.0) - 1.0;
            let y1 = -(rect.y + rect.height) as f32 / (self.height as f32 / 2.0) - 1.0;
            let color = [rect.color.red, rect.color.green, rect.color.blue, rect.color.alpha];

            vertices.extend_from_slice(&[
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y1], color },
            ]);
        }

        for rect in &scene.rectangles {
            let x0 = rect.x as f32 / (self.width as f32 / 2.0) - 1.0;
            let y0 = -(rect.y as f32 / (self.height as f32 / 2.0) - 1.0);
            let x1 = (rect.x + rect.width) as f32 / (self.width as f32 / 2.0) - 1.0;
            let y1 = -(rect.y + rect.height) as f32 / (self.height as f32 / 2.0) - 1.0;
            let color = [rect.color.red, rect.color.green, rect.color.blue, rect.color.alpha];

            vertices.extend_from_slice(&[
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y1], color },
            ]);
        }

        // Add circles (approximated as quads for simplicity)
        for circle in &scene.circles {
            let cx = circle.x as f32;
            let cy = circle.y as f32;
            let r = circle.diameter as f32 / 2.0;
            let color = [circle.color.red, circle.color.green, circle.color.blue, circle.color.alpha];

            // Simple quad approximation
            let x0 = (cx - r) as f32 / (self.width as f32 / 2.0) - 1.0;
            let y0 = -(cy - r) as f32 / (self.height as f32 / 2.0) - 1.0;
            let x1 = (cx + r) as f32 / (self.width as f32 / 2.0) - 1.0;
            let y1 = -(cy + r) as f32 / (self.height as f32 / 2.0) - 1.0;

            vertices.extend_from_slice(&[
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y0], color },
                Vertex { position: [x1, y1], color },
                Vertex { position: [x0, y1], color },
            ]);
        }

        vertices
    }
}

impl RenderBackendTrait for WgpuBackend {
    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.width = width;
        self.height = height;

        self.config.width = width;
        self.config.height = height;

        self.surface.configure(&self.device, &self.config);
        self.create_depth_texture();

        // Update uniform buffer
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms {
                resolution: [width as f32, height as f32],
            }]),
        );
    }

    fn render(&mut self, scene: &RenderSceneFrame) -> Result<(), RenderError> {
        let size = self.surface.get_current_texture()
            .map_err(|e| RenderError(format!("failed to get surface texture: {e}")))?
            .texture
            .size();

        if size.width != self.width || size.height != self.height {
            self.resize(size.width, size.height);
        }

        let frame = self.surface.get_current_texture()
            .map_err(|e| RenderError(format!("failed to acquire next frame: {e}")))?;

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Generate vertices from scene
        let vertices = self.generate_rect_vertices(scene);
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: scene.clear_color.red as f64,
                            g: scene.clear_color.green as f64,
                            b: scene.clear_color.blue as f64,
                            a: scene.clear_color.alpha as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }
}

// ============================================================================
// Unified Backend Wrapper
// ============================================================================

pub enum UnifiedRenderBackend {
    Softbuffer(SoftbufferBackend),
    Wgpu(WgpuBackend),
}

impl UnifiedRenderBackend {
    pub async fn new(window: Arc<Window>, kind: BackendKind) -> Result<Self, RenderError> {
        match kind {
            BackendKind::Wgpu => {
                match WgpuBackend::new(Arc::clone(&window)).await {
                    Ok(backend) => {
                        eprintln!("WGPU backend initialized successfully");
                        Ok(Self::Wgpu(backend))
                    }
                    Err(e) => {
                        eprintln!("WGPU backend failed: {e}, falling back to softbuffer");
                        let softbuffer = SoftbufferBackend::new(window)?;
                        Ok(Self::Softbuffer(softbuffer))
                    }
                }
            }
            BackendKind::Softbuffer => {
                let backend = SoftbufferBackend::new(window)?;
                Ok(Self::Softbuffer(backend))
            }
        }
    }

    pub fn try_wgpu_first(window: Arc<Window>) -> impl futures_intrusive::future::SharedFutureSlice<Output = Result<Self, RenderError>> + Send {
        // This is a helper for async initialization
        // In practice, we'll initialize synchronously in the compositor
        unimplemented!()
    }
}

impl RenderBackendTrait for UnifiedRenderBackend {
    fn resize(&mut self, width: u32, height: u32) {
        match self {
            Self::Softbuffer(backend) => backend.resize(width, height),
            Self::Wgpu(backend) => backend.resize(width, height),
        }
    }

    fn render(&mut self, scene: &RenderSceneFrame) -> Result<(), RenderError> {
        match self {
            Self::Softbuffer(backend) => backend.render(scene),
            Self::Wgpu(backend) => backend.render(scene),
        }
    }
}
