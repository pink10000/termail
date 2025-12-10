pub mod backends;
pub mod error;
pub mod config;
pub mod auth;
pub mod cli;
pub mod ui;
pub mod plugins;
pub mod maildir;
pub mod core;
use plugins::plugins::PluginManager;
use clap::{Parser, ArgAction};
use backends::{BackendType, Backend};
use cli::command::Command;
use config::Config;
use ui::app::App;
use std::path::PathBuf;
use std::sync::Arc;

async fn create_authenticated_backend(config: &Config) -> Box<dyn Backend> {
    let mut backend: Box<dyn Backend> = config.get_backend();
    
    if backend.needs_oauth() {
        if let Err(e) = backend.authenticate().await {
            eprintln!("Authentication failed: {}", e);
            std::process::exit(1);
        }
    }
    backend
}

#[derive(Parser, Debug)]
pub struct Args {
    /// Use cli mode instead of tui
    #[arg(long, action = ArgAction::SetTrue)]
    cli: bool,

    /// Use a specific email backend (available: greenmail, gmail)
    #[arg(long, value_parser = clap::value_parser!(BackendType))]
    backend: Option<BackendType>,

    /// The command to execute
    #[command(subcommand)]
    command: Option<Command>,

    /// Config file location
    #[arg(long, value_parser = clap::value_parser!(PathBuf))]
    config_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let mut config = Config::load(args.config_file.clone()).unwrap_or_else(|e| {
        eprintln!("Error loading config: {}", e);
        std::process::exit(1);
    });
    config.merge(&args);
    
    let mut plugin_manager = PluginManager::new().unwrap();
    let enabled_plugins = config.termail.plugins.clone();

    if config.termail.cli {
        if let Err(code) = run_cli(
            args.command, 
            config, 
            &mut plugin_manager, 
            &enabled_plugins
        ).await {
            std::process::exit(code);
        }
        return;
    }

    if let Err(code) = run_tui(
        config, 
        plugin_manager, 
        enabled_plugins
    ).await {
        std::process::exit(code);
    }
}

async fn run_tui(
    config: Config,
    plugin_manager: PluginManager,
    enabled_plugins: Vec<String>,
) -> Result<(), i32> {
    let backend: Box<dyn Backend> = create_authenticated_backend(&config).await;
    let terminal = ratatui::init();
    let app = App::new(config, backend, plugin_manager);

    let plugin_loader_manager = Arc::clone(&app.plugin_manager);
    tokio::spawn(async move {
        let mut manager = plugin_loader_manager.lock().await;
        let _ = manager.load_plugins(&enabled_plugins);
    });

    let tui_result = app.run(terminal).await;
    ratatui::restore();
    match tui_result {
        Ok(_) => {
            println!("TUI exited successfully");
            Ok(())
        }
        Err(e) => {
            eprintln!("TUI error: {}", e);
            Err(1)
        }
    }
}

async fn run_cli(
    command: Option<Command>,
    config: Config,
    plugin_manager: &mut PluginManager,
    enabled_plugins: &[String],
) -> Result<(), i32> {
    let command = match command {
        Some(cmd) => cmd,
        None => {
            eprintln!("Missing Subcommand for CLI mode.");
            return Err(1);
        }
    };

    match plugin_manager.load_plugins(enabled_plugins) {
        Ok(count) => println!("Loaded successfully: {} plugins", count),
        Err(e) => {
            eprintln!("Error loading plugins: {}", e);
            return Err(1);
        }
    }

    // Some commands do not require authentication. In particular, we might just want to read
    // from Maildir directly, so we can create a backend that does not require authentication
    // and only do the authentication if we need to. 
    // 
    // The commands that require authentication should be defined by the particular backennd 
    // implementations. 
    let mut backend = config.get_backend();
    match backend.requires_authentication(&command) {
        Some(true) => {
            backend.authenticate().await.unwrap_or_else(|e| {
                eprintln!("Authentication failed: {}", e);
                std::process::exit(1);
            });
        },
        Some(false) => {}
        None => {
            println!("Command undefined for authentication.");
            println!("Executing command without authentication.");
        }
    }
    
    println!("Backend Created: {}", config.termail.default_backend);
    match backend.do_command(command, Some(plugin_manager)).await {
        Ok(result) => {
            println!("RESULT:\n{}", result);
            Ok(())
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(1)
        }
    }
}
