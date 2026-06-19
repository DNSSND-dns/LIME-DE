//! calloop source registration and native frame scheduling.

use std::time::Duration;

use smithay::{
    backend::{
        session::{libseat::LibSeatSessionNotifier, Event as SessionEvent},
        udev::UdevEvent,
    },
    reexports::calloop::EventLoop,
};

use super::{session::NativeSessionState, udev::UdevState};

pub fn run_native_bootstrap(
    session: NativeSessionState,
    session_notifier: LibSeatSessionNotifier,
    mut udev: UdevState,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<()>::try_new()?;
    let loop_signal = event_loop.get_signal();

    event_loop
        .handle()
        .insert_source(session_notifier, |event, _, _| match event {
            SessionEvent::PauseSession => println!("[native] session paused"),
            SessionEvent::ActivateSession => println!("[native] session resumed"),
        })?;

    event_loop
        .handle()
        .insert_source(udev.take_backend()?, |event, _, _| match event {
            UdevEvent::Added { device_id, path } => {
                println!(
                    "[native] DRM device added: {device_id:?} {}",
                    path.display()
                );
            }
            UdevEvent::Changed { device_id } => {
                println!("[native] DRM device changed: {device_id:?}");
            }
            UdevEvent::Removed { device_id } => {
                println!("[native] DRM device removed: {device_id:?}");
            }
        })?;

    ctrlc::set_handler(move || {
        println!("[native] shutdown requested");
        loop_signal.stop();
        loop_signal.wakeup();
    })?;

    println!("[native] event loop running");
    event_loop.run(Duration::from_millis(250), &mut (), |_| {})?;

    // Drop event sources before releasing the libseat session handle.
    drop(event_loop);
    drop(session);
    println!("[native] stopped");
    Ok(())
}
