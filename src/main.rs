use anyhow::Result;
use clap::Parser;
use tracing::info;

use mitm2openapi::cli::{Cli, Command};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Command::Discover(args) => {
            info!(input = %args.input.display(), output = %args.output.display(), "Starting discovery");
            eprintln!(
                "discover: input={}, output={}, prefix={}",
                args.input.display(),
                args.output.display(),
                args.prefix
            );
        }
        Command::Generate(args) => {
            info!(input = %args.input.display(), output = %args.output.display(), "Starting generation");
            eprintln!(
                "generate: input={}, output={}, templates={}, prefix={}",
                args.input.display(),
                args.output.display(),
                args.templates.display(),
                args.prefix
            );
        }
    }

    Ok(())
}
