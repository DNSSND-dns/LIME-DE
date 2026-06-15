use crate::config::StyleConfig;

use super::{
    dock::{Dock, DockItem},
    panel::Panel,
    DockStyle, PanelStyle,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    dock: Dock,
    panel: Panel,
}

impl Shell {
    #[must_use]
    pub fn new(style: &StyleConfig) -> Self {
        Self {
            dock: Dock::new(DockStyle::from_config(&style.dock)),
            panel: Panel::new(PanelStyle::from_config(&style.panel)),
        }
    }

    pub fn layout(&mut self, output_width: u32, output_height: u32) {
        self.dock.layout(output_width, output_height);
        self.panel.layout(output_width, output_height);
    }

    pub fn update_hover(&mut self, x: f64, y: f64) -> bool {
        self.dock.update_hover(x, y) | self.panel.update_hover(x, y)
    }

    #[must_use]
    pub fn dock(&self) -> &Dock {
        &self.dock
    }

    pub fn dock_mut(&mut self) -> &mut Dock {
        &mut self.dock
    }

    #[must_use]
    pub fn panel(&self) -> &Panel {
        &self.panel
    }

    #[must_use]
    pub fn reserved_top_height(&self) -> i32 {
        self.panel.height()
    }

    #[must_use]
    pub fn panel_contains(&self, x: f64, y: f64) -> bool {
        x >= 0.0 && y >= 0.0 && y < f64::from(self.reserved_top_height())
    }

    #[must_use]
    pub fn dock_item_at(&self, x: f64, y: f64) -> Option<&DockItem> {
        self.dock.item_at(x, y)
    }
}
