use std::{env, fs, path::Path};

pub const CONFIG_ENV_VAR: &str = "LIME_CONFIG";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimeUiTheme {
    pub background: u32,
    pub surface: u32,
    pub surface_hover: u32,
    pub accent: u32,
    pub text: u32,
    pub muted_text: u32,
    pub file: u32,
    pub radius: i32,
}

impl LimeUiTheme {
    #[must_use]
    pub fn load() -> Self {
        let mut theme = Self::default();
        let path = env::var(CONFIG_ENV_VAR).unwrap_or_else(|_| "config/lime.toml".to_string());
        let Ok(contents) = fs::read_to_string(Path::new(&path)) else {
            return theme;
        };
        let Ok(document) = contents.parse::<toml::Table>() else {
            return theme;
        };
        let colors = document
            .get("style")
            .and_then(toml::Value::as_table)
            .and_then(|style| style.get("colors"))
            .and_then(toml::Value::as_table);
        let window = document
            .get("style")
            .and_then(toml::Value::as_table)
            .and_then(|style| style.get("window"))
            .and_then(toml::Value::as_table);

        if let Some(colors) = colors {
            theme.background = color(colors, "background", theme.background);
            theme.surface = color(colors, "panel_background", theme.surface);
            theme.surface_hover = color(colors, "titlebar", theme.surface_hover);
            theme.accent = color(colors, "border_focused", theme.accent);
            theme.text = color(colors, "panel_text", theme.text);
            theme.muted_text = color(colors, "title_text", theme.muted_text);
            theme.file = color(colors, "placeholder_window", theme.file);
        }
        if let Some(radius) = window
            .and_then(|window| window.get("corner_radius"))
            .and_then(toml::Value::as_integer)
        {
            theme.radius = i32::try_from(radius).unwrap_or(theme.radius).max(0);
        }

        theme
    }
}

impl Default for LimeUiTheme {
    fn default() -> Self {
        Self {
            background: 0xff000000,
            surface: 0xff202629,
            surface_hover: 0xff22282b,
            accent: 0xffb3ff40,
            text: 0xffdbeae0,
            muted_text: 0xffaebdc2,
            file: 0xff1a8cf2,
            radius: 10,
        }
    }
}

fn color(table: &toml::Table, key: &str, fallback: u32) -> u32 {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .and_then(parse_color)
        .unwrap_or(fallback)
}

fn parse_color(value: &str) -> Option<u32> {
    let value = value.strip_prefix('#').unwrap_or(value);
    (value.len() == 6)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
        .map(|rgb| 0xff00_0000 | rgb)
}

pub struct PixelCanvas<'a> {
    bytes: &'a mut [u8],
    width: u32,
    height: u32,
}

impl<'a> PixelCanvas<'a> {
    #[must_use]
    pub fn new(bytes: &'a mut [u8], width: u32, height: u32) -> Self {
        Self {
            bytes,
            width,
            height,
        }
    }

    pub fn clear(&mut self, color: u32) {
        for pixel in self.bytes.chunks_exact_mut(4) {
            pixel.copy_from_slice(&color.to_ne_bytes());
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: u32) {
        let x0 = x.max(0).min(self.width as i32);
        let y0 = y.max(0).min(self.height as i32);
        let x1 = x.saturating_add(width).max(0).min(self.width as i32);
        let y1 = y.saturating_add(height).max(0).min(self.height as i32);
        for py in y0..y1 {
            for px in x0..x1 {
                self.pixel(px, py, color);
            }
        }
    }

    pub fn rounded_rect(
        &mut self,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        radius: i32,
        color: u32,
    ) {
        let radius = radius.max(0).min(width / 2).min(height / 2);
        for py in y.max(0)..y.saturating_add(height).min(self.height as i32) {
            for px in x.max(0)..x.saturating_add(width).min(self.width as i32) {
                let dx = if px < x + radius {
                    x + radius - px
                } else if px >= x + width - radius {
                    px - (x + width - radius - 1)
                } else {
                    0
                };
                let dy = if py < y + radius {
                    y + radius - py
                } else if py >= y + height - radius {
                    py - (y + height - radius - 1)
                } else {
                    0
                };
                if dx == 0 || dy == 0 || dx * dx + dy * dy <= radius * radius {
                    self.pixel(px, py, color);
                }
            }
        }
    }

    pub fn text(&mut self, x: i32, y: i32, text: &str, color: u32) {
        let mut cursor = x;
        for character in text.chars() {
            self.glyph(cursor, y, character, color);
            cursor += 8;
        }
    }

    fn glyph(&mut self, x: i32, y: i32, character: char, color: u32) {
        for (row, bits) in glyph(character).iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    self.rect(x + column, y + row as i32 * 2, 1, 2, color);
                }
            }
        }
    }

    fn pixel(&mut self, x: i32, y: i32, color: u32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        self.bytes[offset..offset + 4].copy_from_slice(&color.to_ne_bytes());
    }
}

fn glyph(character: char) -> [u8; 7] {
    match character.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [15, 16, 16, 16, 16, 16, 15],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [15, 16, 16, 19, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '<' => [2, 4, 8, 16, 8, 4, 2],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        ' ' => [0; 7],
        _ => [31, 17, 2, 4, 4, 0, 4],
    }
}
