use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// Example value.
    value: Option<String>,
}

fn main() {
    let _ = Cli::parse();
}
