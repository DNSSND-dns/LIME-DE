use std::slice;

use smithay::{
    reexports::wayland_server::protocol::{wl_buffer, wl_shm},
    wayland::shm::{with_buffer_contents, BufferData},
};

use crate::window::ClientBufferPixels;

const MAX_CLIENT_BUFFER_DIMENSION: u32 = 8192;
const BYTES_PER_PIXEL: usize = 4;

pub fn read_shm_pixels(buffer: &wl_buffer::WlBuffer) -> Option<ClientBufferPixels> {
    with_buffer_contents(buffer, copy_shm_pixels).ok().flatten()
}

#[allow(unsafe_code)]
fn copy_shm_pixels(ptr: *const u8, len: usize, data: BufferData) -> Option<ClientBufferPixels> {
    let width = u32::try_from(data.width).ok()?;
    let height = u32::try_from(data.height).ok()?;
    let stride = usize::try_from(data.stride).ok()?;
    let offset = usize::try_from(data.offset).ok()?;
    let format = data.format;

    if width == 0 || height == 0 {
        return None;
    }
    if width > MAX_CLIENT_BUFFER_DIMENSION || height > MAX_CLIENT_BUFFER_DIMENSION {
        eprintln!(
            "Rejected SHM buffer: dimensions {width}x{height} exceed {MAX_CLIENT_BUFFER_DIMENSION}x{MAX_CLIENT_BUFFER_DIMENSION}"
        );
        return None;
    }
    if !matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888) {
        return None;
    }

    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let row_bytes = width_usize.checked_mul(BYTES_PER_PIXEL)?;
    if stride < row_bytes {
        eprintln!("Rejected SHM buffer: stride {stride} is smaller than row bytes {row_bytes}");
        return None;
    }
    let pixel_count = width_usize.checked_mul(height_usize)?;
    let mut pixels = Vec::with_capacity(pixel_count);

    for y in 0..height_usize {
        let row_start = offset.checked_add(y.checked_mul(stride)?)?;
        let row_end = row_start.checked_add(row_bytes)?;

        if row_end > len {
            return None;
        }

        // Smithay exposes SHM as a raw pointer because the client owns the memory.
        // LIME copies it immediately and never stores borrowed SHM references.
        let row = unsafe { slice::from_raw_parts(ptr.add(row_start), row_bytes) };

        for chunk in row.chunks_exact(4) {
            let mut pixel = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if format == wl_shm::Format::Xrgb8888 {
                pixel |= 0xff00_0000;
            }
            pixels.push(pixel);
        }
    }

    Some(ClientBufferPixels::new(width, height, pixels))
}
