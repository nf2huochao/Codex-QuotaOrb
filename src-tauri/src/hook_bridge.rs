use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

#[derive(Clone, Default)]
pub struct HookBridge {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
}

impl HookBridge {
    pub async fn register(&self, request_id: String) -> oneshot::Receiver<String> {
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(request_id, sender);
        receiver
    }

    pub async fn resolve(&self, request_id: &str, decision: &str) -> bool {
        let sender = self.pending.lock().await.remove(request_id);
        sender.is_some_and(|sender| sender.send(decision.to_owned()).is_ok())
    }

    pub async fn remove(&self, request_id: &str) {
        self.pending.lock().await.remove(request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::HookBridge;

    #[tokio::test]
    async fn resolves_a_waiting_hook_once() {
        let bridge = HookBridge::default();
        let receiver = bridge.register("hook:test".into()).await;
        assert!(bridge.resolve("hook:test", "accept").await);
        assert!(!bridge.resolve("hook:test", "decline").await);
        assert_eq!(receiver.await.unwrap(), "accept");
    }
}
