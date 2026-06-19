//! Minimal native TTY backend bootstrap.

pub mod drm;
pub mod egl;
pub mod event_loop;
pub mod gbm;
pub mod gles;
pub mod input;
pub mod output;
pub mod session;
pub mod udev;

use smithay::reexports::wayland_server::Display;

use crate::{
    compositor::CompositorState,
    config::{AnimationConfig, BehaviorConfig, StyleConfig},
};

pub fn run(
    launch_test_client: bool,
    test_client_commands: Vec<String>,
    style: StyleConfig,
    behavior: BehaviorConfig,
    animations: AnimationConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("[native] starting");

    let (mut session, notifier) = session::NativeSessionState::new()?;
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

    let mut drm = drm::NativeDrmState::new(&mut session, udev.primary_gpu())?;
    println!("[native] DRM initialized");
    println!("[native] connected outputs: {}", drm.connected_outputs());

    let gbm = gbm::GbmState::new(drm.fd())?;
    println!("[native] GBM initialized");
    let mut egl = egl::EglState::new(gbm.device())?;
    let mut gles = gles::GlesState::new(egl.take_context()?)?;
    let mut output = output::NativeOutputState::new(drm.take_device()?, gbm.device(), &mut gles)?;

    let display = Display::<CompositorState>::new()?;
    let mut core = CompositorState::new(display.handle(), style, behavior, animations);
    let (width, height) = output.size();
    core.sync_primary_output_size(width, height);
    output.submit_scene_frame(&mut gles, &core.render_frame())?;
    let (input, input_backend) = input::NativeInputState::new(&session)?;

    event_loop::run_native_bootstrap(
        session,
        notifier,
        udev,
        drm,
        gbm,
        egl,
        output,
        gles,
        core,
        display,
        input,
        input_backend,
        launch_test_client,
        test_client_commands,
    )
}
