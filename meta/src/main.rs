use metarepo::MetarepoCli;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cli = MetarepoCli::new();

    if let Err(e) = cli.run(args) {
        // Check if this is a clap error for help or version
        if let Some(clap_err) = e.downcast_ref::<clap::Error>() {
            match clap_err.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    // These are not actual errors, just display and exit normally
                    clap_err.print().expect("Failed to print clap output");
                    process::exit(0);
                }
                _ => {
                    eprintln!("Error: {}", e);
                    process::exit(1);
                }
            }
        } else {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}
