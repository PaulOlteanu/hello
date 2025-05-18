use std::net::{Ipv4Addr, SocketAddr};

use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    http_port: u16,
    devnull_port: u16,
    metrics_port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_port: 3000,
            devnull_port: 3001,
            metrics_port: 6666,
        }
    }
}

#[tokio::main]
async fn main() {
    let config: Config = config::Config::builder()
        .add_source(config::Environment::default())
        .build()
        .unwrap()
        .try_deserialize()
        .unwrap();

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

    PrometheusBuilder::new()
        .with_http_listener(SocketAddr::new(
            Ipv4Addr::UNSPECIFIED.into(),
            config.metrics_port,
        ))
        .add_global_label("app", "hello")
        .install()
        .unwrap();

    hello::run(config.http_port, config.devnull_port)
        .await
        .unwrap();
}
