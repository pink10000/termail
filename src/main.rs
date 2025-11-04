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

use std::path::PathBuf;

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
    let mut config = Config::load(args.config_file.clone()).unwrap();
    config.merge(&args);
    
    if !config.termail.cli {
        let terminal = ratatui::init();
        let tui_result = crate::ui::app::App::new(config).run(terminal).await;
        match tui_result {
            Ok(_) => println!("TUI exited successfully"),
            Err(e) => eprintln!("TUI error: {}", e),
        }
        ratatui::restore();
        std::process::exit(0);
    }

    let backend_type = config.termail.default_backend;
    let mut backend: Box<dyn Backend> = config.get_backend();
    
    if backend_type.needs_oauth() {
        if let Err(e) = backend.authenticate() {
            eprintln!("Authentication failed: {}", e);
            std::process::exit(1);
        }
    }
    
    // Execute the command using the selected backend
    let result = match backend.do_command(args.command.unwrap()) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };    
    println!("RESULT:\n{}", result);
}
