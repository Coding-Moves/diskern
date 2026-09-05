use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
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
        /// Show at most N findings per category; 0 shows every one
        #[arg(long, value_name = "N", default_value_t = 5)]
        top: usize,
        /// Only show findings with this verdict
        #[arg(long, value_enum)]
        verdict: Option<VerdictFilter>,
    },
}

/// Mirrors `Verdict` for clap. The core enum can't derive `ValueEnum`
/// without diskern-core taking a dependency on the CLI's argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum VerdictFilter {
    Safe,
    Review,
    Risky,
    Protected,
}

impl From<VerdictFilter> for Verdict {
    fn from(v: VerdictFilter) -> Self {
        match v {
            VerdictFilter::Safe => Verdict::Safe,
            VerdictFilter::Review => Verdict::Review,
            VerdictFilter::Risky => Verdict::Risky,
            VerdictFilter::Protected => Verdict::Protected,
        }
    }
}

/// Bytes at the largest unit that keeps the number short. Decimal units,
/// matching what disk vendors and the rest of the UI report.
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    // 999.95, not 1000.0: at one decimal place anything at or above that
    // rounds to "1000.0", which belongs in the next unit up. Choosing the
    // unit before rounding printed 999_999 as "1000.0 KB".
    while value >= 999.95 && unit < UNITS.len() - 1 {
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
        // position(), not iter_mut().find() — the latter keeps `groups`
        // mutably borrowed through the None arm, which then can't push.
        match groups.iter().position(|(c, _)| *c == f.category) {
            Some(i) => groups[i].1.push(f),
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

            let shown = if top == 0 {
                items.len()
            } else {
                top.min(items.len())
            };
            for f in &items[..shown] {
                println!(
                    "    {:>9}  {}",
                    human_bytes(f.reclaimable),
                    f.entry.path.display()
                );
                // Every reason, not just the matched rule. The rule says
                // what the file is; the rest say why this copy of it got
                // the verdict it did — "referenced by 3 projects" is the
                // line that explains a risky node_modules, and printing
                // only the first hid exactly that.
                for reason in &f.reasons {
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
        Command::Scan {
            roots,
            json,
            top,
            verdict,
        } => {
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
                // Filter after the summary line, so the headline totals
                // still describe the whole scan rather than the slice.
                let wanted = verdict.map(Verdict::from);
                let findings: Vec<&Finding> = report
                    .findings
                    .iter()
                    .filter(|f| wanted.is_none_or(|v| f.verdict == v))
                    .collect();
                print_findings(&findings, top);

                // Duplicates have no verdict of their own, so a --verdict
                // filter is asking about findings only; hide them then.
                if verdict.is_none() && !report.duplicate_sets.is_empty() {
                    let wasted: u64 = report.duplicate_sets.iter().map(|d| d.wasted).sum();
                    println!();
                    println!(
                        "Duplicate files — {} sets · {} wasted",
                        report.duplicate_sets.len(),
                        human_bytes(wasted)
                    );
                    let dup_shown = if top == 0 {
                        report.duplicate_sets.len()
                    } else {
                        top.min(report.duplicate_sets.len())
                    };
                    for set in report.duplicate_sets.iter().take(dup_shown) {
                        println!(
                            "    {:>9}  x{} copies  {}",
                            human_bytes(set.wasted),
                            set.paths.len(),
                            set.paths[0].display()
                        );
                    }
                    if dup_shown < report.duplicate_sets.len() {
                        println!(
                            "    {:>9}  … {} more",
                            "",
                            report.duplicate_sets.len() - dup_shown
                        );
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
    fn rolls_over_instead_of_printing_a_four_digit_mantissa() {
        assert_eq!(human_bytes(999_949), "999.9 KB");
        assert_eq!(human_bytes(999_999), "1.0 MB");
        assert_eq!(human_bytes(999_999_999), "1.0 GB");
    }

    #[test]
    fn stops_at_the_largest_unit_it_knows() {
        assert_eq!(human_bytes(u64::MAX), "18446744.1 TB");
    }
}
