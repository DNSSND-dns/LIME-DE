use std::{fmt, sync::Arc};

use crate::config::WindowStyleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl WindowId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowButtonGeometry {
    pub x: i32,
    pub y: i32,
    pub diameter: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowDecoration {
    pub titlebar_height: i32,
    pub border_width: i32,
    pub corner_radius: i32,
    pub bottom_corner_radius: i32,
    pub button_diameter: i32,
    pub button_spacing: i32,
    pub button_left_padding: i32,
}

impl WindowDecoration {
    pub const TITLEBAR_HEIGHT: i32 = 32;
    pub const BORDER_WIDTH: i32 = 1;

    #[must_use]
    pub fn new() -> Self {
        Self {
            titlebar_height: Self::TITLEBAR_HEIGHT,
            border_width: Self::BORDER_WIDTH,
            corner_radius: 10,
            bottom_corner_radius: 0,
            button_diameter: 12,
            button_spacing: 8,
            button_left_padding: 12,
        }
    }

    #[must_use]
    pub fn from_style(style: &WindowStyleConfig) -> Self {
        Self {
            titlebar_height: style.titlebar_height.max(1),
            border_width: style.border_width.max(1),
            corner_radius: style.corner_radius.max(0),
            bottom_corner_radius: style.bottom_corner_radius.max(0),
            button_diameter: style.button_diameter.max(1),
            button_spacing: style.button_spacing.max(0),
            button_left_padding: style.button_left_padding.max(0),
        }
    }

    #[must_use]
    pub fn total_size(self, client_width: i32, client_height: i32) -> (i32, i32) {
        (
            client_width + self.border_width * 2,
            client_height + self.titlebar_height + self.border_width,
        )
    }

    #[must_use]
    pub fn client_origin(self, geometry: WindowGeometry) -> (i32, i32) {
        (
            geometry.x + self.border_width,
            geometry.y + self.titlebar_height,
        )
    }

    #[must_use]
    pub fn close_button(self, geometry: WindowGeometry) -> WindowButtonGeometry {
        self.button_at(geometry, 0)
    }

    #[must_use]
    pub fn minimize_button(self, geometry: WindowGeometry) -> WindowButtonGeometry {
        self.button_at(geometry, 1)
    }

    #[must_use]
    pub fn maximize_button(self, geometry: WindowGeometry) -> WindowButtonGeometry {
        self.button_at(geometry, 2)
    }

    fn button_at(self, geometry: WindowGeometry, index: i32) -> WindowButtonGeometry {
        let x = geometry.x
            + self.button_left_padding
            + index * (self.button_diameter + self.button_spacing);
        let y = geometry.y + (self.titlebar_height - self.button_diameter) / 2;

        WindowButtonGeometry {
            x,
            y,
            diameter: self.button_diameter,
        }
    }
}

impl Default for WindowDecoration {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientBufferMetadata {
    pub width: Option<i32>,
    pub height: Option<i32>,
}

impl ClientBufferMetadata {
    #[must_use]
    pub fn from_size(width: i32, height: i32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }

    #[must_use]
    pub fn unknown_size() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientBufferPixels {
    pub width: u32,
    pub height: u32,
    pub pixels_argb: Arc<[u32]>,
}

impl ClientBufferPixels {
    #[must_use]
    pub fn new(width: u32, height: u32, pixels_argb: Vec<u32>) -> Self {
        Self {
            width,
            height,
            pixels_argb: pixels_argb.into(),
        }
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixels_argb.len() * std::mem::size_of::<u32>()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: WindowId,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub mapped: bool,
    pub minimized: bool,
    pub animating: bool,
    pub geometry: WindowGeometry,
    pub user_resized: bool,
    pub client_size: Option<(i32, i32)>,
    pub pending_configure_size: Option<(i32, i32)>,
    pub maximized: bool,
    pub restore_geometry: Option<WindowGeometry>,
    pub client_buffer: Option<ClientBufferMetadata>,
    pub client_pixels: Option<ClientBufferPixels>,
    pub cached_client_pixels: Option<ClientBufferPixels>,
    pub animation_client_pixels: Option<ClientBufferPixels>,
}

impl Window {
    #[must_use]
    pub fn new(id: WindowId) -> Self {
        Self {
            id,
            title: None,
            app_id: None,
            mapped: false,
            minimized: false,
            animating: false,
            geometry: WindowGeometry::default(),
            user_resized: false,
            client_size: None,
            pending_configure_size: None,
            maximized: false,
            restore_geometry: None,
            client_buffer: None,
            client_pixels: None,
            cached_client_pixels: None,
            animation_client_pixels: None,
        }
    }
}

impl WindowGeometry {
    #[must_use]
    pub fn with_default_for_output(self, output_width: u32, output_height: u32) -> Self {
        if self.width > 0 && self.height > 0 {
            return self;
        }

        let output_width = output_width.max(1);
        let output_height = output_height.max(1);
        let x = (f64::from(output_width) * 0.10).round() as i32;
        let y = (f64::from(output_height) * 0.10).round() as i32;
        let available_width = (output_width as i32 - x).max(1);
        let available_height = (output_height as i32 - y).max(1);
        let width = ((f64::from(output_width) * 0.40).round() as i32)
            .max(320)
            .min(available_width);
        let height = ((f64::from(output_height) * 0.35).round() as i32)
            .max(200)
            .min(available_height);

        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Default)]
pub struct WindowState;
