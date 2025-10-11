// ─── import packages ───
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status::UiInstruction;
use std::{hash::{DefaultHasher, Hash, Hasher}, sync::Arc};
use tokio::sync::Mutex;

// ─── import crates ───
use crate::globals::constants::*;
use crate::globals::pubkeys::*;
use crate::{
    eventsmatch,
    streaming::events::{
        protocols::{
            bonk::BonkTradeEvent,
            pumpfun::PumpFunTradeEvent,
            pumpswap::{PumpSwapBuyEvent, PumpSwapSellEvent},
            raydiumamm::RaydiumAmmV4SwapEvent,
            raydiumclmm::{RaydiumClmmSwapEvent, RaydiumClmmSwapV2Event},
            raydiumcpmm::RaydiumCpmmSwapEvent,
        },
        UnifiedEvent
    },
};

// ─── struct 'EventMetadataPool' ───
/// struct description
pub struct EventMetadataPool {
    pool: Arc<Mutex<Vec<EventMetadata>>>,
}

// ─── impl 'Default for EventMetadataPool' ───
/// impl description
impl Default for EventMetadataPool {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {
        Self::new()
    }
}

// ─── impl 'EventMetadataPool' ───
/// impl description
impl EventMetadataPool {

    // ─── fn 'new' ───
    /// fn description
    pub fn new() -> Self {
        Self { pool: Arc::new(Mutex::new(Vec::with_capacity(METADATAEVENTPOOLSIZE))) }
    }

    // ─── fn 'acquire' ───
    /// fn description
    pub async fn acquire(&self) -> Option<EventMetadata> {

        // ─── define 'pool' ───
        let mut pool = self.pool.lock().await;
        pool.pop()
    }

    // ─── fn 'release' ───
    /// fn description
    pub async fn release(&self, metadata: EventMetadata) {

        // ─── define 'pool' ───
        let mut pool = self.pool.lock().await;

        // ─── compare 'pool.len()' ───
        if pool.len() < METADATAEVENTPOOLSIZE {
            pool.push(metadata);
        }
    }
}

// ─── struct 'TransferDataPool' ───
/// struct description
pub struct TransferDataPool {
    pool: Arc<Mutex<Vec<TransferData>>>
}

// ─── impl 'Default for TransferDataPool' ───
/// impl description
impl Default for TransferDataPool {

    // ─── fn 'default' ───
    /// fn description
    fn default() -> Self {
        Self::new()
    }
}

// ─── impl 'TransferDataPool' ───
/// impl description
impl TransferDataPool {

    // ─── fn 'new' ───
    /// fn description
    pub fn new() -> Self {
        Self { pool: Arc::new(Mutex::new(Vec::with_capacity(METADATATRANSFERPOOLSIZE))) }
    }

    // ─── fn 'acquire' ───
    /// fn description
    pub async fn acquire(&self) -> Option<TransferData> {

        // ─── define 'pool' ───
        let mut pool = self.pool.lock().await;
        pool.pop()
    }

    // ─── fn 'release' ───
    /// fn description
    pub async fn release(&self, transfer_data: TransferData) {

        // ─── define 'pool' ───
        let mut pool = self.pool.lock().await;

        // ─── compare 'pool.len()' ───
        if pool.len() < METADATATRANSFERPOOLSIZE {
            pool.push(transfer_data);
        }
    }
}

// ─── declare 'lazy_static' ───
lazy_static::lazy_static! {
    pub static ref EVENT_METADATA_POOL: EventMetadataPool = EventMetadataPool::new();
    pub static ref TRANSFER_DATA_POOL: TransferDataPool = TransferDataPool::new();
}

// ─── enum 'ProtocolType' ───
/// enum description
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum ProtocolType {
    #[default]
    PumpSwap,
    PumpFun,
    Bonk,
    RaydiumCpmm,
    RaydiumClmm,
    RaydiumAmmV4,
    Common
}

