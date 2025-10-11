// ─── import packages ───
use std::sync::Arc;
use tokio::sync::Mutex;

// ─── import crates ───
use crate::globals::constants::*;
use crate::streaming::common::config::StreamClientConfig;

// ─── struct 'EventMetrics' ───
/// struct description
#[derive(Debug, Clone)]
pub struct EventMetrics {
    pub process_count: u64,
    pub events_processed: u64,
    pub events_per_second: f64,
    pub events_in_window: u64,
    pub window_start_time: std::time::Instant
}

// ─── impl 'EventMetrics' ───
/// impl description
impl EventMetrics {

    // ─── fn 'new' ───
    /// fn description
    fn new(now: std::time::Instant) -> Self {

        // ─── return 'Self' ───
        Self { process_count: 0, events_processed: 0, events_per_second: 0.0, events_in_window: 0, window_start_time: now }
    }
}

// ─── struct 'PerformanceMetrics' ───
/// struct description
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub start_time: std::time::Instant,
    pub event_metrics: [EventMetrics; 3],
    pub average_processing_time_ms: f64,
    pub min_processing_time_ms: f64,
    pub max_processing_time_ms: f64,
    pub last_update_time: std::time::Instant
}

// ─── impl 'Default for PerformanceMetrics' ───
/// impl description
impl Default for PerformanceMetrics {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {
        Self::new()
    }
}

// ─── enum 'MetricsEventType' ───
/// enum description
#[derive(Debug, Clone, Copy)]
pub enum MetricsEventType {
    Tx,
    Account,
    BlockMeta
}

// ─── impl 'MetricsEventType' ───
/// impl description
impl MetricsEventType {

    // ─── fn 'as_index' ───
    /// fn description
    fn as_index(&self) -> usize {

        // ─── match 'self' ───
        match self {
            MetricsEventType::Tx => 0,
            MetricsEventType::Account => 1,
            MetricsEventType::BlockMeta => 2
        }
    }

    // ─── const 'ALL' ───
    const ALL: [MetricsEventType; 3] = [
        MetricsEventType::Tx,
        MetricsEventType::Account,
        MetricsEventType::BlockMeta
    ];
}

// ─── impl 'PerformanceMetrics' ───
/// impl description
impl PerformanceMetrics {

    // ─── fn 'new' ───
    /// fn description
    pub fn new() -> Self {

        // ─── define 'now' ───
        let now = std::time::Instant::now();

        // ─── return 'Self' ───
        Self { start_time: now, event_metrics: [EventMetrics::new(now), EventMetrics::new(now), EventMetrics::new(now)],
            average_processing_time_ms: 0.0, min_processing_time_ms: 0.0, max_processing_time_ms: 0.0, last_update_time: now }
    }

    // ─── fn 'update_window_metrics' ───
    /// fn description
    fn update_window_metrics(&mut self, event_type: &MetricsEventType, now: std::time::Instant, window_duration: std::time::Duration) {

        // ─── define 'index' ───
        let index = event_type.as_index();

        // ─── define 'event_metric' ───
        let event_metric = &mut self.event_metrics[index];

        // ─── compare 'window_duration' ───
        if now.duration_since(event_metric.window_start_time) >= window_duration {

            // ─── define 'window_seconds' ───
            let window_seconds = now.duration_since(event_metric.window_start_time).as_secs_f64();
            event_metric.events_per_second = if window_seconds > 0.001 {
                event_metric.events_in_window as f64 / window_seconds
            } else {
                0.0
            };
            event_metric.events_in_window = 0;
            event_metric.window_start_time = now;
        }
    }

    // ─── fn 'calculate_real_time_events_per_second' ───
    /// fn description
    fn calculate_real_time_events_per_second(&self, event_type: &MetricsEventType, now: std::time::Instant) -> f64 {

        // ─── define 'index' ───
        let index = event_type.as_index();

        // ─── define 'event_metric' ───
        let event_metric = &self.event_metrics[index];

        // ─── define 'current_window_duration' ───
        let current_window_duration = now.duration_since(event_metric.window_start_time).as_secs_f64();

        // ─── compare 'current_window_duration' ───
        if current_window_duration > 1.0 && event_metric.events_in_window > 0 {
            event_metric.events_in_window as f64 / current_window_duration
        } else if event_metric.events_per_second > 0.0 {
            event_metric.events_per_second
        } else {

            // ─── define 'total_duration' ───
            let total_duration = now.duration_since(self.start_time).as_secs_f64();

            // ─── compare 'total_duration' ───
            if total_duration > 1.0 && event_metric.events_processed > 0 {
                event_metric.events_processed as f64 / total_duration
            } else {
                0.0
            }
        }
    }
}

