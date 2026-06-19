#[cfg(not(feature = "dev_winit"))]
use crate::{error::AppError, render::RenderSceneFrame};

#[cfg(feature = "dev_winit")]
pub mod dev_winit;

#[cfg(feature = "native_tty")]
pub mod native;

#[derive(Debug, Default)]
pub struct BackendState;

#[cfg(feature = "dev_winit")]
pub use dev_winit::WinitBackend;

#[cfg(not(feature = "dev_winit"))]
#[derive(Debug, Default)]
pub struct WinitBackend;

#[cfg(not(feature = "dev_winit"))]
impl WinitBackend {
    pub fn new(_width: u32, _height: u32) -> Result<Self, AppError> {
        let _event_shape = (
            BackendInputEvent::PointerMotionAbsolute { x: 0.0, y: 0.0 },
            #[cfg(feature = "native_tty")]
            BackendInputEvent::PointerMotion { dx: 0.0, dy: 0.0 },
            BackendInputEvent::PointerButton {
                button: PointerButton::Left,
                pressed: false,
            },
            BackendInputEvent::PointerButton {
                button: PointerButton::Middle,
                pressed: false,
            },
            BackendInputEvent::PointerButton {
                button: PointerButton::Right,
                pressed: false,
            },
            BackendInputEvent::Keyboard {
                keycode: 0,
                pressed: false,
            },
            BackendInputEvent::OutputResized {
                width: 0,
                height: 0,
            },
            BackendInputEvent::FramePresented,
        );
        Err(AppError::new(
            "dev-winit backend requires the Cargo feature 'dev_winit'",
        ))
    }

    pub fn shutdown(&mut self) {}

    #[must_use]
    pub fn draw_frame(&self, _frame: RenderSceneFrame) -> bool {
        false
    }

    #[must_use]
    pub fn current_size(&self) -> Option<(u32, u32)> {
        None
    }

    #[must_use]
    pub fn poll_events(&self) -> Vec<BackendInputEvent> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendInputEvent {
    #[cfg(feature = "native_tty")]
    PointerMotion {
        dx: f64,
        dy: f64,
    },
    PointerMotionAbsolute {
        x: f64,
        y: f64,
    },
    PointerButton {
        button: PointerButton,
        pressed: bool,
    },
    Keyboard {
        keycode: u32,
        pressed: bool,
    },
    OutputResized {
        width: u32,
        height: u32,
    },
    FramePresented,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Middle,
    Right,
}