// ─── enum 'EventType' ───
/// enum description
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum EventType {

    #[default]
    // ─── 'Bonk Events' ───
    BonkBuyExactIn,
    BonkBuyExactOut,
    BonkSellExactIn,
    BonkSellExactOut,
    BonkInitialize,
    BonkInitializeV2,
    BonkMigrateToAmm,
    BonkMigrateToCpswap,

    // ─── 'PumpFun Events' ───
    PumpFunCreateToken,
    PumpFunBuy,
    PumpFunSell,
    PumpFunMigrate,

    // ─── 'PumpSwap Events' ───
    PumpSwapBuy,
    PumpSwapSell,
    PumpSwapCreatePool,
    PumpSwapDeposit,
    PumpSwapWithdraw,

    // ─── 'RaydiumAMM Events' ───
    RaydiumAmmV4SwapBaseIn,
    RaydiumAmmV4SwapBaseOut,
    RaydiumAmmV4Deposit,
    RaydiumAmmV4Initialize2,
    RaydiumAmmV4Withdraw,
    RaydiumAmmV4WithdrawPnl,

    // ─── 'RaydiumCLMM Events' ───
    RaydiumClmmSwap,
    RaydiumClmmSwapV2,
    RaydiumClmmClosePosition,
    RaydiumClmmIncreaseLiquidityV2,
    RaydiumClmmDecreaseLiquidityV2,
    RaydiumClmmCreatePool,
    RaydiumClmmOpenPositionWithToken22Nft,
    RaydiumClmmOpenPositionV2,

    // ─── 'RaydiumCPMM Events' ───
    RaydiumCpmmSwapBaseInput,
    RaydiumCpmmSwapBaseOutput,
    RaydiumCpmmDeposit,
    RaydiumCpmmInitialize,
    RaydiumCpmmWithdraw,

    // ─── 'Account Events' ───
    AccountRaydiumAmmV4AmmInfo,
    AccountPumpSwapGlobalConfig,
    AccountPumpSwapPool,
    AccountBonkPoolState,
    AccountBonkGlobalConfig,
    AccountBonkPlatformConfig,
    AccountBonkVestingRecord,
    AccountPumpFunBondingCurve,
    AccountPumpFunGlobal,
    AccountRaydiumClmmAmmConfig,
    AccountRaydiumClmmPoolState,
    AccountRaydiumClmmTickArrayState,
    AccountRaydiumCpmmAmmConfig,
    AccountRaydiumCpmmPoolState,

    // ─── 'Block Events' ───
    BlockMeta,
    Unknown
}

// ─── const 'ACCOUNT_EVENT_TYPES' ───
/// const description
pub const ACCOUNT_EVENT_TYPES: &[EventType] = &[
    EventType::AccountRaydiumAmmV4AmmInfo,
    EventType::AccountPumpSwapGlobalConfig,
    EventType::AccountPumpSwapPool,
    EventType::AccountBonkPoolState,
    EventType::AccountBonkGlobalConfig,
    EventType::AccountBonkPlatformConfig,
    EventType::AccountBonkVestingRecord,
    EventType::AccountPumpFunBondingCurve,
    EventType::AccountPumpFunGlobal,
    EventType::AccountRaydiumClmmAmmConfig,
    EventType::AccountRaydiumClmmPoolState,
    EventType::AccountRaydiumClmmTickArrayState,
    EventType::AccountRaydiumCpmmAmmConfig,
    EventType::AccountRaydiumCpmmPoolState
];

// ─── const 'BLOCK_EVENT_TYPES' ───
/// const description
pub const BLOCK_EVENT_TYPES: &[EventType] = &[EventType::BlockMeta];

// ─── impl 'EventType' ───
/// impl description
impl EventType {
    #[allow(clippy::inherent_to_string)]

