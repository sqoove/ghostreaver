// ─── import crates ───
use crate::globals::constants::*;

// ─── enum 'BackpressureStrategy' ───
/// enum description
#[derive(Debug, Clone, Copy)]
pub enum BackpressureStrategy {
    Block,
    Drop,
    Retry { max_attempts: usize, wait_ms: u64 },
}

// ─── impl 'Default for BackpressureStrategy' ───
/// impl description
impl Default for BackpressureStrategy {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {

        // ─── return 'Self' ───
        Self::Retry {
            max_attempts: DEFRETRYATTEMPTS,
            wait_ms: DEFRETRYWAITMS
        }
    }
}

// ─── struct 'BatchConfig' ───
/// struct description
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub enabled: bool
}

// ─── impl 'Default for BatchConfig' ───
/// impl description
impl Default for BatchConfig {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {

        // ─── return 'Self' ───
        Self {
            batch_size: DEFBATCHSIZE,
            batch_timeout_ms: DEFBATCHTIMEOUT,
            enabled: true
        }
    }
}

// ─── struct 'BackpressureConfig' ───
/// struct description
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    pub channel_size: usize,
    pub strategy: BackpressureStrategy,
}

// ─── impl 'Default for BackpressureConfig' ───
/// impl description
impl Default for BackpressureConfig {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {

        // ─── return 'Self' ───
        Self {
            channel_size: DEFCHANNELSIZE,
            strategy: BackpressureStrategy::default(),
        }
    }
}

// ─── struct 'ConnectionConfig' ───
/// struct description
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub connect_timeout: u64,
    pub request_timeout: u64,
    pub max_decoding_message_size: usize
}

// ─── impl 'Default for ConnectionConfig' ───
/// impl description
impl Default for ConnectionConfig {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {

        // ─── return 'Self' ───
        Self {
            connect_timeout: DEFTIMEOUTCONNECT,
            request_timeout: DEFTIMEOUTREQUEST,
            max_decoding_message_size: DEFMAXDECODINGSIZE
        }
    }
}

// ─── struct 'StreamClientConfig' ───
/// struct description
#[derive(Debug, Clone)]
pub struct StreamClientConfig {
    pub connection: ConnectionConfig,
    pub batch: BatchConfig,
    pub backpressure: BackpressureConfig,
    pub enable_metrics: bool,
    pub processor_concurrency: Option<usize>
}

// ─── impl 'Default for StreamClientConfig' ───
/// impl description
impl Default for StreamClientConfig {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {

        // ─── return 'Self' ───
        Self {
            connection: ConnectionConfig::default(),
            batch: BatchConfig::default(),
            backpressure: BackpressureConfig::default(),
            enable_metrics: false,
            processor_concurrency: None
        }
    }
}

// ─── impl 'StreamClientConfig' ───
/// impl description
impl StreamClientConfig {

    // ─── fn 'high_performance' ───
    /// fn description
    pub fn high_performance() -> Self {

        // ─── return 'Self' ───
        Self {
            connection: ConnectionConfig::default(),
            batch: BatchConfig {
                batch_size: 200,
                batch_timeout_ms: 5,
                enabled: true,
            },
            backpressure: BackpressureConfig {
                channel_size: DEFHPCHANNELSIZE,
                strategy: BackpressureStrategy::Retry {
                    max_attempts: DEFRETRYATTEMPTS,
                    wait_ms: DEFRETRYWAITMS,
                },
            },
            enable_metrics: false,
            processor_concurrency: None
        }
    }

    // ─── fn 'low_latency' ───
    /// fn description
    pub fn low_latency() -> Self {

        // ─── return 'Self' ───
        Self {
            connection: ConnectionConfig::default(),
            batch: BatchConfig {
                batch_size: 10,
                batch_timeout_ms: 1,
                enabled: false,
            },
            backpressure: BackpressureConfig {
                channel_size: DEFLLCHANNELSIZE,
                strategy: BackpressureStrategy::Retry {
                    max_attempts: DEFRETRYATTEMPTS,
                    wait_ms: DEFRETRYWAITMS,
                },
            },
            enable_metrics: false,
            processor_concurrency: None
        }
    }

    // ─── fn 'low_latency' ───
    /// fn description
    pub fn lossless_blocking() -> Self {

        // ─── return 'Self' ───
        Self {
            connection: ConnectionConfig::default(),
            batch: BatchConfig::default(),
            backpressure: BackpressureConfig {
                channel_size: DEFLBCHANNELSIZE,
                strategy: BackpressureStrategy::Block,
            },
            enable_metrics: false,
            processor_concurrency: None
        }
    }
}