use anyhow::Result;
use prost_types::Timestamp;
use solana_sdk::{instruction::CompiledInstruction, pubkey::Pubkey, transaction::VersionedTransaction};
use solana_transaction_status::{
    EncodedTransactionWithStatusMeta, UiCompiledInstruction, UiInnerInstructions, UiInstruction,
};
use std::fmt::Debug;
use std::{collections::HashMap, str::FromStr};

use crate::utils::scripts::Scripts;
use crate::streaming::events::common::{parse_transfer_datas_from_next_instructions, SwapData, TransferData};
use crate::streaming::events::protocols::pumpswap::{PumpSwapBuyEvent, PumpSwapSellEvent};
use crate::streaming::events::{
    common::{EventMetadata, EventType, ProtocolType},
    protocols::{
        bonk::{BonkPoolCreateEvent, BonkTradeEvent},
        pumpfun::{PumpFunCreateTokenEvent, PumpFunTradeEvent},
    },
};

/// Unified Event Interface - All protocol events must implement this trait
pub trait UnifiedEvent: Debug + Send + Sync {
    fn id(&self) -> &str;
    fn event_type(&self) -> EventType;
    fn signature(&self) -> &str;
    fn slot(&self) -> u64;
    fn program_received_time_ms(&self) -> i64;
    fn program_handle_time_consuming_ms(&self) -> i64;
    fn set_program_handle_time_consuming_ms(&mut self, program_handle_time_consuming_ms: i64);
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn clone_boxed(&self) -> Box<dyn UnifiedEvent>;
    fn merge(&mut self, _other: Box<dyn UnifiedEvent>) {}
    fn set_transfer_datas(&mut self, transfer_datas: Vec<TransferData>, swap_data: Option<SwapData>);
    fn index(&self) -> String;
}

