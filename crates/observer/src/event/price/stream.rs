use tokio::sync::mpsc;

use crate::event::core::EventBatch;
use crate::event::error::ObserverError;
use crate::event::swap::stream::RawSwapEvent;

/// Price data derived from swap events.
#[derive(Debug, Clone)]
pub struct PriceUpdate {
    pub token_id: String,
    pub price: String,
    pub volume: String,
    pub block_number: u64,
    pub tx_hash: String,
}

/// Price stream does not poll RPC directly. Instead it derives price data
/// from swap events. This function converts raw swap data into price updates
/// for a given set of pool-to-token mappings.
pub async fn derive_price_updates(
    swap_batch: &EventBatch<RawSwapEvent>,
    pool_token_map: &std::collections::HashMap<String, (String, bool)>,
    tx: &mpsc::Sender<EventBatch<PriceUpdate>>,
) -> Result<(), ObserverError> {
    let mut updates = Vec::new();

    for swap in &swap_batch.events {
        if let Some((token_id, is_token0)) = pool_token_map.get(&swap.pool_id) {
            let (native_amount, token_amount) = extract_amounts(swap, *is_token0);
            let price = compute_price_from_amounts(&native_amount, &token_amount);

            updates.push(PriceUpdate {
                token_id: token_id.clone(),
                price,
                volume: native_amount,
                block_number: swap.block_number,
                tx_hash: swap.tx_hash.clone(),
            });
        }
    }

    // Always send the batch (even if empty) so the receive side can
    // call mark_completed and advance its block progress cursor.
    let batch = EventBatch::new(
        updates,
        swap_batch.from_block,
        swap_batch.to_block,
    );
    tx.send(batch)
        .await
        .map_err(|e| ObserverError::fatal(anyhow::anyhow!("Price channel send failed: {e}")))?;

    Ok(())
}

fn extract_amounts(swap: &RawSwapEvent, is_token0: bool) -> (String, String) {
    let amount0: i128 = swap.amount0.parse().unwrap_or(0);
    let amount1: i128 = swap.amount1.parse().unwrap_or(0);

    if is_token0 {
        (amount1.unsigned_abs().to_string(), amount0.unsigned_abs().to_string())
    } else {
        (amount0.unsigned_abs().to_string(), amount1.unsigned_abs().to_string())
    }
}

/// Compute price as native_amount / token_amount, adjusted for decimal difference.
/// The "native" side is USDC (6 decimals) and tokens have 18 decimals.
/// price = (usdc / 1e6) / (tokens / 1e18) = usdc * 1e12 / tokens
///
/// Bug 43 fix: Align with the WS price computation which correctly accounts
/// for the 12-decimal difference between USDC (6) and project tokens (18).
fn compute_price_from_amounts(native_amount: &str, token_amount: &str) -> String {
    let native: f64 = native_amount.parse().unwrap_or(0.0);
    let token: f64 = token_amount.parse().unwrap_or(1.0);
    if token == 0.0 {
        return "0".to_string();
    }
    let price = (native * 1e12) / token;
    format!("{price:.18}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_swap(amount0: &str, amount1: &str) -> RawSwapEvent {
        RawSwapEvent {
            pool_id: "0xpool".to_string(),
            sender: "0xsender".to_string(),
            amount0: amount0.to_string(),
            amount1: amount1.to_string(),
            sqrt_price_x96: "0".to_string(),
            liquidity: "0".to_string(),
            tick: 0,
            fee: 0,
            block_number: 1,
            tx_hash: "0xtx".to_string(),
        }
    }

    // ── extract_amounts tests ───────────────────────────────────────

    #[test]
    fn extract_amounts_token_is_token0() {
        // is_token0 = true => native = |amount1|, token = |amount0|
        let swap = make_swap("1000", "-500");
        let (native, token) = extract_amounts(&swap, true);
        assert_eq!(native, "500"); // |amount1| = 500
        assert_eq!(token, "1000"); // |amount0| = 1000
    }

    #[test]
    fn extract_amounts_token_is_token1() {
        // is_token0 = false => native = |amount0|, token = |amount1|
        let swap = make_swap("-300", "600");
        let (native, token) = extract_amounts(&swap, false);
        assert_eq!(native, "300"); // |amount0| = 300
        assert_eq!(token, "600"); // |amount1| = 600
    }

    #[test]
    fn extract_amounts_negative_values_absolute() {
        let swap = make_swap("-1000", "-2000");
        let (native, token) = extract_amounts(&swap, true);
        assert_eq!(native, "2000");
        assert_eq!(token, "1000");
    }

    #[test]
    fn extract_amounts_zero_values() {
        let swap = make_swap("0", "0");
        let (native, token) = extract_amounts(&swap, true);
        assert_eq!(native, "0");
        assert_eq!(token, "0");
    }

    #[test]
    fn extract_amounts_unparseable_defaults_to_zero() {
        let swap = make_swap("bad", "also_bad");
        let (native, token) = extract_amounts(&swap, false);
        assert_eq!(native, "0");
        assert_eq!(token, "0");
    }

    // ── compute_price_from_amounts tests ────────────────────────────

    #[test]
    fn compute_price_normal_case() {
        // 500 USDC-units / 1000 token-units with 1e12 decimal adjustment
        // = (500 * 1e12) / 1000 = 5e11
        let price = compute_price_from_amounts("500", "1000");
        let parsed: f64 = price.parse().unwrap();
        assert!((parsed - 5e11).abs() < 1e2);
    }

    #[test]
    fn compute_price_zero_token_returns_zero() {
        let price = compute_price_from_amounts("100", "0");
        assert_eq!(price, "0");
    }

    #[test]
    fn compute_price_zero_native_returns_zero() {
        let price = compute_price_from_amounts("0", "100");
        let parsed: f64 = price.parse().unwrap();
        assert!(parsed.abs() < 1e-6);
    }

    #[test]
    fn compute_price_unparseable_native_defaults_zero() {
        let price = compute_price_from_amounts("bad", "100");
        let parsed: f64 = price.parse().unwrap();
        assert!(parsed.abs() < 1e-6);
    }

    #[test]
    fn compute_price_unparseable_token_defaults_one() {
        // token defaults to 1.0 when unparseable
        // 42 * 1e12 / 1.0 = 42e12
        let price = compute_price_from_amounts("42", "bad");
        let parsed: f64 = price.parse().unwrap();
        assert!((parsed - 42e12).abs() < 1e3);
    }

    #[test]
    fn compute_price_has_18_decimal_places() {
        let price = compute_price_from_amounts("1", "3");
        // Should have 18 decimal places
        let decimal_part = price.split('.').nth(1).unwrap();
        assert_eq!(decimal_part.len(), 18);
    }

    // ── PriceUpdate struct test ─────────────────────────────────────

    #[test]
    fn price_update_clone() {
        let update = PriceUpdate {
            token_id: "tok1".to_string(),
            price: "1.5".to_string(),
            volume: "100".to_string(),
            block_number: 42,
            tx_hash: "0xabc".to_string(),
        };
        let cloned = update.clone();
        assert_eq!(cloned.token_id, "tok1");
        assert_eq!(cloned.price, "1.5");
        assert_eq!(cloned.volume, "100");
        assert_eq!(cloned.block_number, 42);
        assert_eq!(cloned.tx_hash, "0xabc");
    }
}
