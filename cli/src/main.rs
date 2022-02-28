use anyhow::Result;
use clap::{Parser, Subcommand};

mod decode;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    DecodeAccount,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::DecodeAccount => decode::decode_account(),
    }
}
