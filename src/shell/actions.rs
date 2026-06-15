use crate::window::WindowId;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    Launch(Vec<String>),
    MinimizeWindow(WindowId),
    RestoreWindow(WindowId),
    CloseWindow(WindowId),
    ToggleMaximizeWindow(WindowId),
}
