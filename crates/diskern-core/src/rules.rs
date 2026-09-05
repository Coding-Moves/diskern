//! Deterministic rules database — the safety authority.
//!
//! Rules are data, not code: shipped as JSON, versioned, embedded at build
//! time, updatable independently of the binary later. A rule matches path
//! patterns and yields a Category + base Verdict. The risk module may
//! DOWNGRADE a verdict (Safe -> Review) based on evidence; it may never
//! upgrade one (Risky -> Safe). Protected is final.
//!
//! Patterns are globs, matched against the normalized path (lowercased,
//! `\` rewritten to `/`) with `*` stopping at a path separator. That
//! anchoring is the point: a plain substring test made `/tmp/` fire on
//! `/home/user/tmp/tax-return.pdf`, which is user data, not scratch space.

use crate::{Category, Verdict};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    /// Globs matched against the normalized path. `*` matches within one
    /// path component, `**` spans components, so `**/node_modules/**`
    /// reaches any depth while `/tmp/**` stays at the filesystem root.
    pub patterns: Vec<String>,
    pub category: Category,
    pub verdict: Verdict,
    /// Shown to the user as evidence.
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RulesDb {
    pub version: u32,
    pub rules: Vec<Rule>,
    /// One compiled matcher per rule, in rule order. Built on the first
    /// `classify` and reused after: a home directory is millions of
    /// entries, and compiling a glob per entry would cost more than the
    /// walk that found them.
    ///
    /// Skipped by serde and rebuilt on demand, so a db that arrives over
    /// the wire behaves exactly like one built here.
    #[serde(skip)]
    matchers: OnceLock<Vec<GlobSet>>,
}

/// Hand-written because `OnceLock<GlobSet>` isn't `Clone`. A clone starts
/// with an empty cache rather than sharing one — the rules are the value,
/// the compiled form is an optimization.
impl Clone for RulesDb {
    fn clone(&self) -> Self {
        Self {
            version: self.version,
            rules: self.rules.clone(),
            matchers: OnceLock::new(),
        }
    }
}

impl RulesDb {
    /// Starter rules embedded from rules/base.json. Deliberately tiny —
    /// growing this database (per-platform, per-app) IS the product work.
    pub fn embedded() -> Self {
        serde_json::from_str(include_str!("../rules/base.json"))
            .expect("embedded rules db must parse")
    }

    /// Build a db from rules held in memory (tests, future remote rule
    /// updates). Same matching path as [`RulesDb::embedded`].
    pub fn new(version: u32, rules: Vec<Rule>) -> Self {
        Self {
            version,
            rules,
            matchers: OnceLock::new(),
        }
    }

    /// First matching rule wins; order in the db is priority order.
    /// Protected rules are listed first for exactly that reason.
    pub fn classify(&self, path: &std::path::Path) -> (Category, Verdict, Option<&Rule>) {
        let p = normalize(path);
        let candidate = std::path::Path::new(&p);
        for (rule, matcher) in self.rules.iter().zip(self.matchers()) {
            if matcher.is_match(candidate) {
                return (rule.category, rule.verdict, Some(rule));
            }
        }
        (Category::Unknown, Verdict::Review, None)
    }

    fn matchers(&self) -> &[GlobSet] {
        self.matchers
            .get_or_init(|| self.rules.iter().map(compile).collect())
    }
}

