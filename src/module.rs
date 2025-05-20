use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

use tokio::{io::AsyncReadExt, net::TcpStream};

use crate::throttled_reader::ThrottledReader;

#[derive(Debug, Copy, Clone)]
pub struct MyStruct {
    value: u32,
}

impl MyStruct {
    #[tracing::instrument]
    pub fn new(value: u32) -> Self {
        Self { value }
    }

    #[tracing::instrument]
    pub fn do_fast_thing(&self) -> u32 {
        black_box(black_box(1) + black_box(1))
    }

    #[tracing::instrument]
    pub async fn do_sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    #[tracing::instrument]
    pub async fn do_network_thing(
        &self,
        addr: &str,
        write_limit: u64,
        bits_per_sec: u64,
    ) -> anyhow::Result<()> {
        let mut conn = TcpStream::connect(addr).await?;

        let input_stream = tokio::io::repeat(1).take(write_limit);
        let mut input_stream = ThrottledReader::new(input_stream, bits_per_sec / 8);
        tokio::io::copy(&mut input_stream, &mut conn).await?;

        Ok(())
    }

    #[tracing::instrument]
    pub async fn do_disk_thing(&self, write_limit: u64) -> anyhow::Result<()> {
        let mut tempfile = tokio::task::spawn_blocking(|| -> anyhow::Result<tokio::fs::File> {
            let tempfile = tempfile::tempfile()?;
            Ok(tokio::fs::File::from_std(tempfile))
        })
        .await??;

        // TODO: Maybe instead this should write chunks and flush once in a while?
        let mut input_stream = tokio::io::repeat(1).take(write_limit);
        tokio::io::copy(&mut input_stream, &mut tempfile).await?;

        Ok(())
    }

    #[tracing::instrument]
    pub async fn do_cpu_thing_block_worker(&self, duration: Duration, cpu_percent: f64) {
        black_box(do_cpu_thing(duration, cpu_percent));
    }

    #[tracing::instrument]
    pub async fn do_cpu_thing_spawn_blocking(&self, duration: Duration, cpu_percent: f64) {
        let current_span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _span = current_span.enter();
            black_box(do_cpu_thing(duration, cpu_percent));
        })
        .await
        .unwrap();
    }

    #[tracing::instrument]
    pub async fn do_cpu_thing_spawn_thread(&self, duration: Duration, cpu_percent: f64) {
        let (send, recv) = tokio::sync::oneshot::channel();
        let current_span = tracing::Span::current();
        std::thread::spawn(move || {
            let _span = current_span.enter();
            black_box(do_cpu_thing(duration, cpu_percent));
            let _ = send.send(());
        });

        let _ = recv.await;
    }
}

#[tracing::instrument]
pub fn do_cpu_thing(duration: Duration, cpu_percent: f64) -> u64 {
    let end_time = Instant::now() + duration;

    let cycle = Duration::from_millis(10);
    let work_time = cycle.mul_f64(cpu_percent);
    let sleep_time = cycle - work_time;

    let mut counter = 0u64;

    while Instant::now() < end_time {
        let work_start = Instant::now();

        while Instant::now() - work_start < work_time {
            let mut hasher = Sha256::new();
            hasher.update(counter.to_le_bytes());
            let _ = hasher.finalize();
            counter = counter.wrapping_add(1);
        }

        if sleep_time > Duration::ZERO {
            std::thread::sleep(sleep_time);
        }
    }

    counter
}
