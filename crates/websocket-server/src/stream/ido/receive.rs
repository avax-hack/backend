use std::sync::Arc;

use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;
use sqlx::PgPool;

use openlaunch_shared::contracts::ido::IIDO;
use openlaunch_shared::db::postgres::controller::project as project_ctrl;

use crate::cache::PriceCache;
use crate::candle::CandleManager;
use crate::event::EventProducers;
use crate::event::core::{SubscriptionKey, WsEvent};

/// Handle a raw log from the IDO contract. Parses the event and forwards
/// it to the appropriate event producers.
pub async fn handle_ido_log(
    log: &Log,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db_pool: &PgPool,
) -> anyhow::Result<()> {
    let topics = log.topics();
    if topics.is_empty() {
        return Ok(());
    }

    let signature = topics[0];

    if signature == IIDO::ProjectCreated::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::ProjectCreated>()?;
        handle_project_created(&decoded.inner.data, producers);
    } else if signature == IIDO::TokensPurchased::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::TokensPurchased>()?;
        handle_tokens_purchased(&decoded.inner.data, producers, price_cache, candle_mgr, db_pool).await;
    } else if signature == IIDO::Graduated::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::Graduated>()?;
        handle_graduated(&decoded.inner.data, producers);
    } else if signature == IIDO::MilestoneApproved::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::MilestoneApproved>()?;
        handle_milestone_approved(&decoded.inner.data, producers);
    } else if signature == IIDO::ProjectFailed::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::ProjectFailed>()?;
        handle_project_failed(&decoded.inner.data, producers);
    } else if signature == IIDO::Refunded::SIGNATURE_HASH {
        let decoded = log.log_decode::<IIDO::Refunded>()?;
        handle_refunded(&decoded.inner.data, producers);
    }

    Ok(())
}

fn handle_project_created(event: &IIDO::ProjectCreated, producers: &Arc<EventProducers>) {
    let token = format!("{:#x}", event.token);
    let data = serde_json::json!({
        "type": "PROJECT_CREATED",
        "token": token,
        "creator": format!("{:#x}", event.creator),
        "name": event.name,
        "symbol": event.symbol,
        "token_uri": event.tokenURI,
        "ido_token_amount": event.idoTokenAmount.to_string(),
        "token_price": event.tokenPrice.to_string(),
        "deadline": event.deadline.to_string(),
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data: data.clone(),
    });

    let new_content_key = SubscriptionKey::NewContent.to_channel_key();
    producers.new_content.publish(&new_content_key, WsEvent {
        method: "new_content_subscribe".to_string(),
        data,
    });

    tracing::info!(token = %token, "ProjectCreated event forwarded");
}

async fn handle_tokens_purchased(
    event: &IIDO::TokensPurchased,
    producers: &Arc<EventProducers>,
    price_cache: &Arc<PriceCache>,
    candle_mgr: &Arc<CandleManager>,
    db_pool: &PgPool,
) {
    let token = format!("{:#x}", event.token);
    let buyer = format!("{:#x}", event.buyer);
    let usdc_amount = event.usdcAmount.to_string();
    let token_amount = event.tokenAmount.to_string();

    // Fetch market info from DB and add current purchase amount
    // (observer may not have processed this event yet)
    let market_info = match project_ctrl::fetch_market_snapshot(db_pool, &token).await {
        Ok(Some(snapshot)) => {
            let target_f: f64 = snapshot.target_raise.parse().unwrap_or(0.0);
            let db_committed: f64 = snapshot.total_committed.parse().unwrap_or(0.0);
            let current_usdc: f64 = {
                use openlaunch_shared::utils::price::wei_to_display;
                wei_to_display(&usdc_amount, 6)
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0)
            };
            let total_committed = db_committed + current_usdc;
            let funded_percent = if target_f > 0.0 {
                (total_committed / target_f * 100.0).min(100.0)
            } else {
                0.0
            };
            serde_json::json!({
                "project_id": snapshot.project_id,
                "status": snapshot.status,
                "target_raise": snapshot.target_raise,
                "total_committed": format!("{total_committed}"),
                "funded_percent": funded_percent,
                "investor_count": snapshot.investor_count + 1,
            })
        }
        _ => serde_json::json!(null),
    };

    // Publish to project channel with market_info.
    let project_data = serde_json::json!({
        "type": "TOKENS_PURCHASED",
        "token": token,
        "buyer": buyer,
        "usdc_amount": usdc_amount,
        "token_amount": token_amount,
        "market_info": market_info,
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data: project_data,
    });

    // Publish to trade channel so trade subscribers receive buy events.
    let trade_data = serde_json::json!({
        "type": "TRADE",
        "token": token,
        "buyer": buyer,
        "event_type": "BUY",
        "usdc_amount": usdc_amount,
        "token_amount": token_amount,
    });

    let trade_key = SubscriptionKey::Trade(token.clone()).to_channel_key();
    producers.trade.publish(&trade_key, WsEvent {
        method: "trade_subscribe".to_string(),
        data: trade_data,
    });

    // Compute a basic price (usdc per token) and publish to the price channel.
    let price_str = compute_price(&usdc_amount, &token_amount);
    price_cache.set_price(&token, price_str.clone());

    let price_data = serde_json::json!({
        "type": "PRICE_UPDATE",
        "token_id": token,
        "usdc_amount": usdc_amount,
        "token_amount": token_amount,
        "price": price_str,
    });

    let price_key = SubscriptionKey::Price(token.clone()).to_channel_key();
    producers.price.publish(&price_key, WsEvent {
        method: "price_subscribe".to_string(),
        data: price_data,
    });

    // Update in-memory candles and broadcast to all intervals
    let price_f64: f64 = price_str.parse().unwrap_or(0.0);
    let volume_f64: f64 = usdc_amount.parse().unwrap_or(0.0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    crate::stream::update_and_broadcast_candles(&token, price_f64, volume_f64, now, candle_mgr, producers);

    tracing::info!(token = %token, buyer = %buyer, "TokensPurchased event forwarded to project, trade, and price channels");
}

