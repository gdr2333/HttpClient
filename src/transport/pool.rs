//! Connection pool. A `HashMap` of idle streams keyed on `(scheme, host,
//! port)`, plus a maximum-connection cap. Cloneable: all internal state is
//! behind `Arc<Mutex<...>>`.
//!
//! The current iteration is bookkeeping-only: it does not store real
//! streams (which would be enum `Plain(TcpStream) | Tls(TlsStream<...>)`).
//! That's deferred to when we add per-stream expiry and rotation. For now
//! the pool serves as a "is this connection reusable" gate, with the
//! transport always opening a fresh stream on each call.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The pool's key: scheme + host + port.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PoolKey {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
}

/// HTTP scheme.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

/// A minimal connection pool. The `Mutex` is coarse-grained but sufficient
/// for a hand-rolled HTTP/1.1 client; the pool never holds large amounts of
/// data, just bookkeeping.
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    inner: Arc<Mutex<PoolInner>>,
}

#[derive(Debug)]
struct PoolInner {
    /// Idle streams keyed by `PoolKey`.
    idle: HashMap<PoolKey, Vec<String>>,
    /// Max idle streams per key.
    max_idle_per_key: usize,
    /// Max total open connections.
    max_total: usize,
    /// Currently open connections.
    open: usize,
}

/// Builder for `ConnectionPool` — used to set the cap at construction time.
#[derive(Debug, Clone)]
pub struct ConnectionPoolBuilder {
    max_idle_per_key: usize,
    max_total: usize,
}

impl Default for ConnectionPoolBuilder {
    fn default() -> Self {
        Self {
            max_idle_per_key: 4,
            max_total: 100,
        }
    }
}

impl ConnectionPoolBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn max_idle_per_key(mut self, n: usize) -> Self {
        self.max_idle_per_key = n;
        self
    }
    pub fn max_total(mut self, n: usize) -> Self {
        self.max_total = n;
        self
    }
    pub fn build(self) -> ConnectionPool {
        ConnectionPool {
            inner: Arc::new(Mutex::new(PoolInner {
                idle: HashMap::new(),
                max_idle_per_key: self.max_idle_per_key,
                max_total: self.max_total,
                open: 0,
            })),
        }
    }
}

impl ConnectionPool {
    /// Create a new pool with default limits (4 idle per key, 100 total).
    pub fn new() -> Self {
        ConnectionPoolBuilder::new().build()
    }

    /// Builder entry point.
    pub fn builder() -> ConnectionPoolBuilder {
        ConnectionPoolBuilder::new()
    }

    /// Take an idle stream for `key`, if any.
    pub async fn try_get(&self, key: &PoolKey) -> Option<String> {
        let mut g = self.inner.lock().await;
        if let Some(bucket) = g.idle.get_mut(key) {
            return bucket.pop();
        }
        None
    }

    /// Return a stream to the pool. If the bucket is full, the stream is
    /// dropped.
    pub async fn put(&self, key: PoolKey, conn: String) {
        let mut g = self.inner.lock().await;
        let limit = g.max_idle_per_key;
        if let Some(bucket) = g.idle.get_mut(&key) {
            if bucket.len() < limit {
                bucket.push(conn);
                return;
            }
        } else {
            g.idle.insert(key, vec![conn]);
            return;
        }
        // Bucket is full — drop the connection.
        g.open = g.open.saturating_sub(1);
    }

    /// Notify the pool that a new connection has been opened.
    pub async fn notify_opened(&self) {
        let mut g = self.inner.lock().await;
        g.open += 1;
    }

    /// Notify the pool that a connection has been closed.
    pub async fn notify_closed(&self) {
        let mut g = self.inner.lock().await;
        g.open = g.open.saturating_sub(1);
    }

    /// `true` if more connections are allowed.
    pub async fn can_open(&self) -> bool {
        let g = self.inner.lock().await;
        g.open < g.max_total
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}
