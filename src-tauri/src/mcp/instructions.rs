//! Server instructions returned to MCP clients on connect.
//!
//! These are surfaced to agents via `ServerCapabilities::instructions` and act
//! as the canonical "what is this server and how do I use it" debrief. Keep
//! them concise and action-oriented; agents read them on every connection.
//!
//! The content lives in `docs/agent-debrief.md` so it can be copy-pasted
//! into an agent's system prompt before the MCP connection exists. The file
//! begins with an HTML comment block (invisible in rendered views) that
//! explains the copy-paste workflow; agents receive and safely ignore it.

pub const INSTRUCTIONS: &str = include_str!("../../../docs/agent-debrief.md");
