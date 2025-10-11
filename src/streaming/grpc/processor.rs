use std::sync::Arc;

use once_cell::sync::OnceCell;
use solana_sdk::pubkey::Pubkey;

use super::types::EventPretty;
use crate::common::AnyResult;
use crate::streaming::common::{
    EventBatchProcessor as EventBatchCollector, MetricsEventType, MetricsManager,
    StreamClientConfig as ClientConfig,
};
use crate::streaming::events::common::filter::EventTypeFilter;
use crate::streaming::events::core::eventparser::{AccountEventParser, CommonEventParser};
use crate::streaming::events::EventParser;
use crate::streaming::events::{
    core::traits::UnifiedEvent, protocols::mutil::parser::MutilEventParser, Protocol,
};

/// 事件处理器
pub struct EventProcessor {
    pub(crate) metrics_manager: MetricsManager,
    pub(crate) config: ClientConfig,
    /// 解析器缓存（首次使用时按传入 protocols/event_type_filter 构建一次）
    parser_cache: OnceCell<Arc<dyn EventParser>>,
}

impl EventProcessor {
    /// 创建新的事件处理器
    pub fn new(metrics_manager: MetricsManager, config: ClientConfig) -> Self {
        Self {
            metrics_manager,
            config,
            parser_cache: OnceCell::new(),
        }
    }

