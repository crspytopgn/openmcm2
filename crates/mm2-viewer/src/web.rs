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
    #[wasm_bindgen(js_name = trackStage, catch)]
    fn js_track_stage(stage: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = trackFailed, catch)]
    fn js_track_failed(name: &str, why: &str) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = building)]
    fn js_building(on: bool);
    /// The graphics device was lost: the page starts the game again.
    #[wasm_bindgen(js_name = gpuLost, catch)]
    fn js_gpu_lost(why: &str) -> Result<(), JsValue>;
    /// The steps of effects this computer needs set aside (dynres), to
    /// start with next time; a page without it is left alone.
    #[wasm_bindgen(js_name = autoShed, catch)]
    fn js_auto_shed(steps: u32) -> Result<(), JsValue>;
    /// The pad named `id` (or the first that can) runs its heavy and
    /// light motors at these strengths, 0 to 1.
    #[wasm_bindgen(js_name = rumble, catch)]
    fn js_rumble(id: &str, strong: f32, weak: f32) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = canRumble, catch)]
    fn js_can_rumble(id: &str) -> Result<bool, JsValue>;
    /// Saves the canvas as a PNG to the downloads; false if it can't.
    /// A box is being typed in (its text), or none (null): a phone's
    /// keyboard comes up for it.
    #[wasm_bindgen(js_name = typing, catch)]
    fn js_typing(text: Option<String>) -> Result<(), JsValue>;
    #[wasm_bindgen(js_name = savePicture, catch)]
    fn js_save_picture() -> Result<bool, JsValue>;
}

/// Whether the browser can vibrate the pad named `id` (gilrs can't here).
pub fn can_rumble(id: &str) -> bool {
    js_can_rumble(id).unwrap_or(false)
}

/// Runs the pad's motors, telling the page only when the strengths change.
pub fn rumble(id: Option<&str>, strength: [f32; 2]) {
    thread_local!(static WAS: Cell<[f32; 2]> = const { Cell::new([0.0; 2]) });
    let Some(id) = id else {
        if WAS.with(|w| w.replace([0.0; 2])) != [0.0; 2] {
            let _ = js_rumble("", 0.0, 0.0);
        }
        return;
    };
    let was = WAS.with(Cell::get);
    let off = strength == [0.0; 2];
    let moved = strength.iter().zip(was).any(|(s, w)| (s - w).abs() > 0.02);
    if moved || (off && was != [0.0; 2]) {
        WAS.with(|w| w.set(strength));
        let _ = js_rumble(id, strength[0], strength[1]);
    }
}

/// Photo mode's picture: the canvas just shown, to the downloads.
pub fn save_picture() -> bool {
    js_save_picture().unwrap_or(false)
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

/// A track couldn't be loaded (the game goes back to the menus): the
/// page says so, rather than the race silently not starting.
pub fn track_failed(name: &str, why: &str) {
    web_sys::console::error_1(&format!("couldn't load the track {name}: {why}").into());
    let _ = js_track_failed(name, why);
}

/// What a track's loading is doing now (`web/index.html` trackStage):
/// "build", "track", "ground", "scenery", "gpu".
pub fn track_stage(stage: &str) {
    let _ = js_track_stage(stage);
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

/// Tells the page when a box wants typing and what it holds, as it
/// changes.
pub fn typing(text: Option<String>) {
    thread_local!(static WAS: RefCell<Option<String>> = const { RefCell::new(None) });
    if WAS.with(|w| *w.borrow() != text) {
        WAS.with(|w| *w.borrow_mut() = text.clone());
        let _ = js_typing(text);
    }
}

/// The page calls this as its tab is hidden or closed: the time played
/// goes to the server now, as the tab may not come back.
#[wasm_bindgen(js_name = flushPlaytime)]
pub fn flush_playtime() {
    crate::playclock::flush();
}
