// ─── imports packages ───
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use dashmap::DashMap;
use once_cell::sync::{Lazy, OnceCell};
use solana_program::pubkey::Pubkey;
use tokio::sync::{broadcast, Semaphore};

// ─── imports crates ───
use crate::globals::pubkeys::{bonk_pubkeys, pumpfun_pubkeys, pumpswap_pubkeys, raydiumamm_pubkeys, raydiumclmm_pubkeys, raydiumcpmm_pubkeys};
use crate::streaming::events::core::eventparser::AccountEventParseConfig;
use crate::streaming::events::Protocol;
use crate::trading::monitor::CloseCmd;
use crate::utils::storage::TokenRow;

// ─── const 'OPENFLIGHT' ───
/// const description
pub static OPENFLIGHT: OnceCell<Arc<Semaphore>> = OnceCell::new();

// ─── const 'MONITORBUS' ───
/// const description
pub static MONITORBUS: OnceCell<broadcast::Sender<CloseCmd>> = OnceCell::new();

// ─── const 'POSTGRESLASTENRICH' ───
/// const description
pub static POSTGRESLASTENRICH: Lazy<DashMap<(String, String), (i64, i64)>> = Lazy::new(|| DashMap::new());

// ─── const 'POSTGRESOPENTRADE' ───
/// const description
pub static POSTGRESOPENTRADE: Lazy<DashMap<String, u32>> = Lazy::new(|| DashMap::new());

// ─── const 'POSTGRESLASTTICK' ───
/// const description
pub static POSTGRESLASTTICK: Lazy<DashMap<String, (i64, f64)>> = Lazy::new(|| DashMap::new());

// ─── const 'POSTGRESSEMAPHORE' ───
/// const description
pub static POSTGRESSEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));

// ─── const 'POSTGRESTOKENSCACHE' ───
/// const description
pub static POSTGRESTOKENSCACHE: Lazy<DashMap<(String, String), TokenRow>> = Lazy::new(|| DashMap::new());

// ─── const 'PROTOCOLCACHECONFIG' ───
/// const description
pub static PROTOCOLCACHECONFIG: OnceLock<HashMap<Protocol, Vec<AccountEventParseConfig>>> = OnceLock::new();

// ─── const 'PROGRAMIDSTRINGS' ───
/// const description
pub static PROGRAMIDSTRINGS: OnceLock<HashMap<Pubkey, &'static str>> = OnceLock::new();

// ─── const 'PROGRAMLABELS' ───
/// const description
pub static PROGRAMLABELS: Lazy<HashMap<Pubkey, &'static str>> = Lazy::new(||{
    HashMap::from([
        (bonk_pubkeys::PROGRAM, "Bonk"),
        (pumpfun_pubkeys::PROGRAM, "PumpFun"),
        (pumpswap_pubkeys::PROGRAM, "PumpSwap"),
        (raydiumamm_pubkeys::PROGRAM, "RaydiumAMM"),
        (raydiumclmm_pubkeys::PROGRAM, "RaydiumCLMM"),
        (raydiumcpmm_pubkeys::PROGRAM, "RaydiumCPMM"),
    ])
});