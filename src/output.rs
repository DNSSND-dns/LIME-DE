use std::fmt;

pub const DEFAULT_OUTPUT_WIDTH: u32 = 1280;
pub const DEFAULT_OUTPUT_HEIGHT: u32 = 720;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputId(u64);

impl OutputId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for OutputId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub id: OutputId,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub refresh_rate: u32,
}

impl Output {
    #[must_use]
    pub fn new(
        id: OutputId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        scale: f64,
        refresh_rate: u32,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            width,
            height,
            scale,
            refresh_rate,
        }
    }

    #[must_use]
    pub fn virtual_default() -> Self {
        Self::new(
            OutputId::new(1),
            "LIME-Virtual-1",
            DEFAULT_OUTPUT_WIDTH,
            DEFAULT_OUTPUT_HEIGHT,
            1.0,
            60_000,
        )
    }

    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        if self.width == width && self.height == height {
            return false;
        }

        self.width = width;
        self.height = height;
        println!(
            "Output resized: {} {}x{}",
            self.name, self.width, self.height
        );

        true
    }
}

#[derive(Debug, Default)]
pub struct OutputState;
