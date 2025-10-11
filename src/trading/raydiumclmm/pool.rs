// ─── import packages ───
use anyhow::{anyhow, Context, Result};
use solana_sdk::pubkey::Pubkey;

// ─── import crates ───
use crate::core::client::RPCClient;
use crate::globals::constants::*;
use crate::trading::shared::{Bytes, ClmmMath};
use crate::utils::loader::ServerConfig;

/// ─── struct 'RaydiumClmmPool' ───
/// struct description
pub struct RaydiumClmmPool {
    client: RPCClient,
    poolstate: Pubkey,
    basedec: u8,
    quotedec: u8,
    basex: bool,
}

// ─── impl 'RaydiumClmmPool' ───
/// impl description
impl RaydiumClmmPool {

    // ─── fn 'new' ───
    /// fn description
    pub async fn new(poolstate: &str, basedec: u8, quotedec: u8, basex: bool, confserv: &ServerConfig) -> Result<Self> {

        // ─── define 'client' ───
        let client = RPCClient::new(&confserv.endpoint.rpc)?;

        // ─── define 'poolstate' ───
        let poolstate: Pubkey = poolstate.parse().context("invalid poolstate pubkey")?;

        // ─── return 'Result' ───
        Ok(Self { client, poolstate, basedec, quotedec, basex })
    }

    // ─── fn 'load_state' ───
    /// fn description
    async fn loadstate(&self) -> Result<Vec<u8>> {

        // ─── define 'data' ───
        let data = self.client
            .getaccountdata(&self.poolstate)
            .await
            .with_context(|| format!("Failed to load Raydium CLMM pool state {}", self.poolstate))?;

        // ─── compare 'data.len()' ───
        if data.len() < RAYDIUMCLMMMINLEN {
            return Err(anyhow!("Raydium CLMM pool state too short: {} bytes (< {})", data.len(), RAYDIUMCLMMMINLEN));
        }

        // ─── return 'Result' ───
        Ok(data)
    }

    // ─── fn 'getpricebase' ───
    /// fn description
    pub async fn getpricebase(&self) -> Result<f64> {

        // ─── define 'data' ───
        let data = self.loadstate().await?;

        // ─── define 'sqrtprice' ───
        let sqrtprice = Bytes::readu128le(&data, RAYDIUMCLMMOFFSQRTPRICE)?;

        // ─── define 'price' ───
        let price = ClmmMath::pricebasequote(
            sqrtprice,
            self.basedec,
            self.quotedec,
            self.basex,
        );

        // ─── return 'Result' ───
        Ok(price)
    }

    // ─── fn 'getliquidity' ───
    /// fn description
    pub async fn getliquidity(&self) -> Result<(f64, f64)> {

        // ─── define 'data' ───
        let data = self.loadstate().await?;

        // ─── define 'endx' ───
        let endx = RAYDIUMCLMMOFFVAULTX.checked_add(32).ok_or_else(|| anyhow!("vault X offset overflow"))?;

        // ─── define 'endy' ───
        let endy = RAYDIUMCLMMOFFVAULTY.checked_add(32).ok_or_else(|| anyhow!("vault Y offset overflow"))?;

        // ─── define 'data' ───
        if endx > data.len() || endy > data.len() {
            return Err(anyhow!("Pool state too short for vault pubkeys"));
        }

        // ─── define 'vaultx' ───
        let vaultx = Pubkey::new_from_array(
            data[RAYDIUMCLMMOFFVAULTX .. endx].try_into()?
        );

        // ─── define 'vaulty' ───
        let vaulty = Pubkey::new_from_array(
            data[RAYDIUMCLMMOFFVAULTY .. endy].try_into()?
        );

        // ─── define 'data' ───
        let (basevault, basedec, quotevault, quotedec) = if self.basex {
            (vaultx, self.basedec, vaulty, self.quotedec)
        } else {
            (vaulty, self.basedec, vaultx, self.quotedec)
        };

        // ─── define '(baseui, quoteui)' ───
        let (baseui, quoteui) = self.client.getpoolsbalance(&basevault, basedec, &quotevault, quotedec)
            .await?;

        Ok((baseui, quoteui))
    }
}