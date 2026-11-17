//! The port's own graphics upgrades, none of them the game's: anti-aliasing,
//! per-pixel lighting, shadows from the sun for every object, a longer view
//! with soft haze, HDR with bloom, ambient occlusion and a sun in the sky.
//!
//! Off unless asked for (`--enhanced`), so the game draws as it always did.
//! They change only the drawing: the riding, the camera and the clock are
//! the same either way. With them on, F3 switches between the enhanced look
//! and the original one, to compare.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// The colour the enhanced scene is drawn in before it is tone mapped.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Which upgrades are on.
#[derive(Debug, Clone, Copy)]
pub struct Enhanced {
    /// Samples a pixel for anti-aliasing (1 is none).
    pub msaa: u32,
    /// Per-pixel lighting, bumps from the textures, highlights.
    pub lighting: bool,
    /// The sun's shadows from every object.
    pub shadows: bool,
    /// The whole track drawn, fading into haze rather than ending at the fog.
    pub distance: bool,
    /// Bright light glows.
    pub bloom: bool,
    /// Ambient occlusion: corners and contacts darkened.
    pub ao: bool,
    /// A glow round the sun and haze along the horizon.
    pub sky: bool,
    /// The bolder look: colour, contrast and a vignette.
    pub grade: bool,
    /// The ground re-made from photo materials and lit by the sun.
    pub ground: bool,
    /// Grass growing on the grassy ground near the camera.
    pub grass: bool,
    /// A real sky with clouds in place of the painted one.
    pub clouds: bool,
    /// Tyre ruts that stay, and clouds of dust (spray on wet ground)
    /// behind the bikes.
    pub ruts: bool,
    pub dust: bool,
    /// Shiny paint and metal on the bikes, and smoother riders.
    pub bikes: bool,
    /// Shafts of sunlight past hills, trees and riders.
    pub shafts: bool,
    /// Replays filmed as through a lens: depth of field and motion blur.
    pub cinematic: bool,
    /// Smooth hills round the eye, drawn only.
    pub smooth: bool,
    /// Props along an outdoor course and a paddock by the start, drawn
    /// only.
    pub scenery: bool,
    /// Mud and dust caking the bikes and riders as they ride.
    pub mud: bool,
    /// Photo materials' grain (wood, metal, concrete, straw) on the
    /// objects (materials::surface).
    pub surfaces: bool,
    /// Trees and bushes swaying in the wind, their leaves lit through.
    pub trees: bool,
    /// Shadows sharp where they touch and softer further from what casts
    /// them.
    pub soft_shadows: bool,
    /// A gently darker, packed racing line that builds up over the race.
    pub wear: bool,
    /// Bikes and riders drawn rounder, from their own models.
    pub round: bool,
    /// Supercross stadiums lit by their lamps from above.
    pub floodlights: bool,
    /// At night, a stadium lit by its own lamp banks (crate::night).
    pub night_lights: bool,
    /// Ambient occlusion worked out from the horizons round each pixel
    /// (GTAO), truer in creases and corners than the plain estimate.
    pub gtao: bool,
    /// A short, sharp shadow where tyres, boots and props meet the ground,
    /// finer than the shadow maps can draw (screen space).
    pub contact: bool,
    /// Puddles and wet ground reflecting what is on screen (screen space);
    /// off when ray tracing draws the reflections.
    pub ssr: bool,
    /// The sky's light baked over the land once a race: dimmer in dips,
    /// under banks and between hills.
    pub skylight: bool,
    /// The sun's shadows traced as rays through the scene, in place of
    /// the shadow maps, and reflections in wet ground and glossy paint,
    /// where the GPU can (crate::rt); off unless asked.
    pub raytracing: bool,
    /// Temporal anti-aliasing: each frame drawn a fraction of a pixel
    /// apart and blended with the ones before, so fine textures and thin
    /// edges hold still (crate::post).
    pub taa: bool,
    /// How dirty the bikes start, 0 to 1.
    pub mud_start: f32,
    /// How strongly the new ground's detail shows, 0 to 1.
    pub detail: f32,
    pub weather: Weather,
    /// The hour the sun is placed at, 0 to 24; the scene's own sun if none.
    pub time: Option<f32>,
    /// The most pixels the 3D scene is drawn with: on a bigger screen
    /// (Retina, 4K) it is drawn smaller and stretched over it, the menus
    /// and race display still sharp. None for the screen's own.
    pub pixels: Option<u32>,
    /// Or a share of the screen's width and height to draw it at, in
    /// place of `pixels` (`resolution=85%`).
    pub scale: Option<f32>,
    /// Draws the scene smaller while frames come late, and back up once
    /// they keep time (crate::dynres): on with `quality=auto`.
    pub dynamic: bool,
}

