use std::{
    fmt,
    num::NonZeroU32,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

use crate::{
    error::AppError,
    render::{
        RenderCircle, RenderCommand, RenderImage, RenderRect, RenderRoundedRect, RenderSceneFrame,
        RenderText,
    },
};

#[derive(Debug, Default)]
pub struct BackendState;

pub struct WinitBackend {
    proxy: EventLoopProxy<WinitBackendEvent>,
    output_rx: Receiver<WinitBackendOutputEvent>,
    current_size: Arc<Mutex<Option<(u32, u32)>>>,
    thread: Option<JoinHandle<()>>,
}

impl WinitBackend {
    pub fn new(width: u32, height: u32) -> Result<Self, AppError> {
        let (init_tx, init_rx) = mpsc::channel();
        let (output_tx, output_rx) = mpsc::channel();
        let current_size = Arc::new(Mutex::new(Some((width, height))));
        let app_current_size = Arc::clone(&current_size);

        let thread = thread::spawn(move || {
            let mut event_loop_builder = EventLoop::<WinitBackendEvent>::with_user_event();

            #[cfg(target_os = "linux")]
            {
                use winit::platform::wayland::EventLoopBuilderExtWayland;

                EventLoopBuilderExtWayland::with_any_thread(&mut event_loop_builder, true);
            }

            let event_loop = match event_loop_builder.build() {
                Ok(event_loop) => event_loop,
                Err(error) => {
                    let _ =
                        init_tx.send(Err(format!("failed to create winit event loop: {error}")));
                    return;
                }
            };
            let proxy = event_loop.create_proxy();
            let mut app = WinitBackendApp::new(
                width,
                height,
                proxy.clone(),
                init_tx,
                output_tx,
                app_current_size,
            );

            if let Err(error) = event_loop.run_app(&mut app) {
                eprintln!("LIME DE winit test backend failed: {error}");
            }
        });

        let proxy = init_rx
            .recv()
            .map_err(|error| {
                AppError::new(format!(
                    "failed to receive winit backend initialization result: {error}"
                ))
            })?
            .map_err(AppError::new)?;

        println!("Winit test backend initialized");

        Ok(Self {
            proxy,
            output_rx,
            current_size,
            thread: Some(thread),
        })
    }

    pub fn shutdown(&mut self) {
        let _ = self.proxy.send_event(WinitBackendEvent::Shutdown);

        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("LIME DE winit test backend thread panicked");
            }
        }
    }

    pub fn draw_frame(&self, frame: RenderSceneFrame) -> bool {
        self.proxy
            .send_event(WinitBackendEvent::DrawFrame(frame))
            .is_ok()
    }

    pub fn current_size(&self) -> Option<(u32, u32)> {
        self.current_size.lock().ok().and_then(|size| *size)
    }

    pub fn poll_events(&self) -> Vec<WinitBackendOutputEvent> {
        let mut events = Vec::new();

        while let Ok(event) = self.output_rx.try_recv() {
            events.push(event);
        }

        events
    }
}

impl fmt::Debug for WinitBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WinitBackend")
            .field("running", &self.thread.is_some())
            .finish()
    }
}

