use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use axum::{extract::Query, response::IntoResponse, routing::get, Router};
use serde::Deserialize;
use tokio::{io::AsyncReadExt, net::TcpListener};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::module::MyStruct;

pub async fn run(server_port: u16, devnull_port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(hello_handler))
        .route("/do_fast_thing", get(do_fast_thing))
        .route("/do_sleep", get(do_sleep))
        .route("/do_network_thing", get(do_network_thing))
        .route("/do_disk_thing", get(do_disk_thing))
        .route("/do_cpu_thing_block_worker", get(do_cpu_thing_block_worker))
        .route(
            "/do_cpu_thing_spawn_blocking",
            get(do_cpu_thing_spawn_blocking),
        )
        .route("/do_cpu_thing_spawn_thread", get(do_cpu_thing_spawn_thread))
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), server_port))
        .await
        .unwrap();

    let devnull = tokio::spawn(devnull_as_a_service(devnull_port));

    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    tokio::select! {
        res = devnull => {
            Ok(res??)
        }

        res = server => {
            Ok(res??)
        }
    }
}

async fn devnull_as_a_service(port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port))
        .await
        .unwrap();

    loop {
        let (mut conn, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf = [0; 4096];
            loop {
                let n = conn.read(&mut buf).await.unwrap();

                if n == 0 {
                    break;
                }
            }
        });
    }
}

#[tracing::instrument]
async fn hello_handler() -> impl IntoResponse {
    metrics::counter!("http_requests_total", "path" => "/").increment(1);
    info!("HELLO");

    "Hello World"
}

#[tracing::instrument]
async fn do_fast_thing() -> impl IntoResponse {
    let my_struct = MyStruct::new(1);
    my_struct.do_fast_thing();

    "done"
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct SleepQuery {
    duration: Option<f32>,
}

#[tracing::instrument]
async fn do_sleep(Query(query): Query<SleepQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(1);
    let sleep_duration = query.duration.unwrap_or(2.0);

    my_struct
        .do_sleep(Duration::from_secs_f32(sleep_duration))
        .await;

    "done"
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct AsyncOpQuery {
    amount: Option<u64>,
    kbps: Option<f64>,
}

#[tracing::instrument]
async fn do_network_thing(Query(query): Query<AsyncOpQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(2);
    let amount = query.amount.unwrap_or(1024 * 1024);
    let kbps = query.kbps.unwrap_or(1024.0);
    let bps = (kbps * 1024.0) as u64;

    my_struct
        .do_network_thing("127.0.0.1:3001", amount, bps)
        .await
        .unwrap();

    "done"
}

#[tracing::instrument]
async fn do_disk_thing(Query(query): Query<AsyncOpQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(3);
    let amount = query.amount.unwrap_or(1024 * 1024);

    my_struct.do_disk_thing(amount).await.unwrap();

    "done"
}

#[derive(Deserialize, Debug, Clone, Copy)]
struct CpuQuery {
    duration: Option<f64>,
    percent: Option<f64>,
}

#[tracing::instrument]
async fn do_cpu_thing_block_worker(Query(query): Query<CpuQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(4);
    let duration = Duration::from_secs_f64(query.duration.unwrap_or(2.0));
    let pct = query.percent.unwrap_or(0.5);
    my_struct.do_cpu_thing_block_worker(duration, pct).await;

    "done"
}

#[tracing::instrument]
async fn do_cpu_thing_spawn_blocking(Query(query): Query<CpuQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(5);
    let duration = Duration::from_secs_f64(query.duration.unwrap_or(2.0));
    let pct = query.percent.unwrap_or(0.5);
    my_struct.do_cpu_thing_spawn_blocking(duration, pct).await;

    "done"
}

#[tracing::instrument]
async fn do_cpu_thing_spawn_thread(Query(query): Query<CpuQuery>) -> impl IntoResponse {
    let my_struct = MyStruct::new(6);
    let duration = Duration::from_secs_f64(query.duration.unwrap_or(2.0));
    let pct = query.percent.unwrap_or(0.5);
    my_struct.do_cpu_thing_spawn_thread(duration, pct).await;

    "done"
}
