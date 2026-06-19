//! Smithay GLES renderer state and runtime capability checks.

use std::{ffi::CStr, io, os::raw::c_char};

use smithay::backend::{
    allocator::Fourcc,
    egl::EGLContext,
    renderer::gles::{ffi, GlesRenderer},
};

#[derive(Debug)]
pub struct GlesState {
    renderer: GlesRenderer,
}

impl GlesState {
    #[allow(unsafe_code)]
    pub fn new(context: EGLContext) -> Result<Self, Box<dyn std::error::Error>> {
        let mut renderer = unsafe { GlesRenderer::new(context) }.map_err(|error| {
            io::Error::other(format!(
                "[native] failed to initialize Smithay GLES renderer: {error}"
            ))
        })?;

        let (renderer_name, version) = renderer.with_context(|gl| unsafe {
            (gl_string(gl, ffi::RENDERER), gl_string(gl, ffi::VERSION))
        })?;

        println!("[native] GLES renderer: {renderer_name}");
        println!("[native] GLES version: {version}");
        let selected_formats = renderer
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .filter(|format| matches!(format.code, Fourcc::Xrgb8888 | Fourcc::Argb8888))
            .map(|format| format.code.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        println!(
            "[native] selected scanout/render formats: {}",
            if selected_formats.is_empty() {
                "none".to_string()
            } else {
                selected_formats.join("/")
            }
        );

        Ok(Self { renderer })
    }

    #[must_use]
    pub(crate) fn renderer(&self) -> &GlesRenderer {
        &self.renderer
    }

    pub(crate) fn renderer_mut(&mut self) -> &mut GlesRenderer {
        &mut self.renderer
    }
}

#[allow(unsafe_code)]
unsafe fn gl_string(gl: &ffi::Gles2, name: ffi::types::GLenum) -> String {
    let value = unsafe { gl.GetString(name) };
    if value.is_null() {
        return "unknown".to_string();
    }

    unsafe { CStr::from_ptr(value.cast::<c_char>()) }
        .to_string_lossy()
        .into_owned()
}
