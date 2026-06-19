//! Native KMS output creation and rendering of the shared core scene.

use std::io;

use smithay::{
    backend::{
        allocator::{
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            compositor::{FrameFlags, PrimaryPlaneElement},
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd,
        },
        renderer::element::{
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            render_elements, Kind,
        },
        renderer::gles::GlesRenderer,
    },
    output::OutputModeSource,
    reexports::drm::control::{connector, crtc, Device as ControlDevice, Mode, ModeTypeFlags},
    utils::{Scale, Transform},
};

use super::gles::GlesState;
use crate::{core::rasterizer::draw_scene, render::RenderSceneFrame};

type NativeAllocator = GbmAllocator<DrmDeviceFd>;
type NativeExporter = GbmFramebufferExporter<DrmDeviceFd>;
type NativeDrmOutputManager = DrmOutputManager<NativeAllocator, NativeExporter, (), DrmDeviceFd>;
type NativeDrmOutput = DrmOutput<NativeAllocator, NativeExporter, (), DrmDeviceFd>;

render_elements! {
    NativeSceneElement<=GlesRenderer>;
    Memory=MemoryRenderBufferRenderElement<GlesRenderer>,
}

impl std::fmt::Debug for NativeSceneElement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Memory(element) => formatter.debug_tuple("Memory").field(element).finish(),
            Self::_GenericCatcher(element) => formatter
                .debug_tuple("_GenericCatcher")
                .field(element)
                .finish(),
        }
    }
}

#[derive(Debug)]
pub struct NativeOutputState {
    _manager: NativeDrmOutputManager,
    output: NativeDrmOutput,
    crtc: crtc::Handle,
    connector_name: String,
    size: (u32, u32),
    vblank_count: u64,
    scene_pixels: Vec<u32>,
    scene_buffer: MemoryRenderBuffer,
}

impl NativeOutputState {
    pub fn new(
        device: DrmDevice,
        gbm: GbmDevice<DrmDeviceFd>,
        gles: &mut GlesState,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (connector, mode, crtc) = select_output(&device)?;
        let connector_name = format!(
            "{}-{}",
            connector.interface().as_str(),
            connector.interface_id()
        );
        let (width, height) = mode.size();

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let exporter = GbmFramebufferExporter::new(gbm.clone(), None);
        let renderer_formats = matching_renderer_formats(gles);
        if renderer_formats.iter().next().is_none() {
            return Err(io::Error::other(
                "[native] GLES exposes no Xrgb8888/Argb8888 render formats for KMS",
            )
            .into());
        }

        let mut manager = DrmOutputManager::new(
            device,
            allocator,
            exporter,
            Some(gbm),
            [Fourcc::Xrgb8888, Fourcc::Argb8888],
            renderer_formats,
        );

        let mode_source = OutputModeSource::Static {
            size: (i32::from(width), i32::from(height)).into(),
            scale: Scale::from(1.0),
            transform: Transform::Normal,
        };
        let planes = manager.device().planes(&crtc).map_err(|error| {
            io::Error::other(format!(
                "[native] failed to query planes for {crtc:?}: {error}"
            ))
        })?;
        let render_elements = DrmOutputRenderElements::<_, NativeSceneElement>::default();
        let output = manager
            .initialize_output(
                crtc,
                mode,
                &[connector.handle()],
                mode_source,
                Some(planes),
                gles.renderer_mut(),
                &render_elements,
            )
            .map_err(|error| {
                io::Error::other(format!(
                    "[native] failed to create DRM output {connector_name}: {error}"
                ))
            })?;

        println!(
            "[native] output created: {connector_name} {}x{}@{}",
            width,
            height,
            mode.vrefresh()
        );
        println!("[native] scanout format: {}", output.format());

        Ok(Self {
            _manager: manager,
            output,
            crtc,
            connector_name,
            size: (u32::from(width), u32::from(height)),
            vblank_count: 0,
            scene_pixels: vec![0; usize::from(width) * usize::from(height)],
            scene_buffer: MemoryRenderBuffer::new(
                Fourcc::Argb8888,
                (i32::from(width), i32::from(height)),
                1,
                Transform::Normal,
                None,
            ),
        })
    }

    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn submit_scene_frame(
        &mut self,
        gles: &mut GlesState,
        frame: &RenderSceneFrame,
    ) -> Result<(), Box<dyn std::error::Error>> {
        draw_scene(&mut self.scene_pixels, self.size.0, self.size.1, frame);
        let size = self.size;
        self.scene_buffer
            .render()
            .draw(|bytes| {
                for (destination, pixel) in bytes.chunks_exact_mut(4).zip(&self.scene_pixels) {
                    destination.copy_from_slice(&pixel.to_ne_bytes());
                }
                Ok::<_, std::convert::Infallible>(vec![smithay::utils::Rectangle::from_size(
                    (size.0 as i32, size.1 as i32).into(),
                )])
            })
            .expect("infallible native scene buffer update");
        let element = MemoryRenderBufferRenderElement::from_buffer(
            gles.renderer_mut(),
            (0.0, 0.0),
            &self.scene_buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        )?;
        let elements = vec![NativeSceneElement::from(element)];
        let result = self
            .output
            .render_frame(
                gles.renderer_mut(),
                &elements,
                color_array(frame.clear_color),
                FrameFlags::empty(),
            )
            .map_err(|error| {
                io::Error::other(format!(
                    "[native] failed to render frame for {}: {error}",
                    self.connector_name
                ))
            })?;

        if result.needs_sync() {
            if let PrimaryPlaneElement::Swapchain(element) = &result.primary_element {
                element.sync.wait().map_err(|error| {
                    io::Error::other(format!(
                        "[native] failed to synchronize GLES frame for {}: {error}",
                        self.connector_name
                    ))
                })?;
            }
        }
        let is_empty = result.is_empty;
        drop(result);

        if is_empty {
            return Err(io::Error::other(format!(
                "[native] renderer produced an empty frame for {}",
                self.connector_name
            ))
            .into());
        }

        self.output.queue_frame(()).map_err(|error| {
            io::Error::other(format!(
                "[native] failed to submit frame for {}: {error}",
                self.connector_name
            ))
            .into()
        })
    }

