use std::{fs, path::Path};

use serde::Deserialize;

pub const DEFAULT_CONFIG_PATH: &str = "config/lime.toml";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub app_name: String,
    pub runtime: RuntimeConfig,
    pub style: StyleConfig,
    pub behavior: BehaviorConfig,
    pub animations: AnimationConfig,
}

impl Config {
    #[must_use]
    pub fn load_or_default() -> Self {
        match Self::load_from_path(DEFAULT_CONFIG_PATH) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("LIME DE config fallback: {error}");
                Self::default()
            }
        }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;

        toml::from_str(&contents)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))
    }

    #[must_use]
    pub fn use_winit_test_backend(&self) -> bool {
        self.runtime.use_winit_test_backend
    }

    #[must_use]
    pub fn launch_test_client(&self) -> bool {
        self.runtime.launch_test_client
    }

    #[must_use]
    pub fn test_client_commands(&self) -> &[String] {
        &self.runtime.test_client_commands
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: String::from("LIME DE"),
            runtime: RuntimeConfig::default(),
            style: StyleConfig::default(),
            behavior: BehaviorConfig::default(),
            animations: AnimationConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub use_winit_test_backend: bool,
    pub launch_test_client: bool,
    pub test_client_commands: Vec<String>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            use_winit_test_backend: true,
            launch_test_client: true,
            test_client_commands: vec![
                String::from("foot"),
                String::from("weston-terminal"),
                String::from("alacritty"),
                String::from("kitty"),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    pub window: WindowStyleConfig,
    pub dock: DockStyleConfig,
    pub panel: PanelStyleConfig,
    pub colors: ColorStyleConfig,
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            window: WindowStyleConfig::default(),
            dock: DockStyleConfig::default(),
            panel: PanelStyleConfig::default(),
            colors: ColorStyleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WindowStyleConfig {
    pub titlebar_height: i32,
    pub border_width: i32,
    pub corner_radius: i32,
    pub bottom_corner_radius: i32,
    pub button_diameter: i32,
    pub button_spacing: i32,
    pub button_left_padding: i32,
}

impl Default for WindowStyleConfig {
    fn default() -> Self {
        Self {
            titlebar_height: 34,
            border_width: 1,
            corner_radius: 10,
            bottom_corner_radius: 0,
            button_diameter: 12,
            button_spacing: 8,
            button_left_padding: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct DockStyleConfig {
    pub bubble_size: i32,
    pub bubble_gap: i32,
    pub bottom_margin: i32,
    pub bubble_radius: i32,
    pub active_scale: f32,
    pub hover_scale: f32,
}

impl Default for DockStyleConfig {
    fn default() -> Self {
        Self {
            bubble_size: 56,
            bubble_gap: 14,
            bottom_margin: 24,
            bubble_radius: 18,
            active_scale: 1.10,
            hover_scale: 1.18,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct PanelStyleConfig {
    pub height: i32,
    pub item_width: i32,
    pub item_spacing: i32,
    pub left_margin: i32,
    pub right_margin: i32,
    pub top_margin: i32,
    pub radius: i32,
}

impl Default for PanelStyleConfig {
    fn default() -> Self {
        Self {
            height: 28,
            item_width: 80,
            item_spacing: 12,
            left_margin: 16,
            right_margin: 16,
            top_margin: 8,
            radius: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ColorStyleConfig {
    pub background: String,
    pub window_frame: String,
    pub titlebar: String,
    pub border_focused: String,
    pub title_text: String,
    pub placeholder_window: String,
    pub cursor: String,
    pub close_button: String,
    pub minimize_button: String,
    pub maximize_button: String,
    pub dock_bubble: String,
    pub dock_active: String,
    pub dock_text: String,
    pub panel_background: String,
    pub panel_text: String,
}

impl Default for ColorStyleConfig {
    fn default() -> Self {
        Self {
            background: String::from("#000000"),
            window_frame: String::from("#14191c"),
            titlebar: String::from("#22282b"),
            border_focused: String::from("#b3ff40"),
            title_text: String::from("#dbeae0"),
            placeholder_window: String::from("#1a8cf2"),
            cursor: String::from("#ffffff"),
            close_button: String::from("#ff5f57"),
            minimize_button: String::from("#ffbd2e"),
            maximize_button: String::from("#28c840"),
            dock_bubble: String::from("#202629"),
            dock_active: String::from("#b3ff40"),
            dock_text: String::from("#dbeae0"),
            panel_background: String::from("#202629"),
            panel_text: String::from("#dbeae0"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub windows: WindowBehaviorConfig,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            windows: WindowBehaviorConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WindowBehaviorConfig {
    pub resize_model: String,
    pub client_buffer_mode: String,
    pub allow_client_geometry_before_user_resize: bool,
    pub accept_client_geometry_during_live_resize: bool,
    pub send_configure_during_live_resize: bool,
    pub send_configure_on_resize_release: bool,
}

impl WindowBehaviorConfig {
    #[must_use]
    pub fn keeps_user_resized_geometry_fixed(&self) -> bool {
        self.resize_model == "fixed_after_user_resize"
    }

    #[must_use]
    pub fn fits_client_buffer_to_window(&self) -> bool {
        self.client_buffer_mode == "fit_to_window"
    }
}

impl Default for WindowBehaviorConfig {
    fn default() -> Self {
        Self {
            resize_model: String::from("fixed_after_user_resize"),
            client_buffer_mode: String::from("fit_to_window"),
            allow_client_geometry_before_user_resize: true,
            accept_client_geometry_during_live_resize: false,
            send_configure_during_live_resize: true,
            send_configure_on_resize_release: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub window_open_ms: u32,
    pub window_close_ms: u32,
    pub resize_ms: u32,
    pub curve: String,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            window_open_ms: 120,
            window_close_ms: 90,
            resize_ms: 80,
            curve: String::from("ease-out"),
        }
    }
}
