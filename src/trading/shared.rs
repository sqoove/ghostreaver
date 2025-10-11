// ─── import packages ───
use anyhow::{anyhow, Context, Result};
use solana_sdk::pubkey::Pubkey;

// ─── import crates ───
use crate::core::client::RPCClient;
use crate::globals::constants::*;
use crate::utils::loader::ServerConfig;

// ─── struct 'BaseQuotePool' ───
/// struct description
pub struct BaseQuotePool {
    client: RPCClient,
    basevault: Pubkey,
    quotevault: Pubkey,
    decimals: u8,
}

// ─── impl 'BaseQuotePool' ───
/// impl description
impl BaseQuotePool {

    // ─── fn 'new' ───
    /// fn description
    pub fn new(basevault: &str, quotevault: &str, decimals: u8, confserv: &ServerConfig) -> Result<Self> {

        // ─── define 'base' ───
        let base = basevault.parse::<Pubkey>().context("invalid base vault")?;

        // ─── define 'quote' ───
        let quote = quotevault.parse::<Pubkey>().context("invalid quote vault")?;

        // ─── return 'Self::frompubkeys()' ───
        Self::frompubkeys(base, quote, decimals, confserv)
    }

    // ─── fn 'frompubkeys' ───
    /// fn description
    pub fn frompubkeys(basevault: Pubkey, quotevault: Pubkey, decimals: u8, confserv: &ServerConfig) -> Result<Self> {

        // ─── return 'Self' ───
        Ok(Self {
            client: RPCClient::new(&confserv.endpoint.rpc)?,
            basevault,
            quotevault,
            decimals
        })
    }

    // ─── fn 'getpricebase' ───
    /// fn description
    pub async fn getpricebase(&self) -> Result<f64> {

        // ─── define '(baseoutput, quoteoutput)' ───
        let (baseoutput, quoteoutput) = self.client.getpoolsbalance(&self.basevault, BASEDECIMALS, &self.quotevault, self.decimals).await?;
        Ok(if quoteoutput > 0.0 {
            baseoutput / quoteoutput
        } else {
            0.0
        })
    }

    // ─── fn 'getliquidity' ───
    /// fn description
    pub async fn getliquidity(&self) -> Result<(f64, f64)> {

        // ─── return 'Self' ───
        self.client.getpoolsbalance(&self.basevault, BASEDECIMALS, &self.quotevault, self.decimals).await
    }
}

// ─── struct 'DerivedAddress' ───
/// struct description
pub struct DerivedAddress;

// ─── impl 'DerivedAddress' ───
/// impl description
impl DerivedAddress {

    // ─── fn 'find' ───
    /// fn description
    pub fn find(program: &Pubkey, seeds: &[&[u8]]) -> Pubkey {

        // ─── return 'Pubkey' ───
        Pubkey::find_program_address(seeds, program).0
    }
}

// ─── struct 'Bytes' ───
/// struct description
pub struct Bytes;

// ─── impl 'Bytes' ───
/// impl description
impl Bytes {

    // ─── fn 'readu128le' ───
    /// fn description
    pub fn readu128le(data: &[u8], off: usize) -> Result<u128> {

        // ─── define 'end' ───
        let end = off.checked_add(16).ok_or_else(|| anyhow!("u128 read overflow"))?;

        // ─── compare 'data.len()' ───
        if end > data.len() {
            return Err(anyhow!("buffer too short for u128 at {}", off));
        }

        // ─── return 'Result' ───
        Ok(u128::from_le_bytes(data[off..end].try_into()?))
    }
}

// ─── struct 'ClmmMath' ───
/// struct description
pub struct ClmmMath;

// ─── impl 'ClmmMath' ───
/// impl description
impl ClmmMath {

    // ─── fn 'pricebasequote' ───
    /// fn description
    pub fn pricebasequote(sqrtprice: u128, basedecimals: u8, quotedecimals: u8, basex: bool) -> f64 {

        // ─── compare 'sqrt_price_x64' ───
        if sqrtprice == 0 {
            return 0.0;
        }

        // ─── define 'sqrt' ───
        let sqrt = sqrtprice as f64;

        // ─── define 'two64' ───
        let two64 = 18446744073709551616.0_f64;

        // ─── define 'ratiopx' ───
        let ratiopx = (sqrt / two64) * (sqrt / two64);

        // ─── define 'scale' ───
        let scale = 10f64.powi((quotedecimals as i32) - (basedecimals as i32));

        // ─── define 'pxui' ───
        let pxui = ratiopx * scale;

        // ─── compare 'base_is_x' ───
        if basex {
            if pxui > 0.0 { 1.0 / pxui } else { 0.0 }
        } else {
            pxui
        }
    }
}

// ─── macro 'basequotepool' ───
/// macro description
#[macro_export]
macro_rules! basequotepool {
    ($name:ident) => {

        // ─── struct 'BaseQuotePool' ───
        /// struct description
        pub struct $name($crate::trading::shared::BaseQuotePool);

        // ─── impl 'BaseQuotePool' ───
        /// impl description
        impl $name {

            // ─── fn 'new' ───
            /// fn description
            pub fn new(basevault: &str, quotevault: &str, decimals: u8, confserv: &$crate::utils::loader::ServerConfig) -> anyhow::Result<Self> {

                // ─── return 'Result' ───
                Ok(Self($crate::trading::shared::BaseQuotePool::new(basevault, quotevault, decimals, confserv)?))
            }
        }

        // ─── impl 'Deref' ───
        /// impl description
        impl ::std::ops::Deref for $name {

            // ─── type 'Target' ───
            type Target = $crate::trading::shared::BaseQuotePool;

            // ─── fn 'deref' ───
            /// fn description
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}