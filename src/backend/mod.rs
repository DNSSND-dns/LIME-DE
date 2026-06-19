pub mod dev_winit;

#[cfg(feature = "native_tty")]
pub mod native;

#[derive(Debug, Default)]
pub struct BackendState;

pub use dev_winit::{WinitBackend, WinitBackendOutputEvent, WinitMouseButton};