    // ─── fn 'to_string' ───
    /// fn description
    pub fn to_string(&self) -> String {
        match self {
            EventType::PumpSwapBuy => "PumpSwapBuy".to_string(),
            EventType::PumpSwapSell => "PumpSwapSell".to_string(),
            EventType::PumpSwapCreatePool => "PumpSwapCreatePool".to_string(),
            EventType::PumpSwapDeposit => "PumpSwapDeposit".to_string(),
            EventType::PumpSwapWithdraw => "PumpSwapWithdraw".to_string(),
            EventType::PumpFunCreateToken => "PumpFunCreateToken".to_string(),
            EventType::PumpFunBuy => "PumpFunBuy".to_string(),
            EventType::PumpFunSell => "PumpFunSell".to_string(),
            EventType::PumpFunMigrate => "PumpFunMigrate".to_string(),
            EventType::BonkBuyExactIn => "BonkBuyExactIn".to_string(),
            EventType::BonkBuyExactOut => "BonkBuyExactOut".to_string(),
            EventType::BonkSellExactIn => "BonkSellExactIn".to_string(),
            EventType::BonkSellExactOut => "BonkSellExactOut".to_string(),
            EventType::BonkInitialize => "BonkInitialize".to_string(),
            EventType::BonkInitializeV2 => "BonkInitializeV2".to_string(),
            EventType::BonkMigrateToAmm => "BonkMigrateToAmm".to_string(),
            EventType::BonkMigrateToCpswap => "BonkMigrateToCpswap".to_string(),
            EventType::AccountPumpFunBondingCurve => "AccountPumpFunBondingCurve".to_string(),
            EventType::AccountPumpFunGlobal => "AccountPumpFunGlobal".to_string(),
            EventType::AccountPumpSwapGlobalConfig => "AccountPumpSwapGlobalConfig".to_string(),
            EventType::AccountPumpSwapPool => "AccountPumpSwapPool".to_string(),
            EventType::AccountBonkPoolState => "AccountBonkPoolState".to_string(),
            EventType::AccountBonkGlobalConfig => "AccountBonkGlobalConfig".to_string(),
            EventType::AccountBonkPlatformConfig => "AccountBonkPlatformConfig".to_string(),
            EventType::AccountBonkVestingRecord => "AccountBonkVestingRecord".to_string(),
            EventType::RaydiumCpmmSwapBaseInput => "RaydiumCpmmSwapBaseInput".to_string(),
            EventType::RaydiumCpmmSwapBaseOutput => "RaydiumCpmmSwapBaseOutput".to_string(),
            EventType::RaydiumCpmmDeposit => "RaydiumCpmmDeposit".to_string(),
            EventType::RaydiumCpmmInitialize => "RaydiumCpmmInitialize".to_string(),
            EventType::RaydiumCpmmWithdraw => "RaydiumCpmmWithdraw".to_string(),
            EventType::RaydiumClmmSwap => "RaydiumClmmSwap".to_string(),
            EventType::RaydiumClmmSwapV2 => "RaydiumClmmSwapV2".to_string(),
            EventType::RaydiumClmmClosePosition => "RaydiumClmmClosePosition".to_string(),
            EventType::RaydiumClmmDecreaseLiquidityV2 => {"RaydiumClmmDecreaseLiquidityV2".to_string()}
            EventType::RaydiumClmmCreatePool => "RaydiumClmmCreatePool".to_string(),
            EventType::RaydiumClmmIncreaseLiquidityV2 => {"RaydiumClmmIncreaseLiquidityV2".to_string()}
            EventType::RaydiumClmmOpenPositionWithToken22Nft => {"RaydiumClmmOpenPositionWithToken22Nft".to_string()}
            EventType::RaydiumClmmOpenPositionV2 => "RaydiumClmmOpenPositionV2".to_string(),
            EventType::RaydiumAmmV4SwapBaseIn => "RaydiumAmmV4SwapBaseIn".to_string(),
            EventType::RaydiumAmmV4SwapBaseOut => "RaydiumAmmV4SwapBaseOut".to_string(),
            EventType::RaydiumAmmV4Deposit => "RaydiumAmmV4Deposit".to_string(),
            EventType::RaydiumAmmV4Initialize2 => "RaydiumAmmV4Initialize2".to_string(),
            EventType::RaydiumAmmV4Withdraw => "RaydiumAmmV4Withdraw".to_string(),
            EventType::RaydiumAmmV4WithdrawPnl => "RaydiumAmmV4WithdrawPnl".to_string(),
            EventType::AccountRaydiumAmmV4AmmInfo => "AccountRaydiumAmmV4AmmInfo".to_string(),
            EventType::AccountRaydiumClmmAmmConfig => "AccountRaydiumClmmAmmConfig".to_string(),
            EventType::AccountRaydiumClmmPoolState => "AccountRaydiumClmmPoolState".to_string(),
            EventType::AccountRaydiumClmmTickArrayState => {"AccountRaydiumClmmTickArrayState".to_string()}
            EventType::AccountRaydiumCpmmAmmConfig => "AccountRaydiumCpmmAmmConfig".to_string(),
            EventType::AccountRaydiumCpmmPoolState => "AccountRaydiumCpmmPoolState".to_string(),
            EventType::BlockMeta => "BlockMeta".to_string(),
            EventType::Unknown => "Unknown".to_string()
        }
    }
}

// ─── struct 'ParseResult' ───
/// struct description
#[derive(Debug, Clone)]
pub struct ParseResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>
}

// ─── impl 'ParseResult' ───
/// impl description
impl<T> ParseResult<T> {

    // ─── fn 'success' ───
    /// fn description
    pub fn success(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }

