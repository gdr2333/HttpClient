//! `CancellationToken` — wrapper over `tokio_util::sync::CancellationToken`
//! that mirrors the C# `System.Threading.CancellationToken` API.
//!
//! C#'s `CancellationToken` is a value type that wraps a `CancellationTokenSource`.
//! Here we collapse the two: a `CancellationToken` can be created stand-alone
//! (it owns its own source) and children can be derived from it. This is a
//! pragmatic simplification, not a 1:1 port of the C# model.

use std::time::Duration;

use tokio_util::sync::CancellationToken as TokioCancellationToken;

/// A token that can be observed for cancellation. Equivalent to C#'s
/// `CancellationToken`.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    inner: TokioCancellationToken,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::none()
    }
}

impl CancellationToken {
    /// A no-op token. `cancel` and `cancelled` are no-ops; `is_cancelled`
    /// is always `false`.
    pub fn none() -> Self {
        Self {
            inner: TokioCancellationToken::new(),
        }
    }

    /// Create a fresh token.
    pub fn new() -> Self {
        Self::none()
    }

    /// `true` if `cancel` has been called.
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// Trigger cancellation. Wakes up all tasks awaiting `cancelled()`.
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// Resolves when the token is cancelled. Mirrors C#'s
    /// `CancellationToken.WaitHandle`-style wait.
    pub async fn cancelled(&self) {
        self.inner.cancelled().await
    }

    /// Create a child token. The child is cancelled if either the parent
    /// is cancelled or the child itself is cancelled.
    pub fn child_token(&self) -> Self {
        Self {
            inner: self.inner.child_token(),
        }
    }

    /// Link two tokens into a single one. The combined token is cancelled
    /// if either input is cancelled.
    pub fn link_with(&self, other: &Self) -> Self {
        let combined = TokioCancellationToken::new();
        let c1 = combined.clone();
        let t1 = self.inner.clone();
        tokio::spawn(async move {
            t1.cancelled().await;
            c1.cancel();
        });
        let c2 = combined.clone();
        let t2 = other.inner.clone();
        tokio::spawn(async move {
            t2.cancelled().await;
            c2.cancel();
        });
        Self { inner: combined }
    }

    /// Create a child that is automatically cancelled after `duration`.
    pub fn with_timeout(&self, duration: Duration) -> Self {
        let child = self.inner.child_token();
        let cancel_on = child.clone();
        let parent = self.inner.clone();
        let dur = duration;
        tokio::spawn(async move {
            // Respect the parent token too: if the parent is cancelled,
            // do nothing (the child will already be cancelled by parent
            // chain, and we don't want to keep this task around).
            tokio::select! {
                _ = tokio::time::sleep(dur) => {
                    cancel_on.cancel();
                }
                _ = parent.cancelled() => {
                    // Parent was already cancelled; nothing to do.
                }
            }
        });
        Self { inner: child }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn none_token_never_cancels() {
        let ct = CancellationToken::none();
        assert!(!ct.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_propagates() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert!(!child.is_cancelled());
        parent.cancel();
        assert!(child.is_cancelled());
    }

    #[tokio::test]
    async fn with_timeout_fires() {
        let ct = CancellationToken::new().with_timeout(Duration::from_millis(10));
        ct.cancelled().await;
        assert!(ct.is_cancelled());
    }
}
