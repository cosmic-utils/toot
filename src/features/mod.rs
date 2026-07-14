//! Feature modules: each owns its own state, messages, view, update, and
//! data-fetching for one Mastodon capability, rather than being split across
//! separate pages/widgets/subscriptions trees.

pub mod accounts;
pub mod compose;
pub mod hashtags;
pub mod lists;
pub mod notifications;
pub mod search;
pub mod status;
pub mod timeline;
