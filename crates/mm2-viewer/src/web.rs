//! Running in a browser: the GPU, which arrives after the window does,
//! and telling the page (`web/index.html`) how the start went.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use winit::window::Window;

type Gpu = (wgpu::Adapter, wgpu::Device, wgpu::Queue);

thread_local! {
    /// Whether the GPU has been asked for.
    pub static ASKED: Cell<bool> = const { Cell::new(false) };
    /// The window, its surface and the GPU, once the browser answers.
    static READY: RefCell<Option<(Arc<Window>, wgpu::Surface<'static>, Result<Gpu, String>)>> =
        const { RefCell::new(None) };
}

#[wasm_bindgen(js_namespace = mm2page)]
extern "C" {
    #[wasm_bindgen(js_name = started)]
    fn js_started();
    #[wasm_bindgen(js_name = fail)]
    fn js_fail(why: &str);
}

/// Asks the browser for the GPU, and wakes the window once it answers.
pub fn ask_gpu(window: Arc<Window>, instance: wgpu::Instance, surface: wgpu::Surface<'static>) {
    ASKED.with(|a| a.set(true));
    wasm_bindgen_futures::spawn_local(async move {
        let gpu = crate::gpu_from(&instance, Some(&surface)).await.map_err(|e| e.to_string());
        window.request_redraw();
        READY.with(|r| *r.borrow_mut() = Some((window, surface, gpu)));
    });
}

/// The window, surface and GPU once the browser has answered.
pub fn take_gpu() -> Option<(Arc<Window>, wgpu::Surface<'static>, Result<Gpu, String>)> {
    READY.with(|r| r.borrow_mut().take())
}

pub fn started() {
    js_started();
}

pub fn fail(why: &str) {
    web_sys::console::error_1(&why.into());
    js_fail(why);
}
