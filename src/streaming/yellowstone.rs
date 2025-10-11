use log::error;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Semaphore};
use yellowstone_grpc_proto::geyser::CommitmentLevel;

use futures::StreamExt; // for .next()

use crate::common::AnyResult;
use crate::streaming::common::{
    EventBatchProcessor, MetricsManager, PerformanceMetrics, StreamClientConfig, SubscriptionHandle,
};
use crate::globals::constants::*;
use crate::streaming::events::common::filter::EventTypeFilter;
use crate::streaming::events::{Protocol, UnifiedEvent};
use crate::streaming::grpc::{
    processor::EventProcessor,
    streamhandler::StreamHandler,
    subscription::SubscriptionManager,
    types::EventPretty,
};

pub struct TransactionFilter {
    pub account_include: Vec<String>,
    pub account_exclude: Vec<String>,
    pub account_required: Vec<String>,
}

pub struct AccountFilter {
    pub account: Vec<String>,
    pub owner: Vec<String>,
}

pub struct YellowstoneGrpc {
    pub endpoint: String,
    pub x_token: Option<String>,
    pub config: StreamClientConfig,
    pub metrics: Arc<Mutex<PerformanceMetrics>>,
    pub subscription_manager: SubscriptionManager,
    pub metrics_manager: MetricsManager,
    /// Event processor shared across tasks
    pub event_processor: Arc<EventProcessor>,
    pub subscription_handle: Arc<Mutex<Option<SubscriptionHandle>>>,
}

impl YellowstoneGrpc {
    pub fn new(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        Self::new_with_config(endpoint, x_token, StreamClientConfig::default())
    }

    pub fn new_with_config(
        endpoint: String,
        x_token: Option<String>,
        config: StreamClientConfig,
    ) -> AnyResult<Self> {
        let _ = rustls::crypto::ring::default_provider().install_default().ok();
        let metrics = Arc::new(Mutex::new(PerformanceMetrics::new()));
        let config_arc = Arc::new(config.clone());

        let subscription_manager =
            SubscriptionManager::new(endpoint.clone(), x_token.clone(), config.clone());
        let metrics_manager =
            MetricsManager::new(metrics.clone(), config_arc.clone(), "YellowstoneGrpc".to_string());
        let event_processor = Arc::new(EventProcessor::new(metrics_manager.clone(), config.clone()));

        Ok(Self {
            endpoint,
            x_token,
            config,
            metrics,
            subscription_manager,
            metrics_manager,
            event_processor,
            subscription_handle: Arc::new(Mutex::new(None)),
        })
    }

    pub fn new_high_performance(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        Self::new_with_config(endpoint, x_token, StreamClientConfig::high_performance())
    }

    pub fn new_low_latency(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        Self::new_with_config(endpoint, x_token, StreamClientConfig::low_latency())
    }

    pub fn new_immediate(endpoint: String, x_token: Option<String>) -> AnyResult<Self> {
        let mut config = StreamClientConfig::low_latency();
        config.enable_metrics = false;
        Self::new_with_config(endpoint, x_token, config)
    }

    pub fn get_config(&self) -> &StreamClientConfig {
        &self.config
    }

