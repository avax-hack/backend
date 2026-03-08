use std::sync::Arc;

use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;

use openlaunch_shared::contracts::lp_manager::ILpManager;

use crate::cache::PriceCache;
use crate::event::EventProducers;
use crate::event::core::{SubscriptionKey, WsEvent};

/// Handle a raw log from the LpManager contract. Parses the event and forwards
/// it to the appropriate event producers.
pub fn handle_pool_log(
    log: &Log,
    producers: &Arc<EventProducers>,
    _price_cache: &Arc<PriceCache>,
) -> anyhow::Result<()> {
    let topics = log.topics();
    if topics.is_empty() {
        return Ok(());
    }

    let signature = topics[0];

    if signature == ILpManager::LiquidityAllocated::SIGNATURE_HASH {
        let decoded = log.log_decode::<ILpManager::LiquidityAllocated>()?;
        handle_liquidity_allocated(&decoded.inner.data, producers);
    } else if signature == ILpManager::FeesCollected::SIGNATURE_HASH {
        let decoded = log.log_decode::<ILpManager::FeesCollected>()?;
        handle_fees_collected(&decoded.inner.data, producers);
    }

    Ok(())
}

fn handle_liquidity_allocated(
    event: &ILpManager::LiquidityAllocated,
    producers: &Arc<EventProducers>,
) {
    let token = format!("{:#x}", event.token);
    let pool = format!("{:#x}", event.pool);

    let data = serde_json::json!({
        "type": "LIQUIDITY_ALLOCATED",
        "token": token,
        "pool": pool,
        "token_amount": event.tokenAmount.to_string(),
        "tick_lower": event.tickLower,
        "tick_upper": event.tickUpper,
    });

    // Publish to trade channel for the token.
    let trade_key = SubscriptionKey::Trade(token.clone()).to_channel_key();
    producers.trade.publish(&trade_key, WsEvent {
        method: "trade_subscribe".to_string(),
        data: data.clone(),
    });

    // Publish to new_content as this is a significant pool event.
    let new_content_key = SubscriptionKey::NewContent.to_channel_key();
    producers.new_content.publish(&new_content_key, WsEvent {
        method: "new_content_subscribe".to_string(),
        data,
    });

    tracing::info!(token = %token, pool = %pool, "LiquidityAllocated event forwarded");
}

fn handle_fees_collected(
    event: &ILpManager::FeesCollected,
    producers: &Arc<EventProducers>,
) {
    let token = format!("{:#x}", event.token);

    let data = serde_json::json!({
        "type": "FEES_COLLECTED",
        "token": token,
        "amount0": event.amount0.to_string(),
        "amount1": event.amount1.to_string(),
    });

    let trade_key = SubscriptionKey::Trade(token.clone()).to_channel_key();
    producers.trade.publish(&trade_key, WsEvent {
        method: "trade_subscribe".to_string(),
        data,
    });
}
