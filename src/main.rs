pub mod imap_client;
pub mod backends;

use clap::Parser;
use backends::{BackendType, Backend, Command};

#[derive(Parser, Debug)]
struct Args {
    /// Use cli mode instead of tui
    #[arg(long, action)]
    cli: bool,

    /// Use a specific email backend (available: greenmail, gmail)
    #[arg(long, value_parser = clap::value_parser!(BackendType))]
    backend: BackendType,

    /// The command to execute
    #[command(subcommand)]
    command: Command,
}

fn main() {
    let args = Args::parse();

    if !args.cli {
        unimplemented!("tui mode not implemented yet");
    }

    let backend: Box<dyn Backend> = args.backend.get_backend();
    
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
