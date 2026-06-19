//! Minimal native TTY backend bootstrap.

#![allow(dead_code)]

pub mod drm;
pub mod egl;
pub mod event_loop;
pub mod gbm;
pub mod gles;
pub mod input;
pub mod output;
pub mod session;
pub mod udev;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("[native] starting");

    let (session, notifier) = session::NativeSessionState::new()?;
    println!(
        "[native] session acquired: {} ({})",
        session.seat_name(),
        if session.is_active() {
            "active"
        } else {
            "inactive"
        }
    );

    let udev = udev::UdevState::new(&session.seat_name())?;
    println!("[native] primary gpu: {}", udev.primary_gpu().display());

    event_loop::run_native_bootstrap(session, notifier, udev)
}
