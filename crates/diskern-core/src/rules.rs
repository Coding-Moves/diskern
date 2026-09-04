//! Deterministic rules database — the safety authority.
//!
//! Rules are data, not code: shipped as JSON, versioned, embedded at build
//! time, updatable independently of the binary later. A rule matches path
//! patterns and yields a Category + base Verdict. The risk module may
//! DOWNGRADE a verdict (Safe -> Review) based on evidence; it may never
//! upgrade one (Risky -> Safe). Protected is final.

use crate::{Category, Verdict};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    /// Substring / suffix patterns matched against the normalized path.
    /// TODO: replace with proper glob matching (globset crate).
    pub patterns: Vec<String>,
    pub category: Category,
    pub verdict: Verdict,
    /// Shown to the user as evidence.
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesDb {
    pub version: u32,
    pub rules: Vec<Rule>,
}

impl RulesDb {
    /// Starter rules embedded from rules/base.json. Deliberately tiny —
    /// growing this database (per-platform, per-app) IS the product work.
    pub fn embedded() -> Self {
        serde_json::from_str(include_str!("../rules/base.json"))
            .expect("embedded rules db must parse")
    }

    /// First matching rule wins; order in the db is priority order.
    /// Protected rules are listed first for exactly that reason.
    pub fn classify(&self, path: &std::path::Path) -> (Category, Verdict, Option<&Rule>) {
        let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
        for rule in &self.rules {
            if rule.patterns.iter().any(|pat| p.contains(pat.as_str())) {
                return (rule.category, rule.verdict, Some(rule));
            }
        }
        (Category::Unknown, Verdict::Review, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_db_parses_and_protects_driverstore() {
        let db = RulesDb::embedded();
        let (cat, verdict, _) = db.classify(std::path::Path::new(
            "C:/Windows/System32/DriverStore/x.inf",
        ));
        assert_eq!(cat, Category::SystemCritical);
        assert_eq!(verdict, Verdict::Protected);
    }

    #[test]
    fn unknown_paths_default_to_review_not_safe() {
        let db = RulesDb::embedded();
        let (_, verdict, _) = db.classify(std::path::Path::new("/home/user/mystery.bin"));
        assert_eq!(verdict, Verdict::Review);
    }

    #[test]
    fn logs_are_reachable() {
        let db = RulesDb::embedded();
        for path in [
            "/var/log/syslog.1",
            "/Users/x/Library/Logs/SomeApp/app.log",
            "C:\\Users\\x\\AppData\\Local\\CrashDumps\\a.dmp",
        ] {
            let (cat, verdict, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::Log, "{path}");
            assert_eq!(verdict, Verdict::Review, "{path}");
        }
    }

    #[test]
    fn installers_are_reachable() {
        let db = RulesDb::embedded();
        for path in [
            "/home/u/Downloads/Diskern_0.1.0_amd64.dmg",
            "/home/u/Downloads/node-v22.msi",
            "/home/u/Downloads/tool.pkg",
            "C:\\Users\\x\\Downloads\\vscode-setup.exe",
        ] {
            let (cat, verdict, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::Installer, "{path}");
            assert_eq!(verdict, Verdict::Review, "{path}");
        }
    }

    /// The installer patterns are bare extensions, so they match anywhere in
    /// a path. First-match-wins is the only thing keeping them from
    /// classifying an OS component as a reclaimable download — which makes
    /// rule order a safety property, not a stylistic one.
    #[test]
    fn protected_rules_still_win_over_installer_patterns() {
        let db = RulesDb::embedded();
        for path in [
            "C:/Windows/System32/DriverStore/setup.exe",
            "C:/Windows/WinSxS/component.msi",
        ] {
            let (cat, verdict, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::SystemCritical, "{path}");
            assert_eq!(verdict, Verdict::Protected, "{path}");
        }
    }
}
