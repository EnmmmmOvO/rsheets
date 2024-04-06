use std::error::Error;

use clap::Parser;
use rsheet::start_server;
use rsheet_lib::connect::{resolve_address, ConnectionManager, TerminalManager};

#[derive(Parser, Debug)]
struct Args {
    /// Address to listen on
    addr: Option<String>,

    /// Hides the contents of error messages
    #[arg(short, long, default_value_t = false)]
    mark_mode: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let args = Args::parse();

    if let Some(addr) = args.addr {
        let addr = resolve_address(&addr)?;
        let manager = ConnectionManager::launch(addr.ip(), addr.port());
        start_server(manager)
    } else {
        let manager = TerminalManager::launch(args.mark_mode);
        start_server(manager)
    }
}
