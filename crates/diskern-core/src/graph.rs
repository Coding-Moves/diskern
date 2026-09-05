//! Reference/impact graph — the research core of Diskern.
//!
//! Goal: answer "what breaks if I remove X?" with evidence. Nodes are
//! filesystem artifacts (files, dirs, SDKs, projects); edges are typed
//! references (lockfile pins, symlinks, PATH entries, project configs).
//!
//! v0 scope: detect project roots (Cargo.toml, package.json, etc.) and
//! link them to the dependency stores they reference. Everything else
//! (dynamic linking, registry, plists) comes later.

use crate::FileEntry;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A file whose presence makes the directory holding it a project root,
/// and the stores a project of that kind owns.
///
/// Root-level stores only, in v0. `__pycache__` and nested `node_modules`
/// exist at every depth and belong to whatever encloses them, which is a
/// containment question rather than a reference one.
const PROJECTS: &[(&str, ProjectKind, &[&str])] = &[
    ("cargo.toml", ProjectKind::Cargo, &["target"]),
    ("package.json", ProjectKind::Npm, &["node_modules"]),
    ("pyproject.toml", ProjectKind::Python, &[".venv", "venv"]),
];

/// Directory names that are dependency stores wherever they appear.
const STORE_NAMES: &[&str] = &["target", "node_modules", ".venv", "venv"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Node {
    File(PathBuf),
    ProjectRoot { path: PathBuf, kind: ProjectKind },
    DependencyStore(PathBuf), // e.g. a node_modules dir, ~/.cargo/registry
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectKind {
    Cargo,
    Npm,
    Python,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Edge {
    /// Project depends on this store/file (from lockfile or config).
    References,
    /// Filesystem containment.
    Contains,
    /// Symlink or hard link.
    LinksTo,
}

#[derive(Default)]
pub struct ImpactGraph {
    pub graph: DiGraph<Node, Edge>,
    index: HashMap<PathBuf, NodeIndex>,
}

impl ImpactGraph {
    /// Build the graph from one pass over the scanned entries.
    ///
    /// Two things come out of that pass: which directories are project
    /// roots (they hold a marker file), and which are dependency stores
    /// (an entry lives under one). A `References` edge is added for each
    /// project to the store it owns.
    ///
    /// A project whose own store wasn't scanned is linked to the nearest
    /// enclosing project's store of the same kind instead. That is not a
    /// guess: npm workspaces hoist dependencies to the repository root and
    /// Cargo workspaces share one `target/`, so the members really do
    /// reference it — and it is where the "referenced by 3 projects"
    /// number comes from rather than always being 1.
    pub fn from_entries(entries: &[FileEntry]) -> Self {
        let mut roots: HashMap<PathBuf, ProjectKind> = HashMap::new();
        let mut stores: HashSet<PathBuf> = HashSet::new();

        for entry in entries {
            let store = enclosing_store(&entry.path);
            if let Some(store) = &store {
                stores.insert(store.clone());
            }

            // A marker inside a dependency store marks nothing: every npm
            // package ships a package.json, and there are tens of
            // thousands of them under one node_modules.
            if store.is_some() {
                continue;
            }
            let Some(kind) = marker_kind(&entry.path) else {
                continue;
            };
            if let Some(dir) = entry.path.parent() {
                roots.insert(dir.to_path_buf(), kind);
            }
        }

        let mut graph = Self::default();
        for (root, kind) in &roots {
            let owned = stores_of(root, *kind, &stores);
            let targets = if owned.is_empty() {
                shared_store(root, *kind, &roots, &stores)
            } else {
                owned
            };

            for store in targets {
                let from = graph.node(Node::ProjectRoot {
                    path: root.clone(),
                    kind: *kind,
                });
                let to = graph.node(Node::DependencyStore(store));
                graph.graph.add_edge(from, to, Edge::References);
            }
        }
        graph
    }

    pub fn node(&mut self, node: Node) -> NodeIndex {
        let key = match &node {
            Node::File(p) | Node::DependencyStore(p) => p.clone(),
            Node::ProjectRoot { path, .. } => path.clone(),
        };
        if let Some(&ix) = self.index.get(&key) {
            return ix;
        }
        let ix = self.graph.add_node(node);
        self.index.insert(key, ix);
        ix
    }

    /// How many project roots reference this path (directly, v0)?
    /// This number feeds risk::downgrade — the "breaks 17 projects" number.
    ///
    /// Answers for the nearest enclosing node as well as for the path
    /// itself, because findings are files and the thing projects reference
    /// is the store above them: nothing points at
    /// `proj/node_modules/react/index.js`, but three projects may well
    /// point at the `proj/node_modules` it sits in.
    pub fn referencing_projects(&self, path: &std::path::Path) -> usize {
        for ancestor in path.ancestors() {
            let Some(&ix) = self.index.get(ancestor) else {
                continue;
            };
            return self
                .graph
                .neighbors_directed(ix, petgraph::Direction::Incoming)
                .filter(|&n| matches!(self.graph[n], Node::ProjectRoot { .. }))
                .count();
        }
        0
    }
}

/// The outermost dependency store this path sits inside, if any.
///
/// Outermost, not nearest: `proj/node_modules/a/node_modules/b` belongs to
/// `proj/node_modules`, which is the store a project actually references.
fn enclosing_store(path: &Path) -> Option<PathBuf> {
    let mut found = None;
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| STORE_NAMES.contains(&n))
        {
            found = Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    found
}

/// Which kind of project a file marks, if it marks one.
fn marker_kind(path: &Path) -> Option<ProjectKind> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    PROJECTS
        .iter()
        .find(|(marker, _, _)| *marker == name)
        .map(|(_, kind, _)| *kind)
}

/// The stores this project owns that the scan actually saw.
fn stores_of(root: &Path, kind: ProjectKind, seen: &HashSet<PathBuf>) -> Vec<PathBuf> {
    PROJECTS
        .iter()
        .filter(|(_, k, _)| *k == kind)
        .flat_map(|(_, _, names)| names.iter())
        .map(|name| root.join(name))
        .filter(|store| seen.contains(store))
        .collect()
}

/// The store an enclosing project of the same kind owns — what a workspace
/// member uses when the dependencies were hoisted above it.
fn shared_store(
    root: &Path,
    kind: ProjectKind,
    roots: &HashMap<PathBuf, ProjectKind>,
    seen: &HashSet<PathBuf>,
) -> Vec<PathBuf> {
    for ancestor in root.ancestors().skip(1) {
        if roots.get(ancestor) != Some(&kind) {
            continue;
        }
        let stores = stores_of(ancestor, kind, seen);
        if !stores.is_empty() {
            return stores;
        }
    }
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[&str]) -> Vec<FileEntry> {
        paths
            .iter()
            .map(|p| FileEntry {
                path: PathBuf::from(p),
                size: 1,
                modified: None,
                accessed: None,
                is_symlink: false,
                hash: None,
            })
            .collect()
    }

    #[test]
    fn a_project_references_the_store_beside_it() {
        let graph = ImpactGraph::from_entries(&entries(&[
            "/home/u/proj/package.json",
            "/home/u/proj/src/index.js",
            "/home/u/proj/node_modules/react/index.js",
        ]));

        assert_eq!(
            graph.referencing_projects(Path::new("/home/u/proj/node_modules/react/index.js")),
            1
        );
        // The project's own source is referenced by nothing.
        assert_eq!(
            graph.referencing_projects(Path::new("/home/u/proj/src/index.js")),
            0
        );
    }

    /// An abandoned store — no project alongside it — is the case this
    /// whole module exists to tell apart from the one above.
    #[test]
    fn an_unreferenced_store_is_referenced_by_nobody() {
        let graph =
            ImpactGraph::from_entries(&entries(&["/home/u/old-thing/node_modules/react/index.js"]));

        assert_eq!(
            graph.referencing_projects(Path::new("/home/u/old-thing/node_modules/react/index.js")),
            0
        );
    }

    /// Every npm package ships a package.json. Treating those as project
    /// roots would make one node_modules look like ten thousand projects.
    #[test]
    fn a_package_json_inside_node_modules_is_not_a_project() {
        let graph = ImpactGraph::from_entries(&entries(&[
            "/home/u/proj/package.json",
            "/home/u/proj/node_modules/react/package.json",
            "/home/u/proj/node_modules/react/node_modules/loose/package.json",
        ]));

        assert_eq!(
            graph.referencing_projects(Path::new("/home/u/proj/node_modules/react/package.json")),
            1
        );
    }

    /// The README's headline evidence. npm workspaces hoist dependencies
    /// to the repository root, so all three members really do reference
    /// the one store.
    #[test]
    fn hoisted_workspace_members_all_reference_the_shared_store() {
        let graph = ImpactGraph::from_entries(&entries(&[
            "/repo/package.json",
            "/repo/packages/a/package.json",
            "/repo/packages/b/package.json",
            "/repo/node_modules/react/index.js",
        ]));

        assert_eq!(
            graph.referencing_projects(Path::new("/repo/node_modules/react/index.js")),
            3
        );
    }

    /// A member with its own store uses that one, not the root's.
    #[test]
    fn a_member_with_its_own_store_does_not_borrow_the_roots() {
        let graph = ImpactGraph::from_entries(&entries(&[
            "/repo/package.json",
            "/repo/packages/a/package.json",
            "/repo/packages/a/node_modules/x/index.js",
            "/repo/node_modules/react/index.js",
        ]));

        assert_eq!(
            graph.referencing_projects(Path::new("/repo/packages/a/node_modules/x/index.js")),
            1
        );
        assert_eq!(
            graph.referencing_projects(Path::new("/repo/node_modules/react/index.js")),
            1
        );
    }

    #[test]
    fn cargo_workspace_members_share_one_target() {
        let graph = ImpactGraph::from_entries(&entries(&[
            "/repo/Cargo.toml",
            "/repo/crates/core/Cargo.toml",
            "/repo/crates/cli/Cargo.toml",
            "/repo/target/debug/app",
        ]));

        assert_eq!(
            graph.referencing_projects(Path::new("/repo/target/debug/app")),
            3
        );
    }
}
