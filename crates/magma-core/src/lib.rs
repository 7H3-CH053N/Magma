//! Magma core: pure, platform-independent vault logic with no UI or desktop
//! dependencies. Both the Tauri desktop shell and the (upcoming) MCP server
//! build on this crate, so the LLM and the user always act on the same model
//! of the vault.

pub mod links;
pub mod vault;

pub use links::{
    backlinks, build_graph, extract_links, search, Graph, GraphEdge, GraphNode, SearchHit,
};
pub use vault::{
    create_note, delete_note, list_notes, read_note, rename_note, safe_join, save_asset, slugify,
    write_note, Note, NoteMeta,
};
