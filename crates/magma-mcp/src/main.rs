//! Thin CLI wrapper around `magma_mcp::serve_stdio`. The vault comes from the
//! first argument or `MAGMA_VAULT`; `MAGMA_MCP_ALLOW_WRITE=0` makes it read-only.
use std::path::PathBuf;

fn main() {
    let vault = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("MAGMA_VAULT").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("magma-mcp: no vault given (arg 1 or MAGMA_VAULT)");
            std::process::exit(2);
        });
    let allow_write = !matches!(
        std::env::var("MAGMA_MCP_ALLOW_WRITE").ok().as_deref(),
        Some("0") | Some("false") | Some("no")
    );
    magma_mcp::serve_stdio(vault, allow_write);
}
