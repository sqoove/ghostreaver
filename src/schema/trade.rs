// ─── import packages ───
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// ─── struct 'TradeInfo' ───
/// struct description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeInfo {
    pub uuid: String,
    pub mint: Pubkey,
    pub amount: f64,
    pub token: f64,
    pub hash: String,
    pub program: Pubkey,
    pub slot: i64,
    pub servtime: i64
}

// ─── impl 'TradeInfo' ───
/// impl description
impl TradeInfo {

    // ─── fn 'init' ───
    /// fn description
    pub fn init(uuid: String, mint: Pubkey, amount: f64, token: f64, hash: String, program: Pubkey, slot: i64) -> Self {

        // ─── Return ───
        Self {
            uuid,
            mint,
            amount,
            token,
            hash,
            program,
            slot,
            servtime: chrono::Utc::now().timestamp_millis()
        }
    }
}