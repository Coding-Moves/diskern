use anyhow::Result;
use clap::{Parser, Subcommand};
use diskern_core::{report, rules::RulesDb, scanner};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "diskern", about = "Diskern — understand your disk before you clean it")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Read-only scan: find duplicates, caches, and reclaimable space.
    Scan {
        /// Directories to scan
        roots: Vec<PathBuf>,
        /// Emit full JSON report instead of a summary
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan { roots, json } => {
            let opts = scanner::ScanOptions { roots, ..Default::default() };
            let progress = Arc::new(scanner::ScanProgress::default());
            let entries = scanner::scan(&opts, progress)?;
            let report = report::build(entries, &RulesDb::embedded());

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Scanned {} files.", report.files_scanned);
                println!(
                    "Reclaimable: {:.2} GB across {} findings and {} duplicate sets.",
                    report.total_reclaimable as f64 / 1e9,
                    report.findings.len(),
                    report.duplicate_sets.len()
                );
                for set in report.duplicate_sets.iter().take(10) {
                    println!(
                        "  dup x{}  {:.1} MB wasted  {}",
                        set.paths.len(),
                        set.wasted as f64 / 1e6,
                        set.paths[0].display()
                    );
                }
            }
        }
    }
    Ok(())
}