fn handle_graduated(event: &IIDO::Graduated, producers: &Arc<EventProducers>) {
    let token = format!("{:#x}", event.token);
    let data = serde_json::json!({
        "type": "GRADUATED",
        "token": token,
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data: data.clone(),
    });

    let new_content_key = SubscriptionKey::NewContent.to_channel_key();
    producers.new_content.publish(&new_content_key, WsEvent {
        method: "new_content_subscribe".to_string(),
        data,
    });

    tracing::info!(token = %token, "Graduated event forwarded");
}

fn handle_milestone_approved(
    event: &IIDO::MilestoneApproved,
    producers: &Arc<EventProducers>,
) {
    let token = format!("{:#x}", event.token);
    let data = serde_json::json!({
        "type": "MILESTONE_APPROVED",
        "token": token,
        "milestone_index": event.milestoneIndex.to_string(),
        "usdc_released": event.usdcReleased.to_string(),
    });

    let milestone_key = SubscriptionKey::Milestone(token.clone()).to_channel_key();
    producers.milestone.publish(&milestone_key, WsEvent {
        method: "milestone_subscribe".to_string(),
        data: data.clone(),
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data,
    });
}

fn handle_project_failed(event: &IIDO::ProjectFailed, producers: &Arc<EventProducers>) {
    let token = format!("{:#x}", event.token);
    let data = serde_json::json!({
        "type": "PROJECT_FAILED",
        "token": token,
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data: data.clone(),
    });

    let new_content_key = SubscriptionKey::NewContent.to_channel_key();
    producers.new_content.publish(&new_content_key, WsEvent {
        method: "new_content_subscribe".to_string(),
        data,
    });
}

fn handle_refunded(event: &IIDO::Refunded, producers: &Arc<EventProducers>) {
    let token = format!("{:#x}", event.token);
    let data = serde_json::json!({
        "type": "REFUNDED",
        "token": token,
        "buyer": format!("{:#x}", event.buyer),
        "tokens_burned": event.tokensBurned.to_string(),
        "usdc_returned": event.usdcReturned.to_string(),
    });

    let project_key = SubscriptionKey::Project(token.clone()).to_channel_key();
    producers.project.publish(&project_key, WsEvent {
        method: "project_subscribe".to_string(),
        data,
    });
}

/// Compute a basic price as usdc_amount / token_amount using BigDecimal
/// to avoid f64 precision loss on large token amounts.
/// Both values are expected as decimal integer strings.
/// Returns "0" if inputs are invalid or token_amount is zero.
fn compute_price(usdc_amount: &str, token_amount: &str) -> String {
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    let Ok(usdc) = BigDecimal::from_str(usdc_amount) else {
        return "0".to_string();
    };
    let Ok(tokens) = BigDecimal::from_str(token_amount) else {
        return "0".to_string();
    };
    if tokens == BigDecimal::from(0) {
        return "0".to_string();
    }
    // USDC has 6 decimals, token has 18 decimals.
    // price = (usdc / 1e6) / (tokens / 1e18) = usdc * 1e12 / tokens
    let scale = BigDecimal::from_str("1000000000000").unwrap(); // 1e12
    let price = (usdc * scale) / tokens;
    // Round to 18 decimal places for consistent output.
    format!("{}", price.round(18))
}
