#[derive(Debug, Default)]
pub struct InputState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorState {
    pub x: f64,
    pub y: f64,
    pub visible: bool,
}

impl CursorState {
    #[must_use]
    pub fn centered(width: u32, height: u32) -> Self {
        Self {
            x: f64::from(width) / 2.0,
            y: f64::from(height) / 2.0,
            visible: true,
        }
    }

    pub fn move_to(&mut self, x: f64, y: f64, width: u32, height: u32) {
        self.x = x.clamp(0.0, f64::from(width.saturating_sub(1)));
        self.y = y.clamp(0.0, f64::from(height.saturating_sub(1)));
    }
}