    pub fn handle_vblank(
        &mut self,
        crtc: crtc::Handle,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if crtc != self.crtc {
            return Ok(false);
        }

        self.output.frame_submitted().map_err(|error| {
            io::Error::other(format!(
                "[native] failed to finish frame on {}: {error}",
                self.connector_name
            ))
        })?;
        self.vblank_count = self.vblank_count.wrapping_add(1);
        if self.vblank_count == 1 {
            println!("[native] pageflip/vblank");
        }
        Ok(true)
    }
}

fn color_array(color: crate::render::RenderColor) -> [f32; 4] {
    [color.red, color.green, color.blue, color.alpha]
}

fn select_output(
    device: &DrmDevice,
) -> Result<(connector::Info, Mode, crtc::Handle), Box<dyn std::error::Error>> {
    let resources = device.resource_handles().map_err(|error| {
        io::Error::other(format!("[native] failed to read DRM resources: {error}"))
    })?;

    for connector_handle in resources.connectors() {
        let connector = device
            .get_connector(*connector_handle, true)
            .map_err(|error| {
                io::Error::other(format!(
                    "[native] failed to read connector {connector_handle:?}: {error}"
                ))
            })?;
        if connector.state() != connector::State::Connected || connector.modes().is_empty() {
            continue;
        }

        let mode = connector
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .or_else(|| connector.modes().first())
            .copied()
            .expect("connected connector with modes should have a selected mode");

        let current_crtc = connector
            .current_encoder()
            .and_then(|handle| device.get_encoder(handle).ok())
            .and_then(|encoder| encoder.crtc());
        let crtc = current_crtc.or_else(|| {
            connector.encoders().iter().find_map(|handle| {
                let encoder = device.get_encoder(*handle).ok()?;
                resources
                    .filter_crtcs(encoder.possible_crtcs())
                    .first()
                    .copied()
            })
        });

        if let Some(crtc) = crtc {
            return Ok((connector, mode, crtc));
        }
    }

    Err(
        io::Error::other("[native] no connected connector with a compatible mode and CRTC found")
            .into(),
    )
}

fn matching_renderer_formats(gles: &GlesState) -> FormatSet {
    gles.renderer()
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .filter(|format| matches!(format.code, Fourcc::Xrgb8888 | Fourcc::Argb8888))
        .copied()
        .collect()
}
