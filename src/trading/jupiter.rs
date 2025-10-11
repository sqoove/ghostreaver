// ─── import packages ───
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bincode;
use bs58;
use reqwest::header::HeaderMap;
use reqwest::{Client, Method};
use serde::Deserialize;
use serde_json::Value;
use serde_json;
use sha2::{Digest, Sha256};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcSendTransactionConfig, RpcSimulateTransactionConfig};
use solana_commitment_config::{CommitmentConfig, CommitmentLevel};
use solana_sdk::{message::VersionedMessage, signature::{Keypair, Signer}, transaction::VersionedTransaction};

// ─── import crates ───
use crate::globals::constants::*;
use crate::globals::pubkeys::*;

// ─── struct 'FetchResponse' ───
/// struct description
#[derive(Debug, Deserialize)]
struct FetchResponse {
    #[serde(rename = "outAmount")]
    pub outamount: String,
    #[serde(rename = "routePlan")]
    pub routeplan: Vec<RoutePlan>,
}

// ─── struct 'RoutePlan' ───
/// struct description
#[derive(Debug, Deserialize)]
struct RoutePlan {
    #[serde(rename = "swapInfo")]
    pub fetchinfo: Option<FetchInfo>,
}

// ─── struct 'FetchInfo' ───
/// struct description
#[derive(Debug, Deserialize)]
struct FetchInfo {
    #[serde(rename = "feeAmount")]
    pub feevalue: String,
    #[serde(rename = "feeMint")]
    pub feemint: String,
}

// ─── struct 'JupiterHttp' ───
/// struct description
struct JupiterHttp {
    client: Client,
}

// ─── impl 'JupiterHttp' ───
/// impl description
impl JupiterHttp {

    // ─── fn 'new' ───
    /// fn description
    fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'client' ───
        let client = Client::builder().build()?;

        // ─── return 'Result' ───
        Ok(Self { client })
    }

    // ─── fn 'headers' ───
    /// fn description
    fn headers(&self) -> Result<HeaderMap, Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'headers' ───
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        headers.insert("Accept", "application/json".parse()?);

        // ─── return 'Result' ───
        Ok(headers)
    }
}

// ─── struct 'SwapClient' ───
/// struct description
pub struct SwapClient {
    http: JupiterHttp,
}

// ─── impl 'SwapClient' ───
/// impl description
impl SwapClient {

    // ─── fn 'new' ───
    /// fn description
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {

        // ─── return 'Result' ───
        Ok(Self { http: JupiterHttp::new()? })
    }

    // ─── fn 'fetchquote' ───
    /// fn description
    pub async fn fetchquote(&self, inputmint: &str, outputmint: &str, rawamount: u64) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'url' ───
        let url = format!(
            "https://lite-api.jup.ag/swap/v1/quote?inputMint={}&outputMint={}&amount={}&swapMode=ExactIn&dynamicSlippage=true&wrapUnwrapSOL=true",
            inputmint, outputmint, rawamount
        );

        // ─── define 'resp' ───
        let resp = self
            .http
            .client
            .request(Method::GET, &url)
            .headers(self.http.headers()?)
            .send()
            .await?;

        // ─── define 'status' ───
        let status = resp.status();

        // ─── define 'body' ───
        let body = resp.text().await.unwrap_or_else(|_| "<failed to read response body>".into());

        // ─── compare 'status.is_success()' ───
        if !status.is_success() {
            return Err(format!("Jupiter /quote {}. Body: {}", status, body).into());
        }

