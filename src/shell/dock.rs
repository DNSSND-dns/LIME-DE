use std::fmt;

use crate::{config::DockStyleConfig, window::WindowGeometry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockItemId(u64);

impl DockItemId {
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for DockItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DockPosition {
    #[default]
    BottomCenter,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockStyle {
    pub bubble_size: i32,
    pub bubble_gap: i32,
    pub bottom_margin: i32,
    pub bubble_radius: i32,
    pub active_scale: f32,
    pub hover_scale: f32,
}

impl DockStyle {
    #[must_use]
    pub fn from_config(config: &DockStyleConfig) -> Self {
        Self {
            bubble_size: config.bubble_size.max(1),
            bubble_gap: config.bubble_gap.max(0),
            bottom_margin: config.bottom_margin.max(0),
            bubble_radius: config.bubble_radius.max(0),
            active_scale: config.active_scale.max(1.0),
            hover_scale: config.hover_scale.max(1.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockItem {
    pub id: DockItemId,
    pub app_id: String,
    pub aliases: Vec<String>,
    pub label: String,
    pub commands: Vec<String>,
    pub x: i32,
    pub y: i32,
    pub size: i32,
    pub hovered: bool,
    pub active: bool,
}

impl DockItem {
    #[must_use]
    pub fn new(id: DockItemId, app_id: impl Into<String>, label: impl Into<String>) -> Self {
        let app_id = app_id.into();
        let commands = default_commands_for_app(&app_id);
        let aliases = default_aliases_for_app(&app_id)
            .into_iter()
            .map(String::from)
            .collect();

        Self {
            id,
            app_id,
            aliases,
            label: label.into(),
            commands,
            x: 0,
            y: 0,
            size: 0,
            hovered: false,
            active: false,
        }
    }

    #[must_use]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && y >= f64::from(self.y)
            && x < f64::from(self.x + self.size)
            && y < f64::from(self.y + self.size)
    }
}

fn default_aliases_for_app(app_id: &str) -> Vec<&'static str> {
    match app_id {
        "terminal" => vec![
            "terminal",
            "foot",
            "weston-terminal",
            "alacritty",
            "kitty",
            "kgx",
            "gnome-terminal",
            "konsole",
            "xterm",
        ],
        "files" => vec![
            "files",
            "file-manager",
            "nautilus",
            "org.gnome.nautilus",
            "dolphin",
            "org.kde.dolphin",
            "thunar",
            "pcmanfm",
            "nemo",
        ],
        "browser" => vec![
            "browser",
            "firefox",
            "org.mozilla.firefox",
            "chromium",
            "google-chrome",
            "chrome",
            "brave",
            "brave-browser",
            "microsoft-edge",
        ],
        "settings" => vec!["settings", "lime-settings"],
        _ => vec![],
    }
}

fn default_commands_for_app(app_id: &str) -> Vec<String> {
    match app_id {
        "terminal" => ["foot", "weston-terminal", "alacritty", "kitty"]
            .into_iter()
            .map(String::from)
            .collect(),
        "files" => ["nautilus", "dolphin", "thunar", "pcmanfm"]
            .into_iter()
            .map(String::from)
            .collect(),
        "browser" => [
            "brave-browser --user-data-dir=/tmp/lime-de-brave --ozone-platform=wayland --no-first-run --no-default-browser-check",
            "brave --user-data-dir=/tmp/lime-de-brave --ozone-platform=wayland --no-first-run --no-default-browser-check",
            "firefox",
            "chromium --user-data-dir=/tmp/lime-de-chromium --ozone-platform=wayland --no-first-run --no-default-browser-check",
            "google-chrome --user-data-dir=/tmp/lime-de-chrome --ozone-platform=wayland --no-first-run --no-default-browser-check",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        "settings" => ["lime-settings"].into_iter().map(String::from).collect(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dock {
    pub position: DockPosition,
    pub style: DockStyle,
    items: Vec<DockItem>,
}

impl Dock {
    #[must_use]
    pub fn new(style: DockStyle) -> Self {
        Self {
            position: DockPosition::BottomCenter,
            style,
            items: vec![
                DockItem::new(DockItemId::new(1), "terminal", "Terminal"),
                DockItem::new(DockItemId::new(2), "files", "Files"),
                DockItem::new(DockItemId::new(3), "browser", "Brave"),
                DockItem::new(DockItemId::new(4), "settings", "Settings"),
            ],
        }
    }

    pub fn layout(&mut self, output_width: u32, output_height: u32) {
        if self.items.is_empty() {
            return;
        }

        let slot_size = self.style.bubble_size;
        let total_width = self.items.len() as i32 * slot_size
            + (self.items.len().saturating_sub(1) as i32 * self.style.bubble_gap);
        let start_x = (output_width as i32 - total_width).max(0) / 2;
        let slot_y =
            output_height as i32 - self.style.bottom_margin - slot_size.max(self.style.bubble_size);

        for (index, item) in self.items.iter_mut().enumerate() {
            let size = self.style.bubble_size;
            let slot_x = start_x + index as i32 * (slot_size + self.style.bubble_gap);

            item.size = size;
            item.x = slot_x + (slot_size - size) / 2;
            item.y = slot_y + (slot_size - size);
        }
    }

    pub fn update_hover(&mut self, x: f64, y: f64) -> bool {
        let mut changed = false;

        for item in &mut self.items {
            let hovered = item.contains(x, y);
            if item.hovered != hovered {
                item.hovered = hovered;
                changed = true;
            }
        }

        changed
    }

    #[must_use]
    pub fn item_at(&self, x: f64, y: f64) -> Option<&DockItem> {
        self.items.iter().find(|item| item.contains(x, y))
    }

    #[must_use]
    pub fn item_rect_for_app(&self, app_id: Option<&str>) -> Option<WindowGeometry> {
        self.matching_item(app_id).map(dock_item_rect)
    }

    pub fn set_active_for_app(&mut self, app_id: Option<&str>, active: bool) {
        if let Some(item) = self.matching_item_mut(app_id) {
            item.active = active;
        }
    }

    #[must_use]
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    #[must_use]
    pub fn app_matches_item(&self, item_id: DockItemId, app_id: &str) -> bool {
        self.items
            .iter()
            .find(|item| item.id == item_id)
            .is_some_and(|item| app_matches_dock_item(app_id, item))
    }

    fn matching_item(&self, app_id: Option<&str>) -> Option<&DockItem> {
        app_id.and_then(|app_id| {
            self.items
                .iter()
                .find(|item| app_matches_dock_item(app_id, item))
        })
    }

    fn matching_item_mut(&mut self, app_id: Option<&str>) -> Option<&mut DockItem> {
        if let Some(app_id) = app_id {
            if let Some(index) = self
                .items
                .iter()
                .position(|item| app_matches_dock_item(app_id, item))
            {
                return self.items.get_mut(index);
            }
        }

        None
    }
}

fn dock_item_rect(item: &DockItem) -> WindowGeometry {
    WindowGeometry {
        x: item.x,
        y: item.y,
        width: item.size,
        height: item.size,
    }
}

fn app_matches_dock_item(app_id: &str, item: &DockItem) -> bool {
    let app_id = app_id.to_ascii_lowercase();
    let item_app_id = item.app_id.to_ascii_lowercase();

    app_id_matches(&app_id, &item_app_id)
        || item
            .aliases
            .iter()
            .map(|alias| alias.to_ascii_lowercase())
            .any(|alias| app_id_matches(&app_id, &alias))
}

fn app_id_matches(app_id: &str, candidate: &str) -> bool {
    app_id == candidate
        || app_id.strip_suffix(".desktop") == Some(candidate)
        || candidate.strip_suffix(".desktop") == Some(app_id)
        || app_id
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix == candidate)
        || candidate
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix == app_id)
}
