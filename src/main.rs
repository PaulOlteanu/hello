use std::net::{Ipv4Addr, SocketAddr};

use metrics_exporter_prometheus::PrometheusBuilder;
use opentelemetry::{trace::TracerProvider, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use serde::Deserialize;
use tracing_subscriber::{
    layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt, EnvFilter, Layer,
};
use url::Url;

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    http_port: u16,
    devnull_port: u16,
    metrics_port: u16,

    victorialogs_host: Option<String>,
    victorialogs_port: Option<u16>,

    opentelemetry_trace_host: Option<String>,
    opentelemetry_trace_port: Option<u16>,

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

            opentelemetry_trace_host: None,
            opentelemetry_trace_port: None,

            kubernetes_node_name: None,
            kubernetes_namespace: None,
            kubernetes_pod_name: None,
            kubernetes_pod_ip: None,
        }
    }
}

// TODO: Just have this use opentel too
fn get_victorialogs_logger(config: &Config) -> Option<tracing_loki::Layer> {
    let (Some(host), Some(port)) = (&config.victorialogs_host, config.victorialogs_port) else {
        return None;
    };

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

    let url = format!("http://{host}:{port}/insert/");
    let url = Url::parse(&url).unwrap();
    let (loki_layer, task) = loki_builder.build_url(url).unwrap();

    tokio::spawn(task);

    Some(loki_layer)
}

fn get_opentel_layer<S>(config: &Config) -> Option<impl Layer<S>>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    let (Some(host), Some(port)) = (
        &config.opentelemetry_trace_host,
        config.opentelemetry_trace_port,
    ) else {
        return None;
    };

    let url = format!("http://{host}:{port}/v1/traces");
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(url)
        .build()
        .unwrap();

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_attributes([KeyValue::new("service.name", "hello")])
                .build(),
        )
        .build();

    let tracer = provider.tracer("tracing-opentelemetry");

    // TODO: Better filter here
    let filter_otel = EnvFilter::new("info")
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());

    let layer = tracing_opentelemetry::layer()
        .with_tracer(tracer)
        .with_filter(filter_otel);

    Some(layer)
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

    let opentel_layer = get_opentel_layer(&config);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(loki_layer)
        .with(opentel_layer)
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