    // ─── fn 'failure' ───
    /// fn description
    pub fn failure(error: String) -> Self {
        Self { success: false, data: None, error: Some(error) }
    }

    // ─── fn 'is_success' ───
    /// fn description
    pub fn is_success(&self) -> bool {
        self.success
    }

    // ─── fn 'is_failure' ───
    /// fn description
    pub fn is_failure(&self) -> bool {
        !self.success
    }
}

// ─── struct 'ProtocolInfo' ───
/// struct description
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInfo {
    pub name: String,
    pub program_ids: Vec<Pubkey>
}

// ─── impl 'ProtocolInfo' ───
/// impl description
impl ProtocolInfo {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(name: String, program_ids: Vec<Pubkey>) -> Self {
        Self { name, program_ids }
    }

    // ─── fn 'supports_program' ───
    /// fn description
    pub fn supports_program(&self, program_id: &Pubkey) -> bool {
        self.program_ids.contains(program_id)
    }
}

// ─── struct 'TransferData' ───
/// struct description
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct TransferData {
    pub token_program: Pubkey,
    pub source: Pubkey,
    pub destination: Pubkey,
    pub authority: Option<Pubkey>,
    pub amount: u64,
    pub decimals: Option<u8>,
    pub mint: Option<Pubkey>
}

// ─── struct 'SwapData' ───
/// struct description
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct SwapData {
    pub from_mint: Pubkey,
    pub to_mint: Pubkey,
    pub from_amount: u64,
    pub to_amount: u64,
    pub description: Option<String>
}

// ─── struct 'EventMetadata' ───
/// struct description
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct EventMetadata {
    pub id: String,
    pub signature: String,
    pub slot: u64,
    pub block_time: i64,
    pub block_time_ms: i64,
    pub program_received_time_ms: i64,
    pub program_handle_time_consuming_ms: i64,
    pub protocol: ProtocolType,
    pub event_type: EventType,
    pub program_id: Pubkey,
    pub transfer_datas: Vec<TransferData>,
    pub swap_data: Option<SwapData>,
    pub index: String
}

// ─── impl 'EventMetadata' ───
/// impl description
impl EventMetadata {
    #[allow(clippy::too_many_arguments)]

    // ─── fn 'new' ───
    /// fn description
    pub fn new(id: String, signature: String, slot: u64, block_time: i64, block_time_ms: i64, protocol: ProtocolType,
               event_type: EventType, program_id: Pubkey, index: String, program_received_time_ms: i64) -> Self {

        // ─── return 'Self' ───
        Self {
            id,
            signature,
            slot,
            block_time,
            block_time_ms,
            program_received_time_ms,
            program_handle_time_consuming_ms: 0,
            protocol,
            event_type,
            program_id,
            transfer_datas: Vec::with_capacity(4),
            swap_data: None,
            index,
        }
    }

    // ─── fn 'set_id' ───
    /// fn description
    pub fn set_id(&mut self, id: String) {

        // ─── define 'id' ───
        let id = format!("{}-{}-{}", self.signature, self.event_type.to_string(), id);

        // ─── define 'hasher' ───
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);

        // ─── define 'hashval' ───
        let hashval = hasher.finish();
        self.id = format!("{:x}", hashval);
    }

    // ─── fn 'set_transfer_datas' ───
    /// fn description
    pub fn set_transfer_datas(&mut self, transfer_datas: Vec<TransferData>, swap_data: Option<SwapData>) {
        self.transfer_datas = transfer_datas;
        self.swap_data = swap_data;
    }

    // ─── fn 'recycle' ───
    /// fn description
    pub async fn recycle(self) {
        EVENT_METADATA_POOL.release(self).await;
    }
}

