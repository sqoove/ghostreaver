// ─── imports packages ───
use anyhow::{anyhow, Context, Result};
use solana_sdk::pubkey::Pubkey;
use std::future::Future;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep, timeout};

// ─── import crates ───
use crate::core::client::RPCClient;
use crate::globals::constants::*;
use crate::globals::pubkeys::*;
use crate::trading::bonk::pool::BonkPool;
use crate::trading::pumpfun::pool::PumpfunPool;
use crate::trading::pumpswap::pool::PumpswapPool;
use crate::trading::raydiumamm::pool::RaydiumAmmPool;
use crate::trading::raydiumclmm::pool::RaydiumClmmPool;
use crate::trading::raydiumcpmm::pool::RaydiumCpmmPool;
use crate::utils::loader::ServerConfig;
use crate::utils::scripts::Scripts;
use crate::utils::storage::{TokenRow};

// ─── struct 'EnrichedToken' ───
/// struct description
#[derive(Debug, Clone)]
pub struct EnrichedToken {
    pub price: f64,
    pub initbase: i64,
    pub initquote: i64,
    pub decimals: u8,
    pub supply: i64
}

// ─── struct 'Scanner' ───
/// struct description
pub struct Scanner;

// ─── impl 'Scanner' ───
/// impl description
impl Scanner {

    // ─── fn 'retrybackoff' ───
    /// fn description
    async fn retrybackoff<T, E, F, Fut>(mut op: F, attempts: usize, basedelay: u64, callms: u64, what: &str) -> Result<T>
    where F: FnMut() -> Fut, Fut: Future<Output = std::result::Result<T, E>>, E: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static {

        // ─── define 'delay' ───
        let mut delay = basedelay;

        // ─── define 'lastidx' ───
        let lastidx = attempts.saturating_sub(1);

        // ─── proceed 'for' ───
        for i in 0..attempts {

            // ─── define 'res' ───
            let res = timeout(Duration::from_millis(callms), op()).await;

            // ─── match 'res' ───
            match res {
                Ok(Ok(v)) => return Ok(v),
                Ok(Err(e)) => {

                    // ─── compare 'lastidx' ───
                    if i == lastidx {
                        return Err(anyhow!("{what} failed after {attempts} attempts: {e:?}"));
                    }
                }
                Err(_) => {

                    // ─── compare 'lastidx' ───
                    if i == lastidx {
                        return Err(anyhow!("{what} timed out after {attempts} attempts"));
                    }
                }
            }

            // ─── callback 'sleep()' ───
            sleep(Duration::from_millis(delay)).await;
            delay = delay.saturating_mul(2).min(1500);
        }
        Err(anyhow!("{what} exhausted retries"))
    }

    // ─── fn 'tokeninfo' ───
    /// fn description
    async fn tokeninfo(client: &RPCClient, mintaddr: &str) -> Result<(u8, i64)> {

        // ─── define 'mint' ───
        let mint = Pubkey::from_str(mintaddr).context("invalid mint pubkey")?;

        // ─── define 'decimals' ───
        let decimals: u8 = Self::retrybackoff(
            || client.getmintdecimals(&mint),
            SCANNERATTEMPTS,
            SCANNERBASEDELAY,
            SCANNECALLTIMEOUT,
            "getmintdecimals"
        ).await.context("Failed to get decimals")?;

        // ─── define 'uisupply' ───
        let uisupply: f64 = Self::retrybackoff(
            || client.getmintsupply(&mint),
            SCANNERATTEMPTS,
            SCANNERBASEDELAY,
            SCANNECALLTIMEOUT,
            "getmintsupply",
        ).await.context("Failed to get token supply")?;

        // ─── define 'rawsupply' ───
        let rawsupply = Scripts::uiconv(uisupply, decimals as u32);

        // ─── return 'Result' ───
        Ok((decimals, rawsupply))
    }

    // ─── fn 'tokenenrich' ───
    /// fn description
    pub async fn tokenenrich(row: &TokenRow) -> Result<EnrichedToken> {

        // ─── define 'confserv' ───
        let confserv = ServerConfig::loadconfig(PATHCONFIGENDPOINT)
            .map_err(|e| anyhow!(e.to_string()))
            .context("load ServerConfig failed")?;

        // ─── define 'rpcclient' ───
        let rpcclient = RPCClient::new(&confserv.endpoint.rpc).context("failed to build RPCClient")?;

        // ─── define '(decimals, rawsupply)' ───
        let (decimals, rawsupply) = Self::tokeninfo(&rpcclient, &row.mint)
            .await
            .with_context(|| format!("tokeninfo failed for mint {}", row.mint))?;

        // ─── define 'rawdata' ──
        let rawdata = |uibase: f64, uiquote: f64| -> (i64, i64) {
            (Scripts::uiconv(uibase, BASEDECIMALS as u32), Scripts::uiconv(uiquote, decimals as u32))
        };

        // ─── define 'pid' ──
        let pid = row.program.as_str();

        // ─── define 'enriched' ──
        let enriched = if pid == bonk_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = BonkPool::new(&row.mint, None, &confserv).await?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else if pid == pumpfun_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = PumpfunPool::new(&row.mint, &confserv).await?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else if pid == pumpswap_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = PumpswapPool::new(&row.basevault, &row.quotevault, decimals, &confserv)?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else if pid == raydiumamm_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = RaydiumAmmPool::new(&row.basevault, &row.quotevault, decimals, &confserv)?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else if pid == raydiumclmm_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = RaydiumClmmPool::new(&row.pool, BASEDECIMALS, decimals, true, &confserv).await?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else if pid == raydiumcpmm_pubkeys::PROGRAM.to_string() {

            // ─── define 'pool' ──
            let pool = RaydiumCpmmPool::new(&row.basevault, &row.quotevault, decimals, &confserv)?;

            // ─── define 'price' ──
            let price = pool.getpricebase().await?;

            // ─── define '(uibase, uiquote)' ──
            let (uibase, uiquote) = pool.getliquidity().await?;

            // ─── define '(initbase, initquote)' ──
            let (initbase, initquote) = rawdata(uibase, uiquote);

            // ─── callback 'EnrichedToken' ──
            EnrichedToken { price, initbase, initquote, decimals, supply: rawsupply }
        } else {
            return Err(anyhow!("unknown program id: {}", pid));
        };

        // ─── return 'Result' ───
        Ok(enriched)
    }
}