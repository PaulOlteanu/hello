use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::{response::IntoResponse, routing::get, Router};
use metrics_exporter_prometheus::PrometheusBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/", get(handler))
        .route("/slow", get(slow_handler))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    PrometheusBuilder::new()
        .with_http_listener(SocketAddr::new(IpAddr::from([0, 0, 0, 0]), 6666))
        .add_global_label("app", "hello")
        .install()
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    })
    .await
    .unwrap();
}

#[tracing::instrument]
async fn handler() -> impl IntoResponse {
    metrics::counter!("http_requests_total", "path" => "/").increment(1);
    "Hello World"
}

#[tracing::instrument]
async fn slow_handler() -> impl IntoResponse {
    metrics::counter!("http_requests_total", "path" => "/slow").increment(1);
    tokio::time::sleep(Duration::from_secs(2)).await;
    "slow"
}
