//! Work done beside the game, such as loading a track: on a thread, or in
//! a browser, which has none, in one go once the loading screen is up.

#[cfg(not(target_arch = "wasm32"))]
pub struct Job<T>(std::thread::JoinHandle<T>);

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Runs work written in steps (`next_frame` between them) to its end.
pub fn block_on<T>(work: impl Future<Output = T>) -> T {
    let mut work = std::pin::pin!(work);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(out) = work.as_mut().poll(&mut cx) {
            return out;
        }
    }
}

/// Lets a browser draw a frame before the work goes on (it has no
/// threads, so long work runs between its frames, a step a frame).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn next_frame() -> impl Future<Output = ()> {
    struct Once(bool);
    impl Future for Once {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
            if std::mem::replace(&mut self.0, true) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }
    Once(false)
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        Job(std::thread::spawn(f))
    }

    /// Work written in steps, run to its end on a thread.
    pub fn spawn_steps<W: Future<Output = T>>(work: impl FnOnce() -> W + Send + 'static) -> Self {
        Job(std::thread::spawn(move || block_on(work())))
    }

    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }

    pub fn join(self) -> std::thread::Result<T> {
        self.0.join()
    }
}

#[cfg(target_arch = "wasm32")]
pub struct Job<T> {
    work: std::cell::RefCell<Option<Box<dyn FnOnce() -> T>>>,
    /// Or work written in steps: one step a frame.
    steps: std::cell::RefCell<Option<Pin<Box<dyn Future<Output = T>>>>>,
    done: std::cell::RefCell<Option<T>>,
    /// Frames drawn since it was started.
    waited: std::cell::Cell<u32>,
}

#[cfg(target_arch = "wasm32")]
impl<T: 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + 'static) -> Self {
        Job {
            work: std::cell::RefCell::new(Some(Box::new(f))),
            steps: std::cell::RefCell::new(None),
            done: std::cell::RefCell::new(None),
            waited: std::cell::Cell::new(0),
        }
    }

    /// Work written in steps (`next_frame` between them): a step runs
    /// each frame, so the loading screen keeps moving.
    pub fn spawn_steps<W: Future<Output = T> + 'static>(work: impl FnOnce() -> W + 'static) -> Self {
        Job {
            work: std::cell::RefCell::new(None),
            steps: std::cell::RefCell::new(Some(Box::pin(work()))),
            done: std::cell::RefCell::new(None),
            waited: std::cell::Cell::new(0),
        }
    }

    /// Asked once a frame: the work runs on the third ask, so the frames
    /// before show the loading screen, and once every file has been read
    /// into memory.
    pub fn is_finished(&self) -> bool {
        if self.done.borrow().is_some() {
            return true;
        }
        self.waited.set(self.waited.get() + 1);
        // The track's files may still be coming into memory (fs::all_read).
        if self.waited.get() < 3 || !crate::fs::all_read() {
            return false;
        }
        let mut steps = self.steps.borrow_mut();
        if let Some(work) = steps.as_mut() {
            let mut cx = Context::from_waker(Waker::noop());
            match work.as_mut().poll(&mut cx) {
                Poll::Ready(out) => {
                    *steps = None;
                    *self.done.borrow_mut() = Some(out);
                    return true;
                }
                Poll::Pending => return false,
            }
        }
        let work = self.work.borrow_mut().take();
        *self.done.borrow_mut() = work.map(|f| f());
        true
    }

    pub fn join(self) -> std::thread::Result<T> {
        // Work in steps is joined once finished (see `is_finished`).
        if self.steps.borrow().is_none() {
            self.is_finished();
        }
        self.done.into_inner().ok_or_else(|| Box::new("the job never ran") as _)
    }
}
