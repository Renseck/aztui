use std::any::Any;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::errors::AppError;

/* ============================================================================================== */
/*                                           CacheEntry                                           */
/* ============================================================================================== */

/// A single cached value with soft and hard TTL semantics.
#[derive(Debug)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub soft_ttl: Duration,
    pub hard_ttl: Duration,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, soft_ttl: Duration, hard_ttl: Duration) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            soft_ttl,
            hard_ttl,
        }
    }

    /* ========================================================================================== */

    /// Fresh: within soft TTL - serve immediately without refresh.
    pub fn is_fresh(&self) -> bool {
        self.created_at.elapsed() < self.soft_ttl
    }

    /* ========================================================================================== */

    /// Stale: past soft TTL but within hard TTL - serve cached while refreshing in background.
    pub fn is_stale(&self) -> bool {
        let elapsed = self.created_at.elapsed();
        elapsed >= self.soft_ttl && elapsed < self.hard_ttl
    }

    /* ========================================================================================== */

    /// Expired: past hard TTL - must refresh synchronously before serving.
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.hard_ttl
    }
}

/* ============================================================================================== */
/*                                       Cache key and scope                                      */
/* ============================================================================================== */

/// Scoping prevents cross-tenant / cross-subscription data leakage.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheScope {
    Global,
    Tenant(String),
    Subscription(String),
}

/* ============================================================================================== */

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub scope: CacheScope,
    pub kind: String,
}

impl CacheKey {
    pub fn global(kind: impl Into<String>) -> Self {
        Self {
            scope: CacheScope::Global,
            kind: kind.into(),
        }
    }

    pub fn tenant(tenant_id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            scope: CacheScope::Tenant(tenant_id.into()),
            kind: kind.into(),
        }
    }

    pub fn subscription(subscription_id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            scope: CacheScope::Subscription(subscription_id.into()),
            kind: kind.into(),
        }
    }
}

/* ============================================================================================== */
/*                                           CacheStore                                           */
/* ============================================================================================== */

type BoxedEntry = Box<dyn Any + Send + Sync>;

/// In-memory cache store. Phase 1: no disk persistence or encryption.
/// Phase 2 will add AES-256-GCM encrypted persistence.
pub struct CacheStore {
    entries: HashMap<CacheKey, BoxedEntry>,
}

impl CacheStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /* ========================================================================================== */
    /// Retrieves a typed entry if present. Returns `None` if missing or wrong type.
    pub fn get<T: 'static>(&self, key: &CacheKey) -> Option<&CacheEntry<T>> {
        self.entries
            .get(key)
            .and_then(|boxed| boxed.downcast_ref::<CacheEntry<T>>())
    }

    /* ========================================================================================== */
    /// Stores a typed entry, replacing any existing entry for the same key.
    pub fn put<T: Send + Sync + 'static>(
        &mut self,
        key: CacheKey,
        value: T,
        soft_ttl: Duration,
        hard_ttl: Duration,
    ) {
        let entry = CacheEntry::new(value, soft_ttl, hard_ttl);
        self.entries.insert(key, Box::new(entry));
    }

    /* ========================================================================================== */
    /// Removes a single cache entry.
    pub fn invalidate(&mut self, key: &CacheKey) {
        self.entries.remove(key);
    }

    /* ========================================================================================== */
    /// Removes all entries within a scope (e.g. after tenant switch).
    pub fn invalidate_scope(&mut self, scope: &CacheScope) {
        self.entries.retain(|k, _| &k.scope != scope);
    }

    /* ========================================================================================== */
    /// Removes all cache entries.
    pub fn invalidate_all(&mut self) {
        self.entries.clear();
    }
}

impl Default for CacheStore {
    fn default() -> Self {
        Self::new()
    }
}

/* ============================================================================================== */
/*                                            Tests                                               */
/* ============================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_entry_within_soft_ttl() {
        let entry = CacheEntry::new(
            "value".to_string(),
            Duration::from_secs(300),
            Duration::from_secs(3600),
        );
        assert!(entry.is_fresh());
        assert!(!entry.is_stale());
        assert!(!entry.is_expired());
    }

    #[test]
    fn cache_store_put_and_get() {
        let mut store = CacheStore::new();
        let key = CacheKey::global("tenants");
        store.put(key.clone(), vec!["a", "b"], Duration::from_secs(60), Duration::from_secs(3600));
        let entry = store.get::<Vec<&str>>(&key).unwrap();
        assert_eq!(entry.value, vec!["a", "b"]);
    }

    #[test]
    fn invalidate_scope_clears_matching_entries() {
        let mut store = CacheStore::new();
        let k1 = CacheKey::tenant("t1", "subs");
        let k2 = CacheKey::global("tenants");
        store.put(k1.clone(), "v1", Duration::from_secs(60), Duration::from_secs(3600));
        store.put(k2.clone(), "v2", Duration::from_secs(60), Duration::from_secs(3600));
        store.invalidate_scope(&CacheScope::Tenant("t1".into()));
        assert!(store.get::<&str>(&k1).is_none());
        assert!(store.get::<&str>(&k2).is_some());
    }
}