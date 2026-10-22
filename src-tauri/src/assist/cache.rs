//! Memory-only sentence cache, keyed by text hash.
//!
//! Word re-sends the same paragraph text while the user edits elsewhere in
//! the document; re-linting an unchanged sentence is wasted work. The cache
//! maps SHA-256(text) → cached issues for Tier 0 and the last accepted
//! Tier 1 verdict for Tier 1, so repeated checks are O(hash).
//!
//! PRIVACY: entries live in memory only, are never serialized, never
//! logged, and the key is a hash — the original text is NOT retained (the
//! cache stores results, which the caller must treat as derived data with
//! the same lifetime as the process). Capacity is bounded; insertion
//! evicts the least-recently-used entry.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// Capacity used by the shared state (settings.rs references this so the
/// bound is documented in one place).
pub const DEFAULT_SENTENCE_CACHE_CAPACITY: usize = 512;

/// Tier 1 outcome for one sentence, in the wire shape.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Tier1Status {
    /// Tier 1 is disabled in settings.
    Off,
    /// A model call is queued/running for this sentence; the add-in
    /// re-posts /v1/check to pick up the verdict.
    Pending,
    /// A validated rewrite is available. Edits are word-aligned,
    /// sentence-relative, scalar offsets (the add-in maps them onto the
    /// annotation range it owns; the server also provides utf16 maps).
    Ready(Box<Tier1Suggestion>),
    /// The model answered but a validation rule rejected the rewrite
    /// (six rules, validate.rs). `rule` is the stable rule number.
    Rejected { rule: u8 },
    /// The daemon call failed (not running, timeout, model missing).
    Failed { reason: String },
}

/// A validated Tier 1 rewrite for one sentence.
#[derive(Debug, Clone, Serialize)]
pub struct Tier1Suggestion {
    /// Scalar offsets of the sentence inside the paragraph.
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub revised: String,
    pub explanation: Option<String>,
    /// Word-level edits against the ORIGINAL SENTENCE, scalar offsets.
    pub edits: Vec<super::diff::WordEdit>,
}

/// Everything cached for one sentence, keyed by hash of the sentence text.
#[derive(Debug, Clone)]
pub struct SentenceVerdict {
    /// Tier 0 issues (sentence-relative, scalar + utf16 spans).
    pub issues: Vec<super::tier0::AssistIssue>,
    pub tier1: Tier1Status,
}

impl SentenceVerdict {
    pub fn tier0_only(issues: Vec<super::tier0::AssistIssue>) -> Self {
        Self {
            issues,
            tier1: Tier1Status::Off,
        }
    }
}

/// Hash a text the way every cache key is formed (single definition so no
/// call site invents its own normalization).
pub fn text_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

struct Inner {
    map: HashMap<String, Arc<SentenceVerdict>>,
    order: Vec<String>, // LRU order, front = most recent
    capacity: usize,
}

/// Process-wide cache instance is owned by the caller (1b state); the type
/// itself is a plain mutex-guarded LRU.
pub struct SentenceCache {
    inner: Mutex<Inner>,
}

impl SentenceCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                map: HashMap::new(),
                order: Vec::new(),
                capacity: capacity.max(1),
            }),
        }
    }

    pub fn get(&self, text: &str) -> Option<Arc<SentenceVerdict>> {
        let key = text_hash(text);
        let mut inner = self.inner.lock().expect("sentence cache lock");
        if let Some(hit) = inner.map.get(&key) {
            let hit = Arc::clone(hit);
            touch(&mut inner.order, key);
            return Some(hit);
        }
        None
    }

    pub fn put(&self, text: &str, verdict: SentenceVerdict) {
        let key = text_hash(text);
        let mut inner = self.inner.lock().expect("sentence cache lock");
        if !inner.map.contains_key(&key) && inner.map.len() >= inner.capacity {
            // Evict LRU (back of the order vec).
            if let Some(oldest) = inner.order.pop() {
                inner.map.remove(&oldest);
            }
        }
        inner.map.insert(key.clone(), Arc::new(verdict));
        touch(&mut inner.order, key);
    }

    /// Invalidate one sentence (after an accepted edit, the old cached
    /// verdict for the pre-edit text is stale).
    pub fn invalidate(&self, text: &str) {
        let key = text_hash(text);
        let mut inner = self.inner.lock().expect("sentence cache lock");
        inner.map.remove(&key);
        inner.order.retain(|k| *k != key);
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("sentence cache lock");
        inner.map.clear();
        inner.order.clear();
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("sentence cache lock").map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_capacity_bounded(&self) -> bool {
        self.inner.lock().expect("sentence cache lock").capacity > 0
    }
}

fn touch(order: &mut Vec<String>, key: String) {
    if let Some(pos) = order.iter().position(|k| *k == key) {
        order.remove(pos);
    }
    order.insert(0, key);
    order.truncate(order.len().min(4096));
}

#[cfg(test)]
mod tests {
    use super::{text_hash, SentenceCache, SentenceVerdict};

    #[test]
    fn hash_is_stable_and_text_is_not_stored() {
        let a = text_hash("The cat sat.");
        let b = text_hash("The cat sat.");
        let c = text_hash("The cat sat ");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64); // sha256 hex
    }

    #[test]
    fn put_get_invalidate() {
        let cache = SentenceCache::new(8);
        cache.put("one sentence.", SentenceVerdict::tier0_only(vec![]));
        assert!(cache.get("one sentence.").is_some());
        assert!(cache.get("other sentence.").is_none());
        cache.invalidate("one sentence.");
        assert!(cache.get("one sentence.").is_none());
    }

    #[test]
    fn eviction_bounded() {
        let cache = SentenceCache::new(2);
        cache.put("a", SentenceVerdict::tier0_only(vec![]));
        cache.put("b", SentenceVerdict::tier0_only(vec![]));
        cache.put("c", SentenceVerdict::tier0_only(vec![])); // evicts "a"
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
        assert_eq!(cache.len(), 2);
        assert!(cache.is_capacity_bounded());
    }

    #[test]
    fn lru_refresh_keeps_hot_entries() {
        let cache = SentenceCache::new(2);
        cache.put("a", SentenceVerdict::tier0_only(vec![]));
        cache.put("b", SentenceVerdict::tier0_only(vec![]));
        cache.get("a"); // refresh a
        cache.put("c", SentenceVerdict::tier0_only(vec![])); // evicts b, not a
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
    }
}
