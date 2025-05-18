use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::{Duration, Instant},
};
use tokio::io::{AsyncRead, ReadBuf};

pub struct ThrottledReader<R> {
    inner: R,
    bytes_per_sec: u64,
    last_reset: Instant,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
    bytes_sent_this_period: u64,
}

const LIMIT_INTERVAL: Duration = Duration::from_millis(100);

impl<R: AsyncRead + Unpin> ThrottledReader<R> {
    pub fn new(inner: R, bytes_per_sec: u64) -> Self {
        Self {
            inner,
            bytes_per_sec,
            last_reset: Instant::now(),
            sleep: None,
            bytes_sent_this_period: 0,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for ThrottledReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.last_reset.elapsed() >= LIMIT_INTERVAL {
            self.last_reset = Instant::now();
            self.bytes_sent_this_period = 0;
            self.sleep = None;
        }

        let bytes_per_period = (self.bytes_per_sec as f64 * LIMIT_INTERVAL.as_secs_f64()) as u64;

        let remaining = bytes_per_period.saturating_sub(self.bytes_sent_this_period);

        if remaining == 0 {
            let deadline = self.last_reset + LIMIT_INTERVAL;
            if self.sleep.is_none() {
                self.sleep = Some(Box::pin(tokio::time::sleep_until(deadline.into())));
            }

            let sleep = self.sleep.as_mut().unwrap();

            match sleep.as_mut().poll(cx) {
                Poll::Ready(_) => {
                    self.last_reset = Instant::now();
                    self.bytes_sent_this_period = 0;
                    self.sleep = None;
                }

                Poll::Pending => return Poll::Pending,
            }
        }

        let max_read = remaining.min(buf.remaining() as u64) as usize;
        let limit_buf = buf.initialize_unfilled_to(max_read);
        let mut limited_buf = ReadBuf::new(limit_buf);

        match Pin::new(&mut self.inner).poll_read(cx, &mut limited_buf) {
            Poll::Ready(Ok(())) => {
                let bytes_read = limited_buf.filled().len();
                buf.advance(bytes_read);
                self.bytes_sent_this_period += bytes_read as u64;
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}
