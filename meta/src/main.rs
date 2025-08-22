use meta::MetaCli;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = MetaCli::new();
    
    if let Err(e) = cli.run(args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