/// The weather, for looks only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weather {
    Clear,
    Overcast,
    Rain,
}

impl Default for Enhanced {
    fn default() -> Self {
        Enhanced {
            msaa: 4,
            lighting: true,
            shadows: true,
            distance: true,
            // Off unless asked: a matter of taste (Michael's, for one).
            bloom: false,
            ao: true,
            sky: true,
            grade: false,
            ground: true,
            grass: true,
            clouds: false,
            ruts: false,
            dust: true,
            bikes: false,
            shafts: true,
            cinematic: false,
            smooth: true,
            scenery: false,
            mud: true,
            surfaces: true,
            trees: true,
            soft_shadows: true,
            wear: true,
            round: true,
            floodlights: true,
            night_lights: true,
            gtao: true,
            contact: true,
            ssr: true,
            skylight: true,
            raytracing: false,
            taa: false,
            mud_start: 0.0,
            detail: 0.5,
            weather: Weather::Clear,
            time: None,
            pixels: Some(QHD),
            scale: None,
            dynamic: false,
        }
    }
}

/// 2560 × 1440.
const QHD: u32 = 2560 * 1440;

/// The quality presets (`quality=`): what each turns off, and the most
/// pixels it draws the scene with.
fn preset(e: &mut Enhanced, name: &str) -> Result<(), String> {
    match name {
        "low" => {
            e.msaa = 1;
            e.ao = false;
            e.shafts = false;
            e.smooth = false;
            e.cinematic = false;
            e.grass = false;
            e.gtao = false;
            e.contact = false;
            e.ssr = false;
            // The plain 3×3 filter on the shadows' edges: the soft one's
            // 32 taps a pixel were a third of the frame.
            e.soft_shadows = false;
            e.pixels = Some(1280 * 720);
        }
        // Temporal anti-aliasing in place of MSAA: about as cheap as 2x, and
        // it rebuilds the 1080p-worth scene at the screen's own sharpness.
        "medium" => {
            e.msaa = 1;
            e.taa = true;
            e.shafts = false;
            e.smooth = false;
            e.gtao = false;
            e.ssr = false;
            e.pixels = Some(1920 * 1080);
        }
        // Every effect that is on unless turned off. (The trackside props
        // and crowd and the colour grade are in no preset: a matter of
        // taste, left to the player.)
        "high" => {}
        // High's look with ray tracing, at 2x anti-aliasing for the frame
        // rate the rays take.
        "raytraced" | "rt" => {
            e.msaa = 2;
            e.raytracing = true;
        }
        // Everything but ray tracing (its own preset, for the cards that
        // can), with lasting ruts.
        "ultra" => {
            e.msaa = 8;
            e.pixels = None;
            e.ruts = true;
        }
        // For the computer it's on: Medium until the graphics card is
        // known, then [`pick_auto`]'s.
        "auto" => {
            preset(e, "medium")?;
            e.dynamic = true;
        }
        _ => return Err(format!("quality must be auto, low, medium, high, raytraced or ultra, not {name}")),
    }
    Ok(())
}

