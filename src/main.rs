pub mod imap_client;
pub mod backends;
pub mod error;
pub mod config;
pub mod auth;

use clap::Parser;
use backends::{BackendType, Backend, Command};
use config::Config;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Args {
    /// Use cli mode instead of tui
    #[arg(long, action)]
    cli: bool,

    /// Use a specific email backend (available: greenmail, gmail)
    #[arg(long, value_parser = clap::value_parser!(BackendType))]
    backend: BackendType,

    /// The command to execute
    #[command(subcommand)]
    command: Command,

    /// Config file location
    #[arg(long, value_parser = clap::value_parser!(PathBuf))]
    config_file: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let mut config = Config::load(args.config_file.clone());
    config.merge(&args);

    if !config.termail.cli {
        unimplemented!("tui mode not implemented yet");
    }

    let backend: Box<dyn Backend> = config.termail.default_backend.get_backend();
    
    // Execute the command using the selected backend
    let result = match backend.do_command(args.command) {
        Ok(Some(s)) => s,
        Ok(None) => "NO EMAILS FOUND".to_string(),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    println!("{}", result);
}
