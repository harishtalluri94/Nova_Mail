use crate::{handlers, AppState};
use axum::{
    routing::{delete, get, post, put},
    Router,
};

pub fn api_routes(state: AppState) -> Router {
    Router::new()
        // Tenants
        .route("/api/v1/tenants", get(handlers::tenants::list_tenants))
        .route("/api/v1/tenants", post(handlers::tenants::create_tenant))
        .route(
            "/api/v1/tenants/:id",
            get(handlers::tenants::get_tenant),
        )
        .route(
            "/api/v1/tenants/:id",
            put(handlers::tenants::update_tenant),
        )
        .route(
            "/api/v1/tenants/:id",
            delete(handlers::tenants::delete_tenant),
        )
        // Domains
        .route(
            "/api/v1/tenants/:tenant_id/domains",
            get(handlers::domains::list_domains),
        )
        .route(
            "/api/v1/tenants/:tenant_id/domains",
            post(handlers::domains::create_domain),
        )
        .route(
            "/api/v1/domains/:id",
            get(handlers::domains::get_domain),
        )
        .route(
            "/api/v1/domains/:id/dkim",
            post(handlers::domains::rotate_dkim),
        )
        .route(
            "/api/v1/domains/:id/verify",
            post(handlers::domains::verify_domain),
        )
        // Users
        .route(
            "/api/v1/tenants/:tenant_id/users",
            get(handlers::users::list_users),
        )
        .route(
            "/api/v1/tenants/:tenant_id/users",
            post(handlers::users::create_user),
        )
        .route("/api/v1/users/:id", get(handlers::users::get_user))
        .route("/api/v1/users/:id", put(handlers::users::update_user))
        .route(
            "/api/v1/users/:id",
            delete(handlers::users::delete_user),
        )
        // Billing (Stripe integration)
        .route(
            "/api/v1/billing/webhook",
            post(handlers::billing::stripe_webhook),
        )
        .with_state(state)
}

pub fn health_routes(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::health::liveness))
        .route("/readyz", get(handlers::health::readiness))
        .route("/metrics", get(handlers::health::metrics))
        .with_state(state)
}
