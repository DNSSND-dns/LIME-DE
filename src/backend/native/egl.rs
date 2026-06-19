//! EGL display and context initialization over GBM.

use std::io;

use smithay::backend::{
    allocator::gbm::GbmDevice,
    drm::DrmDeviceFd,
    egl::{
        context::{GlAttributes, PixelFormatRequirements},
        EGLContext, EGLDisplay,
    },
};

#[derive(Debug)]
pub struct EglState {
    context: Option<EGLContext>,
}

impl EglState {
    #[allow(unsafe_code)]
    pub fn new(gbm: GbmDevice<DrmDeviceFd>) -> Result<Self, Box<dyn std::error::Error>> {
        let display = unsafe { EGLDisplay::new(gbm) }.map_err(|error| {
            io::Error::other(format!(
                "[native] failed to initialize EGL display: {error}"
            ))
        })?;
        let context = EGLContext::new_with_config(
            &display,
            GlAttributes {
                version: (3, 0),
                profile: None,
                debug: cfg!(debug_assertions),
                vsync: false,
            },
            PixelFormatRequirements::_8_bit(),
        )
        .map_err(|error| {
            io::Error::other(format!(
                "[native] failed to initialize EGL context for GLES 3.0: {error}"
            ))
        })?;

        let (major, minor) = display.get_egl_version();
        println!("[native] EGL initialized");
        println!("[native] EGL version: {major}.{minor}");

        Ok(Self {
            context: Some(context),
        })
    }

    pub(crate) fn take_context(&mut self) -> Result<EGLContext, Box<dyn std::error::Error>> {
        self.context.take().ok_or_else(|| {
            io::Error::other("EGL context was already assigned to a renderer").into()
        })
    }
}
