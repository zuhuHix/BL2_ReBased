//! Owner: lane H1. A std-only way to wait for a future on the current thread, for wgpu's async calls
//! (`request_adapter`, `request_device`). On the native backends those futures are ready on their first poll; the
//! loop below also handles one that is not, by parking the thread until its waker unparks it.
//!
//! AI-assisted (Claude Code). No third-party executor (pollster) on purpose: the maintainer's approval of
//! 2026-11-16 covers wgpu and winit only.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

/// Wakes the thread that is blocked in [`block_on`].
struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Polls `future` on this thread until it is ready. Between polls the thread parks; a spurious unpark only costs one
/// more poll.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::Mutex;

    #[test]
    fn a_ready_future_returns_its_value() {
        assert_eq!(block_on(async { 7 }), 7);
    }

    /// Pending on the first poll; wakes itself, so the second poll is ready.
    struct YieldOnce(bool);

    impl Future for YieldOnce {
        type Output = &'static str;
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.0 {
                Poll::Ready("second poll")
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    #[test]
    fn a_future_that_wakes_itself_is_polled_again() {
        assert_eq!(block_on(YieldOnce(false)), "second poll");
    }

    type Shared = Arc<Mutex<(Option<u32>, Option<Waker>)>>;

    /// Ready once another thread has set the value and called the stored waker.
    struct Slot(Shared);

    impl Future for Slot {
        type Output = u32;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
            let mut guard = self.0.lock().unwrap();
            match guard.0 {
                Some(value) => Poll::Ready(value),
                None => {
                    guard.1 = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }
    }

    #[test]
    fn a_future_woken_from_another_thread_completes() {
        let shared: Shared = Arc::new(Mutex::new((None, None)));
        let writer = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            loop {
                let mut guard = writer.lock().unwrap();
                if let Some(waker) = guard.1.take() {
                    guard.0 = Some(42);
                    drop(guard);
                    waker.wake();
                    return;
                }
                drop(guard);
                thread::yield_now();
            }
        });
        assert_eq!(block_on(Slot(shared)), 42);
        handle.join().unwrap();
    }
}
