use std::net::{Ipv4Addr, SocketAddr};

use metrics_exporter_prometheus::PrometheusBuilder;
use serde::Deserialize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use url::Url;

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    http_port: u16,
    devnull_port: u16,
    metrics_port: u16,

    victorialogs_host: Option<String>,
    victorialogs_port: Option<u16>,

    kubernetes_node_name: Option<String>,
    kubernetes_namespace: Option<String>,
    kubernetes_pod_name: Option<String>,
    kubernetes_pod_ip: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_port: 3000,
            devnull_port: 3001,
            metrics_port: 6666,

            victorialogs_host: None,
            victorialogs_port: None,

            kubernetes_node_name: None,
            kubernetes_namespace: None,
            kubernetes_pod_name: None,
            kubernetes_pod_ip: None,
        }
    }
}

fn get_victorialogs_logger(config: &Config) -> Option<tracing_loki::Layer> {
    match (&config.victorialogs_host, config.victorialogs_port) {
        (Some(host), Some(port)) => {
            // TODO: If kubernetes env vars are set, add those to the vl-stream-fields header
            let mut loki_builder = tracing_loki::builder()
                .http_header("VL-Msg-Field", "message")
                .unwrap()
                .http_header("VL-Stream-Fields", "app")
                .unwrap()
                .label("app", "hello")
                .unwrap();

            if let Some(node_name) = &config.kubernetes_node_name {
                loki_builder = loki_builder
                    .extra_field("kubernetes_node_name", node_name)
                    .unwrap();
            }
            if let Some(namespace) = &config.kubernetes_namespace {
                loki_builder = loki_builder
                    .extra_field("kubernetes_namespace", namespace)
                    .unwrap();
            }
            if let Some(pod_name) = &config.kubernetes_pod_name {
                loki_builder = loki_builder
                    .extra_field("kubernetes_pod_name", pod_name)
                    .unwrap();
            }
            if let Some(pod_ip) = &config.kubernetes_pod_ip {
                loki_builder = loki_builder
                    .extra_field("kubernetes_pod_ip", pod_ip)
                    .unwrap();
            }

            let (loki_layer, task) = loki_builder
                .build_url(Url::parse(&format!("http://{host}:{port}/insert/")).unwrap())
                .unwrap();

            tokio::spawn(task);

            Some(loki_layer)
        }
        _ => None,
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

    let loki_layer = get_victorialogs_logger(&config);

    let filter_fmt = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        format!(
            "info,{}=debug,tower_http=debug,axum::rejection=trace",
            env!("CARGO_CRATE_NAME")
        )
        .into()
    });
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_thread_names(true)
        .with_filter(filter_fmt);

    tracing_subscriber::registry()
        .with(loki_layer)
        .with(fmt_layer)
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