// ─── fn 'parse_transfer_datas_from_next_instructions' ───
/// fn description
pub fn parse_transfer_datas_from_next_instructions(event: Box<dyn UnifiedEvent>, 
    inner_instruction: &solana_transaction_status::UiInnerInstructions,
    current_index: i8, accounts: &[Pubkey]) -> (Vec<TransferData>, Option<SwapData>) {

    // ─── define 'transfer_datas' ───
    let mut transfer_datas = vec![];

    // ─── define 'next_instructions' ───
    let next_instructions: Vec<&UiInstruction> = inner_instruction.instructions.iter().skip((current_index + 1) as usize).collect();

    // ─── define 'system_programs' ───
    let system_programs = vec![
        system_pubkeys::TOKEN,
        system_pubkeys::TOKEN2022,
        system_pubkeys::SYSTEM
    ];

    // ─── proceed 'for' ───
    for instruction in next_instructions {

        // ─── compare 'instruction' ───
        if let UiInstruction::Compiled(compiled) = instruction {

            // ─── compare 'system_programs.contains()' ───
            if !system_programs.contains(&accounts[compiled.program_id_index as usize]) {
                break;
            }

            // ─── compare 'data' ───
            if let Ok(data) = bs58::decode(compiled.data.clone()).into_vec() {

                // ─── compare 'data' ───
                if data[0] == 12 {

                    // ─── define 'account_pubkeys' ───
                    let account_pubkeys: Vec<Pubkey> = compiled.accounts.iter().map(|a| accounts[*a as usize]).collect();

                    // ─── compare 'account_pubkeys' ───
                    if account_pubkeys.len() < 4 {
                        continue;
                    }

                    // ─── define '(source, mint, destination, authority)' ───
                    let (source, mint, destination, authority) = (
                        account_pubkeys[0],
                        account_pubkeys[1],
                        account_pubkeys[2],
                        account_pubkeys[3],
                    );

                    // ─── define 'amount' ───
                    let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());

                    // ─── define 'decimals' ───
                    let decimals = data[9];

                    // ─── define 'token_program' ───
                    let token_program = accounts[compiled.program_id_index as usize];
                    transfer_datas.push(TransferData {
                        amount,
                        decimals: Some(decimals),
                        mint: Some(mint),
                        source,
                        destination,
                        authority: Some(authority),
                        token_program,
                    });
                }
                else if data[0] == 3 {

                    // ─── define 'account_pubkeys' ───
                    let account_pubkeys: Vec<Pubkey> = compiled.accounts.iter().map(|a| accounts[*a as usize]).collect();

                    // ─── compare 'account_pubkeys' ───
                    if account_pubkeys.len() < 3 {
                        continue;
                    }

                    // ─── define '(source, destination, authority)' ───
                    let (source, destination, authority) = (account_pubkeys[0], account_pubkeys[1], account_pubkeys[2]);

                    // ─── define 'amount' ───
                    let amount = u64::from_le_bytes(data[1..9].try_into().unwrap());

                    // ─── define 'token_program' ───
                    let token_program = accounts[compiled.program_id_index as usize];
                    transfer_datas.push(TransferData {
                        amount,
                        decimals: None,
                        mint: None,
                        source,
                        destination,
                        authority: Some(authority),
                        token_program,
                    });
                }
                else if data[0] == 2 {

                    // ─── define 'account_pubkeys' ───
                    let account_pubkeys: Vec<Pubkey> = compiled.accounts.iter().map(|a| accounts[*a as usize]).collect();

                    // ─── compare 'account_pubkeys' ───
                    if account_pubkeys.len() < 2 {
                        continue;
                    }

                    // ─── define '(source, destination)' ───
                    let (source, destination) = (account_pubkeys[0], account_pubkeys[1]);

                    // ─── define 'amount' ───
                    let amount = u64::from_le_bytes(data[4..12].try_into().unwrap());

                    // ─── define 'token_program' ───
                    let token_program = accounts[compiled.program_id_index as usize];
                    transfer_datas.push(TransferData {
                        amount,
                        decimals: None,
                        mint: None,
                        source,
                        destination,
                        authority: None,
                        token_program,
                    });
                }
            }
        }
    }

    // ─── define 'swap_data' ───
    let mut swap_data: SwapData = SwapData {
        from_mint: Pubkey::default(),
        to_mint: Pubkey::default(),
        from_amount: 0,
        to_amount: 0,
        description: None,
    };

    // ─── define 'sol_mint' ───
    let sol_mint = system_pubkeys::WRAPPER;

    // ─── compare 'transfer_datas' ───
    if transfer_datas.len() > 0 {

        // ─── define 'user' ───
        let mut user: Option<Pubkey> = None;

        // ─── define 'from_mint' ───
        let mut from_mint: Option<Pubkey> = None;

        // ─── define 'to_mint' ───
        let mut to_mint: Option<Pubkey> = None;

        // ─── define 'user_from_token' ───
        let mut user_from_token: Option<Pubkey> = None;

        // ─── define 'user_to_token' ───
        let mut user_to_token: Option<Pubkey> = None;

        // ─── define 'from_vault' ───
        let mut from_vault: Option<Pubkey> = None;

        // ─── define 'to_vault' ───
        let mut to_vault: Option<Pubkey> = None;

        // ─── eventsmatch 'event' ───
        eventsmatch!(event, {
            BonkTradeEvent => |e: BonkTradeEvent| {
                user = Some(e.payer);
                from_mint = Some(e.base_token_mint);
                to_mint = Some(e.quote_token_mint);
                user_from_token = Some(e.user_base_token);
                user_to_token = Some(e.user_quote_token);
                from_vault = Some(e.base_vault);
                to_vault = Some(e.quote_vault);
            },
            PumpFunTradeEvent => |e: PumpFunTradeEvent| {
                swap_data.from_mint = if e.is_buy {
                    sol_mint
                } else {
                    e.mint
                };
                swap_data.to_mint = if e.is_buy {
                    e.mint
                } else {
                    sol_mint
                };
            },
            PumpSwapBuyEvent => |e: PumpSwapBuyEvent| {
                swap_data.from_mint = e.quote_mint;
                swap_data.to_mint = e.base_mint;
            },
            PumpSwapSellEvent => |e: PumpSwapSellEvent| {
                swap_data.from_mint = e.base_mint;
                swap_data.to_mint = e.quote_mint;
            },
            RaydiumCpmmSwapEvent => |e: RaydiumCpmmSwapEvent| {
                user = Some(e.payer);
                from_mint = Some(e.input_token_mint);
                to_mint = Some(e.output_token_mint);
                user_from_token = Some(e.input_token_account);
                user_to_token = Some(e.output_token_account);
                from_vault = Some(e.input_vault);
                to_vault = Some(e.output_vault);
            },
            RaydiumClmmSwapEvent => |e: RaydiumClmmSwapEvent| {
                user = Some(e.payer);
                swap_data.description = Some("Unable to get from_mint and to_mint from RaydiumClmmSwapEvent".to_string());
                user_from_token = Some(e.input_token_account);
                user_to_token = Some(e.output_token_account);
                from_vault = Some(e.input_vault);
                to_vault = Some(e.output_vault);
            },
            RaydiumClmmSwapV2Event => |e: RaydiumClmmSwapV2Event| {
                user = Some(e.payer);
                from_mint = Some(e.input_vault_mint);
                to_mint = Some(e.output_vault_mint);
                user_from_token = Some(e.input_token_account);
                user_to_token = Some(e.output_token_account);
                from_vault = Some(e.input_vault);
                to_vault = Some(e.output_vault);
            },
            RaydiumAmmV4SwapEvent => |e: RaydiumAmmV4SwapEvent| {
                user = Some(e.user_source_owner);
                swap_data.description = Some("Unable to get from_mint and to_mint from RaydiumAmmV4SwapEvent".to_string());
                user_from_token = Some(e.user_source_token_account);
                user_to_token = Some(e.user_destination_token_account);
                from_vault = Some(e.pool_pc_token_account);
                to_vault = Some(e.pool_coin_token_account);
            },
        });

        // ─── proceed 'for' ───
        for transfer_data in transfer_datas.clone() {

            // ─── compare 'user_to_token' ───
            if transfer_data.source == user_to_token.unwrap_or_default()
                && transfer_data.destination == to_vault.unwrap_or_default() {
                swap_data.from_mint = to_mint.unwrap_or_default();
                swap_data.from_amount = transfer_data.amount;
            } else if transfer_data.source == from_vault.unwrap_or_default()
                && transfer_data.destination == user_from_token.unwrap_or_default() {
                swap_data.to_mint = from_mint.unwrap_or_default();
                swap_data.to_amount = transfer_data.amount;
            } else if transfer_data.source == user_from_token.unwrap_or_default()
                && transfer_data.destination == from_vault.unwrap_or_default() {
                swap_data.from_mint = from_mint.unwrap_or_default();
                swap_data.from_amount = transfer_data.amount;
            } else if transfer_data.source == to_vault.unwrap_or_default()
                && transfer_data.destination == user_to_token.unwrap_or_default() {
                swap_data.to_mint = to_mint.unwrap_or_default();
                swap_data.to_amount = transfer_data.amount;
            }
        }
    }

    // ─── compare 'swap_data' ───
    if swap_data.from_mint != Pubkey::default()
        || swap_data.to_mint != Pubkey::default()
        || swap_data.from_amount != 0
        || swap_data.to_amount != 0 {
        (transfer_datas, Some(swap_data))
    } else {
        (transfer_datas, None)
    }
}