// ─── import packages ───
use anyhow::{anyhow, Context, Result};
use futures::try_join;
use log::{debug, error, info};
use reqwest::Client as AsyncClient;
use serde_json::{json, Value};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

// ─── import crates ───
use crate::utils::scripts::Scripts;

// ─── struct 'RPCClient' ───
/// struct description
pub struct RPCClient {
    http: AsyncClient,
    limit: Arc<Semaphore>,
    rpcaddr: String,
    rpcclient: RpcClient
}

// ─── impl 'RPCClient' ───
/// impl description
impl RPCClient {
    // ─── fn 'new' ───
    /// fn description
    pub fn new(rpcurl: &str) -> Result<Self> {

        // ─── define 'http' ───
        let http = AsyncClient::builder()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(8)
            .tcp_nodelay(true)
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to build HTTP client")?;

        // ─── define 'rpc_client' ───
        let rpcclient = RpcClient::new_with_timeout_and_commitment(
            rpcurl.to_string(),
            Duration::from_secs(10),
            CommitmentConfig::confirmed(),
        );

        // ─── return 'Result' ───
        Ok(Self { http, limit: Arc::new(Semaphore::new(16)), rpcaddr: rpcurl.to_string(), rpcclient })
    }

    // ─── fn 'rpc' ───
    /// fn description
    fn callrpc(&self) -> &RpcClient {

        // ─── return 'RpcClient' ───
        &self.rpcclient
    }

    // ─── fn 'rpcerror' ───
    /// fn description
    async fn rpcerror(err: reqwest::Error, rpcurl: &str) {

        // ─── compare 'err' ───
        if err.is_timeout() {
            error!("RPC request timed out after 10 seconds: {}", rpcurl);
            error!("The private Solana RPC server may have blocked your requests.");

            // ─── match 'showip' ───
            match Scripts::showip().await {
                Some(ip) => error!("Your public IP: {}", ip),
                None => debug!("Could not determine public IP"),
            }
        } else if err.is_connect() {
            error!("Failed to connect to RPC (connect error): {}", rpcurl);

            // ─── match 'showip' ───
            match Scripts::showip().await {
                Some(ip) => error!("Your public IP: {}", ip),
                None => debug!("Could not determine public IP"),
            }
        } else {
            error!("Failed to reach RPC: {} — {}", err, rpcurl);

            // ─── match 'showip' ───
            match Scripts::showip().await {
                Some(ip) => error!("Your public IP: {}", ip),
                None => debug!("Could not determine public IP"),
            }
        }
    }

    // ─── fn 'rpcresponse' ───
    /// fn description
    async fn rpcresponse(response: reqwest::Response, rpcurl: &str) -> bool {

        // ─── compare 'response' ───
        if !response.status().is_success() {
            error!("RPC responded with non-200 status: {} — {}", response.status(), rpcurl);

            // ─── match 'showip' ───
            match Scripts::showip().await {
                Some(ip) => error!("Your public IP: {}", ip),
                None => debug!("Could not determine public IP"),
            }

            // ─── return 'bool' ───
            return false;
        }

        // ─── match 'response' ───
        match response.json::<Value>().await {
            Ok(json) => match json.get("result").and_then(Value::as_str) {
                Some("ok") => {
                    info!("Valid Solana RPC: {}", rpcurl);
                    true
                }
                _ => {
                    error!("Unexpected RPC response: {}", json);

                    // ─── match 'showip' ───
                    match Scripts::showip().await {
                        Some(ip) => error!("Your public IP: {}", ip),
                        None => debug!("Could not determine public IP"),
                    }

                    false
                }
            },
            Err(err) => {
                error!("Failed to parse RPC response: {}", err);

                // ─── match 'showip' ───
                match Scripts::showip().await {
                    Some(ip) => error!("Your public IP: {}", ip),
                    None => debug!("Could not determine public IP"),
                }

                false
            }
        }
    }

