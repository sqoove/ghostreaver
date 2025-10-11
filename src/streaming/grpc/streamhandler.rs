use chrono::Local;
use futures::{sink::Sink, SinkExt};
use futures::channel::mpsc as futures_mpsc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use yellowstone_grpc_proto::geyser::{
    subscribe_update::UpdateOneof, SubscribeRequest, SubscribeRequestPing, SubscribeUpdate,
};

use super::types::{BlockMetaPretty, EventPretty, TransactionPretty};
use crate::common::AnyResult;
use crate::streaming::common::BackpressureStrategy;
use crate::streaming::grpc::AccountPretty;

/// 流消息处理器
pub struct StreamHandler;

impl StreamHandler {
    /// 处理单个流消息
    pub async fn handle_stream_message(
        msg: SubscribeUpdate,
        tx: &mpsc::Sender<EventPretty>,
        subscribe_tx: &mut (impl Sink<SubscribeRequest, Error = futures_mpsc::SendError> + Unpin),
        backpressure_strategy: BackpressureStrategy,
    ) -> AnyResult<()> {
        let created_at = msg.created_at;
        match msg.update_oneof {
            Some(UpdateOneof::Account(account)) => {
                let account_pretty = AccountPretty::from(account);
                log::debug!("Received account: {:?}", account_pretty);
                Self::handle_backpressure(
                    tx,
                    EventPretty::Account(account_pretty),
                    backpressure_strategy,
                )
                    .await?;
            }
            Some(UpdateOneof::BlockMeta(sut)) => {
                let block_meta_pretty = BlockMetaPretty::from((sut, created_at));
                log::debug!("Received block meta: {:?}", block_meta_pretty);
                Self::handle_backpressure(
                    tx,
                    EventPretty::BlockMeta(block_meta_pretty),
                    backpressure_strategy,
                )
                    .await?;
            }
            Some(UpdateOneof::Transaction(sut)) => {
                let transaction_pretty = TransactionPretty::from((sut, created_at));
                log::debug!(
                    "Received transaction: {} at slot {}",
                    transaction_pretty.signature,
                    transaction_pretty.slot
                );
                Self::handle_backpressure(
                    tx,
                    EventPretty::Transaction(transaction_pretty),
                    backpressure_strategy,
                )
                    .await?;
            }
            Some(UpdateOneof::Ping(_)) => {
                subscribe_tx
                    .send(SubscribeRequest {
                        ping: Some(SubscribeRequestPing { id: 1 }),
                        ..Default::default()
                    })
                    .await?;
                log::debug!("service is ping: {}", Local::now());
            }
            Some(UpdateOneof::Pong(_)) => {
                log::debug!("service is pong: {}", Local::now());
            }
            _ => {
                log::debug!("Received other message type");
            }
        }
        Ok(())
    }

    /// 处理背压策略（tokio::mpsc 版本；无克隆重试）
    async fn handle_backpressure(
        tx: &mpsc::Sender<EventPretty>,
        event_pretty: EventPretty,
        backpressure_strategy: BackpressureStrategy,
    ) -> AnyResult<()> {
        match backpressure_strategy {
            BackpressureStrategy::Block => {
                // 阻塞直到有容量
                if let Err(e) = tx.send(event_pretty).await {
                    log::error!("Channel send failed: {}", e);
                    return Err(anyhow::anyhow!("Channel send failed: {}", e));
                }
            }
            BackpressureStrategy::Drop => {
                if let Err(e) = tx.try_send(event_pretty) {
                    match e {
                        TrySendError::Full(_) => {
                            log::warn!("Channel is full, dropping event");
                        }
                        TrySendError::Closed(_) => {
                            log::error!("Channel is closed");
                            return Err(anyhow::anyhow!("Channel is closed"));
                        }
                    }
                }
            }
            BackpressureStrategy::Retry { max_attempts, wait_ms } => {
                let mut attempts = 0usize;
                // 持有同一个值重试，不进行 clone
                let mut ev = Some(event_pretty);
                loop {
                    match tx.try_send(ev.take().expect("event consumed")) {
                        Ok(()) => break,
                        Err(e) => match e {
                            TrySendError::Full(v) => {
                                attempts += 1;
                                if attempts >= max_attempts {
                                    log::warn!(
                                        "Channel full after {} attempts, dropping event",
                                        attempts
                                    );
                                    break;
                                }
                                // 取回被消费失败的值以便下次重试
                                ev = Some(v);
                                tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms))
                                    .await;
                            }
                            TrySendError::Closed(_) => {
                                log::error!("Channel is closed");
                                return Err(anyhow::anyhow!("Channel is closed"));
                            }
                        },
                    }
                }
            }
        }
        Ok(())
    }
}
