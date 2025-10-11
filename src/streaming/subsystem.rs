use crate::{
    common::AnyResult,
    streaming::{
        common::BackpressureStrategy,
        grpc::{types::EventPretty, streamhandler::StreamHandler},
        yellowstone::YellowstoneGrpc,
    },
};
use futures::StreamExt;
use log::error;
use solana_program::pubkey;
use solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction};
use solana_transaction_status::EncodedTransactionWithStatusMeta;
use tokio::sync::mpsc;

const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
/// 根据实际并发量调整通道大小，避免背压
const CHANNEL_SIZE: usize = 50_000;

#[derive(Debug)]
pub enum SystemEvent {
    NewTransfer(TransferInfo),
    Error(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TransferInfo {
    pub slot: u64,
    pub signature: String,
    pub tx: Option<VersionedTransaction>,
}

impl YellowstoneGrpc {
    pub async fn subscribe_system<F>(
        &self,
        callback: F,
        account_include: Option<Vec<String>>,
        account_exclude: Option<Vec<String>>,
    ) -> AnyResult<()>
    where
        F: Fn(SystemEvent) + Send + Sync + 'static,
    {
        let addrs = vec![SYSTEM_PROGRAM_ID.to_string()];
        let account_include = account_include.unwrap_or_default();
        let account_exclude = account_exclude.unwrap_or_default();

        // 仅订阅与 System Program 相关且交易成功的事务
        let transactions = self.subscription_manager.get_subscribe_request_filter(
            account_include,
            account_exclude,
            addrs,
            None,
        );

        let (mut subscribe_tx, mut stream) = self
            .subscription_manager
            .subscribe_with_request(transactions, None, None, None)
            .await?;

        // Tokio mpsc：更适合 Tokio 运行时；并支持 try_send / send
        let (tx, mut rx) = mpsc::channel::<EventPretty>(CHANNEL_SIZE);

        // 回调装箱，便于 move 进闭包
        let callback = Box::new(callback);

        // 读取 Yellowstone 流并将事件按背压策略写入通道
        let tx_for_task = tx;
        tokio::spawn(async move {
            // 使用 Retry 背压策略，避免在高峰期阻塞读取
            let bp = BackpressureStrategy::Retry {
                max_attempts: 3,
                wait_ms: 1,
            };

            while let Some(message) = stream.next().await {
                match message {
                    Ok(msg) => {
                        if let Err(e) =
                            StreamHandler::handle_stream_message(msg, &tx_for_task, &mut subscribe_tx, bp).await
                        {
                            error!("Error handling message: {e:?}");
                            break;
                        }
                    }
                    Err(err) => {
                        error!("Stream error: {err:?}");
                        break;
                    }
                }
            }
        });

        // 消费事件并执行业务处理
        while let Some(event_pretty) = rx.recv().await {
            if let Err(e) = Self::process_system_transaction(event_pretty, &*callback).await {
                error!("Error processing transaction: {e:?}");
            }
        }

        Ok(())
    }

    async fn process_system_transaction<F>(event_pretty: EventPretty, callback: &F) -> AnyResult<()>
    where
        F: Fn(SystemEvent) + Send + Sync,
    {
        match event_pretty {
            EventPretty::Transaction(transaction_pretty) => {
                let trade_raw: EncodedTransactionWithStatusMeta = transaction_pretty.tx;
                let meta = trade_raw
                    .meta
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Missing transaction metadata"))?;

                // 只处理成功交易
                if meta.err.is_some() {
                    return Ok(());
                }

                callback(SystemEvent::NewTransfer(TransferInfo {
                    slot: transaction_pretty.slot,
                    signature: transaction_pretty.signature.to_string(),
                    tx: trade_raw.transaction.decode(),
                }));
            }
            _ => { /* ignore non-transaction events for this subsystem */ }
        }
        Ok(())
    }
}