/// Lowercase, `/`-separated. Windows hands us `C:\Users\...`; the rules
/// are written once, in one shape, and the path is bent to fit them.
pub(crate) fn normalize(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// `literal_separator` is the whole reason this module compiles globs at
/// all: without it `*` swallows `/`, and `**/target/*/**` would reach back
/// across directories the rule never named.
fn build_glob(pattern: &str) -> std::result::Result<Glob, globset::Error> {
    GlobBuilder::new(pattern).literal_separator(true).build()
}

/// A pattern that doesn't compile is dropped, not fatal. The alternative —
/// panicking inside `classify` — would take the whole scan down over one
/// bad line in a rules file that may not even be ours. `patterns_compile`
/// below keeps the shipped db honest.
fn compile(rule: &Rule) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in &rule.patterns {
        match build_glob(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => tracing::warn!(
                rule = %rule.id,
                pattern = %pattern,
                "ignoring rule pattern that is not a valid glob: {e}"
            ),
        }
    }
    builder.build().unwrap_or_else(|e| {
        tracing::warn!(rule = %rule.id, "rule matches nothing: {e}");
        GlobSet::empty()
    })
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
    ///
    /// The installer-cache entries are the ones that bite hardest: `review`
    /// is an actionable verdict in the UI, so without a protected rule above
    /// `installer-packages` the app would offer to quarantine the files
    /// Windows needs to uninstall or repair installed software.
    #[test]
    fn protected_rules_still_win_over_installer_patterns() {
        let db = RulesDb::embedded();
        for path in [
            "C:/Windows/System32/DriverStore/setup.exe",
            "C:/Windows/WinSxS/component.msi",
            "C:/Windows/Installer/1a2b3c.msi",
            "C:/Windows/Installer/$PatchCache$/Managed/x.msi",
            "C:/ProgramData/Package Cache/{guid}/vs_setup.exe",
            "C:/Users/x/AppData/Local/Package Cache/{guid}/dotnet.msi",
        ] {
            let (cat, verdict, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::SystemCritical, "{path}");
            assert_eq!(verdict, Verdict::Protected, "{path}");
        }
    }

    /// Every shipped pattern has to compile, because `compile` drops the
    /// ones that don't. A protected rule that silently matches nothing is
    /// the worst failure this module has.
    #[test]
    fn every_shipped_pattern_is_a_valid_glob() {
        for rule in &RulesDb::embedded().rules {
            for pattern in &rule.patterns {
                assert!(
                    build_glob(pattern).is_ok(),
                    "{}: {pattern} is not a valid glob",
                    rule.id
                );
            }
        }
    }

    /// Issue #41. Under substring matching every one of these matched a
    /// rule written for somewhere else on the disk, and `review` is an
    /// actionable verdict — the app offered to move them.
    #[test]
    fn rules_do_not_reach_outside_the_paths_they_name() {
        let db = RulesDb::embedded();
        for path in [
            "/home/user/tmp/tax-return.pdf",     // not /tmp
            "/home/user/var/log/notes.txt",      // not /var/log
            "/home/user/Downloads/holiday.dmgx", // not a .dmg
            "/home/user/mytmp/scratch.bin",
        ] {
            let (cat, _, rule) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::Unknown, "{path} matched {rule:?}");
        }
    }

    /// The same rules still have to fire where they were meant to.
    #[test]
    fn rules_still_reach_the_paths_they_do_name() {
        let db = RulesDb::embedded();
        for (path, expected) in [
            ("/tmp/build-9a2f/out.o", Category::TempFile),
            ("/var/tmp/systemd-private/x", Category::TempFile),
            (
                "C:\\Users\\x\\AppData\\Local\\Temp\\a.tmp",
                Category::TempFile,
            ),
            (
                "/home/u/proj/node_modules/react/index.js",
                Category::BuildArtifact,
            ),
            ("/home/u/proj/target/debug/app", Category::BuildArtifact),
            (
                "/home/u/.cache/pip/wheels/a.whl",
                Category::PackageManagerCache,
            ),
        ] {
            let (cat, _, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, expected, "{path}");
        }
    }

    /// Issue #40. `/mozilla/firefox/profiles` covered the whole profile
    /// directory, so bookmarks, history, logins and cookies were listed
    /// under "Safe to remove" and `quarantine_finding` accepted them.
    /// Only cache2 is cache; everything else in the profile is user data.
    #[test]
    fn firefox_profile_data_is_not_safe_to_remove() {
        let db = RulesDb::embedded();
        for path in [
            "C:\\Users\\x\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\ab12.default\\logins.json",
            "C:\\Users\\x\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\ab12.default\\places.sqlite",
            "C:\\Users\\x\\AppData\\Roaming\\Mozilla\\Firefox\\Profiles\\ab12.default\\cookies.sqlite",
            "/home/u/.mozilla/firefox/ab12.default/key4.db",
        ] {
            let (_, verdict, rule) = db.classify(std::path::Path::new(path));
            assert_ne!(verdict, Verdict::Safe, "{path} matched {rule:?}");
        }
    }

    /// The cache the rule is actually named after still classifies.
    #[test]
    fn firefox_cache_is_still_safe_to_remove() {
        let db = RulesDb::embedded();
        for path in [
            "/home/u/.cache/mozilla/firefox/ab12.default/cache2/entries/A1B2",
            "C:\\Users\\x\\AppData\\Local\\Mozilla\\Firefox\\Profiles\\ab12.default\\cache2\\entries\\A1B2",
            "/Users/x/Library/Caches/Firefox/Profiles/ab12.default/cache2/entries/A1B2",
        ] {
            let (cat, verdict, _) = db.classify(std::path::Path::new(path));
            assert_eq!(cat, Category::BrowserCache, "{path}");
            assert_eq!(verdict, Verdict::Safe, "{path}");
        }
    }

    /// An installed application's own repair binary is not a reclaimable
    /// download. `unknown` is the right answer: report::build drops those,
    /// so it never reaches the user as an actionable row.
    #[test]
    fn installed_application_binaries_are_not_installers() {
        let db = RulesDb::embedded();
        let (cat, _, rule) =
            db.classify(std::path::Path::new("C:/Program Files/SomeApp/setup.exe"));
        assert_eq!(cat, Category::Unknown);
        assert!(rule.is_none());
    }
}
