use std::sync::Arc;

use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;

use crate::cache::PriceCache;
use crate::candle::CandleManager;
use crate::event::EventProducers;
use crate::event::core::{SubscriptionKey, WsEvent};
use crate::stream::update_and_broadcast_candles;

use super::stream::PoolMapping;

// Uniswap V4 PoolManager Swap event.
sol! {
    event Swap(
        bytes32 indexed id,
        address indexed sender,
        int128 amount0,
        int128 amount1,
        uint160 sqrtPriceX96,
        uint128 liquidity,
        int24 tick,
        uint24 fee
    );
}

/// Handle a raw Swap log from the PoolManager contract.
pub async fn handle_swap_log(
    log: &Log,
    mappings: &[PoolMapping],
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db_pool: &sqlx::PgPool,
) -> anyhow::Result<()> {
    let topics = log.topics();
    if topics.is_empty() {
        return Ok(());
    }

    let signature = topics[0];
    if signature != Swap::SIGNATURE_HASH {
        return Ok(());
    }

    let decoded = log.log_decode::<Swap>()?;
    let event = &decoded.inner;

    let pool_id = format!("{:#x}", event.id);
    let mapping = match mappings.iter().find(|m| m.pool_id == pool_id) {
        Some(m) => m,
        None => return Ok(()), // Unknown pool, skip
    };

    let amount0: i128 = event.amount0;
    let amount1: i128 = event.amount1;

    // Uniswap V4 sign convention (from Pool.sol):
    //   amount > 0 → user RECEIVES (gains) that token → BUY
    //   amount < 0 → user SENDS (pays) that token → SELL
    let (usdc_amount, token_amount, event_type): (u128, u128, &str) = if mapping.is_token0 {
        let token_amt = amount0.unsigned_abs();
        let usdc_amt = amount1.unsigned_abs();
        let evt = if amount0 > 0 { "BUY" } else { "SELL" };
        (usdc_amt, token_amt, evt)
    } else {
        let token_amt = amount1.unsigned_abs();
        let usdc_amt = amount0.unsigned_abs();
        let evt = if amount1 > 0 { "BUY" } else { "SELL" };
        (usdc_amt, token_amt, evt)
    };

    if token_amount == 0 {
        return Ok(());
    }

    // price = (usdc / 1e6) / (token / 1e18) = usdc * 1e12 / token
    // Use BigDecimal for precision with large amounts.
    let price_str = {
        use bigdecimal::BigDecimal;
        use std::str::FromStr;
        let usdc_bd = BigDecimal::from(usdc_amount);
        let token_bd = BigDecimal::from(token_amount);
        let scale = BigDecimal::from_str("1000000000000").unwrap(); // 1e12
        let price_bd = (usdc_bd * scale) / token_bd;
        format!("{}", price_bd.round(18))
    };
    let price: f64 = price_str.parse().unwrap_or(0.0);
    let volume = usdc_amount as f64;
    let token_id = &mapping.token_id;
    price_cache.set_price(token_id, price_str.clone());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    update_and_broadcast_candles(token_id, price, volume, now, candle_mgr, producers);

    // Broadcast trade event
    let token_lower = token_id.to_lowercase();
    let buyer = format!("{:#x}", event.sender);
    let usdc_display = openlaunch_shared::utils::price::wei_to_display(&usdc_amount.to_string(), 6)
        .unwrap_or_else(|_| usdc_amount.to_string());
    let token_display = openlaunch_shared::utils::price::wei_to_display(&token_amount.to_string(), 18)
        .unwrap_or_else(|_| token_amount.to_string());
    let trade_data = serde_json::json!({
        "type": "TRADE",
        "token": token_lower,
        "buyer": buyer,
        "event_type": event_type,
        "usdc_amount": usdc_display,
        "token_amount": token_display,
    });
    let trade_key = SubscriptionKey::Trade(token_lower.clone()).to_channel_key();
    producers.trade.publish(
        &trade_key,
        WsEvent {
            method: "trade_subscribe".to_string(),
            data: trade_data,
        },
    );

    // Broadcast price update
    let price_data = serde_json::json!({
        "type": "PRICE_UPDATE",
        "token_id": token_lower,
        "usdc_amount": usdc_display,
        "token_amount": token_display,
        "price": price_str,
    });
    let price_key = SubscriptionKey::Price(token_lower.clone()).to_channel_key();
    producers.price.publish(
        &price_key,
        WsEvent {
            method: "price_subscribe".to_string(),
            data: price_data,
        },
    );

    // Insert swap into DB so REST API can serve swap history
    let tx_hash = log.transaction_hash
        .map(|h| format!("{:#x}", h))
        .unwrap_or_default();
    let block_number = log.block_number.unwrap_or(0) as i64;
    if let Err(e) = openlaunch_shared::db::postgres::controller::swap::insert(
        db_pool,
        &token_lower,
        &buyer,
        event_type,
        &usdc_display,
        &token_display,
        &price_str,
        &usdc_display,
        &tx_hash,
        block_number,
        now,
    ).await {
        tracing::warn!(error = %e, "Failed to insert swap into DB");
    }

    tracing::info!(
        token = %token_lower,
        event_type = %event_type,
        price = %price_str,
        "DEX Swap event forwarded to chart, trade, and price channels"
    );

    Ok(())
}
