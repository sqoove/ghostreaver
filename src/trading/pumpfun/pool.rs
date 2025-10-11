// ─── import packages ───
use anyhow::{Context, Result, anyhow};
use solana_sdk::{pubkey::Pubkey};

// ─── import crates ───
use crate::core::client::RPCClient;
use crate::globals::pubkeys::*;
use crate::globals::constants::*;
use crate::trading::shared::{DerivedAddress};
use crate::utils::loader::ServerConfig;

// ─── struct 'PumpfunPool' ───
/// struct description
pub struct PumpfunPool {
    client: RPCClient,
    bondingcurve: Pubkey,
    decimals: u8,
}

// ─── impl 'PumpfunPool' ───
/// impl description
impl PumpfunPool {

    // ─── fn 'new' ───
    /// fn description
    pub async fn new(mint: &str, confserv: &ServerConfig) -> Result<Self> {

        // ─── define 'client' ───
        let client = RPCClient::new(&confserv.endpoint.rpc)?;

        // ─── define 'mintaddr' ───
        let mintaddr: Pubkey = mint.parse().context("invalid mint pubkey")?;

        // ─── define 'bondingcurve' ───
        let bondingcurve = DerivedAddress::find(&pumpfun_pubkeys::PROGRAM, &[b"bonding-curve", mintaddr.as_ref()]);

        // ─── define 'decimals' ───
        let decimals = client.getmintdecimals(&mintaddr).await?;

        // ─── return 'Result' ───
        Ok(Self { client, bondingcurve, decimals })
    }

    // ─── fn 'getvirtualreserves' ───
    /// fn description
    async fn getvirtualreserves(&self) -> Result<(u64, u64)> {

        // ─── define 'data' ───
        let data = self.client
            .getaccountdata(&self.bondingcurve)
            .await
            .with_context(|| format!("Failed to load bonding curve account {}", self.bondingcurve))?;

        // ─── compare 'data' ───
        if data.len() < 24 {
            return Err(anyhow!("Bonding curve data too short: {} bytes", data.len()));
        }

        // ─── define 'vsol' ───
        let vsol = u64::from_le_bytes(data[16..24].try_into()?);

        // ─── define 'vtok' ───
        let vtok = u64::from_le_bytes(data[8..16].try_into()?);

        // ─── return '(vsol, vtok)' ───
        Ok((vsol, vtok))
    }
    
    // ─── fn 'getpricebase' ───
    /// fn description
    pub async fn getpricebase(&self) -> Result<f64> {

        // ─── define '(vsol, vtok)' ───
        let (vsol, vtok) = self.getvirtualreserves().await?;

        // ─── define 'baseui' ───
        let baseui = vsol as f64 / LAMPORTSPERSOL;

        // ─── define 'quoteui' ───
        let quoteui = vtok as f64 / 10f64.powi(self.decimals as i32);

        // ─── return 'Result' ───
        Ok(if quoteui > 0.0 { baseui / quoteui } else { 0.0 })
    }

    // ─── fn 'getliquidity' ───
    /// fn description
    pub async fn getliquidity(&self) -> Result<(f64, f64)> {

        // ─── define '(vsol, vtok)' ───
        let (vsol, vtok) = self.getvirtualreserves().await?;

        // ─── define 'baseui' ───
        let baseui = vsol as f64 / LAMPORTSPERSOL;

        // ─── define 'quoteui' ───
        let quoteui = vtok as f64 / 10f64.powi(self.decimals as i32);

        // ─── return 'Result' ───
        Ok((baseui, quoteui))
    }
}