impl Drop for WinitBackend {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[derive(Debug, Clone)]
enum WinitBackendEvent {
    DrawFrame(RenderSceneFrame),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WinitBackendOutputEvent {
    MouseMoved {
        x: f64,
        y: f64,
    },
    MouseButton {
        button: WinitMouseButton,
        pressed: bool,
    },
    Keyboard {
        keycode: u32,
        pressed: bool,
    },
    Resized {
        width: u32,
        height: u32,
    },
    FramePresented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinitMouseButton {
    Left,
    Middle,
    Right,
}

struct WinitBackendApp {
    width: u32,
    height: u32,
    proxy: EventLoopProxy<WinitBackendEvent>,
    output_tx: Sender<WinitBackendOutputEvent>,
    current_size: Arc<Mutex<Option<(u32, u32)>>>,
    init_tx: Option<mpsc::Sender<Result<EventLoopProxy<WinitBackendEvent>, String>>>,
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    pending_frame: RenderSceneFrame,
    first_present_done: bool,
    first_real_buffer_done: bool,
    framebuffer_size: Option<(u32, u32)>,
}

impl WinitBackendApp {
    fn new(
        width: u32,
        height: u32,
        proxy: EventLoopProxy<WinitBackendEvent>,
        init_tx: mpsc::Sender<Result<EventLoopProxy<WinitBackendEvent>, String>>,
        output_tx: Sender<WinitBackendOutputEvent>,
        current_size: Arc<Mutex<Option<(u32, u32)>>>,
    ) -> Self {
        Self {
            width,
            height,
            proxy,
            output_tx,
            current_size,
            init_tx: Some(init_tx),
            window: None,
            context: None,
            surface: None,
            pending_frame: RenderSceneFrame::new(crate::render::RenderColor::black()),
            first_present_done: false,
            first_real_buffer_done: false,
            framebuffer_size: None,
        }
    }

    fn send_init_result(&mut self, result: Result<EventLoopProxy<WinitBackendEvent>, String>) {
        if let Some(init_tx) = self.init_tx.take() {
            let _ = init_tx.send(result);
        }
    }

    fn set_current_size(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let Ok(mut current_size) = self.current_size.lock() else {
            return;
        };

        if *current_size != Some((width, height)) {
            *current_size = Some((width, height));
            println!("Winit backend resized: {width}x{height}");
        }
    }

    fn draw_frame(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let size = window.inner_size();
        self.set_current_size(size.width, size.height);
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        let framebuffer_size = (width.get(), height.get());
        if self.framebuffer_size != Some(framebuffer_size) {
            if let Err(error) = surface.resize(width, height) {
                eprintln!("LIME DE winit test backend resize failed: {error}");
                return;
            }
            self.framebuffer_size = Some(framebuffer_size);
        }

        let Ok(mut buffer) = surface.buffer_mut() else {
            eprintln!("LIME DE winit test backend could not acquire pixel buffer");
            return;
        };

        buffer.fill(self.pending_frame.clear_color.to_argb_u32());
        draw_commands(
            &mut buffer,
            width.get(),
            height.get(),
            &self.pending_frame.commands,
        );
        for cursor in &self.pending_frame.cursor {
            draw_rectangle(&mut buffer, width.get(), height.get(), *cursor);
        }

        if let Err(error) = buffer.present() {
            eprintln!("LIME DE winit test backend present failed: {error}");
            return;
        }

        let _ = self.output_tx.send(WinitBackendOutputEvent::FramePresented);

        if !self.first_present_done {
            println!("Winit framebuffer cleared to black");
            self.first_present_done = true;
        }
        if self.pending_frame.has_client_images() && !self.first_real_buffer_done {
            println!("Real client buffer rendered");
            self.first_real_buffer_done = true;
        }
    }
}

fn draw_commands(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    commands: &[RenderCommand],
) {
    for command in commands {
        match command {
            RenderCommand::RoundedRect(rectangle) => {
                draw_rounded_rectangle(buffer, framebuffer_width, framebuffer_height, *rectangle);
            }
            RenderCommand::Rect(rectangle) => {
                draw_rectangle(buffer, framebuffer_width, framebuffer_height, *rectangle);
            }
            RenderCommand::Circle(circle) => {
                draw_circle(buffer, framebuffer_width, framebuffer_height, *circle);
            }
            RenderCommand::Image(image) => {
                draw_image(buffer, framebuffer_width, framebuffer_height, image);
            }
            RenderCommand::Text(text) => {
                draw_text(buffer, framebuffer_width, framebuffer_height, text);
            }
        }
    }
}

impl ApplicationHandler<WinitBackendEvent> for WinitBackendApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("LIME DE Test Backend")
            .with_inner_size(LogicalSize::new(
                f64::from(self.width),
                f64::from(self.height),
            ));

        match event_loop.create_window(window_attributes) {
            Ok(window) => {
                let window = Arc::new(window);
                match Context::new(Arc::clone(&window)).and_then(|context| {
                    Surface::new(&context, Arc::clone(&window)).map(|surface| (context, surface))
                }) {
                    Ok((context, surface)) => {
                        let size = window.inner_size();
                        self.set_current_size(size.width, size.height);
                        self.context = Some(context);
                        self.surface = Some(surface);
                        self.window = Some(window);
                        self.send_init_result(Ok(self.proxy.clone()));
                    }
                    Err(error) => {
                        self.send_init_result(Err(format!(
                            "failed to create winit pixel framebuffer: {error}"
                        )));
                        event_loop.exit();
                    }
                }
            }
            Err(error) => {
                self.send_init_result(Err(format!("failed to create winit window: {error}")));
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: WinitBackendEvent) {
        match event {
            WinitBackendEvent::DrawFrame(frame) => {
                self.pending_frame = frame;
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WinitBackendEvent::Shutdown => event_loop.exit(),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.draw_frame(),
            WindowEvent::Resized(size) => {
                self.set_current_size(size.width, size.height);
                let _ = self.output_tx.send(WinitBackendOutputEvent::Resized {
                    width: size.width,
                    height: size.height,
                });
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let _ = self.output_tx.send(WinitBackendOutputEvent::MouseMoved {
                    x: position.x,
                    y: position.y,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = map_mouse_button(button) {
                    let _ = self.output_tx.send(WinitBackendOutputEvent::MouseButton {
                        button,
                        pressed: state == ElementState::Pressed,
                    });
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat {
                    if let Some(keycode) = map_keycode(event.physical_key) {
                        let _ = self.output_tx.send(WinitBackendOutputEvent::Keyboard {
                            keycode,
                            pressed: event.state == ElementState::Pressed,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn map_mouse_button(button: MouseButton) -> Option<WinitMouseButton> {
    match button {
        MouseButton::Left => Some(WinitMouseButton::Left),
        MouseButton::Middle => Some(WinitMouseButton::Middle),
        MouseButton::Right => Some(WinitMouseButton::Right),
        _ => None,
    }
}

fn map_keycode(physical_key: PhysicalKey) -> Option<u32> {
    let PhysicalKey::Code(code) = physical_key else {
        return None;
    };

    let keycode = match code {
        KeyCode::Escape => 1,
        KeyCode::Digit1 => 2,
        KeyCode::Digit2 => 3,
        KeyCode::Digit3 => 4,
        KeyCode::Digit4 => 5,
        KeyCode::Digit5 => 6,
        KeyCode::Digit6 => 7,
        KeyCode::Digit7 => 8,
        KeyCode::Digit8 => 9,
        KeyCode::Digit9 => 10,
        KeyCode::Digit0 => 11,
        KeyCode::Minus => 12,
        KeyCode::Equal => 13,
        KeyCode::Backspace => 14,
        KeyCode::Tab => 15,
        KeyCode::KeyQ => 16,
        KeyCode::KeyW => 17,
        KeyCode::KeyE => 18,
        KeyCode::KeyR => 19,
        KeyCode::KeyT => 20,
        KeyCode::KeyY => 21,
        KeyCode::KeyU => 22,
        KeyCode::KeyI => 23,
        KeyCode::KeyO => 24,
        KeyCode::KeyP => 25,
        KeyCode::BracketLeft => 26,
        KeyCode::BracketRight => 27,
        KeyCode::Enter => 28,
        KeyCode::ControlLeft => 29,
        KeyCode::KeyA => 30,
        KeyCode::KeyS => 31,
        KeyCode::KeyD => 32,
        KeyCode::KeyF => 33,
        KeyCode::KeyG => 34,
        KeyCode::KeyH => 35,
        KeyCode::KeyJ => 36,
        KeyCode::KeyK => 37,
        KeyCode::KeyL => 38,
        KeyCode::Semicolon => 39,
        KeyCode::Quote => 40,
        KeyCode::Backquote => 41,
        KeyCode::ShiftLeft => 42,
        KeyCode::Backslash => 43,
        KeyCode::KeyZ => 44,
        KeyCode::KeyX => 45,
        KeyCode::KeyC => 46,
        KeyCode::KeyV => 47,
        KeyCode::KeyB => 48,
        KeyCode::KeyN => 49,
        KeyCode::KeyM => 50,
        KeyCode::Comma => 51,
        KeyCode::Period => 52,
        KeyCode::Slash => 53,
        KeyCode::ShiftRight => 54,
        KeyCode::AltLeft => 56,
        KeyCode::Space => 57,
        KeyCode::CapsLock => 58,
        KeyCode::F1 => 59,
        KeyCode::F2 => 60,
        KeyCode::F3 => 61,
        KeyCode::F4 => 62,
        KeyCode::F5 => 63,
        KeyCode::F6 => 64,
        KeyCode::F7 => 65,
        KeyCode::F8 => 66,
        KeyCode::F9 => 67,
        KeyCode::F10 => 68,
        KeyCode::F11 => 87,
        KeyCode::F12 => 88,
        KeyCode::ControlRight => 97,
        KeyCode::SuperLeft => 125,
        KeyCode::SuperRight => 126,
        KeyCode::AltRight => 100,
        KeyCode::ArrowUp => 103,
        KeyCode::ArrowLeft => 105,
        KeyCode::ArrowRight => 106,
        KeyCode::ArrowDown => 108,
        KeyCode::Delete => 111,
        _ => return None,
    };

    Some(keycode)
}

fn draw_image(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    image: &RenderImage,
) {
    let dst_x0 = image.x.max(0) as u32;
    let dst_y0 = image.y.max(0) as u32;
    let dst_x1 = (image.x + image.draw_width as i32).max(0) as u32;
    let dst_y1 = (image.y + image.draw_height as i32).max(0) as u32;

    let dst_x0 = dst_x0.min(framebuffer_width);
    let dst_y0 = dst_y0.min(framebuffer_height);
    let dst_x1 = dst_x1.min(framebuffer_width);
    let dst_y1 = dst_y1.min(framebuffer_height);

    if dst_x0 >= dst_x1 || dst_y0 >= dst_y1 {
        return;
    }

    let framebuffer_stride = framebuffer_width as usize;
    let image_stride = image.width as usize;

    for dst_y in dst_y0 as usize..dst_y1 as usize {
        let local_y = (dst_y as i32 - image.y).max(0) as u32;
        let src_y = ((u64::from(local_y) * u64::from(image.height)) / u64::from(image.draw_height))
            .min(u64::from(image.height.saturating_sub(1))) as usize;
        let dst_row_start = dst_y * framebuffer_stride;
        let src_row_start = src_y * image_stride;

        for dst_x in dst_x0 as usize..dst_x1 as usize {
            let mut coverage = 1.0;
            if let Some(clip) = image.clip {
                coverage = rounded_rect_coverage(dst_x as i32, dst_y as i32, clip);
                if coverage <= 0.0 {
                    continue;
                }
            }

            let local_x = (dst_x as i32 - image.x).max(0) as u32;
            let src_x = ((u64::from(local_x) * u64::from(image.width))
                / u64::from(image.draw_width))
            .min(u64::from(image.width.saturating_sub(1))) as usize;
            let src_index = src_row_start + src_x;

            if let Some(pixel) = image.pixels_argb.get(src_index) {
                let dst_index = dst_row_start + dst_x;
                buffer[dst_index] = blend_argb(buffer[dst_index], *pixel, coverage);
            }
        }
    }
}

fn draw_rounded_rectangle(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    rectangle: RenderRoundedRect,
) {
    let x0 = rectangle.x.max(0) as u32;
    let y0 = rectangle.y.max(0) as u32;
    let x1 = (rectangle.x + rectangle.width).max(0) as u32;
    let y1 = (rectangle.y + rectangle.height).max(0) as u32;
    let x0 = x0.min(framebuffer_width);
    let y0 = y0.min(framebuffer_height);
    let x1 = x1.min(framebuffer_width);
    let y1 = y1.min(framebuffer_height);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let color = rectangle.color.to_argb_u32();
    let framebuffer_stride = framebuffer_width as usize;

    for y in y0 as i32..y1 as i32 {
        for x in x0 as i32..x1 as i32 {
            let coverage = rounded_rect_coverage(x, y, rectangle);
            if coverage <= 0.0 {
                continue;
            }

            let index = y as usize * framebuffer_stride + x as usize;
            buffer[index] = blend_argb(buffer[index], color, coverage);
        }
    }
}

fn rounded_rect_coverage(x: i32, y: i32, rectangle: RenderRoundedRect) -> f32 {
    let top_radius = rectangle
        .radius
        .max(0)
        .min(rectangle.width / 2)
        .min(rectangle.height / 2);
    let bottom_radius = rectangle
        .bottom_radius
        .max(0)
        .min(rectangle.width / 2)
        .min(rectangle.height / 2);

    if top_radius == 0 && bottom_radius == 0 {
        return 1.0;
    }

    let pixel_x = x as f32 + 0.5;
    let pixel_y = y as f32 + 0.5;
    let left = rectangle.x as f32;
    let top = rectangle.y as f32;
    let right = (rectangle.x + rectangle.width) as f32;
    let bottom = (rectangle.y + rectangle.height) as f32;

    let corner = if top_radius > 0 && pixel_y < top + top_radius as f32 {
        if pixel_x < left + top_radius as f32 {
            Some((
                left + top_radius as f32,
                top + top_radius as f32,
                top_radius,
            ))
        } else if pixel_x > right - top_radius as f32 {
            Some((
                right - top_radius as f32,
                top + top_radius as f32,
                top_radius,
            ))
        } else {
            None
        }
    } else if bottom_radius > 0 && pixel_y > bottom - bottom_radius as f32 {
        if pixel_x < left + bottom_radius as f32 {
            Some((
                left + bottom_radius as f32,
                bottom - bottom_radius as f32,
                bottom_radius,
            ))
        } else if pixel_x > right - bottom_radius as f32 {
            Some((
                right - bottom_radius as f32,
                bottom - bottom_radius as f32,
                bottom_radius,
            ))
        } else {
            None
        }
    } else {
        None
    };

    let Some((center_x, center_y, radius)) = corner else {
        return 1.0;
    };

    let dx = pixel_x - center_x;
    let dy = pixel_y - center_y;
    let signed_distance = (dx * dx + dy * dy).sqrt() - radius as f32;

    (0.5 - signed_distance).clamp(0.0, 1.0)
}

fn draw_circle(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    circle: RenderCircle,
) {
    let x0 = circle.x.max(0) as u32;
    let y0 = circle.y.max(0) as u32;
    let x1 = (circle.x + circle.diameter).max(0) as u32;
    let y1 = (circle.y + circle.diameter).max(0) as u32;
    let x0 = x0.min(framebuffer_width);
    let y0 = y0.min(framebuffer_height);
    let x1 = x1.min(framebuffer_width);
    let y1 = y1.min(framebuffer_height);
    let radius = circle.diameter as f32 / 2.0;
    let center_x = circle.x as f32 + radius;
    let center_y = circle.y as f32 + radius;
    let color = circle.color.to_argb_u32();
    let framebuffer_stride = framebuffer_width as usize;

    for y in y0 as i32..y1 as i32 {
        for x in x0 as i32..x1 as i32 {
            let dx = x as f32 + 0.5 - center_x;
            let dy = y as f32 + 0.5 - center_y;
            let signed_distance = (dx * dx + dy * dy).sqrt() - radius;
            let coverage = (0.5 - signed_distance).clamp(0.0, 1.0);
            if coverage <= 0.0 {
                continue;
            }

            let index = y as usize * framebuffer_stride + x as usize;
            buffer[index] = blend_argb(buffer[index], color, coverage);
        }
    }
}

fn blend_argb(destination: u32, source: u32, coverage: f32) -> u32 {
    let source_alpha = f32::from(((source >> 24) & 0xff) as u8) / 255.0 * coverage;
    if source_alpha <= 0.0 {
        return destination;
    }
    if source_alpha >= 1.0 {
        return source | 0xff00_0000;
    }

    let inverse_alpha = 1.0 - source_alpha;
    let source_red = f32::from(((source >> 16) & 0xff) as u8);
    let source_green = f32::from(((source >> 8) & 0xff) as u8);
    let source_blue = f32::from((source & 0xff) as u8);
    let destination_red = f32::from(((destination >> 16) & 0xff) as u8);
    let destination_green = f32::from(((destination >> 8) & 0xff) as u8);
    let destination_blue = f32::from((destination & 0xff) as u8);

    let red = (source_red * source_alpha + destination_red * inverse_alpha).round() as u32;
    let green = (source_green * source_alpha + destination_green * inverse_alpha).round() as u32;
    let blue = (source_blue * source_alpha + destination_blue * inverse_alpha).round() as u32;

    0xff00_0000 | (red << 16) | (green << 8) | blue
}

fn draw_text(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    text: &RenderText,
) {
    if text.y >= framebuffer_height as i32 || text.y + 14 <= 0 {
        return;
    }

    let mut cursor_x = text.x;
    for character in text.text.chars().take(80) {
        if cursor_x >= framebuffer_width as i32 {
            break;
        }
        if cursor_x + 10 <= 0 {
            cursor_x += 12;
            continue;
        }

        draw_glyph(
            buffer,
            framebuffer_width,
            framebuffer_height,
            cursor_x,
            text.y,
            character,
            text.color,
        );
        cursor_x += 12;
    }
}

fn draw_glyph(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    x: i32,
    y: i32,
    character: char,
    color: u32,
) {
    let glyph = glyph_rows(character);
    let scale = 2;
    let framebuffer_stride = framebuffer_width as usize;

    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) == 0 {
                continue;
            }

            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x + col * scale + dx;
                    let py = y + row as i32 * scale + dy;

                    if px < 0 || py < 0 {
                        continue;
                    }

                    let px = px as u32;
                    let py = py as u32;
                    if px >= framebuffer_width || py >= framebuffer_height {
                        continue;
                    }

                    buffer[py as usize * framebuffer_stride + px as usize] = color;
                }
            }
        }
    }
}

fn glyph_rows(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b01110, 0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '_' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b11111,
        ],
        ':' => [
            0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000,
        ],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        '@' => [
            0b01110, 0b10001, 0b10111, 0b10101, 0b10111, 0b10000, 0b01110,
        ],
        '~' => [
            0b00000, 0b00000, 0b01001, 0b10110, 0b00000, 0b00000, 0b00000,
        ],
        ' ' => [0; 7],
        _ => [
            0b11111, 0b10001, 0b00010, 0b00100, 0b00100, 0b00000, 0b00100,
        ],
    }
}

fn draw_rectangle(
    buffer: &mut softbuffer::Buffer<'_, Arc<Window>, Arc<Window>>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    rectangle: RenderRect,
) {
    let x0 = rectangle.x.max(0) as u32;
    let y0 = rectangle.y.max(0) as u32;
    let x1 = (rectangle.x + rectangle.width).max(0) as u32;
    let y1 = (rectangle.y + rectangle.height).max(0) as u32;

    let x0 = x0.min(framebuffer_width);
    let y0 = y0.min(framebuffer_height);
    let x1 = x1.min(framebuffer_width);
    let y1 = y1.min(framebuffer_height);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let color = rectangle.color.to_argb_u32();
    let stride = framebuffer_width as usize;

    for y in y0 as usize..y1 as usize {
        let row_start = y * stride;

        for x in x0 as usize..x1 as usize {
            buffer[row_start + x] = color;
        }
    }
}
