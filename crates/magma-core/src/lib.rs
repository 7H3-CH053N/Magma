//! Magma core: pure, platform-independent vault logic with no UI or desktop
//! dependencies. Both the Tauri desktop shell and the (upcoming) MCP server
//! build on this crate, so the LLM and the user always act on the same model
//! of the vault.

pub mod vault;

pub use vault::{list_notes, read_note, safe_join, write_note, Note, NoteMeta};
