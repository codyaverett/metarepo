use gestalt::GestaltCli;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = GestaltCli::new();

    if let Err(e) = cli.run(args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