// ─── enum 'MetricsMsg' ───
/// enum description
#[derive(Debug)]
enum MetricsMsg {
    IncProcess { event_type: MetricsEventType },
    Update {
        event_type: MetricsEventType,
        events_processed: u64,
        processing_time_ms: f64
    }
}

// ─── struct 'MetricsManager' ───
/// struct description
pub struct MetricsManager {
    metrics: Arc<Mutex<PerformanceMetrics>>,
    config: Arc<StreamClientConfig>,
    stream_name: String,
    tx: tokio::sync::mpsc::Sender<MetricsMsg>
}

// ─── struct 'MetricsManager' ───
/// struct description
impl MetricsManager {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(metrics: Arc<Mutex<PerformanceMetrics>>, config: Arc<StreamClientConfig>, stream_name: String) -> Self {

        // ─── define '(tx, rx)' ───
        let (tx, rx) = tokio::sync::mpsc::channel::<MetricsMsg>(METRICSCHANNELBOUND);

        // ─── define 'manager' ───
        let manager = Self { metrics: metrics.clone(), config: config.clone(), stream_name, tx };

        // ─── compare 'config.enable_metrics' ───
        if config.enable_metrics {
            tokio::spawn(Self::run_aggregator(metrics, rx, std::time::Duration::from_millis(METRICSFLUSHINT), std::time::Duration::from_secs(DEFMETRICSWINSEC)));
        }

        // ─── declare 'manager' ───
        manager
    }