        // ─── return 'Result' ───
        Ok(body)
    }

    // ─── fn 'extractfee' ───
    /// fn description
    fn extractfee(parsed: &FetchResponse) -> f64 {

        // ─── compare 'parsed.routeplan.first()' ───
        if let Some(info) = parsed.routeplan.first().and_then(|r| r.fetchinfo.as_ref()) {

            // ─── define 'feeraw' ───
            let feeraw = info.feevalue.parse::<u64>().unwrap_or(0);

            // ─── compare 'system_pubkeys::WRAPPER' ───
            if info.feemint == system_pubkeys::WRAPPER.to_string() {
                return feeraw as f64 / LAMPORTSPERSOL;
            }
        }
        0.0
    }

    // ─── fn 'gensign' ───
    /// fn description
    fn gensign(seed: &str) -> String {

        // ─── return 'String' ───
        format!("{:x}", Sha256::digest(seed.as_bytes()))
    }

    // ─── fn 'swaptransaction' ───
    /// fn description
    async fn swaptransaction(&self, publicaddr: &str, jsonquote: &str, priolamports: u64)
                             -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    {
        let jsondata = format!(
            r#"{{
            "userPublicKey": "{}",
            "quoteResponse": {},
            "prioritizationFeeLamports": {{
                "priorityLevelWithMaxLamports": {{
                    "maxLamports": {},
                    "priorityLevel": "veryHigh"
                }}
            }},
            "dynamicComputeUnitLimit": true,
            "asLegacyTransaction": true
        }}"#,
            publicaddr, jsonquote, priolamports
        );

        let resp = self.http.client
            .request(Method::POST, "https://lite-api.jup.ag/swap/v1/swap")
            .headers(self.http.headers()?)
            .body(jsondata)
            .send()
            .await?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_else(|_| "<failed to read response body>".into());
        if !status.is_success() {
            return Err(format!("Jupiter /swap {}. Body: {}", status, body).into());
        }

        let v: Value = serde_json::from_str(&body)?;
        let txb = v.get("swapTransaction").and_then(|x| x.as_str())
            .ok_or("Missing swapTransaction in Jupiter /swap response")?;
        Ok(txb.to_string())
    }


    // ─── fn 'signsend' ───
    fn signsend(&self, rpcurl: &str, txb: &str, privatekey: &str)
                -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    {
        // decode & sign
        let mut tx: VersionedTransaction = {
            let bytes = BASE64.decode(txb)?;
            bincode::deserialize(&bytes)?
        };
        let sk = bs58::decode(privatekey).into_vec()?;
        let keypair = Keypair::try_from(&sk[..])?;
        let msgbytes = match &tx.message {
            VersionedMessage::Legacy(m) => m.serialize(),
            VersionedMessage::V0(m)     => m.serialize(),
        };
        let sig = keypair.sign_message(&msgbytes);
        if tx.signatures.is_empty() { tx.signatures.push(sig); } else { tx.signatures[0] = sig; }

        let rpc = RpcClient::new(rpcurl.to_string());

        // PRE-SIM with Processed commitment  ✅ uses CommitmentConfig
        let sim = rpc.simulate_transaction_with_config(
            &tx,
            RpcSimulateTransactionConfig {
                sig_verify: true,
                replace_recent_blockhash: false,
                commitment: Some(CommitmentConfig::processed()),
                ..Default::default()
            },
        )?;
        if let Some(err) = &sim.value.err {
            let logs = sim.value.logs.unwrap_or_default().join("\n");
            return Err(format!("Simulation failed: {err:?}\nLogs:\n{logs}").into());
        }

        // SEND with Processed for preflight & confirm  ✅ CommitmentConfig here too
        let sig = rpc.send_and_confirm_transaction_with_spinner_and_config(
            &tx,
            CommitmentConfig::processed(),
            RpcSendTransactionConfig {
                skip_preflight: false,
                preflight_commitment: Some(CommitmentLevel::Processed),
                ..Default::default()
            },
        )?;

        Ok(sig.to_string())
    }


    // ─── fn 'swapbase' ───
    /// fn description
    pub async fn swapbase(&self, baseamount: f64, tokenmint: &str, decimals: u8, maxtokens: Option<u64>, sandbox: bool, publicaddr: &str,
        privatekey: &str, priolamports: u64, rpcurl: &str) -> Result<(f64, f64, String), Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'basemint' ───
        let basemint = system_pubkeys::WRAPPER.to_string();

        // ─── define 'rawbase' ───
        let rawbase = (baseamount * LAMPORTSPERSOL).round() as u64;

        // ─── define 'jsonquote' ───
        let jsonquote = self.fetchquote(&basemint, tokenmint, rawbase).await?;

        // ─── define 'parsed' ───
        let parsed: FetchResponse = serde_json::from_str(&jsonquote).map_err(|e| format!("Failed to parse Jupiter quote: {e}. Body: {jsonquote}"))?;

        // ─── define 'outraw' ───
        let outraw = parsed.outamount.parse::<u128>()?;

        // ─── define 'totalfee' ───
        let totalfee = Self::extractfee(&parsed);

        // ─── define 'totalswap' ───
        let totalswap = outraw as f64 / 10f64.powi(decimals as i32);

        // ─── compare 'maxtokens' ───
        if let Some(maxallowed) = maxtokens {

            // ─── compare 'maxallowed' ───
            if totalswap > maxallowed as f64 {

                // ─── define 'msg' ───
                let msg = format!("OutOfRangeTokens: returned {:.6} > allowed {}", totalswap, maxallowed);

                // ─── return 'Result' ───
                return Err(std::io::Error::new(std::io::ErrorKind::Other, msg).into());
            }
        }

        // ─── compare 'sandbox' ───
        if sandbox {

            // ─── define 'txsig' ───
            let txsig = Self::gensign(&format!("buy:{}:{}", tokenmint, rawbase));

            // ─── return 'Result' ───
            return Ok((totalswap, totalfee, txsig));
        }

        // ─── define 'txb' ───
        let txb = self.swaptransaction(publicaddr, &jsonquote, priolamports).await?;

        // ─── define 'txsig' ───
        let txsig = self.signsend(rpcurl, &txb, privatekey)?;

        // ─── return 'Result' ───
        Ok((totalswap, totalfee, txsig))
    }

    // ─── fn 'swapquote' ───
    /// fn description
    pub async fn swapquote(&self, quoteamount: f64, tokenmint: &str, decimals: u8, sandbox: bool, publicaddr: &str, privatekey: &str,
        priolamports: u64, rpcurl: &str) -> Result<(f64, f64, String), Box<dyn std::error::Error + Send + Sync>> {

        // ─── define 'basemint' ───
        let basemint = system_pubkeys::WRAPPER.to_string();

        // ─── define 'rawamount' ───
        let rawamount = (quoteamount * 10f64.powi(decimals as i32)).round() as u64;

        // ─── define 'jsonquote' ───
        let jsonquote = self.fetchquote(tokenmint, &basemint, rawamount).await?;

        // ─── define 'parsed' ───
        let parsed: FetchResponse = serde_json::from_str(&jsonquote).map_err(|e| format!("Failed to parse Jupiter quote: {e}. Body: {jsonquote}"))?;

        // ─── define 'outlamports' ───
        let outlamports = parsed.outamount.parse::<u64>()?;

        // ─── define 'totalfee' ───
        let totalfee = Self::extractfee(&parsed);

        // ─── define 'totalswap' ───
        let totalswap = outlamports as f64 / LAMPORTSPERSOL;

        // ─── compare 'sandbox' ───
        if sandbox {

            // ─── define 'txsig' ───
            let txsig = Self::gensign(&format!("sell:{}:{}", tokenmint, rawamount));

            // ─── return 'Result' ───
            return Ok((totalswap, totalfee, txsig));
        }

        // ─── define 'txb' ───
        let txb = self.swaptransaction(publicaddr, &jsonquote, priolamports).await?;

        // ─── define 'txsig' ───
        let txsig = self.signsend(rpcurl, &txb, privatekey)?;

        // ─── return 'Result' ───
        Ok((totalswap, totalfee, txsig))
    }
}