pub mod backends;
pub mod error;
pub mod config;
pub mod auth;
pub mod types;
pub mod ui;

use clap::{Parser, ArgAction};
use backends::{BackendType, Backend};
use types::Command;
use config::Config;
use ui::app::App;

use std::path::PathBuf;

async fn create_authenticated_backend(config: &Config) -> Box<dyn Backend> {
    let backend_type = config.termail.default_backend;
    let mut backend: Box<dyn Backend> = config.get_backend();
    
    if backend_type.needs_oauth() {
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
    
    if !config.termail.cli {
        let backend: Box<dyn Backend> = create_authenticated_backend(&config).await;
        let terminal = ratatui::init();
        let tui_result = App::new(config, backend).run(terminal).await;
        match tui_result {
            Ok(_) => println!("TUI exited successfully"),
            Err(e) => eprintln!("TUI error: {}", e),
        }
        ratatui::restore();
        std::process::exit(0);
    }

    match args.command {
        None => {
            eprintln!("Missing Subcommand for CLI mode.");
            std::process::exit(1);
        }
        Some(_) => {}
    }
        
    let backend_type = config.termail.default_backend;
    let mut backend: Box<dyn Backend> = config.get_backend();
    
    if backend_type.needs_oauth() {
        if let Err(e) = backend.authenticate().await {
            eprintln!("Authentication failed: {}", e);
            std::process::exit(1);
        }
    }
    
    // Execute the command using the selected backend
    let result = match backend.do_command(args.command.unwrap()).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };    
    println!("RESULT:\n{}", result);
}