    /// 获取或创建解析器（首次调用时根据传入的 protocols / filter 构建并缓存）
    #[inline]
    fn get_or_create_parser(
        &self,
        protocols: Vec<Protocol>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> Arc<dyn EventParser> {
        self.parser_cache
            .get_or_init(|| Arc::new(MutilEventParser::new(protocols, event_type_filter)))
            .clone()
    }

    /// 使用性能监控处理事件交易（逐条回调）
    pub async fn process_event_transaction_with_metrics<F>(
        &self,
        event_pretty: EventPretty,
        callback: &F,
        bot_wallet: Option<Pubkey>,
        protocols: Vec<Protocol>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> AnyResult<()>
    where
        F: Fn(Box<dyn UnifiedEvent>) + Send + Sync,
    {
        match event_pretty {
            EventPretty::Account(account_pretty) => {
                self.metrics_manager.add_account_process_count().await;

                let start_time = std::time::Instant::now();
                let program_received_time_ms = chrono::Utc::now().timestamp_millis();

                if let Some(event) = AccountEventParser::parse_account_event(
                    protocols,
                    account_pretty,
                    program_received_time_ms,
                    event_type_filter,
                ) {
                    callback(event);

                    let processing_time_ms = start_time.elapsed().as_millis() as f64;
                    self.metrics_manager
                        .update_metrics(MetricsEventType::Account, 1, processing_time_ms)
                        .await;
                    self.metrics_manager
                        .log_slow_processing(processing_time_ms, 1);
                }
            }
            EventPretty::Transaction(transaction_pretty) => {
                self.metrics_manager.add_tx_process_count().await;

                let start_time = std::time::Instant::now();
                let program_received_time_ms = chrono::Utc::now().timestamp_millis();
                let slot = transaction_pretty.slot;
                let signature = transaction_pretty.signature.to_string();

                // 获取解析器（缓存）
                let parser = self.get_or_create_parser(protocols, event_type_filter);

                // 按引用传递，避免克隆大型交易对象
                let all_events = parser
                    .parse_transaction(
                        &transaction_pretty.tx,
                        &signature,
                        Some(slot),
                        transaction_pretty.block_time.map(|ts| prost_types::Timestamp {
                            seconds: ts.seconds,
                            nanos: ts.nanos,
                        }),
                        program_received_time_ms,
                        bot_wallet,
                    )
                    .await
                    .unwrap_or_else(|_| Vec::new());

                let event_count = all_events.len();

                // 逐条回调（或在上层改为批处理）
                if event_count > 0 {
                    for event in all_events {
                        callback(event);
                    }
                }

                let processing_time_ms = start_time.elapsed().as_millis() as f64;
                self.metrics_manager
                    .update_metrics(MetricsEventType::Tx, event_count as u64, processing_time_ms)
                    .await;
                self.metrics_manager
                    .log_slow_processing(processing_time_ms, event_count);
            }
            EventPretty::BlockMeta(block_meta_pretty) => {
                let start_time = std::time::Instant::now();
                self.metrics_manager.add_block_meta_process_count().await;

                let block_time_ms = block_meta_pretty
                    .block_time
                    .map(|ts| ts.seconds * 1000 + ts.nanos as i64 / 1_000_000)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

                let block_meta_event = CommonEventParser::generate_block_meta_event(
                    block_meta_pretty.slot,
                    &block_meta_pretty.block_hash,
                    block_time_ms,
                );
                callback(block_meta_event);

                let processing_time_ms = start_time.elapsed().as_millis() as f64;
                self.metrics_manager
                    .update_metrics(MetricsEventType::BlockMeta, 1, processing_time_ms)
                    .await;
                self.metrics_manager
                    .log_slow_processing(processing_time_ms, 1);
            }
        }

        Ok(())
    }

    /// 使用批处理处理事件交易（批量回调）
    pub async fn process_event_transaction_with_batch<F>(
        &self,
        event_pretty: EventPretty,
        batch_processor: &mut EventBatchCollector<F>,
        bot_wallet: Option<Pubkey>,
        protocols: Vec<Protocol>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> AnyResult<()>
    where
        F: Fn(Vec<Box<dyn UnifiedEvent>>) + Send + Sync + 'static,
    {
        match event_pretty {
            EventPretty::Account(account_pretty) => {
                self.metrics_manager.add_account_process_count().await;

                let start_time = std::time::Instant::now();
                let program_received_time_ms = chrono::Utc::now().timestamp_millis();

                if let Some(event) = AccountEventParser::parse_account_event(
                    protocols,
                    account_pretty,
                    program_received_time_ms,
                    event_type_filter,
                ) {
                    (batch_processor.callback)(vec![event]);

                    let processing_time_ms = start_time.elapsed().as_millis() as f64;
                    self.metrics_manager
                        .update_metrics(MetricsEventType::Account, 1, processing_time_ms)
                        .await;
                    self.metrics_manager
                        .log_slow_processing(processing_time_ms, 1);
                }
            }
            EventPretty::Transaction(transaction_pretty) => {
                self.metrics_manager.add_tx_process_count().await;

                let start_time = std::time::Instant::now();
                let program_received_time_ms = chrono::Utc::now().timestamp_millis();
                let slot = transaction_pretty.slot;
                let signature = transaction_pretty.signature.to_string();

                // 获取解析器（缓存）
                let parser = self.get_or_create_parser(protocols, event_type_filter);

                // 解析（按引用传递）
                let result = parser
                    .parse_transaction(
                        &transaction_pretty.tx,
                        &signature,
                        Some(slot),
                        transaction_pretty.block_time.map(|ts| prost_types::Timestamp {
                            seconds: ts.seconds,
                            nanos: ts.nanos,
                        }),
                        program_received_time_ms,
                        bot_wallet,
                    )
                    .await;

                // 处理解析结果并进入批处理
                let total_events = match result {
                    Ok(events) => {
                        let count = events.len();
                        if count > 0 {
                            log::debug!("Parsed {} events; enqueueing to batch", count);
                            if self.config.batch.enabled {
                                for event in events {
                                    batch_processor.add_event(event);
                                }
                            } else {
                                // 批处理禁用：直接逐条触发回调（用 Vec 包裹以复用接口）
                                for event in events {
                                    (batch_processor.callback)(vec![event]);
                                }
                            }
                        }
                        count
                    }
                    Err(e) => {
                        log::warn!("Failed to parse transaction: {:?}", e);
                        0
                    }
                };

                let processing_time_ms = start_time.elapsed().as_millis() as f64;
                self.metrics_manager
                    .update_metrics(MetricsEventType::Tx, total_events as u64, processing_time_ms)
                    .await;
                self.metrics_manager
                    .log_slow_processing(processing_time_ms, total_events);
            }
            EventPretty::BlockMeta(block_meta_pretty) => {
                let start_time = std::time::Instant::now();
                self.metrics_manager.add_block_meta_process_count().await;

                let block_time_ms = block_meta_pretty
                    .block_time
                    .map(|ts| ts.seconds * 1000 + ts.nanos as i64 / 1_000_000)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

                let block_meta_event = CommonEventParser::generate_block_meta_event(
                    block_meta_pretty.slot,
                    &block_meta_pretty.block_hash,
                    block_time_ms,
                );
                (batch_processor.callback)(vec![block_meta_event]);

                let processing_time_ms = start_time.elapsed().as_millis() as f64;
                self.metrics_manager
                    .update_metrics(MetricsEventType::BlockMeta, 1, processing_time_ms)
                    .await;
                self.metrics_manager
                    .log_slow_processing(processing_time_ms, 1);
            }
        }

        Ok(())
    }
}