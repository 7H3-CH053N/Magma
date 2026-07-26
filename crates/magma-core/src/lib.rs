//! Magma core: pure, platform-independent vault logic with no UI or desktop
//! dependencies. Both the Tauri desktop shell and the (upcoming) MCP server
//! build on this crate, so the LLM and the user always act on the same model
//! of the vault.

pub mod ai;
pub mod history;
pub mod links;
pub mod related;
pub mod vault;

pub use ai::{
    ai_create_note, ai_update_note, find_link_candidates, stamp_ai_author, validate_links,
    AiWriteResult, BrokenLink, LinkCandidate, LinkCheck,
};
pub use history::{
    forget as forget_history, list_versions, read_version, relocate as relocate_history, restore,
    snapshot, snapshot_if_due, Version,
};
pub use links::{
    backlinks, build_graph, extract_links, link_mentions, link_mentions_in, note_name,
    outgoing_links, rename_note_updating_links, replace_in_vault, replace_link_target, search,
    unlinked_mentions, Graph, GraphEdge, GraphNode, Mention, OutgoingLink, ReplaceHit,
    ReplaceRename, ReplaceReport, SearchHit,
};
pub use related::{related_notes, RelatedNote};
pub use vault::{
    append_note, create_folder, create_note, create_note_in, delete_folder, delete_note, list_folders,
    list_notes, move_note, open_or_create, read_note, rename_note, safe_join, save_asset, slugify, write_note,
    Note, NoteMeta,
};
