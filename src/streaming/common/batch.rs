// ─── import crates ───
use crate::streaming::events::UnifiedEvent;

// ─── struct 'EventBatchProcessor' ───
/// struct description
pub struct EventBatchProcessor<F> where F: FnMut(Vec<Box<dyn UnifiedEvent>>) + Send + Sync + 'static {
    pub(crate) callback: F,
    batch: Vec<Box<dyn UnifiedEvent>>,
    batch_size: usize,
    timeout_ms: u64,
    last_flush_time: std::time::Instant,
}

// ─── impl 'EventBatchProcessor' ───
/// impl description
impl<F> EventBatchProcessor<F> where F: FnMut(Vec<Box<dyn UnifiedEvent>>) + Send + Sync + 'static {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(callback: F, batch_size: usize, timeout_ms: u64) -> Self {

        // ─── return 'Self' ───
        Self { callback, batch: Vec::with_capacity(batch_size), batch_size, timeout_ms, last_flush_time: std::time::Instant::now() }
    }

    // ─── fn 'add_event' ───
    /// fn description
    pub fn add_event(&mut self, event: Box<dyn UnifiedEvent>) {
        log::debug!("Adding event to batch: {} (type: {:?})", event.id(), event.event_type());
        self.batch.push(event);

        // ─── compare 'self.batch_size' ───
        if self.batch.len() >= self.batch_size || self.should_flush_by_timeout() {
            log::debug!("Flushing batch: size={}, timeout={}", self.batch.len(), self.should_flush_by_timeout());
            self.flush();
        }
    }

    // ─── fn 'flush' ───
    /// fn description
    pub fn flush(&mut self) {

        // ─── compare 'self.batch.is_empty()' ───
        if !self.batch.is_empty() {

            // ─── define 'events' ───
            let events = std::mem::replace(&mut self.batch, Vec::with_capacity(self.batch_size));
            log::debug!("Flushing {} events from batch processor", events.len());

            // ─── compare 'log::log_enabled!()' ───
            if log::log_enabled!(log::Level::Debug) {

                // ─── proceed 'for' ───
                for (i, event) in events.iter().enumerate() {
                    log::debug!("Event {}: Type={:?}, ID={}", i, event.event_type(), event.id());
                }
            }

            // ─── match 'std::panic::catch_unwind()' ───
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||{(self.callback)(events)})) {
                Ok(_) => {
                    log::debug!("Batch callback executed successfully");
                }
                Err(e) => {
                    log::error!("Batch callback panicked: {:?}", e);
                }
            }

            // ─── declare 'self.last_flush_time' ───
            self.last_flush_time = std::time::Instant::now();
        } else {
            log::debug!("No events to flush");
        }
    }

    // ─── fn 'current_batch_size' ───
    /// fn description
    pub fn current_batch_size(&self) -> usize {

        // ─── return 'self.batch.len()' ───
        self.batch.len()
    }

    // ─── fn 'should_flush_by_timeout' ───
    /// fn description
    fn should_flush_by_timeout(&self) -> bool {

        // ─── return 'self.last_flush_time.elapsed()' ───
        self.last_flush_time.elapsed().as_millis() >= self.timeout_ms as u128
    }

    // ─── fn 'is_batch_full' ───
    /// fn description
    pub fn is_batch_full(&self) -> bool {

        // ─── return 'self.batch.len()' ───
        self.batch.len() >= self.batch_size
    }

    // ─── fn 'should_flush' ───
    /// fn description
    pub fn should_flush(&self) -> bool {

        // ─── return 'self.is_batch_full()' ───
        self.is_batch_full() || self.should_flush_by_timeout()
    }
}

// ─── impl 'SimpleEventBatchProcessor' ───
/// impl description
pub struct SimpleEventBatchProcessor<F> where F: Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static {
    callback: F
}

// ─── impl 'SimpleEventBatchProcessor' ───
/// impl description
impl<F> SimpleEventBatchProcessor<F> where F: Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(callback: F) -> Self {

        // ─── return 'Self' ───
        Self { callback }
    }

    // ─── fn 'process_batch' ───
    /// fn description
    pub fn process_batch(&self, events: Vec<Box<dyn UnifiedEvent>>) {

        // ─── proceed 'for' ───
        for event in events {
            (self.callback)(event);
        }
    }
}