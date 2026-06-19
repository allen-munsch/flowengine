use flowcore::Value;
use flowpersist::PersistentStore;
use std::collections::HashMap;
use std::sync::Arc;

/// Caches node execution results keyed by (node_type, config_hash, input_hash).
/// Wraps PersistentStore with async-friendly access.
pub struct ExecutorCache {
    store: Arc<PersistentStore>,
}

impl ExecutorCache {
    pub fn new(store: Arc<PersistentStore>) -> Self {
        Self { store }
    }

    /// Check for a cached result. Returns None if no cache hit or cache expired.
    pub async fn check(
        &self,
        node_type: &str,
        config_hash: &str,
        input_hash: &str,
    ) -> Option<HashMap<String, Value>> {
        let store = self.store.clone();
        let nt = node_type.to_string();
        let ch = config_hash.to_string();
        let ih = input_hash.to_string();

        tokio::task::spawn_blocking(move || {
            store.get_cached_result(&nt, &ch, &ih).ok().flatten()
        })
        .await
        .ok()
        .flatten()
    }

    /// Store a result in the cache.
    pub async fn store(
        &self,
        node_type: &str,
        config_hash: &str,
        input_hash: &str,
        outputs: &HashMap<String, Value>,
        ttl_seconds: Option<i64>,
    ) {
        let store = self.store.clone();
        let nt = node_type.to_string();
        let ch = config_hash.to_string();
        let ih = input_hash.to_string();
        let outputs = outputs.clone();

        let _ = tokio::task::spawn_blocking(move || {
            store.cache_result(&nt, &ch, &ih, &outputs, ttl_seconds)
        })
        .await;
    }
}

/// Compute a hash combining node type, config, and inputs.
#[allow(dead_code)]
pub fn compute_cache_key(
    _node_type: &str,
    config: &HashMap<String, Value>,
    inputs: &HashMap<String, Value>,
) -> (String, String) {
    let config_hash = PersistentStore::compute_hash(config);
    let input_hash = PersistentStore::compute_hash(inputs);
    (config_hash, input_hash)
}
