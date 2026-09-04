use anyhow::Result;
use clap::{Parser, Subcommand};
use diskern_core::{report, rules::RulesDb, scanner, Category, Finding, Verdict};
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

/// Display strings live in the CLI, not in diskern-core — the engine is
/// deliberately UI-agnostic. These mirror the labels the desktop app uses
/// so the two front ends describe the same finding the same way.
fn verdict_label(v: Verdict) -> &'static str {
    match v {
        Verdict::Safe => "Safe to remove",
        Verdict::Review => "Review first",
        Verdict::Risky => "Risky — not recommended",
        Verdict::Protected => "Protected — do not touch",
    }
}

fn category_label(c: Category) -> &'static str {
    match c {
        Category::BrowserCache => "Browser cache",
        Category::BuildArtifact => "Build artifacts",
        Category::PackageManagerCache => "Package manager cache",
        Category::TempFile => "Temporary files",
        Category::Log => "Logs",
        Category::Installer => "Installers",
        Category::DuplicateFile => "Duplicate file",
        Category::EmptyDirectory => "Empty folders",
        Category::SystemCritical => "System critical",
        Category::Unknown => "Unrecognized",
    }
}

/// Group by category, biggest reclaimable total first. Findings arrive
/// already sorted by size, so each group keeps that order.
fn by_category<'a>(findings: &[&'a Finding]) -> Vec<(Category, Vec<&'a Finding>)> {
    let mut groups: Vec<(Category, Vec<&'a Finding>)> = Vec::new();
    for &f in findings {
        match groups.iter_mut().find(|(c, _)| *c == f.category) {
            Some((_, items)) => items.push(f),
            None => groups.push((f.category, vec![f])),
        }
    }
    groups.sort_by_key(|(_, items)| {
        std::cmp::Reverse(items.iter().map(|f| f.reclaimable).sum::<u64>())
    });
    groups
}

/// Verdict, then category, then the findings themselves with the rule that
/// matched. Verdicts are printed in ascending order — safest first — which
/// is also what `Verdict`'s Ord derives.
fn print_findings(findings: &[&Finding], top: usize) {
    for verdict in [
        Verdict::Safe,
        Verdict::Review,
        Verdict::Risky,
        Verdict::Protected,
    ] {
        let group: Vec<&Finding> = findings
            .iter()
            .copied()
            .filter(|f| f.verdict == verdict)
            .collect();
        if group.is_empty() {
            continue;
        }

        let total: u64 = group.iter().map(|f| f.reclaimable).sum();
        println!();
        println!(
            "{} — {} finding{} · {}",
            verdict_label(verdict),
            group.len(),
            if group.len() == 1 { "" } else { "s" },
            human_bytes(total)
        );

        for (category, items) in by_category(&group) {
            let subtotal: u64 = items.iter().map(|f| f.reclaimable).sum();
            println!(
                "  {} · {} · {}",
                category_label(category),
                items.len(),
                human_bytes(subtotal)
            );

            let shown = if top == 0 { items.len() } else { top.min(items.len()) };
            for f in &items[..shown] {
                println!(
                    "    {:>9}  {}",
                    human_bytes(f.reclaimable),
                    f.entry.path.display()
                );
                // The first reason is the matched rule; the rest come from
                // the risk module and repeat across a whole category.
                if let Some(reason) = f.reasons.first() {
                    println!("    {:>9}  {reason}", "");
                }
            }
            if shown < items.len() {
                println!("    {:>9}  … {} more", "", items.len() - shown);
            }
        }
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
                let findings: Vec<&Finding> = report.findings.iter().collect();
                print_findings(&findings, 5);

                if !report.duplicate_sets.is_empty() {
                    let wasted: u64 = report.duplicate_sets.iter().map(|d| d.wasted).sum();
                    println!();
                    println!(
                        "Duplicate files — {} sets · {} wasted",
                        report.duplicate_sets.len(),
                        human_bytes(wasted)
                    );
                    for set in report.duplicate_sets.iter().take(5) {
                        println!(
                            "    {:>9}  x{} copies  {}",
                            human_bytes(set.wasted),
                            set.paths.len(),
                            set.paths[0].display()
                        );
                    }
                    if report.duplicate_sets.len() > 5 {
                        println!("    {:>9}  … {} more", "", report.duplicate_sets.len() - 5);
                    }
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
