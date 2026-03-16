use ona_rust::cli;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Err(e) = cli::run(&args) {
        if !e.is_empty() {
            eprintln!("{e}");
        }
        process::exit(1);
    }
}
