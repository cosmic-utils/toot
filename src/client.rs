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

/// All accounts saved in the keychain, and which one is active. Persisted as
/// a single JSON blob under one keychain entry.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Sessions {
    pub active: usize,
    pub sessions: Vec<Session>,
}

impl Sessions {
    /// Parse the stored keychain payload, falling back to interpreting it as
    /// a single legacy (pre-multi-account) session if it isn't a [`Sessions`].
    pub fn parse(data: &str) -> Option<Sessions> {
        if let Ok(sessions) = serde_json::from_str::<Sessions>(data) {
            return Some(sessions);
        }
        serde_json::from_str::<Session>(data)
            .ok()
            .map(|session| Sessions {
                active: 0,
                sessions: vec![session],
            })
    }

    pub fn active_session(&self) -> Option<&Session> {
        self.sessions.get(self.active)
    }

    /// Add a session, or replace an existing one for the same instance, and
    /// make it the active one.
    pub fn upsert_active(&mut self, session: Session) {
        if let Some(index) = self
            .sessions
            .iter()
            .position(|existing| existing.base_url == session.base_url)
        {
            self.sessions[index] = session;
            self.active = index;
        } else {
            self.sessions.push(session);
            self.active = self.sessions.len() - 1;
        }
    }

    /// Remove the session at `index`. If it was the active one, activate the
    /// previous session (or the new one at the same position), if any remain.
    pub fn remove(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }
        self.sessions.remove(index);
        if self.sessions.is_empty() {
            self.active = 0;
        } else {
            self.active = self.active.min(self.sessions.len() - 1);
        }
    }
}
