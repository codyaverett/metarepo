use metarepo::MetarepoCli;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = MetarepoCli::new();
    
    if let Err(e) = cli.run(args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