impl Enhanced {
    /// The size to draw the 3D scene at on a `screen` of that size: the
    /// screen's own, or smaller with the same shape when it has more than
    /// [`Enhanced::pixels`].
    pub fn scene_size(&self, screen: (u32, u32)) -> (u32, u32) {
        if let Some(k) = self.scale {
            let k = k as f64;
            return (((screen.0 as f64 * k).round() as u32).max(1), ((screen.1 as f64 * k).round() as u32).max(1));
        }
        let have = screen.0 as f64 * screen.1 as f64;
        match self.pixels {
            Some(most) if have > most as f64 => {
                let k = (most as f64 / have).sqrt();
                (((screen.0 as f64 * k).round() as u32).max(1), ((screen.1 as f64 * k).round() as u32).max(1))
            }
            _ => screen,
        }
    }
}

impl Enhanced {
    /// From `--enhanced`'s list: each upgrade's name turns it off with a
    /// `no-` before it (`no-bloom,no-ao`), and `msaa=N` sets the samples
    /// (1, 2, 4 or 8).
    pub fn parse(list: &str) -> Result<Enhanced, String> {
        let mut e = Enhanced::default();
        // An hour or weather of its own needs the drawn sky, unless asked
        // not to have it: the scene's sky box shows its own.
        let wants_sky = list.contains("time=") || list.contains("weather=");
        e.clouds = wants_sky && !list.contains("no-clouds");
        for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (off, name) = match item.strip_prefix("no-") {
                Some(name) => (true, name),
                None => (false, item),
            };
            if let Some(w) = name.strip_prefix("weather=") {
                e.weather = match w {
                    "clear" => Weather::Clear,
                    "overcast" | "cloudy" => Weather::Overcast,
                    "rain" | "wet" => Weather::Rain,
                    _ => return Err(format!("weather must be clear, overcast or rain, not {w}")),
                };
                continue;
            }
            if let Some(t) = name.strip_prefix("time=") {
                e.time = Some(match hour(t) {
                    Some(h) => h,
                    None => match t.parse::<f32>() {
                        Ok(h) if (0.0..=24.0).contains(&h) => h,
                        _ => return Err(format!("time must be dawn, morning, noon, afternoon, dusk, night or an hour 0-24, not {t}")),
                    },
                });
                continue;
            }
            if let Some(d) = name.strip_prefix("detail=") {
                e.detail = match d.parse::<f32>() {
                    Ok(d) if (0.0..=1.0).contains(&d) => d,
                    _ => return Err(format!("detail must be 0 to 1, not {d}")),
                };
                continue;
            }
            if let Some(m) = name.strip_prefix("mud=") {
                e.mud_start = match m.parse::<f32>() {
                    Ok(m) if (0.0..=1.0).contains(&m) => m,
                    _ => return Err(format!("mud must be 0 to 1, not {m}")),
                };
                continue;
            }
            if let Some(q) = name.strip_prefix("quality=") {
                preset(&mut e, q)?;
                continue;
            }
            // The most pixels to draw the scene with: a size (1920x1080),
            // a height (1080p), or "native" for the screen's own.
            if let Some(r) = name.strip_prefix("resolution=").or(name.strip_prefix("res=")) {
                // A share of the screen: 50% to 100%.
                if let Some(p) = r.strip_suffix('%') {
                    e.scale = Some(match p.parse::<f32>() {
                        Ok(p) if (25.0..=100.0).contains(&p) => p / 100.0,
                        _ => return Err(format!("resolution must be 25% to 100%, not {r}")),
                    });
                    continue;
                }
                e.scale = None;
                e.pixels = match r {
                    "native" | "full" => None,
                    _ => {
                        let px = match r.trim_end_matches('p').split_once('x') {
                            Some((w, h)) => w.parse::<u32>().ok().zip(h.parse::<u32>().ok()).map(|(w, h)| w * h),
                            None => r.trim_end_matches('p').parse::<u32>().ok().map(|h| h * h * 16 / 9),
                        };
                        match px {
                            Some(px) if px >= 320 * 200 => Some(px),
                            _ => return Err(format!("resolution must be like 1920x1080, 1080p or native, not {r}")),
                        }
                    }
                };
                continue;
            }
            if let Some(n) = name.strip_prefix("msaa=") {
                e.msaa = match n.parse() {
                    Ok(n @ (1 | 2 | 4 | 8)) => n,
                    _ => return Err(format!("msaa must be 1, 2, 4 or 8, not {n}")),
                };
                continue;
            }
            let on = !off;
            match name {
                "msaa" | "aa" => e.msaa = if on { 4 } else { 1 },
                "lighting" => e.lighting = on,
                "shadows" => e.shadows = on,
                "distance" => e.distance = on,
                "bloom" => e.bloom = on,
                "ao" => e.ao = on,
                "sky" => e.sky = on,
                "grade" => e.grade = on,
                "ground" => e.ground = on,
                "grass" => e.grass = on,
                "clouds" => e.clouds = on,
                "ruts" => e.ruts = on,
                "dust" => e.dust = on,
                "bikes" => e.bikes = on,
                "shafts" => e.shafts = on,
                "cinematic" | "blur" => e.cinematic = on,
                "mud" | "splash" => e.mud = on,
                "smooth" => e.smooth = on,
                "scenery" | "props" => e.scenery = on,
                "surfaces" | "materials" => e.surfaces = on,
                "trees" | "wind" => e.trees = on,
                "soft" | "soft-shadows" => e.soft_shadows = on,
                "wear" => e.wear = on,
                "round" => e.round = on,
                "floodlights" | "lamps" => e.floodlights = on,
                "nightlights" => e.night_lights = on,
                "gtao" => e.gtao = on,
                "contact" | "contact-shadows" => e.contact = on,
                "ssr" | "reflections" => e.ssr = on,
                "skylight" | "sky-light" => e.skylight = on,
                "raytracing" | "rt" => e.raytracing = on,
                "taa" | "temporal" => e.taa = on,
                "dynamic" | "dynamic-resolution" => e.dynamic = on,
                _ => {
                    return Err(format!(
                        "unknown upgrade {item:?}: try no-msaa, no-lighting, no-shadows, no-distance, no-bloom, no-ao, no-sky, no-grade, no-ground, no-grass, no-clouds, no-ruts, no-dust, no-bikes, no-shafts, no-cinematic, no-mud, no-smooth, no-scenery, no-surfaces, no-trees, no-soft, no-wear, no-round, no-floodlights, no-nightlights, no-gtao, no-contact, no-ssr, no-skylight, no-dynamic, raytracing, taa, mud=0.5, detail=0.5, msaa=8, quality=low/medium/high/raytraced/ultra, resolution=1080p or 85%, weather=rain or time=dusk"
                    ))
                }
            }
        }
        Ok(e)
    }
}

