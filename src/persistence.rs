//! Disk-persisted snapshots of feed content, so a restart can render the
//! last-seen posts/notifications immediately instead of a blank screen while
//! the network catches up.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use cosmic::Application;
use serde::{de::DeserializeOwned, Serialize};

use crate::app::AppModel;

/// Cap on how many items are persisted per feed, matching existing
/// pagination page sizes.
const MAX_STATUSES: usize = 200;
const MAX_NOTIFICATIONS: usize = 100;
/// Cap on how many distinct hashtag/list snapshot files are kept.
const MAX_SCOPED_FILES: usize = 5;

fn cache_root() -> Option<PathBuf> {
    dirs::cache_dir().map(|dir| dir.join(AppModel::APP_ID))
}

/// Sanitize an account's base URL into a filesystem-safe directory name.
fn account_slug(base_url: &str) -> String {
    base_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn account_dir(base_url: &str) -> Option<PathBuf> {
    Some(cache_root()?.join(account_slug(base_url)))
}

/// Path for a named feed snapshot within an account's cache directory.
fn snapshot_path(base_url: &str, slug: &str) -> Option<PathBuf> {
    Some(account_dir(base_url)?.join(format!("{slug}.json")))
}

/// Save a bounded slice of items to disk as JSON. Errors are logged, not
/// surfaced — a failed cache write should never interrupt the user.
pub fn save_snapshot<T: Serialize>(base_url: &str, slug: &str, items: &[T], max_items: usize) {
    let Some(path) = snapshot_path(base_url, slug) else {
        return;
    };
    let Some(parent) = path.parent() else { return };
    if let Err(err) = std::fs::create_dir_all(parent) {
        tracing::warn!("failed to create cache directory: {err}");
        return;
    }

    // `items` is ordered newest-first (matches `Timeline`/`Notifications`'
    // front-to-back id ordering), so bound by keeping the front (newest).
    let bounded = &items[..max_items.min(items.len())];
    match serde_json::to_vec(bounded) {
        Ok(data) => {
            if let Err(err) = std::fs::write(&path, data) {
                tracing::warn!("failed to write cache snapshot {}: {err}", path.display());
            }
        }
        Err(err) => tracing::warn!("failed to serialize cache snapshot: {err}"),
    }
}

pub fn save_status_snapshot(base_url: &str, slug: &str, items: &[megalodon::entities::Status]) {
    save_snapshot(base_url, slug, items, MAX_STATUSES);
    prune_scoped_snapshots(base_url);
}

pub fn save_notification_snapshot(
    base_url: &str,
    items: &[megalodon::entities::Notification],
) {
    save_snapshot(base_url, "notifications", items, MAX_NOTIFICATIONS);
}

/// Load a previously-saved snapshot, if any. Returns an empty `Vec` on any
/// error (missing file, corrupt JSON, etc.) — a cold cache is not a failure.
pub fn load_snapshot<T: DeserializeOwned>(base_url: &str, slug: &str) -> Vec<T> {
    let Some(path) = snapshot_path(base_url, slug) else {
        return Vec::new();
    };
    match std::fs::read(&path) {
        Ok(data) => serde_json::from_slice(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Keep only the `MAX_SCOPED_FILES` most-recently-modified hashtag/list
/// snapshots, so browsing many tags/lists doesn't accumulate cache files
/// forever. Called after every status snapshot save (cheap: a handful of
/// directory entries at most).
fn prune_scoped_snapshots(base_url: &str) {
    for prefix in ["tag-", "list-"] {
        prune_prefixed(base_url, prefix);
    }
}

fn prune_prefixed(base_url: &str, prefix: &str) {
    let Some(dir) = account_dir(base_url) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let mut files: Vec<(PathBuf, std::time::SystemTime)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect();

    if files.len() <= MAX_SCOPED_FILES {
        return;
    }

    files.sort_by_key(|(_, modified)| *modified);
    let excess = files.len() - MAX_SCOPED_FILES;
    for (path, _) in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(path);
    }
}

/// Content-addressed disk cache for downloaded images, shared across
/// accounts since media URLs are already public.
fn image_cache_path(url: &str) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let hashed = hasher.finish();
    Some(cache_root()?.join("images").join(format!("{hashed:x}")))
}

pub fn load_image(url: &str) -> Option<Vec<u8>> {
    std::fs::read(image_cache_path(url)?).ok()
}

pub fn save_image(url: &str, bytes: &[u8]) {
    let Some(path) = image_cache_path(url) else {
        return;
    };
    let Some(parent) = path.parent() else { return };
    if let Err(err) = std::fs::create_dir_all(parent) {
        tracing::warn!("failed to create image cache directory: {err}");
        return;
    }
    if let Err(err) = std::fs::write(&path, bytes) {
        tracing::warn!("failed to write cached image {}: {err}", path.display());
    }
}

/// Remove all cache files for an account, called on logout/account removal.
pub fn clear_account(base_url: &str) {
    if let Some(dir) = account_dir(base_url) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