#[async_trait::async_trait]
pub trait EventParser: Send + Sync {
    fn inner_instruction_configs(&self) -> HashMap<&'static str, Vec<GenericEventParseConfig>>;
    fn instruction_configs(&self) -> HashMap<Vec<u8>, Vec<GenericEventParseConfig>>;

    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_inner_instruction(
        &self,
        inner_instruction: &UiCompiledInstruction,
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Vec<Box<dyn UnifiedEvent>>;

    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Vec<Box<dyn UnifiedEvent>>;

    /// 从 VersionedTransaction 解析指令事件（零拷贝账户视图，最小化分配）
    #[allow(clippy::too_many_arguments)]
    async fn parse_instruction_events_from_versioned_transaction(
        &self,
        transaction: &VersionedTransaction,
        signature: &str,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        accounts: &[Pubkey],
        inner_instructions: &[UiInnerInstructions],
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let mut instruction_events = Vec::with_capacity(16);
        let compiled_instructions = transaction.message.instructions();
        let mut accounts_buf: Vec<Pubkey> = accounts.to_vec();

        // 快速检查是否包含我们关心的 Program
        let has_program = accounts_buf.iter().any(|account| self.should_handle(account));
        if !has_program {
            return Ok(instruction_events);
        }

        for (index, instruction) in compiled_instructions.iter().enumerate() {
            if let Some(program_id) = accounts_buf.get(instruction.program_id_index as usize) {
                if !self.should_handle(program_id) {
                    continue;
                }

                // 防止账户索引越界：补齐 accounts（稀疏索引填充默认值）
                if let Some(max_idx) = instruction.accounts.iter().max() {
                    let need = *max_idx as usize + 1;
                    if need > accounts_buf.len() {
                        accounts_buf.resize(need, Pubkey::default());
                    }
                }

                if let Ok(mut events) = self
                    .parse_instruction(
                        instruction,
                        &accounts_buf,
                        signature,
                        slot,
                        block_time,
                        program_received_time_ms,
                        format!("{index}"),
                    )
                    .await
                {
                    if !events.is_empty() {
                        if let Some(inn) = inner_instructions.iter().find(|ii| ii.index == index as u8) {
                            events.iter_mut().for_each(|event| {
                                let (transfer_datas, swap_data) = parse_transfer_datas_from_next_instructions(
                                    event.clone_boxed(),
                                    inn,
                                    -1_i8,
                                    &accounts_buf,
                                );
                                event.set_transfer_datas(transfer_datas, swap_data);
                            });
                        }
                        instruction_events.extend(events);
                    }
                }
            }
        }
        Ok(instruction_events)
    }

    /// 解析交易（**按引用**，避免大对象克隆）
    async fn parse_transaction(
        &self,
        tx: &EncodedTransactionWithStatusMeta,
        signature: &str,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        bot_wallet: Option<Pubkey>,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let transaction = &tx.transaction;

        // meta 缺失直接返回错误
        let meta = tx
            .meta
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing transaction metadata"))?;

        // 预分配内存
        let mut address_table_lookups: Vec<Pubkey> = Vec::with_capacity(32);
        let mut inner_instructions: Vec<UiInnerInstructions> = Vec::with_capacity(8);

        if meta.err.is_none() {
            // OptionSerializer::Some
            if let solana_transaction_status::option_serializer::OptionSerializer::Some(meta_inner) =
                &meta.inner_instructions
            {
                inner_instructions = meta_inner.clone();
            }
            if let solana_transaction_status::option_serializer::OptionSerializer::Some(loaded) =
                &meta.loaded_addresses
            {
                address_table_lookups.reserve(loaded.writable.len() + loaded.readonly.len());
                for lookup in &loaded.writable {
                    if let Ok(pk) = Pubkey::from_str(lookup) {
                        address_table_lookups.push(pk);
                    }
                }
                for lookup in &loaded.readonly {
                    if let Ok(pk) = Pubkey::from_str(lookup) {
                        address_table_lookups.push(pk);
                    }
                }
            }
        }

        // 账户集合
        let mut accounts: Vec<Pubkey> = Vec::new();
        accounts.reserve(address_table_lookups.len() + 32);

        // 主事件集合（预估容量）
        let mut instruction_events = Vec::with_capacity(16);

        // 解析指令事件
        if let Some(versioned_tx) = transaction.decode() {
            accounts.extend_from_slice(versioned_tx.message.static_account_keys());
            accounts.extend_from_slice(&address_table_lookups);

            instruction_events = self
                .parse_instruction_events_from_versioned_transaction(
                    &versioned_tx,
                    signature,
                    slot,
                    block_time,
                    program_received_time_ms,
                    &accounts,
                    &inner_instructions,
                )
                .await
                .unwrap_or_else(|_| Vec::new());
        } else {
            accounts.extend_from_slice(&address_table_lookups);
        }

        // 解析内联指令事件
        let mut inner_instruction_events = Vec::with_capacity(8);
        if meta.err.is_none() {
            for inner_instruction in &inner_instructions {
                for (idx, instruction) in inner_instruction.instructions.iter().enumerate() {
                    if let UiInstruction::Compiled(compiled) = instruction {
                        // decode base58 -> bytes；失败时返回空数据避免 panic
                        let data_bytes = bs58::decode(&compiled.data).into_vec().unwrap_or_default();
                        if data_bytes.len() < 1 {
                            continue;
                        }
                        // 构建普通 CompiledInstruction 以复用解析逻辑
                        let compiled_instruction = CompiledInstruction {
                            program_id_index: compiled.program_id_index,
                            accounts: compiled.accounts.clone(),
                            data: data_bytes,
                        };

                        // 指令解析
                        if let Ok(mut events) = self
                            .parse_instruction(
                                &compiled_instruction,
                                &accounts,
                                signature,
                                slot,
                                block_time,
                                program_received_time_ms,
                                format!("{}.{}", inner_instruction.index, idx),
                            )
                            .await
                        {
                            if !events.is_empty() {
                                events.iter_mut().for_each(|event| {
                                    let (transfer_datas, swap_data) = parse_transfer_datas_from_next_instructions(
                                        event.clone_boxed(),
                                        inner_instruction,
                                        idx as i8,
                                        &accounts,
                                    );
                                    event.set_transfer_datas(transfer_datas, swap_data);
                                });
                                instruction_events.extend(events);
                            }
                        }

                        // 内联解析
                        if let Ok(mut events) = self
                            .parse_inner_instruction(
                                compiled,
                                signature,
                                slot,
                                block_time,
                                program_received_time_ms,
                                format!("{}.{}", inner_instruction.index, idx),
                            )
                            .await
                        {
                            if !events.is_empty() {
                                events.iter_mut().for_each(|event| {
                                    let (transfer_datas, swap_data) = parse_transfer_datas_from_next_instructions(
                                        event.clone_boxed(),
                                        inner_instruction,
                                        idx as i8,
                                        &accounts,
                                    );
                                    event.set_transfer_datas(transfer_datas, swap_data);
                                });
                                inner_instruction_events.extend(events);
                            }
                        }
                    }
                }
            }
        }

        // 合并同一 id 的内联/普通事件
        if !instruction_events.is_empty() && !inner_instruction_events.is_empty() {
            for instruction_event in &mut instruction_events {
                for inner_instruction_event in &inner_instruction_events {
                    if instruction_event.id() != inner_instruction_event.id() {
                        continue;
                    }

                    let i_index = instruction_event.index();
                    let in_index = inner_instruction_event.index();

                    if !i_index.contains('.') && in_index.contains('.') {
                        if let Some(first) = in_index.split('.').next() {
                            if first == i_index {
                                instruction_event.merge(inner_instruction_event.clone_boxed());
                                break;
                            }
                        }
                    } else if i_index.contains('.') && in_index.contains('.') {
                        let mut i_iter = i_index.split('.');
                        let mut in_iter = in_index.split('.');

                        if let (Some(a), Some(b)) = (i_iter.next(), in_iter.next()) {
                            if a == b {
                                let i_child = i_iter.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                                let in_child = in_iter.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                                if in_child > i_child {
                                    instruction_event.merge(inner_instruction_event.clone_boxed());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(self.process_events(instruction_events, bot_wallet))
    }

    fn process_events(&self, mut events: Vec<Box<dyn UnifiedEvent>>, bot_wallet: Option<Pubkey>) -> Vec<Box<dyn UnifiedEvent>> {
        let start_time = std::time::Instant::now();

        // 开发者地址追踪（PumpFun / Bonk）
        let mut dev_address: Vec<Pubkey> = Vec::with_capacity(4);
        let mut bonk_dev_address: Option<Pubkey> = None;

        for event in &mut events {
            if let Some(token_info) = event.as_any().downcast_ref::<PumpFunCreateTokenEvent>() {
                dev_address.push(token_info.user);
                if token_info.creator != Pubkey::default() && token_info.creator != token_info.user {
                    dev_address.push(token_info.creator);
                }
            } else if let Some(trade_info) = event.as_any_mut().downcast_mut::<PumpFunTradeEvent>() {
                if dev_address.contains(&trade_info.user) || dev_address.contains(&trade_info.creator) {
                    trade_info.is_dev_create_token_trade = true;
                } else if Some(trade_info.user) == bot_wallet {
                    trade_info.is_bot = true;
                } else {
                    trade_info.is_dev_create_token_trade = false;
                }

                if let Some(swap) = trade_info.metadata.swap_data.as_mut() {
                    if trade_info.is_buy {
                        swap.from_amount = trade_info.sol_amount;
                        swap.to_amount = trade_info.token_amount;
                    } else {
                        swap.from_amount = trade_info.token_amount;
                        swap.to_amount = trade_info.sol_amount;
                    }
                }
            } else if let Some(trade_info) = event.as_any_mut().downcast_mut::<PumpSwapBuyEvent>() {
                if let Some(swap) = trade_info.metadata.swap_data.as_mut() {
                    swap.from_amount = trade_info.user_quote_amount_in;
                    swap.to_amount = trade_info.base_amount_out;
                }
            } else if let Some(trade_info) = event.as_any_mut().downcast_mut::<PumpSwapSellEvent>() {
                if let Some(swap) = trade_info.metadata.swap_data.as_mut() {
                    swap.from_amount = trade_info.base_amount_in;
                    swap.to_amount = trade_info.user_quote_amount_out;
                }
            } else if let Some(pool_info) = event.as_any().downcast_ref::<BonkPoolCreateEvent>() {
                bonk_dev_address = Some(pool_info.creator);
            } else if let Some(trade_info) = event.as_any_mut().downcast_mut::<BonkTradeEvent>() {
                if Some(trade_info.payer) == bonk_dev_address {
                    trade_info.is_dev_create_token_trade = true;
                } else if Some(trade_info.payer) == bot_wallet {
                    trade_info.is_bot = true;
                } else {
                    trade_info.is_dev_create_token_trade = false;
                }
            }

            let now_ms = chrono::Utc::now().timestamp_millis();
            event.set_program_handle_time_consuming_ms(now_ms - event.program_received_time_ms());
        }

        // 轻量级慢日志
        let elapsed = start_time.elapsed();
        if elapsed.as_millis() > 10 {
            log::warn!(
                "Event processing took {}ms for {} events",
                elapsed.as_millis(),
                events.len()
            );
        }

        events
    }

    async fn parse_inner_instruction(
        &self,
        instruction: &UiCompiledInstruction,
        signature: &str,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let slot = slot.unwrap_or(0);
        Ok(self.parse_events_from_inner_instruction(
            instruction,
            signature,
            slot,
            block_time,
            program_received_time_ms,
            index,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn parse_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: Option<u64>,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Result<Vec<Box<dyn UnifiedEvent>>> {
        let slot = slot.unwrap_or(0);
        Ok(self.parse_events_from_instruction(
            instruction,
            accounts,
            signature,
            slot,
            block_time,
            program_received_time_ms,
            index,
        ))
    }

    fn should_handle(&self, program_id: &Pubkey) -> bool;
    fn supported_program_ids(&self) -> Vec<Pubkey>;
}

// 为 Box<dyn UnifiedEvent> 实现 Clone
impl Clone for Box<dyn UnifiedEvent> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

/// 通用事件解析器配置
#[derive(Debug, Clone)]
pub struct GenericEventParseConfig {
    pub program_id: Pubkey,
    pub protocol_type: ProtocolType,
    pub inner_instruction_discriminator: &'static str,
    pub instruction_discriminator: &'static [u8],
    pub event_type: EventType,
    pub inner_instruction_parser: Option<InnerInstructionEventParser>,
    pub instruction_parser: Option<InstructionEventParser>,
}

pub type InnerInstructionEventParser =
fn(data: &[u8], metadata: EventMetadata) -> Option<Box<dyn UnifiedEvent>>;
pub type InstructionEventParser =
fn(data: &[u8], accounts: &[Pubkey], metadata: EventMetadata) -> Option<Box<dyn UnifiedEvent>>;

/// 通用事件解析器基类
pub struct GenericEventParser {
    pub program_ids: Vec<Pubkey>,
    pub inner_instruction_configs: HashMap<&'static str, Vec<GenericEventParseConfig>>,
    pub instruction_configs: HashMap<Vec<u8>, Vec<GenericEventParseConfig>>,
}

impl GenericEventParser {
    pub fn new(program_ids: Vec<Pubkey>, configs: Vec<GenericEventParseConfig>) -> Self {
        let mut inner_instruction_configs = HashMap::with_capacity(configs.len());
        let mut instruction_configs = HashMap::with_capacity(configs.len());

        for config in configs {
            inner_instruction_configs
                .entry(config.inner_instruction_discriminator)
                .or_insert_with(Vec::new)
                .push(config.clone());
            instruction_configs
                .entry(config.instruction_discriminator.to_vec())
                .or_insert_with(Vec::new)
                .push(config.clone());
        }

        Self { program_ids, inner_instruction_configs, instruction_configs }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_inner_instruction_event(
        &self,
        config: &GenericEventParseConfig,
        data: &[u8],
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Option<Box<dyn UnifiedEvent>> {
        if let Some(parser) = config.inner_instruction_parser {
            let timestamp = block_time.unwrap_or(Timestamp { seconds: 0, nanos: 0 });
            let block_time_ms = timestamp.seconds * 1000 + (timestamp.nanos as i64) / 1_000_000;
            let metadata = EventMetadata::new(
                signature.to_string(),
                signature.to_string(),
                slot,
                timestamp.seconds,
                block_time_ms,
                config.protocol_type.clone(),
                config.event_type.clone(),
                config.program_id,
                index,
                program_received_time_ms,
            );
            parser(data, metadata)
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_instruction_event(
        &self,
        config: &GenericEventParseConfig,
        data: &[u8],
        account_pubkeys: &[Pubkey],
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Option<Box<dyn UnifiedEvent>> {
        if let Some(parser) = config.instruction_parser {
            let timestamp = block_time.unwrap_or(Timestamp { seconds: 0, nanos: 0 });
            let block_time_ms = timestamp.seconds * 1000 + (timestamp.nanos as i64) / 1_000_000;
            let metadata = EventMetadata::new(
                signature.to_string(),
                signature.to_string(),
                slot,
                timestamp.seconds,
                block_time_ms,
                config.protocol_type.clone(),
                config.event_type.clone(),
                config.program_id,
                index,
                program_received_time_ms,
            );
            parser(data, account_pubkeys, metadata)
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl EventParser for GenericEventParser {
    fn inner_instruction_configs(&self) -> HashMap<&'static str, Vec<GenericEventParseConfig>> {
        self.inner_instruction_configs.clone()
    }
    fn instruction_configs(&self) -> HashMap<Vec<u8>, Vec<GenericEventParseConfig>> {
        self.instruction_configs.clone()
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_inner_instruction(
        &self,
        inner_instruction: &UiCompiledInstruction,
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Vec<Box<dyn UnifiedEvent>> {
        // base58 -> bytes；不足 16 字节直接返回
        let decoded = bs58::decode(&inner_instruction.data).into_vec().unwrap_or_default();
        if decoded.len() < 16 {
            return Vec::new();
        }
        let disc_hex = format!("0x{}", hex::encode(&decoded));
        let data = &decoded[16..];

        let mut events = Vec::new();
        for (disc, configs) in &self.inner_instruction_configs {
            if Scripts::discmatches(&disc_hex, disc) {
                for config in configs {
                    if let Some(event) = self.parse_inner_instruction_event(
                        config,
                        data,
                        signature,
                        slot,
                        block_time,
                        program_received_time_ms,
                        index.clone(),
                    ) {
                        events.push(event);
                    }
                }
            }
        }
        events
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_events_from_instruction(
        &self,
        instruction: &CompiledInstruction,
        accounts: &[Pubkey],
        signature: &str,
        slot: u64,
        block_time: Option<Timestamp>,
        program_received_time_ms: i64,
        index: String,
    ) -> Vec<Box<dyn UnifiedEvent>> {
        // program id 安全读取
        let pid_idx = instruction.program_id_index as usize;
        if pid_idx >= accounts.len() {
            return Vec::new();
        }
        let program_id = accounts[pid_idx];
        if !self.should_handle(&program_id) {
            return Vec::new();
        }

        let mut events = Vec::new();
        for (disc, configs) in &self.instruction_configs {
            if instruction.data.len() < disc.len() {
                continue;
            }
            let (disc_bytes, data) = instruction.data.split_at(disc.len());
            if disc_bytes != &disc[..] {
                continue;
            }

            if !Scripts::accountindices(&instruction.accounts, accounts.len()) {
                continue;
            }

            let account_pubkeys: Vec<Pubkey> = instruction
                .accounts
                .iter()
                .filter_map(|&idx| accounts.get(idx as usize).copied())
                .collect();

            for config in configs {
                if config.program_id != program_id {
                    continue;
                }
                if let Some(event) = self.parse_instruction_event(
                    config,
                    data,
                    &account_pubkeys,
                    signature,
                    slot,
                    block_time,
                    program_received_time_ms,
                    index.clone(),
                ) {
                    events.push(event);
                }
            }
        }
        events
    }

    fn should_handle(&self, program_id: &Pubkey) -> bool {
        self.program_ids.contains(program_id)
    }

    fn supported_program_ids(&self) -> Vec<Pubkey> {
        self.program_ids.clone()
    }
}