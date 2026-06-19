//! calloop source registration and native frame scheduling.

use std::process::{Child, Command};

use smithay::{
    backend::{
        drm::DrmEvent,
        libinput::LibinputInputBackend,
        session::{libseat::LibSeatSessionNotifier, Event as SessionEvent},
        udev::UdevEvent,
    },
    reexports::{calloop::EventLoop, wayland_server::Display},
    wayland::socket::ListeningSocketSource,
};

use crate::compositor::CompositorState;

use super::{
    drm::NativeDrmState,
    egl::EglState,
    gbm::GbmState,
    gles::GlesState,
    input::{self, NativeInputState},
    output::NativeOutputState,
    session::NativeSessionState,
    udev::UdevState,
};

struct NativeEventLoopState {
    output: NativeOutputState,
    gles: GlesState,
    core: CompositorState,
    display: Display<CompositorState>,
    input: NativeInputState,
    socket_name: String,
    test_client: Option<Child>,
    launched_clients: Vec<Child>,
}

pub fn run_native_bootstrap(
    session: NativeSessionState,
    session_notifier: LibSeatSessionNotifier,
    mut udev: UdevState,
    mut drm: NativeDrmState,
    gbm: GbmState,
    egl: EglState,
    output: NativeOutputState,
    gles: GlesState,
    core: CompositorState,
    display: Display<CompositorState>,
    input: NativeInputState,
    input_backend: LibinputInputBackend,
    launch_test_client: bool,
    test_client_commands: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<NativeEventLoopState>::try_new()?;
    let loop_signal = event_loop.get_signal();
    let listening_socket = ListeningSocketSource::new_auto()?;
    let socket_name = listening_socket
        .socket_name()
        .to_string_lossy()
        .into_owned();
    std::env::set_var("WAYLAND_DISPLAY", &socket_name);

    event_loop
        .handle()
        .insert_source(listening_socket, |client_stream, _, state| {
            if let Err(error) = state.core.insert_client(client_stream) {
                eprintln!("[native] failed to accept Wayland client: {error}");
            }
        })?;

    event_loop
        .handle()
        .insert_source(session_notifier, |event, _, state| match event {
            SessionEvent::PauseSession => {
                println!("[native] session paused");
                state.input.suspend();
            }
            SessionEvent::ActivateSession => {
                println!("[native] session resumed");
                state.input.resume();
            }
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

    event_loop
        .handle()
        .insert_source(drm.take_notifier()?, |event, _, state| match event {
            DrmEvent::VBlank(crtc) => match state.output.handle_vblank(crtc) {
                Ok(true) => {
                    state.core.handle_frame_presented();
                    let frame = state.core.render_frame();
                    if let Err(error) = state.output.submit_scene_frame(&mut state.gles, &frame) {
                        eprintln!("[native] repaint failed after VBlank: {error}");
                    }
                }
                Ok(false) => {}
                Err(error) => {
                    eprintln!("[native] frame completion failed after VBlank: {error}");
                }
            },
            DrmEvent::Error(error) => {
                eprintln!("[native] DRM event error: {error}");
            }
        })?;

    event_loop
        .handle()
        .insert_source(input_backend, |event, _, state| {
            if let Some(event) = input::translate_event(event, state.output.size()) {
                state.core.handle_backend_event(event);
            }
        })?;

    ctrlc::set_handler(move || {
        println!("[native] shutdown requested");
        loop_signal.stop();
        loop_signal.wakeup();
    })?;

    println!("[native] event loop running");
    println!("[native] Wayland socket: {socket_name}");
    println!("[native] WAYLAND_DISPLAY={socket_name}");
    let mut state = NativeEventLoopState {
        output,
        gles,
        core,
        display,
        input,
        socket_name: socket_name.clone(),
        test_client: if launch_test_client {
            launch_client(&test_client_commands, &socket_name)
        } else {
            None
        },
        launched_clients: Vec::new(),
    };
    let panel_exit_signal = event_loop.get_signal();
    event_loop.run(None, &mut state, move |state| {
        let NativeEventLoopState {
            display,
            core,
            socket_name,
            test_client,
            launched_clients,
            ..
        } = state;
        if let Err(error) = display.dispatch_clients(core) {
            eprintln!("[native] Wayland client dispatch failed: {error}");
        }
        if let Err(error) = display.flush_clients() {
            eprintln!("[native] Wayland client flush failed: {error}");
        }

        if test_client
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            *test_client = None;
        }
        launched_clients.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));

        for commands in core.take_pending_launch_commands() {
            if let Some(child) = launch_client(&commands, socket_name) {
                launched_clients.push(child);
            }
        }
        if core.take_exit_requested() {
            println!("[native] panel exit requested");
            panel_exit_signal.stop();
            panel_exit_signal.wakeup();
        }
    })?;

    // Drop event sources before releasing the libseat session handle.
    drop(event_loop);
    if let Some(mut child) = state.test_client.take() {
        terminate_child(&mut child);
    }
    for mut child in state.launched_clients.drain(..) {
        terminate_child(&mut child);
    }
    drop(state);
    drop(egl);
    drop(gbm);
    drop(drm);
    drop(session);
    println!("[native] stopped");
    Ok(())
}

fn launch_client(commands: &[String], socket_name: &str) -> Option<Child> {
    for command_spec in commands {
        let Some(mut parts) = shlex::split(command_spec).map(IntoIterator::into_iter) else {
            continue;
        };
        let Some(program) = parts.next() else {
            continue;
        };

        if let Ok(child) = Command::new(&program)
            .args(parts)
            .env("WAYLAND_DISPLAY", socket_name)
            .env("XDG_CURRENT_DESKTOP", "LIME")
            .env_remove("DISPLAY")
            .env_remove("DESKTOP_STARTUP_ID")
            .spawn()
        {
            println!("[native] client launched: {command_spec}");
            return Some(child);
        }
    }

    eprintln!("[native] no configured Wayland client could be launched");
    None
}

fn terminate_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
        }
        Err(error) => eprintln!("[native] failed to query test client state: {error}"),
    }
}
