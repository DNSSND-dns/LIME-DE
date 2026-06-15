use std::sync::Arc;

use crate::output::Output;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl RenderColor {
    #[must_use]
    pub fn black() -> Self {
        Self {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn from_hex_or(hex: &str, fallback: Self) -> Self {
        Self::from_hex(hex).unwrap_or(fallback)
    }

    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            return None;
        }

        let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;

        Some(Self::from_rgb_u8(red, green, blue))
    }

    #[must_use]
    pub fn from_rgb_u8(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red: f32::from(red) / 255.0,
            green: f32::from(green) / 255.0,
            blue: f32::from(blue) / 255.0,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn window_placeholder() -> Self {
        Self {
            red: 0.1,
            green: 0.55,
            blue: 0.95,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn to_argb_u32(self) -> u32 {
        let alpha = color_channel_to_u32(self.alpha);
        let red = color_channel_to_u32(self.red);
        let green = color_channel_to_u32(self.green);
        let blue = color_channel_to_u32(self.blue);

        (alpha << 24) | (red << 16) | (green << 8) | blue
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderImage {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub draw_width: u32,
    pub draw_height: u32,
    pub clip: Option<RenderRoundedRect>,
    pub pixels_argb: Arc<[u32]>,
}

impl RenderImage {
    #[must_use]
    pub fn new(x: i32, y: i32, width: u32, height: u32, pixels_argb: Arc<[u32]>) -> Self {
        Self {
            x,
            y,
            width,
            height,
            draw_width: width,
            draw_height: height,
            clip: None,
            pixels_argb,
        }
    }

    #[must_use]
    pub fn fit_to(mut self, width: u32, height: u32) -> Self {
        self.draw_width = width.max(1);
        self.draw_height = height.max(1);
        self
    }

    #[must_use]
    pub fn clipped_to(mut self, width: u32, height: u32) -> Self {
        self.draw_width = self.draw_width.min(width.max(1));
        self.draw_height = self.draw_height.min(height.max(1));
        self
    }

    #[must_use]
    pub fn with_clip(mut self, clip: RenderRoundedRect) -> Self {
        self.clip = Some(clip);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderCommand {
    RoundedRect(RenderRoundedRect),
    Rect(RenderRect),
    Circle(RenderCircle),
    Image(RenderImage),
    Text(RenderText),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSceneFrame {
    pub clear_color: RenderColor,
    pub commands: Vec<RenderCommand>,
    pub rounded_rectangles: Vec<RenderRoundedRect>,
    pub rectangles: Vec<RenderRect>,
    pub circles: Vec<RenderCircle>,
    pub images: Vec<RenderImage>,
    pub cursor: Vec<RenderRect>,
    pub text: Vec<RenderText>,
}

impl RenderSceneFrame {
    #[must_use]
    pub fn new(clear_color: RenderColor) -> Self {
        Self {
            clear_color,
            commands: Vec::new(),
            rounded_rectangles: Vec::new(),
            rectangles: Vec::new(),
            circles: Vec::new(),
            images: Vec::new(),
            cursor: Vec::new(),
            text: Vec::new(),
        }
    }

    pub fn push_rounded_rect(&mut self, rectangle: RenderRoundedRect) {
        self.rounded_rectangles.push(rectangle);
        self.commands.push(RenderCommand::RoundedRect(rectangle));
    }

    pub fn push_rect(&mut self, rectangle: RenderRect) {
        self.rectangles.push(rectangle);
        self.commands.push(RenderCommand::Rect(rectangle));
    }

    pub fn push_circle(&mut self, circle: RenderCircle) {
        self.circles.push(circle);
        self.commands.push(RenderCommand::Circle(circle));
    }

    pub fn push_image(&mut self, image: RenderImage) {
        self.images.push(image.clone());
        self.commands.push(RenderCommand::Image(image));
    }

    pub fn push_text(&mut self, text: RenderText) {
        self.text.push(text.clone());
        self.commands.push(RenderCommand::Text(text));
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub color: RenderColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRoundedRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub radius: i32,
    pub bottom_radius: i32,
    pub color: RenderColor,
}

impl RenderRoundedRect {
    #[must_use]
    pub fn with_vertical_radii(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        top_radius: i32,
        bottom_radius: i32,
        color: RenderColor,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            radius: top_radius,
            bottom_radius,
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCircle {
    pub x: i32,
    pub y: i32,
    pub diameter: i32,
    pub color: RenderColor,
}

impl RenderCircle {
    #[must_use]
    pub fn new(x: i32, y: i32, diameter: i32, color: RenderColor) -> Self {
        Self {
            x,
            y,
            diameter,
            color,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderText {
    pub x: i32,
    pub y: i32,
    pub text: String,
    pub color: u32,
}

impl RenderText {
    #[must_use]
    pub fn new(x: i32, y: i32, text: impl Into<String>, color: RenderColor) -> Self {
        Self {
            x,
            y,
            text: text.into(),
            color: color.to_argb_u32(),
        }
    }
}

impl RenderRect {
    #[must_use]
    pub fn cursor_horizontal_with_color(x: i32, y: i32, color: RenderColor) -> Self {
        Self {
            x: x - 6,
            y,
            width: 13,
            height: 1,
            color,
        }
    }

    #[must_use]
    pub fn cursor_vertical_with_color(x: i32, y: i32, color: RenderColor) -> Self {
        Self {
            x,
            y: y - 6,
            width: 1,
            height: 13,
            color,
        }
    }
}

impl RenderColor {
    #[must_use]
    pub fn white() -> Self {
        Self {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn focused_border() -> Self {
        Self {
            red: 0.7,
            green: 1.0,
            blue: 0.25,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn window_frame() -> Self {
        Self {
            red: 0.08,
            green: 0.10,
            blue: 0.11,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn titlebar() -> Self {
        Self {
            red: 0.13,
            green: 0.16,
            blue: 0.17,
            alpha: 1.0,
        }
    }

    #[must_use]
    pub fn title_text() -> Self {
        Self {
            red: 0.86,
            green: 0.92,
            blue: 0.88,
            alpha: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderFrame {
    output_name: String,
}

#[derive(Debug, Default)]
pub struct RenderBackend {
    current_frame: Option<RenderFrame>,
}

impl RenderBackend {
    #[must_use]
    pub fn new() -> Self {
        println!("Render backend initialized");

        Self {
            current_frame: None,
        }
    }

    pub fn begin_frame(&mut self, output: &Output) {
        self.current_frame = Some(RenderFrame {
            output_name: output.name().to_owned(),
        });
    }

    pub fn clear(&mut self, _color: RenderColor) {}

    pub fn draw_rect(&mut self, _rect: RenderRect) {}

    pub fn finish_frame(&mut self) {
        self.current_frame = None;
    }
}

#[derive(Debug, Default)]
pub struct RenderState;

fn color_channel_to_u32(value: f32) -> u32 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u32
}
