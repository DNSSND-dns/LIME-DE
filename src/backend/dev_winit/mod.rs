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
    backend::{BackendInputEvent, PointerButton},
    core::rasterizer::draw_scene,
    error::AppError,
    render::RenderSceneFrame,
};

pub struct WinitBackend {
    proxy: EventLoopProxy<WinitBackendEvent>,
    output_rx: Receiver<BackendInputEvent>,
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

    pub fn poll_events(&self) -> Vec<BackendInputEvent> {
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

struct WinitBackendApp {
    width: u32,
    height: u32,
    proxy: EventLoopProxy<WinitBackendEvent>,
    output_tx: Sender<BackendInputEvent>,
    current_size: Arc<Mutex<Option<(u32, u32)>>>,
    init_tx: Option<mpsc::Sender<Result<EventLoopProxy<WinitBackendEvent>, String>>>,
    window: Option<Arc<Window>>,
    renderer: Option<WindowRenderer>,
    pending_frame: RenderSceneFrame,
}

impl WinitBackendApp {
    fn new(
        width: u32,
        height: u32,
        proxy: EventLoopProxy<WinitBackendEvent>,
        init_tx: mpsc::Sender<Result<EventLoopProxy<WinitBackendEvent>, String>>,
        output_tx: Sender<BackendInputEvent>,
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
            renderer: None,
            pending_frame: RenderSceneFrame::new(crate::render::RenderColor::black()),
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
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        if let Err(error) = renderer.draw_frame(width, height, &self.pending_frame) {
            eprintln!("LIME DE winit test backend render failed: {error}");
            return;
        }

        let _ = self.output_tx.send(BackendInputEvent::FramePresented);
    }
}

struct WindowRenderer(SoftbufferRenderer);

impl WindowRenderer {
    fn new(window: Arc<Window>, width: u32, height: u32) -> Result<Self, String> {
        let _ = (width, height);
        SoftbufferRenderer::new(window).map(Self)
    }

    fn draw_frame(
        &mut self,
        width: NonZeroU32,
        height: NonZeroU32,
        frame: &RenderSceneFrame,
    ) -> Result<(), String> {
        self.0.draw_frame(width, height, frame)
    }
}

struct SoftbufferRenderer {
    _context: Context<Arc<Window>>,
    surface: Surface<Arc<Window>, Arc<Window>>,
    framebuffer_size: Option<(u32, u32)>,
}

impl SoftbufferRenderer {
    fn new(window: Arc<Window>) -> Result<Self, String> {
        let context = Context::new(Arc::clone(&window))
            .map_err(|error| format!("failed to create softbuffer context: {error}"))?;
        let surface = Surface::new(&context, window)
            .map_err(|error| format!("failed to create softbuffer surface: {error}"))?;

        Ok(Self {
            _context: context,
            surface,
            framebuffer_size: None,
        })
    }

    fn draw_frame(
        &mut self,
        width: NonZeroU32,
        height: NonZeroU32,
        frame: &RenderSceneFrame,
    ) -> Result<(), String> {
        let framebuffer_size = (width.get(), height.get());
        if self.framebuffer_size != Some(framebuffer_size) {
            self.surface
                .resize(width, height)
                .map_err(|error| format!("softbuffer resize failed: {error}"))?;
            self.framebuffer_size = Some(framebuffer_size);
        }

        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|error| format!("could not acquire softbuffer pixel buffer: {error}"))?;

        draw_scene(buffer.as_mut(), width.get(), height.get(), frame);

        buffer
            .present()
            .map_err(|error| format!("softbuffer present failed: {error}"))
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
                let size = window.inner_size();
                match WindowRenderer::new(Arc::clone(&window), size.width, size.height) {
                    Ok(renderer) => {
                        let size = window.inner_size();
                        self.set_current_size(size.width, size.height);
                        self.renderer = Some(renderer);
                        self.window = Some(window);
                        self.send_init_result(Ok(self.proxy.clone()));
                    }
                    Err(error) => {
                        self.send_init_result(Err(format!(
                            "failed to create winit renderer: {error}"
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
                let _ = self.output_tx.send(BackendInputEvent::OutputResized {
                    width: size.width,
                    height: size.height,
                });
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let _ = self
                    .output_tx
                    .send(BackendInputEvent::PointerMotionAbsolute {
                        x: position.x,
                        y: position.y,
                    });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = map_mouse_button(button) {
                    let _ = self.output_tx.send(BackendInputEvent::PointerButton {
                        button,
                        pressed: state == ElementState::Pressed,
                    });
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if !event.repeat {
                    if let Some(keycode) = map_keycode(event.physical_key) {
                        let _ = self.output_tx.send(BackendInputEvent::Keyboard {
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

fn map_mouse_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Left),
        MouseButton::Middle => Some(PointerButton::Middle),
        MouseButton::Right => Some(PointerButton::Right),
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
