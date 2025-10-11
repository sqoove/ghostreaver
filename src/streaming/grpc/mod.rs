// gRPC 相关模块
pub mod connection;
pub mod types;
pub mod subscription;
pub mod streamhandler;
pub mod processor;

// 重新导出主要类型
pub use connection::*;
pub use types::*;
pub use subscription::*;
pub use streamhandler::*;
pub use processor::*;

// 从公用模块重新导出
pub use crate::streaming::common::{
    StreamClientConfig as ClientConfig,
    PerformanceMetrics,
    MetricsManager,
    EventBatchProcessor as EventBatchCollector,
    BackpressureStrategy,
    BatchConfig,
    BackpressureConfig,
    ConnectionConfig,
};