use crate::{compositor::Compositor, config::Config, error::AppError, state::AppState};

#[derive(Debug)]
pub struct App {
    state: AppState,
    compositor: Compositor,
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        let mut config = load_config_from_cli_or_default();
        apply_cli_overrides(&mut config);
        let backend = config.backend();
        let launch_test_client = config.launch_test_client();
        let test_client_commands = config.test_client_commands().to_vec();
        let style = config.style.clone();
        let behavior = config.behavior.clone();
        let animations = config.animations.clone();
        let state = AppState::new(config);
        let compositor = Compositor::new(
            backend,
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

fn load_config_from_cli_or_default() -> Config {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(path) = args.next() {
                return Config::load_from_path(&path).unwrap_or_else(|error| {
                    eprintln!("LIME DE config fallback: {error}");
                    Config::default()
                });
            }
        }
    }

    Config::load_or_default()
}

fn apply_cli_overrides(config: &mut Config) {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let _ = args.next();
            }
            "--backend" => {
                if let Some(backend) = args.next() {
                    config.runtime.backend = Some(backend);
                }
            }
            "--tty" | "--native2" => {
                eprintln!("warning: {arg} is deprecated; use --backend native");
                config.runtime.backend = Some(String::from("native"));
            }
            "--winit" => {
                config.runtime.backend = Some(String::from("dev-winit"));
            }
            "--no-test-client" => {
                config.runtime.launch_test_client = false;
            }
            "--test-client" => {
                config.runtime.launch_test_client = true;
            }
            _ => {}
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
