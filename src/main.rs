use clap::Parser;
use std::path::PathBuf;
use update_chat_types::update_types_in_place;

/// Update CHAT files with correct @Types header.
#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Root directory containing CHAT files to modify.
    #[clap(long)]
    chat_dir: PathBuf,

    /// Whether to only output what would be done.
    #[clap(long)]
    dry_run: bool,
}

fn main() {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(args: &Args) -> anyhow::Result<()> {
    let num_updated = update_types_in_place(&args.chat_dir, args.dry_run)?;
    println!(
        "{} {} CHAT files.",
        if args.dry_run {
            "Would update"
        } else {
            "Updated"
        },
        num_updated
    );
    Ok(())
}
