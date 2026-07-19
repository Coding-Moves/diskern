//! Reference/impact graph — the research core of Diskern.
//!
//! Goal: answer "what breaks if I remove X?" with evidence. Nodes are
//! filesystem artifacts (files, dirs, SDKs, projects); edges are typed
//! references (lockfile pins, symlinks, PATH entries, project configs).
//!
//! v0 scope: detect project roots (Cargo.toml, package.json, etc.) and
//! link them to the dependency stores they reference. Everything else
//! (dynamic linking, registry, plists) comes later.

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
    pub fn referencing_projects(&self, path: &std::path::Path) -> usize {
        let Some(&ix) = self.index.get(path) else {
            return 0;
        };
        self.graph
            .neighbors_directed(ix, petgraph::Direction::Incoming)
            .filter(|&n| matches!(self.graph[n], Node::ProjectRoot { .. }))
            .count()
    }
}
