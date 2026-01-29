// Build script to ensure Cargo rebuilds when embedded assets change.
// rust-embed embeds files at compile time, but Cargo's incremental compilation
// may not detect changes to asset files. This tells Cargo to rerun the build
// whenever files in these directories are modified.

fn main() {
    println!("cargo:rerun-if-changed=src/assets/");
    println!("cargo:rerun-if-changed=templates/");
}
