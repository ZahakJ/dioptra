//! `dioptra` command line.
use clap::Parser;

#[derive(Parser)]
#[command(name = "dioptra", version, about = "Dioptra engine command line")]
struct Cli {}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("dioptra {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
