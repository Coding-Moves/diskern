use anyhow::Result;
use clap::{Parser, Subcommand};
use diskern_core::{report, rules::RulesDb, scanner};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "diskern",
    about = "Diskern — understand your disk before you clean it"
)]
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

/// Bytes at the largest unit that keeps the number short. Decimal units,
/// matching what disk vendors and the rest of the UI report.
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Scan { roots, json } => {
            let opts = scanner::ScanOptions {
                roots,
                ..Default::default()
            };
            let progress = Arc::new(scanner::ScanProgress::default());
            let entries = scanner::scan(&opts, progress)?;
            let report = report::build(entries, &RulesDb::embedded());

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("Scanned {} files.", report.files_scanned);
                println!(
                    "Reclaimable: {} across {} findings and {} duplicate sets.",
                    human_bytes(report.total_reclaimable),
                    report.findings.len(),
                    report.duplicate_sets.len()
                );
                for set in report.duplicate_sets.iter().take(10) {
                    println!(
                        "  dup x{}  {} wasted  {}",
                        set.paths.len(),
                        human_bytes(set.wasted),
                        set.paths[0].display()
                    );
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn scales_to_a_readable_unit() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_000), "1.0 KB");
        assert_eq!(human_bytes(1_500_000), "1.5 MB");
        assert_eq!(human_bytes(2_300_000_000), "2.3 GB");
    }

    #[test]
    fn stops_at_the_largest_unit_it_knows() {
        assert_eq!(human_bytes(u64::MAX), "18446744.1 TB");
    }
}
