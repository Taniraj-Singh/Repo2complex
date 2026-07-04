//! A virtual directory tree built from an archive's flat entry list.
//!
//! Archive entries are a flat list of slash-separated paths. [`Tree`] projects
//! them into the hierarchy they imply, creating intermediate directory nodes as
//! needed, so callers can list directories, resolve paths, and walk the archive
//! as a filesystem. Building the tree enforces a maximum path depth to bound the
//! work a pathological set of very deep paths could cause.

use std::collections::BTreeMap;

use crate::errors::{Error, Result};
use crate::format::EntryRecord;
use crate::types::EntryKind;
use crate::util::{path, Limits};

/// A node in the virtual tree: either a directory (with children) or a leaf that
/// maps to an archive entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The node's own name (final path component).
    pub name: String,
    /// Whether this node is a directory.
    pub is_dir: bool,
    /// For leaves and explicit directories, the index of the backing archive
    /// entry; `None` for directories that were only implied by a child path.
    pub entry: Option<usize>,
    /// Child nodes, keyed by name and kept in sorted order.
    pub children: BTreeMap<String, Node>,
}

impl Node {
    fn dir(name: impl Into<String>) -> Node {
        Node {
            name: name.into(),
            is_dir: true,
            entry: None,
            children: BTreeMap::new(),
        }
    }

    /// The number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// The names of direct children, in sorted order.
    pub fn child_names(&self) -> impl Iterator<Item = &str> {
        self.children.keys().map(String::as_str)
    }
}

/// A virtual filesystem view over an archive's entries.
#[derive(Debug, Clone)]
pub struct Tree {
    root: Node,
    entry_count: usize,
}

impl Tree {
    /// Builds a tree from entry records, enforcing `limits.max_path_depth`.
    pub fn build(records: &[EntryRecord], limits: &Limits) -> Result<Tree> {
        let mut root = Node::dir("");
        for (i, rec) in records.iter().enumerate() {
            let depth = path::components(&rec.name).count();
            if depth > limits.max_path_depth {
                return Err(Error::LimitExceeded {
                    limit: "max_path_depth",
                    cap: limits.max_path_depth as u64,
                    got: depth as u64,
                });
            }
            insert(&mut root, &rec.name, rec.kind, i);
        }
        Ok(Tree {
            root,
            entry_count: records.len(),
        })
    }

    /// The root directory node.
    pub fn root(&self) -> &Node {
        &self.root
    }

    /// The number of entries the tree was built from.
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Resolves a slash-separated path to a node.
    pub fn get(&self, path: &str) -> Option<&Node> {
        let mut node = &self.root;
        if path.is_empty() {
            return Some(node);
        }
        for component in path.split('/') {
            if component.is_empty() {
                continue;
            }
            node = node.children.get(component)?;
        }
        Some(node)
    }

    /// Lists the direct child names of the directory at `path`, or `None` if the
    /// path does not resolve to a directory.
    pub fn list(&self, path: &str) -> Option<Vec<&str>> {
        let node = self.get(path)?;
        if !node.is_dir {
            return None;
        }
        Some(node.child_names().collect())
    }

    /// Walks every node depth-first, invoking `visit` with each node's full path
    /// and the node itself. The root is visited with an empty path.
    pub fn walk<F: FnMut(&str, &Node)>(&self, mut visit: F) {
        fn go<F: FnMut(&str, &Node)>(prefix: &str, node: &Node, visit: &mut F) {
            visit(prefix, node);
            for child in node.children.values() {
                let child_path = if prefix.is_empty() {
                    child.name.clone()
                } else {
                    format!("{prefix}/{}", child.name)
                };
                go(&child_path, child, visit);
            }
        }
        go("", &self.root, &mut visit);
    }

    /// Counts directory nodes (including implied ones) in the tree.
    pub fn dir_count(&self) -> usize {
        let mut count: usize = 0;
        self.walk(|_, node| {
            if node.is_dir {
                count += 1;
            }
        });
        // Do not count the synthetic root.
        count.saturating_sub(1)
    }
}

fn insert(root: &mut Node, path: &str, kind: EntryKind, entry: usize) {
    let components: Vec<&str> = path::components(path).collect();
    if components.is_empty() {
        return;
    }
    let mut node = root;
    for (i, component) in components.iter().enumerate() {
        let is_last = i == components.len() - 1;
        node = node
            .children
            .entry((*component).to_string())
            .or_insert_with(|| Node::dir(*component));
        if is_last {
            node.is_dir = kind == EntryKind::Directory;
            node.entry = Some(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Compression;

    fn rec(name: &str, kind: EntryKind) -> EntryRecord {
        EntryRecord {
            name: name.into(),
            kind,
            compression: Compression::Store,
            data_offset: 0,
            stored_len: 0,
            original_len: 0,
            crc32: 0,
            mode: 0,
            mtime: 0,
        }
    }

    fn sample_tree() -> Tree {
        let records = vec![
            rec("src/main.rs", EntryKind::File),
            rec("src/lib.rs", EntryKind::File),
            rec("src/util/mod.rs", EntryKind::File),
            rec("README.md", EntryKind::File),
        ];
        Tree::build(&records, &Limits::DEFAULT).unwrap()
    }

    #[test]
    fn builds_hierarchy_with_implied_dirs() {
        let tree = sample_tree();
        assert!(tree.get("src").unwrap().is_dir);
        assert!(tree.get("src/util").unwrap().is_dir);
        assert!(!tree.get("README.md").unwrap().is_dir);
        assert!(tree.get("nope").is_none());
    }

    #[test]
    fn lists_directory_children() {
        let tree = sample_tree();
        let mut top = tree.list("").unwrap();
        top.sort();
        assert_eq!(top, ["README.md", "src"]);
        let mut src = tree.list("src").unwrap();
        src.sort();
        assert_eq!(src, ["lib.rs", "main.rs", "util"]);
    }

    #[test]
    fn depth_limit_enforced() {
        let deep = rec("a/b/c/d/e/f", EntryKind::File);
        let tight = Limits {
            max_path_depth: 3,
            ..Limits::DEFAULT
        };
        assert!(Tree::build(std::slice::from_ref(&deep), &tight).is_err());
    }
}
