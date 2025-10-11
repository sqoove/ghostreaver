use futures::{channel::mpsc, sink::Sink, Stream};
use maplit::hashmap;
use std::{collections::HashMap, pin::Pin, time::Duration};

use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use yellowstone_grpc_proto::{
    self as proto,
    geyser::{
        CommitmentLevel, SubscribeRequest, SubscribeRequestFilterAccounts,
        SubscribeRequestFilterBlocksMeta, SubscribeRequestFilterTransactions, SubscribeUpdate,
    },
};

// 👇 Fix the tonic::Status version mismatch
use proto::tonic::Status;

use super::types::{AccountsFilterMap, TransactionsFilterMap};
use crate::common::AnyResult;
use crate::streaming::common::StreamClientConfig as ClientConfig;
use crate::streaming::events::common::filter::EventTypeFilter;

/// 订阅管理器
#[derive(Clone)]
pub struct SubscriptionManager {
    endpoint: String,
    x_token: Option<String>,
    config: ClientConfig,
}

impl SubscriptionManager {
    /// 创建新的订阅管理器
    pub fn new(endpoint: String, x_token: Option<String>, config: ClientConfig) -> Self {
        Self { endpoint, x_token, config }
    }

    /// 创建 gRPC 连接
    pub async fn connect(&self) -> AnyResult<GeyserGrpcClient<impl Interceptor + 'static>> {
        let builder = GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
            .x_token(self.x_token.clone())?
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            .max_decoding_message_size(self.config.connection.max_decoding_message_size)
            .connect_timeout(Duration::from_secs(self.config.connection.connect_timeout))
            .timeout(Duration::from_secs(self.config.connection.request_timeout));

        Ok(builder.connect().await?)
    }

    /// 创建订阅请求并返回流
    pub async fn subscribe_with_request(
        &self,
        transactions: Option<TransactionsFilterMap>,
        accounts: Option<AccountsFilterMap>,
        commitment: Option<CommitmentLevel>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> AnyResult<(
        Pin<Box<dyn Sink<SubscribeRequest, Error = mpsc::SendError> + Send + 'static>>,
        Pin<Box<dyn Stream<Item = Result<SubscribeUpdate, Status>> + Send + 'static>>,
    )> {
        let blocks_meta = if event_type_filter.as_ref().is_some_and(|f| f.include_block_event()) {
            hashmap! { "".to_owned() => SubscribeRequestFilterBlocksMeta {} }
        } else if event_type_filter.is_none() {
            hashmap! { "".to_owned() => SubscribeRequestFilterBlocksMeta {} }
        } else {
            hashmap! {}
        };

        let subscribe_request = SubscribeRequest {
            accounts: accounts.unwrap_or_default(),
            transactions: transactions.unwrap_or_default(),
            blocks_meta,
            commitment: commitment
                .map(|c| c as i32)
                .or_else(|| Some(CommitmentLevel::Processed.into())),
            ..Default::default()
        };

        let mut client = self.connect().await?;
        let (sink, stream) = client.subscribe_with_request(Some(subscribe_request)).await?;

        Ok((
            Box::pin(sink)
                as Pin<Box<dyn Sink<SubscribeRequest, Error = mpsc::SendError> + Send + 'static>>,
            Box::pin(stream)
                as Pin<Box<dyn Stream<Item = Result<SubscribeUpdate, Status>> + Send + 'static>>,
        ))
    }

    /// 创建账户订阅请求并返回账户过滤器
    pub fn subscribe_with_account_request(
        &self,
        account: Vec<String>,
        owner: Vec<String>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> Option<AccountsFilterMap> {
        if account.is_empty() && owner.is_empty() {
            return None;
        }
        if event_type_filter.as_ref().is_some_and(|f| !f.include_account_event()) {
            return None;
        }

        let mut accounts = HashMap::new();
        accounts.insert(
            "".to_owned(),
            SubscribeRequestFilterAccounts {
                account,
                owner,
                filters: vec![],
                nonempty_txn_signature: None,
            },
        );
        Some(accounts)
    }

    /// 生成订阅请求过滤器
    pub fn get_subscribe_request_filter(
        &self,
        account_include: Vec<String>,
        account_exclude: Vec<String>,
        account_required: Vec<String>,
        event_type_filter: Option<EventTypeFilter>,
    ) -> Option<TransactionsFilterMap> {
        if event_type_filter.as_ref().is_some_and(|f| !f.include_transaction_event()) {
            return None;
        }

        let mut transactions = HashMap::new();
        transactions.insert(
            "client".to_string(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include,
                account_exclude,
                account_required,
            },
        );
        Some(transactions)
    }

    /// 获取配置
    pub fn get_config(&self) -> &ClientConfig {
        &self.config
    }
}