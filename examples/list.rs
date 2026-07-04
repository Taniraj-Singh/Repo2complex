//! List an archive's contents as a directory tree.
//!
//! Usage: `cargo run --example list`
//!
//! Builds a sample archive, then renders its virtual directory tree — showing
//! how the flat entry list projects into a hierarchy via the `vfs` module.

use quiver::vfs::{Node, Tree};
use quiver::writer::Builder;
use quiver::Archive;

fn main() {
    let bytes = sample_archive();
    let archive = Archive::open(&bytes).unwrap();
    let tree = Tree::build(archive.records(), &archive.limits()).unwrap();

    println!(
        "archive: {} entries, {} directories",
        archive.len(),
        tree.dir_count()
    );
    println!(".");
    print_dir(tree.root(), "");
}

fn print_dir(node: &Node, prefix: &str) {
    let children: Vec<&Node> = node.children.values().collect();
    for (i, child) in children.iter().enumerate() {
        let last = i + 1 == children.len();
        let branch = if last { "└── " } else { "├── " };
        let marker = if child.is_dir { "/" } else { "" };
        println!("{prefix}{branch}{}{marker}", child.name);
        if child.is_dir {
            let next_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
            print_dir(child, &next_prefix);
        }
    }
}

fn sample_archive() -> Vec<u8> {
    let mut b = Builder::new();
    for path in [
        "README.md",
        "src/main.rs",
        "src/lib.rs",
        "src/codec/mod.rs",
        "src/codec/lz.rs",
        "tests/it.rs",
        "docs/guide.md",
    ] {
        b.add_file(path, format!("// {path}\n").into_bytes())
            .unwrap();
    }
    b.finish()
}
