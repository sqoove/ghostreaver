// ─── import packages ───
use anyhow::{Context, Result, anyhow};
use solana_sdk::{pubkey::Pubkey};

// ─── import crates ───
use crate::globals::pubkeys::*;
use crate::core::client::RPCClient;
use crate::globals::constants::*;
use crate::trading::shared::DerivedAddress;
use crate::utils::loader::ServerConfig;

// ─── struct 'BonkPool' ───
/// struct description
pub struct BonkPool {
    client: RPCClient,
    poolstate: Pubkey,
}

// ─── impl 'BonkPool' ───
/// impl description
impl BonkPool {

    // ─── fn 'new' ───
    /// fn description
    pub async fn new(basemint: &str, quotemint: Option<&str>, confserv: &ServerConfig) -> Result<Self> {

        // ─── define 'base' ───
        let base: Pubkey = basemint.parse().context("invalid base mint")?;

        // ─── define 'quote' ───
        let quote: Pubkey = match quotemint {
            Some(q) => q.parse().context("invalid quote mint")?,
            None => system_pubkeys::WRAPPER,
        };

        // ─── define 'poolstate' ───
        let poolstate = DerivedAddress::find(
            &bonk_pubkeys::PROGRAM,
            &[b"pool", base.as_ref(), quote.as_ref()],
        );

        // ─── return 'Self' ───
        Ok(Self { client: RPCClient::new(&confserv.endpoint.rpc)?, poolstate })
    }

    // ─── fn 'readlength' ───
    /// fn description
    fn readlength(data: &[u8], off: usize) -> Result<u64> {

        // ─── compare 'data.len()' ───
        if off + 8 > data.len() {
            return Err(anyhow!("PoolState account too short at offset {}", off));
        }

        // ─── return 'Result' ───
        Ok(u64::from_le_bytes(data[off..off+8].try_into()?))
    }

    // ─── fn 'getpricebase' ───
    /// fn description
    pub async fn getpricebase(&self) -> Result<f64> {

        // ─── define 'data' ───
        let data = self.client
            .getaccountdata(&self.poolstate)
            .await
            .with_context(|| format!("Failed to load pool state {}", self.poolstate))?;

        // ─── compare 'BONKMINLEN' ───
        if data.len() < BONKMINLEN {
            return Err(anyhow!("PoolState data too short: {} bytes", data.len()));
        }

        // ─── define 'bdec' ───
        let bdec = data[BONKBASEDEC] as u32;

        // ─── define 'qdec' ───
        let qdec = data[BONKQUOTEDEC] as u32;

        // ─── define 'vb' ───
        let vb = (Self::readlength(&data, BONKREALBASE)? as u128) + (Self::readlength(&data, BONKVIRTUALBASE)? as u128);

        // ─── define 'vq' ───
        let vq = (Self::readlength(&data, BONKREALQUOTE)? as u128) + (Self::readlength(&data, BONKVIRTUALQUOTE)? as u128);

        // ─── compare 'base' ───
        if vb == 0 {
            return Ok(0.0);
        }

        // ─── define 'vsol' ───
        let vsol = (vb as f64) / 10f64.powi(bdec as i32);

        // ─── define 'vtok' ───
        let vtok = (vq as f64) / 10f64.powi(qdec as i32);

        // ─── return '(vsol, vtok)' ───
        Ok((vsol / vtok) / LAMPORTSPERSOL)
    }

    // ─── fn 'getliquidity' ───
    /// fn description─
    pub async fn getliquidity(&self) -> Result<(f64, f64)> {

        // ─── define 'data' ───
        let data = self.client
            .getaccountdata(&self.poolstate)
            .await
            .with_context(|| format!("Failed to load pool state {}", self.poolstate))?;

        // ─── compare 'BONKMINLEN' ───
        if data.len() < BONKMINLEN {
            return Err(anyhow!("PoolState data too short: {} bytes", data.len()));
        }

        // ─── define 'bdec' ───
        let bdec = data[BONKBASEDEC] as u32;

        // ─── define 'qdec' ───
        let qdec = data[BONKQUOTEDEC] as u32;

        // ─── define 'vb' ───
        let vb = (Self::readlength(&data, BONKREALBASE)? as u128) + (Self::readlength(&data, BONKVIRTUALBASE)? as u128);

        // ─── define 'vq' ───
        let vq = (Self::readlength(&data, BONKREALQUOTE)? as u128) + (Self::readlength(&data, BONKVIRTUALQUOTE)? as u128);

        // ─── define 'baseui' ───
        let baseui = (vq as f64) / 10f64.powi(qdec as i32);

        // ─── define 'quoteui' ───
        let quoteui = (vb as f64) / 10f64.powi(bdec as i32);

        // ─── return 'Result' ───
        Ok((baseui, quoteui))
    }
}
