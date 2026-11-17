//! The `mesh` CLI: the mesh.nvim library operations for scripts and agents.

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(mesh::cli::run(argv));
}
