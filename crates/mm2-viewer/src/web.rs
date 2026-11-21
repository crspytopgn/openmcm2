//! Running in a browser: the GPU, which arrives after the window does,
//! and telling the page (`web/index.html`) how the start went.

use std::cell::{Cell, RefCell};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use winit::window::Window;

type Gpu = (wgpu::Adapter, wgpu::Device, wgpu::Queue);

thread_local! {
    /// Whether the page was last told a race is on.
    static RACING: Cell<bool> = const { Cell::new(false) };
    /// The GPU instance, kept for as long as the game runs: the browser's
    /// GPU (Dawn) wants it alive while its device is.
    static INSTANCE: RefCell<Option<wgpu::Instance>> = const { RefCell::new(None) };
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
    #[wasm_bindgen(js_name = racing)]
    fn js_racing(on: bool);
    /// The loading screen's step: "gpu", then "graphics".
    #[wasm_bindgen(js_name = stage)]
    fn js_stage(step: &str);
    /// Resolves once the browser has shown the page as it is now.
    #[wasm_bindgen(js_name = painted)]
    fn js_painted() -> js_sys::Promise;
    #[wasm_bindgen(js_name = trackLoading)]
    fn js_track_loading(on: bool);
    #[wasm_bindgen(js_name = building)]
    fn js_building(on: bool);
    /// The graphics device was lost: the page starts the game again.
    #[wasm_bindgen(js_name = gpuLost, catch)]
    fn js_gpu_lost(why: &str) -> Result<(), JsValue>;
    /// The steps of effects this computer needs set aside (dynres), to
    /// start with next time; a page without it is left alone.
    #[wasm_bindgen(js_name = autoShed, catch)]
    fn js_auto_shed(steps: u32) -> Result<(), JsValue>;
}

/// The loading screen's step, for one the game takes itself.
pub fn stage(step: &str) {
    js_stage(step);
}

/// Tells the page how many steps of effects to set aside from the start
/// next time (`shed=` with `quality=auto`).
pub fn auto_shed(steps: u8) {
    let _ = js_auto_shed(steps as u32);
}

/// Asks the browser for the GPU, and wakes the window once it answers.
pub fn ask_gpu(window: Arc<Window>, instance: wgpu::Instance, surface: wgpu::Surface<'static>) {
    ASKED.with(|a| a.set(true));
    js_stage("gpu");
    wasm_bindgen_futures::spawn_local(async move {
        let gpu = crate::gpu_from(&instance, Some(&surface)).await.map_err(|e| e.to_string());
        if gpu.is_ok() {
            // Setting up the drawing holds the page: say so first.
            js_stage("graphics");
            let _ = wasm_bindgen_futures::JsFuture::from(js_painted()).await;
        }
        window.request_redraw();
        INSTANCE.with(|i| *i.borrow_mut() = Some(instance));
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

/// Tells the page the GPU was lost (the browser reset it, often for want
/// of graphics memory): nothing can be drawn again, so rather than leave a
/// frozen picture the page starts the game again.
pub fn gpu_lost(why: &str) {
    let _ = js_gpu_lost(why);
}

/// Tells the page a track is loading (the page can't draw while it does).
pub fn track_loading(on: bool) {
    js_track_loading(on);
}

/// Tells the page when the Course Builder is open, so its buttons over
/// the game's corners keep out of the builder's way.
pub fn building(on: bool) {
    thread_local!(static BUILDING: Cell<bool> = const { Cell::new(false) });
    if BUILDING.with(|b| b.replace(on)) != on {
        js_building(on);
    }
}

/// Tells the page when a race starts and ends, so it shows its touch
/// buttons only while riding.
pub fn racing(on: bool) {
    if RACING.with(|r| r.replace(on)) != on {
        js_racing(on);
    }
}

/// The page calls this as its tab is hidden or closed: the time played
/// goes to the server now, as the tab may not come back.
#[wasm_bindgen(js_name = flushPlaytime)]
pub fn flush_playtime() {
    crate::playclock::flush();
}
