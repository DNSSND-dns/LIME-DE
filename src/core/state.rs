use crate::{
    backend::BackendState, config::Config, input::InputState, output::OutputState,
    render::RenderState, window::WindowState,
};

use super::layout::LayoutState;

#[derive(Debug)]
pub struct AppState {
    pub config: Config,
    pub backend: BackendState,
    pub input: InputState,
    pub layout: LayoutState,
    pub output: OutputState,
    pub render: RenderState,
    pub windows: WindowState,
}

impl AppState {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            backend: BackendState,
            input: InputState,
            layout: LayoutState,
            output: OutputState,
            render: RenderState,
            windows: WindowState,
        }
    }

    #[must_use]
    pub fn app_name(&self) -> &str {
        &self.config.app_name
    }

    #[must_use]
    pub fn modules_ready(&self) -> bool {
        let _modules = (
            &self.backend,
            &self.input,
            &self.layout,
            &self.output,
            &self.render,
            &self.windows,
        );

        true
    }
}