/// The hour a named time of day (`time=`, mm2_settings::TIMES) puts the
/// sun at.
pub fn hour(name: &str) -> Option<f32> {
    match name {
        "dawn" => Some(7.0),
        "morning" => Some(9.0),
        "noon" => Some(12.5),
        "afternoon" => Some(15.5),
        "dusk" | "sunset" => Some(18.0),
        "night" => Some(23.0),
        _ => None,
    }
}

/// The weather a name (`weather=`) stands for.
pub fn weather(name: &str) -> Option<Weather> {
    match name {
        "clear" => Some(Weather::Clear),
        "overcast" => Some(Weather::Overcast),
        "rain" => Some(Weather::Rain),
        _ => None,
    }
}

static ENHANCED: std::sync::RwLock<Option<Enhanced>> = std::sync::RwLock::new(None);
/// `--enhanced`'s list when it asks for `quality=auto`, until the
/// graphics card is known.
static AUTO: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// The screen the game opened on, in pixels.
static SCREEN: std::sync::Mutex<Option<(u32, u32)>> = std::sync::Mutex::new(None);
/// The time of day and weather picked on the game's event page, over the
/// launcher's (crate::frontend): from the next race on.
static SKY: std::sync::RwLock<Option<(Option<f32>, Weather)>> = std::sync::RwLock::new(None);