    // ─── fn 'run_aggregator' ───
    /// fn description
    async fn run_aggregator(metrics: Arc<Mutex<PerformanceMetrics>>, mut rx: tokio::sync::mpsc::Receiver<MetricsMsg>,
        flush_every: std::time::Duration, window_every: std::time::Duration) {

        // ─── define 'flush_tick' ───
        let mut flush_tick = tokio::time::interval(flush_every);

        // ─── define 'window_tick' ───
        let mut window_tick = tokio::time::interval(window_every);

        // ─── proceed 'loop' ───
        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {

                    // ─── compare 'maybe_msg' ───
                    if let Some(msg) = maybe_msg {

                        // ─── define 'm' ───
                        let mut m = metrics.lock().await;

                        // ─── define 'now' ───
                        let now = std::time::Instant::now();

                        // ─── match 'msg' ───
                        match msg {
                            MetricsMsg::IncProcess { event_type } => {
                                m.event_metrics[event_type.as_index()].process_count += 1;
                                m.last_update_time = now;
                            }
                            MetricsMsg::Update { event_type, events_processed, processing_time_ms } => {

                                // ─── define 'idx' ───
                                let idx = event_type.as_index();

                                // ─── define 'total_after' ───
                                let total_after = {

                                    // ─── define 'em' ───
                                    let em = &mut m.event_metrics[idx];
                                    em.events_processed += events_processed;
                                    em.events_in_window += events_processed;
                                    em.events_processed
                                };

                                // ─── update 'm.last_update_time' ───
                                m.last_update_time = now;

                                // ─── compare 'm.min_processing_time_ms' ───
                                if m.min_processing_time_ms == 0.0 || processing_time_ms < m.min_processing_time_ms {
                                    m.min_processing_time_ms = processing_time_ms;
                                }

                                // ─── compare 'processing_time_ms' ───
                                if processing_time_ms > m.max_processing_time_ms {
                                    m.max_processing_time_ms = processing_time_ms;
                                }

                                // ─── compare 'total_after' ───
                                if total_after > 0 {

                                    // ─── define 'total_f' ───
                                    let total_f = total_after as f64;

                                    // ─── define 'old_total' ───
                                    let old_total = (total_f - events_processed as f64).max(0.0);
                                    m.average_processing_time_ms = if old_total > 0.0 {
                                        (m.average_processing_time_ms * old_total + processing_time_ms * events_processed as f64) / total_f
                                    } else {
                                        processing_time_ms
                                    };
                                }
                            }
                        }
                    } else {
                        // ─── proceed 'break' ───
                        break;
                    }
                }

                // ─── callback 'flush_tick.tick()' ───
                _ = flush_tick.tick() => {
                }

                // ─── callback 'window_tick.tick()' ───
                _ = window_tick.tick() => {

                    // ─── define 'm' ───
                    let mut m = metrics.lock().await;

                    // ─── define 'now' ───
                    let now = std::time::Instant::now();

                    // ─── proceed 'for' ───
                    for et in &MetricsEventType::ALL {
                        m.update_window_metrics(et, now, window_every);
                    }
                }
            }
        }
    }

    // ─── fn 'get_metrics' ───
    /// fn description
    pub async fn get_metrics(&self) -> PerformanceMetrics {

        // ─── define 'metrics' ───
        let metrics = self.metrics.lock().await;
        metrics.clone()
    }

    // ─── fn 'print_metrics' ───
    /// fn description
    pub async fn print_metrics(&self) {

        // ─── define 'metrics' ───
        let metrics = self.get_metrics().await;

        // ─── define 'event_names' ───
        let event_names = ["TX", "Account", "Block Meta"];

        // ─── define 'event_types' ───
        let event_types = [MetricsEventType::Tx, MetricsEventType::Account, MetricsEventType::BlockMeta];

        // ─── define 'now' ───
        let now = std::time::Instant::now();

        println!("\n{} Performance Metrics", self.stream_name);
        println!("   Run Time: {:?}", metrics.start_time.elapsed());
        println!("┌─────────────┬──────────────┬──────────────────┬─────────────────┐");
        println!("│ Event Type  │ Process Count│ Events Processed │ Events/Second   │");
        println!("├─────────────┼──────────────┼──────────────────┼─────────────────┤");

        // ─── proceed 'for' ───
        for (i, name) in event_names.iter().enumerate() {

            // ─── define 'real_time_eps' ───
            let real_time_eps = metrics.calculate_real_time_events_per_second(&event_types[i], now);

            // ─── define 'em' ───
            let em = &metrics.event_metrics[i];
            println!("│ {:11} │ {:12} │ {:16} │ {:13.2}   │", name, em.process_count, em.events_processed, real_time_eps);
        }

        println!("└─────────────┴──────────────┴──────────────────┴─────────────────┘");
        println!("\nProcessing Time Statistics");
        println!("┌─────────────────────┬─────────────┐");
        println!("│ Metric              │ Value (ms)  │");
        println!("├─────────────────────┼─────────────┤");
        println!("│ Average             │ {:9.2}   │", metrics.average_processing_time_ms);
        println!("│ Minimum             │ {:9.2}   │", metrics.min_processing_time_ms);
        println!("│ Maximum             │ {:9.2}   │", metrics.max_processing_time_ms);
        println!("└─────────────────────┴─────────────┘");
        println!();
    }

    // ─── fn 'start_auto_monitoring' ───
    /// fn description
    pub async fn start_auto_monitoring(&self) -> Option<tokio::task::JoinHandle<()>> {

        // ─── compare 'self.config.enable_metrics' ───
        if !self.config.enable_metrics {
            return None;
        }

        // ─── define 'mgr' ───
        let mgr = self.clone();
        Some(tokio::spawn(async move {

            // ─── define 'interval' ───
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
                DEFMETRICSPRINTINT,
            ));
            loop {
                interval.tick().await;
                mgr.print_metrics().await;
            }
        }))
    }

    // ─── fn 'add_process_count' ───
    /// fn description
    pub async fn add_process_count(&self, event_type: MetricsEventType) {

        // ─── compare 'self.config.enable_metrics' ───
        if !self.config.enable_metrics {
            return;
        }

        // ─── callback 'self.tx.try_send()' ───
        let _ = self.tx.try_send(MetricsMsg::IncProcess { event_type });
    }

    // ─── fn 'add_process_count' ───
    /// fn description
    pub async fn add_tx_process_count(&self) {

        // ─── return 'self.add_process_count()' ───
        self.add_process_count(MetricsEventType::Tx).await;
    }

    // ─── fn 'add_process_count' ───
    /// fn description
    pub async fn add_account_process_count(&self) {

        // ─── return 'self.add_process_count()' ───
        self.add_process_count(MetricsEventType::Account).await;
    }

    // ─── fn 'add_process_count' ───
    /// fn description
    pub async fn add_block_meta_process_count(&self) {

        // ─── return 'self.add_process_count()' ───
        self.add_process_count(MetricsEventType::BlockMeta).await;
    }

    // ─── fn 'add_process_count' ───
    /// fn description
    pub async fn update_metrics(&self, event_type: MetricsEventType, events_processed: u64, processing_time_ms: f64) {

        // ─── compare 'self.config.enable_metrics' ───
        if !self.config.enable_metrics {
            return;
        }

        // ─── callback 'self.tx.try_send()' ───
        let _ = self.tx.try_send(MetricsMsg::Update {
            event_type,
            events_processed,
            processing_time_ms,
        });
    }

    // ─── fn 'log_slow_processing' ───
    /// fn description
    pub fn log_slow_processing(&self, processing_time_ms: f64, event_count: usize) {

        // ─── compare 'processing_time_ms' ───
        if processing_time_ms > SLOWPROCESSINGTHRESHOLD {
            log::warn!("{} slow processing: {processing_time_ms}ms for {event_count} events", self.stream_name);
        }
    }
}

// ─── impl 'MetricsManager' ───
/// impl description
impl Clone for MetricsManager {

    // ─── fn 'clone' ───
    /// fn description
    fn clone(&self) -> Self {

        // ─── return 'Self' ───
        Self { metrics: self.metrics.clone(), config: self.config.clone(), stream_name: self.stream_name.clone(), tx: self.tx.clone() }
    }
}