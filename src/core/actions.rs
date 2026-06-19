#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    Close,
    Minimize,
    Restore,
    ToggleMaximize,
}
