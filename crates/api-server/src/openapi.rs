use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "OpenLaunch API",
        description = "OpenLaunch IDO Platform REST API - Avalanche C-Chain",
        version = "0.1.0"
    ),
    paths(
        crate::router::auth::nonce,
        crate::router::auth::session,
        crate::router::auth::delete_session,
        crate::router::project::get_featured,
        crate::router::project::get_project,
        crate::router::project::create_project,
        crate::router::project::validate_symbol,
        crate::router::project::get_investors,
        crate::router::milestone::submit_milestone,
        crate::router::milestone::get_verification,
        crate::router::token::get_token,
        crate::router::token::get_token_list,
        crate::router::token::get_project_list,
        crate::router::token::get_trend,
        crate::router::trade::get_chart,
        crate::router::trade::get_swap_history,
        crate::router::trade::get_holders,
        crate::router::trade::get_market,
        crate::router::trade::get_metrics,
        crate::router::trade::get_quote,
        crate::router::profile::get_profile,
        crate::router::profile::get_hold_tokens,
        crate::router::profile::get_swap_history,
        crate::router::profile::get_ido_history,
        crate::router::profile::get_refund_history,
        crate::router::profile::get_portfolio,
        crate::router::profile::get_created_tokens,
        crate::router::profile::get_account,
        crate::router::builder::get_overview,
        crate::router::builder::get_stats,
        crate::router::metadata::upload_image,
        crate::router::metadata::upload_evidence,
        crate::router::health::health_check,
    ),
    components(schemas()),
    tags(
        (name = "auth", description = "Wallet authentication"),
        (name = "project", description = "IDO project management"),
        (name = "milestone", description = "Milestone verification"),
        (name = "token", description = "Token data"),
        (name = "trade", description = "Trading data"),
        (name = "profile", description = "User profile & portfolio"),
        (name = "builder", description = "Builder dashboard"),
        (name = "metadata", description = "File uploads"),
        (name = "health", description = "Health check"),
    )
)]
pub struct ApiDoc;
