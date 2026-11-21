//! Work done beside the game, such as loading a track: on a thread, or in
//! a browser, which has none, in one go once the loading screen is up.

#[cfg(not(target_arch = "wasm32"))]
pub struct Job<T>(std::thread::JoinHandle<T>);

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self {
        Job(std::thread::spawn(f))
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
    done: std::cell::RefCell<Option<T>>,
    /// Frames drawn since it was started.
    waited: std::cell::Cell<u32>,
}

#[cfg(target_arch = "wasm32")]
impl<T: 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + 'static) -> Self {
        Job {
            work: std::cell::RefCell::new(Some(Box::new(f))),
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
        let work = self.work.borrow_mut().take();
        *self.done.borrow_mut() = work.map(|f| f());
        true
    }

    pub fn join(self) -> std::thread::Result<T> {
        self.is_finished();
        self.done.into_inner().ok_or_else(|| Box::new("the job never ran") as _)
    }
}
