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
    let updated_files = update_types_in_place(&args.chat_dir, args.dry_run)?;
    let verb = if args.dry_run {
        "Would update"
    } else {
        "Updated"
    };
    let n = updated_files.len();
    if n == 0 {
        println!("{verb} 0 CHAT files.");
    } else {
        println!("{verb} {n} CHAT files:");
        for path in &updated_files {
            if let Ok(rel) = path.strip_prefix(&args.chat_dir) {
                println!("  {}", rel.display());
            } else {
                println!("  {}", path.display());
            }
        }
    }
    Ok(())
}
