//! libinput adapter for the shared core input event path.

use std::io;

use input::Libinput;
use smithay::backend::{
    input::{
        AbsolutePositionEvent, ButtonState, InputEvent, KeyState, KeyboardKeyEvent,
        PointerButtonEvent, PointerMotionEvent,
    },
    libinput::{LibinputInputBackend, LibinputSessionInterface},
};

use crate::backend::{BackendInputEvent, PointerButton};

use super::session::NativeSessionState;

pub struct NativeInputState {
    context: Libinput,
}

impl NativeInputState {
    pub fn new(
        session: &NativeSessionState,
    ) -> Result<(Self, LibinputInputBackend), Box<dyn std::error::Error>> {
        let mut context =
            Libinput::new_with_udev(LibinputSessionInterface::from(session.libseat_session()));
        context
            .udev_assign_seat(&session.seat_name())
            .map_err(|error| {
                io::Error::other(format!(
                    "[native] failed to assign libinput seat: {error:?}"
                ))
            })?;
        let backend = LibinputInputBackend::new(context.clone());
        println!("[native] libinput initialized");
        Ok((Self { context }, backend))
    }

    pub fn suspend(&mut self) {
        self.context.suspend();
    }

    pub fn resume(&mut self) {
        if let Err(error) = self.context.resume() {
            eprintln!("[native] failed to resume libinput: {error:?}");
        }
    }
}

pub fn translate_event(
    event: InputEvent<LibinputInputBackend>,
    output_size: (u32, u32),
) -> Option<BackendInputEvent> {
    match event {
        InputEvent::PointerMotion { event } => Some(BackendInputEvent::PointerMotion {
            dx: event.delta_x(),
            dy: event.delta_y(),
        }),
        InputEvent::PointerMotionAbsolute { event } => {
            let position = event.position_transformed(
                (
                    output_size.0.min(i32::MAX as u32) as i32,
                    output_size.1.min(i32::MAX as u32) as i32,
                )
                    .into(),
            );
            Some(BackendInputEvent::PointerMotionAbsolute {
                x: position.x,
                y: position.y,
            })
        }
        InputEvent::PointerButton { event } => {
            let button = match event.button_code() {
                0x110 => PointerButton::Left,
                0x111 => PointerButton::Right,
                0x112 => PointerButton::Middle,
                _ => return None,
            };
            Some(BackendInputEvent::PointerButton {
                button,
                pressed: event.state() == ButtonState::Pressed,
            })
        }
        InputEvent::Keyboard { event } => Some(BackendInputEvent::Keyboard {
            // Smithay's libinput adapter exposes XKB keycodes (evdev + 8),
            // while the shared core accepts raw evdev codes and adds 8 once.
            keycode: event.key_code().raw().saturating_sub(8),
            pressed: event.state() == KeyState::Pressed,
        }),
        _ => None,
    }
}
