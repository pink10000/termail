pub mod imap_client;

use clap::Parser;

use imap_client::fetch_inbox_top;

#[derive(Parser, Debug)]
struct Args {

    /// Use cli mode instead of tui
    #[arg(long, action)]
    cli: bool,
}

fn main() {
    let args = Args::parse();

    let inbox = match fetch_inbox_top() {
        Ok(x) => match x {
            Some(s) => s,
            None => "NO EMAILS FOUND".to_string(),
        },
        Err(e) => {
            eprintln!("Error fetching inbox: {}", e);
            std::process::exit(1);
        }
    };

    println!("{inbox}");

    if !args.cli {
        unimplemented!("tui mode not implemented yet");
    }
}
