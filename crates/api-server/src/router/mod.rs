use axum::{middleware, Router};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::cors::cors_layer;
use crate::middleware::auth::session_middleware;
use crate::middleware::rate_limit::rate_limit_middleware;
use crate::openapi::ApiDoc;
use crate::state::AppState;

pub mod auth;
pub mod project;
pub mod milestone;
pub mod token;
pub mod trade;
pub mod profile;
pub mod builder;
pub mod metadata;
pub mod health;

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .nest("/auth", auth::router())
        .nest("/project", project::router())
        .nest("/milestone", milestone::router())
        .nest("/token", token::router())
        .nest("/order", token::order_router())
        .nest("/trend", token::trend_router())
        .nest("/trade", trade::router())
        .nest("/profile", profile::router())
        .nest("/account", profile::account_router())
        .nest("/builder", builder::router())
        .nest("/metadata", metadata::router())
        .merge(health::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ))
        .layer(cors_layer())
        .with_state(state)
}
