use crate::{compositor::Compositor, config::Config, error::AppError, state::AppState};

#[derive(Debug)]
pub struct App {
    state: AppState,
    compositor: Compositor,
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        let config = Config::load_or_default();
        let use_winit_test_backend = config.use_winit_test_backend();
        let launch_test_client = config.launch_test_client();
        let test_client_commands = config.test_client_commands().to_vec();
        let style = config.style.clone();
        let behavior = config.behavior.clone();
        let animations = config.animations.clone();
        let state = AppState::new(config);
        let compositor = Compositor::new(
            use_winit_test_backend,
            launch_test_client,
            test_client_commands,
            style,
            behavior,
            animations,
        );

        Self { state, compositor }
    }

    pub fn initialize(&mut self) -> Result<(), AppError> {
        println!("{} initializing", self.state.app_name());

        if !self.state.modules_ready() {
            return Err(AppError::new("application modules are not ready"));
        }

        self.compositor.initialize()
    }

    pub fn run(&mut self) -> Result<(), AppError> {
        println!("{} running", self.state.app_name());

        self.compositor.run()
    }

    pub fn shutdown(&mut self) -> Result<(), AppError> {
        println!("{} shutting down", self.state.app_name());

        self.compositor.shutdown()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
