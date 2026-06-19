//! udev discovery and DRM device hotplug handling.

use std::{
    io,
    path::{Path, PathBuf},
};

use smithay::backend::udev::{primary_gpu, UdevBackend};

pub struct UdevState {
    backend: Option<UdevBackend>,
    primary_gpu: PathBuf,
}

impl UdevState {
    pub fn new(seat_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let backend = UdevBackend::new(seat_name)?;
        let detected_primary = primary_gpu(seat_name)?;
        let primary_gpu = detected_primary
            .filter(|path| is_kms_card_node(path))
            .or_else(|| {
                backend
                    .device_list()
                    .map(|(_, path)| path.to_path_buf())
                    .find(|path| is_kms_card_node(path))
            })
            .ok_or_else(|| io::Error::other("[native] no primary DRM device found"))?;

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

fn is_kms_card_node(path: &Path) -> bool {
    path.parent() == Some(Path::new("/dev/dri"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.strip_prefix("card").is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
                })
            })
}

impl std::fmt::Debug for UdevState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UdevState")
            .field("primary_gpu", &self.primary_gpu)
            .finish_non_exhaustive()
    }
}
