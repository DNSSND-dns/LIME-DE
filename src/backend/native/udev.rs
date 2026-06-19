//! udev discovery and DRM device hotplug handling.

use std::path::{Path, PathBuf};

use smithay::backend::udev::{primary_gpu, UdevBackend};

pub struct UdevState {
    backend: Option<UdevBackend>,
    primary_gpu: PathBuf,
}

impl UdevState {
    pub fn new(seat_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let backend = UdevBackend::new(seat_name)?;
        let primary_gpu = primary_gpu(seat_name)?
            .ok_or_else(|| format!("no primary GPU found for seat {seat_name}"))?;

        Ok(Self {
            backend: Some(backend),
            primary_gpu,
        })
    }

    #[must_use]
    pub fn primary_gpu(&self) -> &Path {
        &self.primary_gpu
    }

    pub(crate) fn take_backend(&mut self) -> Result<UdevBackend, Box<dyn std::error::Error>> {
        self.backend
            .take()
            .ok_or_else(|| "udev backend was already registered".into())
    }
}

impl std::fmt::Debug for UdevState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UdevState")
            .field("primary_gpu", &self.primary_gpu)
            .finish_non_exhaustive()
    }
}
