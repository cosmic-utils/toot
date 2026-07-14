// SPDX-License-Identifier: {{LICENSE}}

use std::sync::Arc;

use megalodon::Megalodon;

/// Wraps a megalodon client together with the connection details we need to
/// track ourselves (megalodon's clients don't expose their base URL or token).
#[derive(Clone)]
pub struct Client {
    pub base_url: String,
    pub token: Option<String>,
    inner: Arc<Box<dyn Megalodon + Send + Sync>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.base_url)
            .field("token", &self.token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl std::hash::Hash for Client {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base_url.hash(state);
        self.token.hash(state);
    }
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.base_url == other.base_url && self.token == other.token
    }
}

impl Eq for Client {}

impl Client {
    pub fn new(base_url: String, token: Option<String>) -> Self {
        let inner = megalodon::generator(
            megalodon::SNS::Mastodon,
            base_url.clone(),
            token.clone(),
            Some("toot".to_string()),
        )
        .expect("failed to create megalodon client");
        Self {
            base_url,
            token,
            inner: Arc::new(inner),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.token.as_ref().is_some_and(|token| !token.is_empty())
    }
}

impl std::ops::Deref for Client {
    type Target = dyn Megalodon + Send + Sync;

    fn deref(&self) -> &Self::Target {
        &**self.inner
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Session {
    pub base_url: String,
    pub token: String,
}
