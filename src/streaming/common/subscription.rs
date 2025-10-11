// ─── import packages ───
use tokio::task::JoinHandle;

// ─── struct 'SubscriptionHandle' ───
/// struct description
pub struct SubscriptionHandle {
    stream_handle: JoinHandle<()>,
    event_handle: JoinHandle<()>,
    metrics_handle: Option<JoinHandle<()>>
}

// ─── impl 'SubscriptionHandle' ───
/// impl description
impl SubscriptionHandle {

    // ─── fn 'SubscriptionHandle' ───
    /// fn description
    pub fn new(stream_handle: JoinHandle<()>, event_handle: JoinHandle<()>, metrics_handle: Option<JoinHandle<()>>) -> Self {

        // ─── return 'Self' ───
        Self { stream_handle, event_handle, metrics_handle }
    }

    // ─── fn 'stop' ───
    /// fn description
    pub fn stop(self) {
        self.stream_handle.abort();
        self.event_handle.abort();

        // ─── compare 'self.metrics_handle' ───
        if let Some(handle) = self.metrics_handle {
            handle.abort();
        }
    }

    // ─── fn 'join' ───
    /// fn description
    pub async fn join(self) -> Result<(), tokio::task::JoinError> {

        // ─── callback 'self.metrics_handle' ───
        let _ = self.stream_handle.await;

        // ─── callback 'self.event_handle' ───
        let _ = self.event_handle.await;

        // ─── compare 'self.metrics_handle' ───
        if let Some(handle) = self.metrics_handle {

            // ─── callback 'handle' ───
            let _ = handle.await;
        }

        // ─── return 'Result' ───
        Ok(())
    }
}