/// Sets the time of day and the weather from the next race on.
pub fn set_sky(time: Option<f32>, weather: Weather) {
    *SKY.write().unwrap() = Some((time, weather));
}
/// The anti-aliasing samples the GPU can draw the scene with, beyond the
/// 1 and 4 every GPU has.
static SAMPLE_COUNTS: OnceLock<Vec<u32>> = OnceLock::new();

/// Says which samples the GPU can draw the scene with.
pub fn set_sample_counts(counts: Vec<u32>) {
    let _ = SAMPLE_COUNTS.set(counts);
}

/// The most samples, up to `wanted`, the GPU can draw the scene with.
pub fn samples(wanted: u32) -> u32 {
    let known = SAMPLE_COUNTS.get().map_or(&[][..], |c| &c[..]);
    [1, 2, 4, 8]
        .into_iter()
        .filter(|&c| c <= wanted && (c == 1 || c == 4 || known.contains(&c)))
        .max()
        .unwrap_or(1)
}
/// Whether the GPU was opened able to trace rays (only when the
/// ray-traced shadows are asked for).
static RAY_QUERIES: OnceLock<bool> = OnceLock::new();

/// Says whether the GPU can trace rays.
pub fn set_ray_queries(on: bool) {
    let _ = RAY_QUERIES.set(on);
}

/// Whether the ray-traced shadows are asked for, whether or not the GPU
/// can draw them: then the GPU is opened able to, if it can.
pub fn wants_ray_tracing() -> bool {
    enhanced().is_some_and(|e| e.raytracing && e.shadows)
}

/// Whether the sun's shadows are ray traced: asked for, and the GPU can.
pub fn ray_traced() -> bool {
    wants_ray_tracing() && RAY_QUERIES.get() == Some(&true)
}

/// F3's switch: false shows the original look on the enhanced drawing.
static SHOWING: AtomicBool = AtomicBool::new(true);

/// Turns the upgrades on, before anything is drawn.
pub fn enable(e: Enhanced) {
    #[cfg(target_arch = "wasm32")]
    let e = e.for_browser();
    *ENHANCED.write().unwrap() = Some(e);
}

/// Turns the upgrades on from `--enhanced`'s list; with `quality=auto`,
/// picked again once the graphics card is known ([`pick_auto`]).
pub fn enable_list(list: &str) -> Result<(), String> {
    enable(Enhanced::parse(list)?);
    if list.split(',').any(|i| i.trim() == "quality=auto") {
        *AUTO.lock().unwrap() = Some(list.to_string());
    }
    Ok(())
}

/// Notes the screen the game opened on, for [`pick_auto`].
pub fn set_screen(pixels: (u32, u32)) {
    *SCREEN.lock().unwrap() = Some(pixels);
}

