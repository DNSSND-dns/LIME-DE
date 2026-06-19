//! GBM device and scanout/render buffer allocation.

use std::io;

use smithay::backend::{allocator::gbm::GbmDevice, drm::DrmDeviceFd};

#[derive(Debug)]
pub struct GbmState {
    device: GbmDevice<DrmDeviceFd>,
}

impl GbmState {
    pub fn new(fd: DrmDeviceFd) -> Result<Self, Box<dyn std::error::Error>> {
        let device = GbmDevice::new(fd).map_err(|error| {
            io::Error::other(format!("[native] failed to create GBM device: {error}"))
        })?;
        Ok(Self { device })
    }

    #[must_use]
    pub(crate) fn device(&self) -> GbmDevice<DrmDeviceFd> {
        self.device.clone()
    }
}
