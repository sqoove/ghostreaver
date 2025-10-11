// ─── import packages ───
use log::{error};
use std::cmp::Ordering;
use std::time::Duration;

// ─── struct 'Scripts' ───
/// struct description
pub struct Scripts;

// ─── impl 'Scripts' ───
/// impl description
impl Scripts {

    // ─── fn 'showip' ───
    /// fn decription
    pub async fn showip() -> Option<String> {

        // ─── define 'client' ───
        let client = match reqwest::Client::builder().timeout(Duration::from_secs(5)).build() {
            Ok(client) => client,
            Err(e) => {
                error!("Failed to initialize HTTP client for IP lookup: {}", e);

                // ─── return 'None' ───
                return None;
            }
        };

        // ─── match 'client.get()' ───
        match client.get("https://api.ipify.org").query(&[("format", "text")]).send().await {
            Ok(resp) => {

                // ─── compare 'resp.status()' ───
                if !resp.status().is_success() {
                    error!("IP API returned error: {}", resp.status());

                    // ─── return 'None' ───
                    return None;
                }

                // ─── match 'resp.text()' ───
                match resp.text().await {
                    Ok(ip) => Some(ip),
                    Err(e) => {
                        error!("Failed to read IP response body: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                error!("Failed to send IP request: {}", e);
                None
            }
        }
    }

    // ─── fn 'uiconv' ───
    /// fn decription
    pub fn uiconv(ui: f64, decimals: u32) -> i64 {

        // ─── define 'scale' ───
        let scale = 10f64.powi(decimals as i32);
        (ui * scale).round() as i64
    }

    // ─── fn 'percentilef64' ───
    /// fn decription
    pub fn percentilef64(v: &mut Vec<f64>, q: f64) -> Option<f64> {

        // ─── compare 'v.is_empty()' ───
        if v.is_empty() {
            return None;
        }

        v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

        // ─── Define 'idx' ───
        let idx = (((v.len() - 1) as f64) * q).floor() as usize;
        v.get(idx).copied()
    }

    // ─── fn 'percentilei64' ───
    /// fn decription
    pub fn percentilei64(v: &mut Vec<i64>, q: f64) -> Option<i64> {

        // ─── compare 'v.is_empty()' ───
        if v.is_empty() {
            return None;
        }

        v.sort_unstable();

        // ─── Define 'idx' ───
        let idx = (((v.len() - 1) as f64) * q).floor() as usize;
        v.get(idx).copied()
    }

    // ─── fn 'clampnonnegf64' ───
    /// fn decription
    pub fn clampnonnegf64(v: f64) -> f64 {

        // ─── compare 'v.is_finite()' ───
        if v.is_finite() && v >= 0.0 {
            v
        } else {
            0.0
        }
    }

    // ─── fn 'clampnonnegi64' ───
    /// fn decription
    pub fn clampnonnegi64(v: i64) -> i64 {

        // ─── compare 'v' ───
        if v >= 0 {
            v
        } else {
            0
        }
    }

    // ─── fn 'clampf64' ───
    /// fn decription
    pub fn clampf64(v: f64, lo: f64, hi: f64) -> f64 {

        // ─── return 'v' ───
        v.max(lo).min(hi)
    }

    // ─── fn 'discmatches' ───
    /// fn description
    pub fn discmatches(data: &str, expected: &str) -> bool {

        // ─── compare 'data.len()' ───
        if data.len() < expected.len() {
            return false;
        }

        // ─── return 'bool' ───
        data.as_bytes().starts_with(expected.as_bytes())
    }

    // ─── fn 'accountindices' ───
    /// fn description
    pub fn accountindices(indices: &[u8], account_count: usize) -> bool {

        // ─── return 'bool' ───
        indices.iter().all(|&idx| (idx as usize) < account_count)
    }

    // ─── fn 'readu8offset' ───
    /// fn description
    pub fn readu8offset(data: &[u8], offset: usize) -> Option<u8> {

        // ─── return 'Option' ───
        data.get(offset).copied()
    }

    // ─── fn 'readu8le' ───
    /// fn description
    pub fn readu8le(data: &[u8], offset: usize) -> Option<u8> {

        // ─── compare 'data.len()' ───
        if data.len() < offset + 1 {
            return None;
        }

        // ─── define 'bytes' ───
        let bytes: [u8; 1] = data[offset..offset + 1].try_into().ok()?;

        // ─── return 'Option' ───
        Some(u8::from_le_bytes(bytes))
    }

    // ─── fn 'readi32le' ───
    /// fn description
    pub fn readi32le(data: &[u8], offset: usize) -> Option<i32> {

        // ─── compare 'data.len()' ───
        if data.len() < offset + 4 {
            return None;
        }

        // ─── define 'bytes' ───
        let bytes: [u8; 4] = data[offset..offset + 4].try_into().ok()?;

        // ─── return 'Option' ───
        Some(i32::from_le_bytes(bytes))
    }

    // ─── fn 'readu32le' ───
    /// fn description
    pub fn readu32le(data: &[u8], offset: usize) -> Option<u32> {

        // ─── compare 'data.len()' ───
        if data.len() < offset + 4 {
            return None;
        }

        // ─── define 'bytes' ───
        let bytes: [u8; 4] = data[offset..offset + 4].try_into().ok()?;

        // ─── return 'Option' ───
        Some(u32::from_le_bytes(bytes))
    }

    // ─── fn 'readu64le' ───
    /// fn description
    pub fn readu64le(data: &[u8], offset: usize) -> Option<u64> {

        // ─── compare 'data.len()' ───
        if data.len() < offset + 8 {
            return None;
        }

        // ─── define 'bytes' ───
        let bytes: [u8; 8] = data[offset..offset + 8].try_into().ok()?;

        // ─── return 'Option' ───
        Some(u64::from_le_bytes(bytes))
    }

    // ─── fn 'readu128le' ───
    /// fn description
    pub fn readu128le(data: &[u8], offset: usize) -> Option<u128> {

        // ─── compare 'data.len()' ───
        if data.len() < offset + 16 {
            return None;
        }

        // ─── define 'bytes' ───
        let bytes: [u8; 16] = data[offset..offset + 16].try_into().ok()?;

        // ─── return 'Option' ───
        Some(u128::from_le_bytes(bytes))
    }

    // ─── fn 'readoptbool' ───
    /// fn description
    pub fn readoptbool(data: &[u8], offset: &mut usize) -> Option<Option<bool>> {

        // ─── define 'hasvalue' ───
        let hasvalue = data.get(*offset)?.clone();
        *offset += 1;

        // ─── define 'hasvalue' ───
        if hasvalue == 0 {
            return Some(None);
        }

        // ─── define 'value' ───
        let value = data.get(*offset)?.clone();
        *offset += 1;

        // ─── return 'Option' ───
        Some(Some(value != 0))
    }

    // ─── fn 'readoptbool' ───
    /// fn description
    pub fn csvescape(s: &str) -> String {

        // ─── define 'needsquotes' ───
        let needsquotes = s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r');

        // ─── compare 'needsquotes' ───
        if needsquotes {

            // ─── define 'g' ───
            let mut q = String::with_capacity(s.len() + 2);

            // ─── proceed 'for' ───
            q.push('"');
            for ch in s.chars() {

                // ─── compare 'ch' ───
                if ch == '"' {
                    q.push('"');
                }
                q.push(ch);
            }
            q.push('"');
            q
        } else {
            s.to_string()
        }
    }

    // ─── fn 'optstring' ───
    /// fn description
    pub fn optstring<T: ToString>(v: Option<T>) -> String {
        v.map(|x| x.to_string()).unwrap_or_default()
    }
}