/// With `quality=auto`: picks Low, Medium or High for this graphics card
/// and screen ([`auto_quality`]), and says what it picked (for the
/// launcher, in `auto-graphics.txt`). Once, before anything is drawn.
pub fn pick_auto(info: &wgpu::AdapterInfo) {
    let Some(list) = AUTO.lock().unwrap().take() else {
        return;
    };
    let screen = *SCREEN.lock().unwrap();
    let pixels = screen.map_or(1920 * 1080, |(w, h)| w as u64 * h as u64);
    let q = auto_quality(&info.name, info.device_type, info.backend, pixels, cfg!(target_arch = "wasm32"));
    let list: Vec<String> = list
        .split(',')
        .map(|i| if i.trim() == "quality=auto" { format!("quality={q},dynamic") } else { i.trim().to_string() })
        .collect();
    match Enhanced::parse(&list.join(",")) {
        Ok(e) => enable(e),
        Err(e) => eprintln!("auto graphics: {e}"),
    }
    let card = if info.name.is_empty() { "this graphics card" } else { info.name.as_str() };
    eprintln!("auto graphics: {q} for {card} ({:?}) on a {} screen", info.device_type, screen.map_or("unknown".into(), |(w, h)| format!("{w}x{h}")));
    #[cfg(not(target_arch = "wasm32"))]
    {
        let size = screen.map_or(String::new(), |(w, h)| format!("{w}x{h}"));
        let _ = std::fs::write(mm2_settings::user_dir().join("auto-graphics.txt"), format!("quality={q}\ncard={card}\nscreen={size}\n"));
    }
}

/// The preset for a graphics card (by its name and kind) and a screen of
/// `pixels`: Low for software drawing, OpenGL and the older Intel chips;
/// Medium for most other built-in graphics and older cards; High for
/// recent cards and Apple's bigger chips. A screen bigger than 1440p
/// steps it down one, unless the card is a strong one (or in a browser); in a browser it
/// is at most Medium, but for a card of its own.
pub fn auto_quality(name: &str, kind: wgpu::DeviceType, backend: wgpu::Backend, pixels: u64, browser: bool) -> &'static str {
    use wgpu::DeviceType as D;
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if kind == D::Cpu || backend == wgpu::Backend::Gl || has(&["llvmpipe", "swiftshader", "software", "microsoft basic"]) {
        return "low";
    }
    // Strong enough for High even on a 4K screen.
    let strong = has(&["rtx", "rx 6", "rx 7", "rx 9", "arc b", " max", " ultra"]);
    let mut level = if n.contains("apple") {
        // Apple's chips: the first M1 and M2 Medium, the rest High.
        match (has(&[" pro", " max", " ultra"]), has(&["m1", "m2"])) {
            (true, _) => 2,
            (false, true) => 1,
            _ => 2,
        }
    } else if has(&["intel"]) && !has(&["arc"]) {
        // Intel's built-in graphics: Iris Xe and newer Medium.
        if has(&["iris xe", "iris(r) xe", "graphics 1", "arc graphics"]) || n.contains("intel(r) graphics") { 1 } else { 0 }
    } else if kind == D::DiscreteGpu {
        // Cards of their own: older ones Medium.
        if has(&["gtx 9", "gtx 7", "gtx 6", "gt ", "mx", "radeon hd", "r7 ", "r9 ", "rx 4", "rx 5", "quadro"]) { 1 } else { 2 }
    } else {
        // Other built-in graphics (AMD's, phones'), or not known.
        1
    };
    // (A browser draws the scene at most 1080p's worth whatever the
    // screen: Enhanced::for_browser.)
    if pixels > 2560 * 1440 && !strong && !browser {
        level -= 1;
    }
    if browser && kind != D::DiscreteGpu {
        level = level.min(1);
    }
    ["low", "medium", "high"][level.clamp(0, 2) as usize]
}

impl Enhanced {
    /// What a browser can carry: a browser's GPU work goes through more
    /// layers, a frame that takes too long can stall the whole computer,
    /// and there are no ray queries. So the scene is drawn at most 1080p's
    /// worth (then stretched, the menus and race display still sharp),
    /// with temporal anti-aliasing in place of 4x or 8x MSAA, and
    /// without the light shafts and replay blur, the costliest passes.
    pub fn for_browser(mut self) -> Enhanced {
        const FULL_HD: u32 = 1920 * 1080;
        self.raytracing = false;
        if self.msaa > 1 {
            self.msaa = 1;
            self.taa = true;
        }
        self.pixels = Some(self.pixels.map_or(FULL_HD, |p| p.min(FULL_HD)));
        if let Some(k) = self.scale {
            // A share of the screen still keeps under 1080p.
            self.scale = None;
            self.pixels = Some(((FULL_HD as f32) * k * k) as u32);
        }
        self.shafts = false;
        self.cinematic = false;
        // Screen-space reflections march far across the screen per pixel,
        // and the horizon-based occlusion many taps a pixel: too heavy here.
        self.ssr = false;
        self.gtao = false;
        self
    }
}

