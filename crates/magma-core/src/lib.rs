//! Magma core: pure, platform-independent vault logic with no UI or desktop
//! dependencies. Both the Tauri desktop shell and the (upcoming) MCP server
//! build on this crate, so the LLM and the user always act on the same model
//! of the vault.

pub mod ai;
pub mod links;
pub mod vault;

pub use ai::{
    ai_create_note, ai_update_note, find_link_candidates, stamp_ai_author, validate_links,
    AiWriteResult, BrokenLink, LinkCandidate, LinkCheck,
};
pub use links::{
    backlinks, build_graph, extract_links, note_name, rename_note_updating_links,
    replace_link_target, search, Graph, GraphEdge, GraphNode, SearchHit,
};
pub use vault::{
    create_folder, create_note, create_note_in, delete_folder, delete_note, list_folders,
    list_notes, move_note, read_note, rename_note, safe_join, save_asset, slugify, write_note,
    Note, NoteMeta,
};
