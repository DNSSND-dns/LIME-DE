//! libseat session ownership and pause/resume handling.

use std::{io, path::Path};

use smithay::backend::session::{
    libseat::{LibSeatSession, LibSeatSessionNotifier},
    Session,
};
use smithay::reexports::rustix::fs::OFlags;

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

    #[must_use]
    pub(crate) fn libseat_session(&self) -> smithay::backend::session::libseat::LibSeatSession {
        self.session.clone()
    }

    pub(crate) fn open_drm_node(
        &mut self,
        path: &Path,
    ) -> Result<std::os::fd::OwnedFd, Box<dyn std::error::Error>> {
        self.session
            .open(
                path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            )
            .map_err(|error| {
                io::Error::other(format!(
                    "[native] failed to open DRM node. Check seatd/logind/video group. \
                     Node: {}. Error: {error}",
                    path.display()
                ))
                .into()
            })
    }
}