    // ─── fn 'getstatus' ───
    /// fn description
    pub async fn getstatus(&self) -> bool {

        // ─── define 'rpcurl' ───
        let rpcurl = &self.rpcaddr;

        // ─── console log ───
        info!("Pinging RPC at {}", rpcurl);

        // ─── define 'payload' ───
        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getHealth"
        });

        // ─── define 'response' ───
        let response = match self.http.post(rpcurl).json(&payload).send().await {
            Ok(resp) => resp,
            Err(err) => {
                Self::rpcerror(err, rpcurl).await;

                // ─── return 'bool' ───
                return false;
            }
        };

        // ─── return 'Self::rpcresponse()' ───
        Self::rpcresponse(response, rpcurl).await
    }

    // ─── fn 'gethealth' ───
    /// fn description
    pub async fn gethealth(&self) -> Result<()> {

        // ─── callback 'self.limit.acquire()' ───
        let _ = self.limit.acquire().await?;

        // ─── callback 'self.callrpc()' ───
        self.callrpc().get_health().await.with_context(|| format!("Failed to check health for RPC: {}", self.rpcaddr))?;

        // ─── return 'Result' ───
        Ok(())
    }

    // ─── fn 'getaccountdata' ───
    /// fn description
    pub async fn getaccountdata(&self, pubkey: &Pubkey) -> Result<Vec<u8>> {

        // ─── define '_permit' ───
        let _permit = self.limit.acquire().await?;

        // ─── define 'account' ───
        let account = self.callrpc()
            .get_account_data(pubkey)
            .await
            .with_context(|| format!("Failed to get account data for {}", pubkey))?;

        // ─── return 'balance' ───
        Ok(account)
    }

    // ─── fn 'getmintdecimals' ───
    /// fn description
    pub async fn getmintdecimals(&self, mint: &Pubkey) -> Result<u8> {

        // ─── define '_permit' ───
        let _permit = self.limit.acquire().await?;

        // ─── define 'supply' ───
        let supply = self.callrpc()
            .get_token_supply(mint)
            .await
            .with_context(|| format!("Failed to get token supply for mint {}", mint))?;

        // ─── return 'supply.decimals' ───
        Ok(supply.decimals)
    }

    // ─── fn 'getmintsupply' ───
    /// fn description
    pub async fn getmintsupply(&self, mint: &Pubkey) -> Result<f64> {

        // concurrency cap
        let _permit = self.limit.acquire().await?;

        // ─── Define 'supply' ───
        let supply = self
            .callrpc()
            .get_token_supply(mint)
            .await
            .with_context(|| format!("Failed to get token supply for mint {}", mint))?;

        // ─── Define 'raw' ───
        let raw: u128 = supply
            .amount
            .parse()
            .map_err(|e| anyhow!("Invalid supply amount {}: {e}", supply.amount))?;

        // ─── Define 'scale' ───
        let scale = 10u128
            .checked_pow(supply.decimals as u32)
            .ok_or_else(|| anyhow!("pow overflow for decimals {}", supply.decimals))?;

        // ─── Return 'Result' ───
        Ok((raw as f64) / (scale as f64))
    }

    // ─── fn 'gettokenaccountbalance' ───
    /// fn description
    pub async fn gettokenaccountbalance(&self, account: &Pubkey, decimals: u8) -> Result<f64> {

        // ─── define '_permit' ───
        let _permit = self.limit.acquire().await?;

        // ─── define 'ui' ───
        let ui = self
            .callrpc()
            .get_token_account_balance(account)
            .await
            .with_context(|| format!("Failed to get token account balance: {}", account))?;

        // ─── define 'raw' ───
        let raw: u128 = ui
            .amount
            .parse()
            .map_err(|e| anyhow!("Invalid amount format {}: {e}", ui.amount))?;

        // ─── define 'scale' ───
        let scale = 10u128
            .checked_pow(decimals as u32)
            .ok_or_else(|| anyhow!("pow overflow for decimals {}", decimals))?;

        // ─── return 'Result' ───
        Ok((raw as f64) / (scale as f64))
    }

    // ─── fn 'getpoolsbalance' ───
    /// fn description
    pub async fn getpoolsbalance(&self, baseaddr: &Pubkey, basedec: u8, quoteaddr: &Pubkey, quotedec: u8) -> Result<(f64, f64)> {

        // ─── define '(av, bv)' ───
        let (av, bv) = try_join!(
            self.gettokenaccountbalance(baseaddr, basedec),
            self.gettokenaccountbalance(quoteaddr, quotedec)
        )?;

        // ─── return '(av, bv)' ───
        Ok((av, bv))
    }

    // ─── fn 'getwalletbalance' ───
    /// fn description
    pub async fn getwalletbalance(&self, address: &str) -> Result<u64> {

        // ─── define '_permit' ───
        let _permit = self.limit.acquire().await?;

        // ─── define 'pubkey' ───
        let pubkey = address
            .parse::<Pubkey>()
            .with_context(|| format!("Invalid public key: {}", address))?;

        // ─── define 'balance' ───
        let balance = self
            .callrpc()
            .get_balance_with_commitment(&pubkey, CommitmentConfig::confirmed())
            .await
            .with_context(|| format!("Failed to get balance for account: {}", address))?
            .value;

        // ─── return 'Result' ───
        Ok(balance)
    }
}