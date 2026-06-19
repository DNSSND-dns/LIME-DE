//! libseat session ownership and pause/resume handling.

use smithay::backend::session::{
    libseat::{LibSeatSession, LibSeatSessionNotifier},
    Session,
};

#[derive(Debug)]
pub struct NativeSessionState {
    session: LibSeatSession,
}

impl NativeSessionState {
    pub fn new() -> Result<(Self, LibSeatSessionNotifier), Box<dyn std::error::Error>> {
        let (session, notifier) = LibSeatSession::new()?;
        Ok((Self { session }, notifier))
    }

    #[must_use]
    pub fn seat_name(&self) -> String {
        self.session.seat()
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.is_active()
    }
}