    pub fn update_config(&mut self, config: StreamClientConfig) {
        self.config = config;
    }

    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.metrics_manager.get_metrics().await
    }

    pub async fn print_metrics(&self) {
        self.metrics_manager.print_metrics().await;
    }

    pub fn set_enable_metrics(&mut self, enabled: bool) {
        self.config.enable_metrics = enabled;
    }

    /// 停止当前订阅
    pub async fn stop(&self) {
        let mut handle_guard = self.subscription_handle.lock().await;
        if let Some(handle) = handle_guard.take() {
            handle.stop();
        }
    }

    /// 简化：即时事件订阅（无批处理），带并发处理
    pub async fn subscribe_events_immediate<F>(
        &self,
        protocols: Vec<Protocol>,
        bot_wallet: Option<Pubkey>,
        transaction_filter: TransactionFilter,
        account_filter: AccountFilter,
        event_type_filter: Option<EventTypeFilter>,
        commitment: Option<CommitmentLevel>,
        callback: F,
    ) -> AnyResult<()>
    where
        F: Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static,
    {
        self.stop().await;

        let mut metrics_handle = None;
        if self.config.enable_metrics {
            metrics_handle = self.metrics_manager.start_auto_monitoring().await;
        }

        let transactions = self.subscription_manager.get_subscribe_request_filter(
            transaction_filter.account_include,
            transaction_filter.account_exclude,
            transaction_filter.account_required,
            event_type_filter.clone(),
        );
        let accounts = self.subscription_manager.subscribe_with_account_request(
            account_filter.account,
            account_filter.owner,
            event_type_filter.clone(),
        );

        let (mut subscribe_tx, mut stream) = self
            .subscription_manager
            .subscribe_with_request(transactions, accounts, commitment, event_type_filter.clone())
            .await?;

        // Tokio mpsc 通道
        let (tx, mut rx) = mpsc::channel::<EventPretty>(self.config.backpressure.channel_size);

        // Yellowstone reader → channel (backpressure inside StreamHandler)
        let bp = self.config.backpressure.strategy;
        let stream_handle = tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) =
                            StreamHandler::handle_stream_message(msg, &tx, &mut subscribe_tx, bp)
                                .await
                        {
                            error!("Error handling message: {e:?}");
                            break;
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        // 并发事件处理（工作池）
        let event_processor = Arc::clone(&self.event_processor);
        let cb_arc: Arc<F> = Arc::new(callback); // keep concrete F
        let protocols_arc = Arc::new(protocols);
        let evt_filter_arc = event_type_filter.clone();
        let bot_wallet_arc = bot_wallet;

        // 并发度
        let mut concurrency = self
            .config
            .processor_concurrency
            .unwrap_or_else(num_cpus::get);
        if concurrency == 0 {
            concurrency = 1;
        }
        concurrency = std::cmp::min(concurrency, PROCMAXCONCURRENCYCAP);
        let semaphore = Arc::new(Semaphore::new(concurrency));

        let event_handle = tokio::spawn(async move {
            while let Some(event_pretty) = rx.recv().await {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Semaphore closed: {e}");
                        break;
                    }
                };

                let ep = Arc::clone(&event_processor);
                let cb = Arc::clone(&cb_arc); // Arc<F>
                let protocols = Arc::clone(&protocols_arc);
                let evt_filter = evt_filter_arc.clone();
                let bot_wallet = bot_wallet_arc;

                tokio::spawn(async move {
                    let _p = permit;
                    if let Err(e) = ep
                        .process_event_transaction_with_metrics(
                            event_pretty,
                            cb.as_ref(),            // &F
                            bot_wallet,
                            (*protocols).clone(),
                            evt_filter,
                        )
                        .await
                    {
                        error!("Error processing transaction: {e:?}");
                    }
                });
            }
        });

        let subscription_handle =
            SubscriptionHandle::new(stream_handle, event_handle, metrics_handle);
        let mut handle_guard = self.subscription_handle.lock().await;
        *handle_guard = Some(subscription_handle);

        Ok(())
    }

    /// 高级订阅：批处理 + 背压（保持批次顺序，因此单消费者）
    pub async fn subscribe_events_advanced<F>(
        &self,
        protocols: Vec<Protocol>,
        bot_wallet: Option<Pubkey>,
        transaction_filter: TransactionFilter,
        account_filter: AccountFilter,
        event_type_filter: Option<EventTypeFilter>,
        commitment: Option<CommitmentLevel>,
        callback: F,
    ) -> AnyResult<()>
    where
        F: Fn(Box<dyn UnifiedEvent>) + Send + Sync + 'static,
    {
        self.stop().await;

        let mut metrics_handle = None;
        if self.config.enable_metrics {
            metrics_handle = self.metrics_manager.start_auto_monitoring().await;
        }

        let transactions = self.subscription_manager.get_subscribe_request_filter(
            transaction_filter.account_include,
            transaction_filter.account_exclude,
            transaction_filter.account_required,
            event_type_filter.clone(),
        );
        let accounts = self.subscription_manager.subscribe_with_account_request(
            account_filter.account,
            account_filter.owner,
            event_type_filter.clone(),
        );

        let (mut subscribe_tx, mut stream) = self
            .subscription_manager
            .subscribe_with_request(transactions, accounts, commitment, event_type_filter.clone())
            .await?;

        let (tx, mut rx) = mpsc::channel::<EventPretty>(self.config.backpressure.channel_size);

        // keep the concrete callback type F
        let outer_callback: Arc<F> = Arc::new(callback);
        let batch_callback = {
            let outer = Arc::clone(&outer_callback); // Arc<F>
            move |events: Vec<Box<dyn UnifiedEvent>>| {
                let f = outer.as_ref(); // &F
                for ev in events {
                    f(ev);
                }
            }
        };

        let mut batch_processor = EventBatchProcessor::new(
            batch_callback,
            self.config.batch.batch_size,
            self.config.batch.batch_timeout_ms,
        );

        // Yellowstone reader → channel
        let bp = self.config.backpressure.strategy;
        let stream_handle = tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) =
                            StreamHandler::handle_stream_message(msg, &tx, &mut subscribe_tx, bp)
                                .await
                        {
                            error!("Error handling message: {e:?}");
                            break;
                        }
                    }
                    Err(error) => {
                        error!("Stream error: {error:?}");
                        break;
                    }
                }
            }
        });

        // 单消费者批处理
        let event_processor = Arc::clone(&self.event_processor);
        let event_handle = tokio::spawn(async move {
            while let Some(event_pretty) = rx.recv().await {
                if let Err(e) = event_processor
                    .process_event_transaction_with_batch(
                        event_pretty,
                        &mut batch_processor,
                        bot_wallet,
                        protocols.clone(),
                        event_type_filter.clone(),
                    )
                    .await
                {
                    error!("Error processing transaction: {e:?}");
                }
            }
            // flush 剩余事件
            batch_processor.flush();
        });

        let subscription_handle =
            SubscriptionHandle::new(stream_handle, event_handle, metrics_handle);
        let mut handle_guard = self.subscription_handle.lock().await;
        *handle_guard = Some(subscription_handle);

        Ok(())
    }
}

// 实现 Clone 以支持共享
impl Clone for YellowstoneGrpc {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint.clone(),
            x_token: self.x_token.clone(),
            config: self.config.clone(),
            metrics: self.metrics.clone(),
            subscription_manager: self.subscription_manager.clone(),
            metrics_manager: self.metrics_manager.clone(),
            event_processor: Arc::clone(&self.event_processor),
            subscription_handle: self.subscription_handle.clone(),
        }
    }
}