/// False once the GPU turned out unable to draw the upgrades (OpenGL).
static DRAWABLE: AtomicBool = AtomicBool::new(true);

/// The GPU cannot draw the upgrades: the original look it is.
pub fn not_drawable() {
    DRAWABLE.store(false, Ordering::Relaxed);
}

/// The upgrades, when on.
pub fn enhanced() -> Option<Enhanced> {
    if !DRAWABLE.load(Ordering::Relaxed) {
        return None;
    }
    let mut e = (*ENHANCED.read().unwrap())?;
    if let Some((time, weather)) = *SKY.read().unwrap() {
        e.time = time;
        e.weather = weather;
    }
    Some(e)
}

/// The upgrades as they show now: none while F3 has switched to the
/// original look.
pub fn showing() -> Option<Enhanced> {
    enhanced().filter(|_| SHOWING.load(Ordering::Relaxed))
}

/// F3: between the enhanced look and the original one. Returns whether
/// the enhanced look now shows.
pub fn toggle() -> bool {
    !SHOWING.fetch_xor(true, Ordering::Relaxed)
}

/// How sharply textures seen at a slant are filtered: the most the GPU
/// allows with the enhanced graphics, else as the port always has.
pub fn anisotropy() -> u16 {
    if enhanced().is_some() {
        16
    } else {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_browser_draws_at_most_1080p_without_the_heaviest_passes() {
        let e = Enhanced::parse("quality=ultra,raytracing").unwrap().for_browser();
        assert!(!e.raytracing && !e.shafts && e.msaa == 1 && e.taa);
        assert_eq!(e.scene_size((3840, 2160)), (1920, 1080));
        let e = Enhanced::parse("quality=low").unwrap().for_browser();
        assert_eq!(e.scene_size((3840, 2160)), (1280, 720));
    }

    #[test]
    fn scene_size() {
        let e = Enhanced::default();
        assert_eq!(e.scene_size((1920, 1080)), (1920, 1080));
        assert_eq!(e.scene_size((3840, 2160)), (2560, 1440));
        assert_eq!(Enhanced::parse("resolution=native").unwrap().scene_size((3840, 2160)), (3840, 2160));
        assert_eq!(Enhanced::parse("res=1080p").unwrap().scene_size((3840, 2160)), (1920, 1080));
        let auto = Enhanced::parse("quality=auto").unwrap();
        assert!(auto.taa && auto.pixels == Some(1920 * 1080));
        let low = Enhanced::parse("quality=low").unwrap();
        assert_eq!((low.msaa, low.ao, low.scene_size((2560, 1440))), (1, false, (1280, 720)));
        assert!(Enhanced::parse("quality=potato").is_err());
        let rt = Enhanced::parse("quality=raytraced").unwrap();
        assert_eq!((rt.msaa, rt.raytracing, rt.scene_size((3840, 2160))), (2, true, (2560, 1440)));
        assert!(rt.grass && rt.smooth && !rt.scenery && !rt.grade);
        let medium = Enhanced::parse("quality=medium").unwrap();
        assert!(medium.taa && medium.msaa == 1);
        let ultra = Enhanced::parse("quality=ultra").unwrap();
        assert!(!ultra.scenery && ultra.ruts && !ultra.grade && !ultra.raytracing);
        let high = Enhanced::parse("quality=high").unwrap();
        assert!(!high.scenery && !high.grade && high.smooth);
        // A preset's anti-aliasing and resolution, then changed after it.
        let mixed = Enhanced::parse("quality=ultra,msaa=4,resolution=1440p").unwrap();
        assert_eq!((mixed.msaa, mixed.scene_size((3840, 2160))), (4, (2560, 1440)));
        // A share of the screen, whatever the preset.
        let share = Enhanced::parse("quality=high,resolution=75%").unwrap();
        assert_eq!(share.scene_size((3440, 1440)), (2580, 1080));
        assert!(Enhanced::parse("resolution=150%").is_err());
    }

    #[test]
    fn auto_picks_for_the_card_and_screen() {
        use wgpu::{Backend as B, DeviceType as D};
        let hd = 1920 * 1080;
        let uhd = 3840 * 2160;
        let q = |name, kind, px| auto_quality(name, kind, B::Vulkan, px, false);
        assert_eq!(q("llvmpipe (LLVM 17.0.6, 256 bits)", D::Cpu, hd), "low");
        assert_eq!(auto_quality("NVIDIA GeForce RTX 3060", D::DiscreteGpu, B::Gl, hd, false), "low");
        assert_eq!(q("Intel(R) HD Graphics 3000", D::IntegratedGpu, hd), "low");
        assert_eq!(q("Intel(R) Iris(R) Xe Graphics", D::IntegratedGpu, hd), "medium");
        assert_eq!(q("AMD Radeon 780M Graphics", D::IntegratedGpu, hd), "medium");
        assert_eq!(q("NVIDIA GeForce GTX 970", D::DiscreteGpu, hd), "medium");
        assert_eq!(q("NVIDIA GeForce RTX 4070", D::DiscreteGpu, hd), "high");
        assert_eq!(q("NVIDIA GeForce RTX 4070", D::DiscreteGpu, uhd), "high");
        assert_eq!(q("AMD Radeon RX 580", D::DiscreteGpu, uhd), "low");
        assert_eq!(q("Apple M1", D::IntegratedGpu, hd), "medium");
        assert_eq!(q("Apple M4", D::IntegratedGpu, hd), "high");
        // A 4K screen on a plain M4: one step down.
        assert_eq!(q("Apple M4", D::IntegratedGpu, uhd), "medium");
        assert_eq!(q("Apple M4 Max", D::IntegratedGpu, uhd), "high");
        // A browser: at most Medium without a card of its own; not known
        // (Safari names no card) is Medium.
        assert_eq!(auto_quality("Apple M4 Pro", D::IntegratedGpu, B::BrowserWebGpu, hd, true), "medium");
        assert_eq!(auto_quality("", D::Other, B::BrowserWebGpu, hd, true), "medium");
    }

    #[test]
    fn upgrades_parse() {
        let all = Enhanced::parse("").unwrap();
        assert_eq!(all.msaa, 4);
        assert!(all.lighting && all.shadows && all.distance && all.ao && all.sky);
        // Bloom and the glossy bikes are there when asked for.
        assert!(!all.bloom && !all.bikes);
        let asked = Enhanced::parse("bloom,bikes").unwrap();
        assert!(asked.bloom && asked.bikes);
        let some = Enhanced::parse("no-bloom, no-ao,msaa=8").unwrap();
        assert!(!some.bloom && !some.ao && some.shadows);
        assert_eq!(some.msaa, 8);
        assert_eq!(Enhanced::parse("no-msaa").unwrap().msaa, 1);
        assert!(Enhanced::parse("msaa=3").is_err());
        assert!(Enhanced::parse("pathtracing").is_err());
        assert!(!all.raytracing && Enhanced::parse("raytracing").unwrap().raytracing);
        let w = Enhanced::parse("weather=rain,time=dusk,no-grass").unwrap();
        assert_eq!(w.weather, Weather::Rain);
        assert_eq!(w.time, Some(18.0));
        assert!(!w.grass && w.ground);
        assert!(!Enhanced::parse("no-shafts").unwrap().shafts);
        assert!(Enhanced::parse("time=25").is_err());
    }
}
