use std::fmt;

use crate::config::PanelStyleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelItemId(u64);

impl PanelItemId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for PanelItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelPosition {
    #[default]
    TopLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelStyle {
    pub height: i32,
    pub item_width: i32,
    pub item_spacing: i32,
    pub left_margin: i32,
    pub right_margin: i32,
    pub top_margin: i32,
    pub radius: i32,
}

impl PanelStyle {
    #[must_use]
    pub fn from_config(config: &PanelStyleConfig) -> Self {
        Self {
            height: config.height.max(1),
            item_width: config.item_width.max(1),
            item_spacing: config.item_spacing.max(0),
            left_margin: config.left_margin.max(0),
            right_margin: config.right_margin.max(0),
            top_margin: config.top_margin.max(0),
            radius: config.radius.max(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelItem {
    pub id: PanelItemId,
    pub label: String,
    pub icon: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub hovered: bool,
    pub active: bool,
}

impl PanelItem {
    #[must_use]
    pub fn new(id: PanelItemId, label: impl Into<String>, icon: Option<String>) -> Self {
        Self {
            id,
            label: label.into(),
            icon,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            hovered: false,
            active: false,
        }
    }

    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x + self.width)
            && y < f64::from(self.y + self.height)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Panel {
    pub position: PanelPosition,
    pub style: PanelStyle,
    left_section: Vec<PanelItem>,
    center_section: Vec<PanelItem>,
    right_section: Vec<PanelItem>,
}

impl Panel {
    #[must_use]
    pub fn new(style: PanelStyle) -> Self {
        let mut panel = Self {
            position: PanelPosition::TopLeft,
            style,
            left_section: Vec::new(),
            center_section: Vec::new(),
            right_section: Vec::new(),
        };

        // Initialize with default macOS-style items
        panel.left_section.push(PanelItem::new(
            PanelItemId::new(1),
            "Applications",
            Some("apple".to_string()),
        ));

        panel.center_section.push(PanelItem::new(
            PanelItemId::new(2),
            "Clock",
            Some("clock".to_string()),
        ));

        panel.right_section.push(PanelItem::new(
            PanelItemId::new(3),
            "Battery",
            Some("battery".to_string()),
        ));
        panel.right_section.push(PanelItem::new(
            PanelItemId::new(4),
            "WiFi",
            Some("wifi".to_string()),
        ));
        panel.right_section.push(PanelItem::new(
            PanelItemId::new(5),
            "Volume",
            Some("volume".to_string()),
        ));

        panel
    }

    pub fn layout(&mut self, output_width: u32, _output_height: u32) {
        let panel_y = self.style.top_margin;
        let panel_height = self.style.height;

        // Layout left section
        let mut current_x = self.style.left_margin;
        for item in &mut self.left_section {
            item.height = panel_height;
            item.width = self.style.item_width;
            item.x = current_x;
            item.y = panel_y;
            current_x += item.width + self.style.item_spacing;
        }

        // Layout center section
        let center_total_width = self.center_section.len() as i32
            * (self.style.item_width + self.style.item_spacing)
            - self.style.item_spacing;
        let mut center_x = ((output_width as i32) - center_total_width) / 2;

        for item in &mut self.center_section {
            item.height = panel_height;
            item.width = self.style.item_width;
            item.x = center_x;
            item.y = panel_y;
            center_x += item.width + self.style.item_spacing;
        }

        // Layout right section
        let right_total_width = self.right_section.len() as i32
            * (self.style.item_width + self.style.item_spacing)
            - self.style.item_spacing;
        let mut right_x = (output_width as i32) - self.style.right_margin - right_total_width;

        for item in &mut self.right_section {
            item.height = panel_height;
            item.width = self.style.item_width;
            item.x = right_x;
            item.y = panel_y;
            right_x += item.width + self.style.item_spacing;
        }
    }

    pub fn update_hover(&mut self, x: f64, y: f64) -> bool {
        let mut changed = false;

        for section in [
            &mut self.left_section,
            &mut self.center_section,
            &mut self.right_section,
        ] {
            for item in section.iter_mut() {
                let hovered = item.contains(x, y);
                if item.hovered != hovered {
                    item.hovered = hovered;
                    changed = true;
                }
            }
        }

        changed
    }

    #[must_use]
    pub fn item_at(&self, x: f64, y: f64) -> Option<&PanelItem> {
        all_sections(
            &self.left_section,
            &self.center_section,
            &self.right_section,
        )
        .into_iter()
        .find(|item| item.contains(x, y))
    }

    #[must_use]
    pub fn items(&self) -> Vec<&PanelItem> {
        all_sections(
            &self.left_section,
            &self.center_section,
            &self.right_section,
        )
    }

    #[must_use]
    pub fn height(&self) -> i32 {
        self.style.height + self.style.top_margin
    }
}

fn all_sections<'a>(
    left: &'a [PanelItem],
    center: &'a [PanelItem],
    right: &'a [PanelItem],
) -> Vec<&'a PanelItem> {
    left.iter()
        .chain(center.iter())
        .chain(right.iter())
        .collect()
}
