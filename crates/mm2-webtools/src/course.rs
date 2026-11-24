//! The Tools page's Course Builder: the same track maths as the game's
//! builder (`mm2-course`), handed to the page as JSON and plain arrays.
//! A track is its `.course` JSON, which the page edits and passes back;
//! the files it saves open in the game's full builder too.

use mm2_course::{auto, bake, env, kit, Course, Mode};
use serde_json::json;
use wasm_bindgen::prelude::*;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

fn mode(name: &str) -> Mode {
    Mode::ALL
        .into_iter()
        .find(|m| m.name().eq_ignore_ascii_case(name) || m.folder().eq_ignore_ascii_case(name))
        .unwrap_or(Mode::National)
}

fn read(text: &str) -> Result<Course, JsError> {
    Course::from_json(text).map_err(err)
}

/// A new track for the event (`Supercross`, `National`, `Baja` or `Enduro`).
#[wasm_bindgen]
pub fn course_new(event: &str) -> String {
    Course::new(mode(event)).to_json()
}

/// A whole track made up at random, as the game's "Surprise me".
#[wasm_bindgen]
pub fn course_surprise(event: &str, seed: u32) -> String {
    auto::surprise(mode(event), seed).to_json()
}

/// The track with jumps laid along its line by themselves.
#[wasm_bindgen]
pub fn course_fill_jumps(text: &str, seed: u32) -> Result<String, JsError> {
    let mut c = read(text)?;
    auto::fill_jumps(&mut c, seed);
    Ok(c.to_json())
}

/// The track for another event, as the game's builder changes it.
#[wasm_bindgen]
pub fn course_change_event(text: &str, event: &str) -> Result<String, JsError> {
    let mut c = read(text)?;
    c.change_mode(mode(event));
    Ok(c.to_json())
}

/// A jump of `kind` at `at` ft along the line, at its usual size.
#[wasm_bindgen]
pub fn course_jump(event: &str, kind: &str, at: f32) -> String {
    let k = kit::Kind::ALL
        .into_iter()
        .find(|k| format!("{k:?}").eq_ignore_ascii_case(kind))
        .unwrap_or(kit::Kind::Tabletop);
    serde_json::to_string(&k.make(mode(event), at)).unwrap_or_default()
}

/// What the page offers: events (with their land's side, ft), jumps and
/// lands, with names and a line about each.
#[wasm_bindgen]
pub fn course_kit() -> String {
    let events: Vec<_> = Mode::ALL
        .iter()
        .map(|m| json!({ "id": format!("{m:?}"), "name": m.name(), "folder": m.folder(), "side": m.side(), "laps": m.laps(), "width": m.track_width() }))
        .collect();
    let jumps: Vec<_> = kit::Kind::ALL
        .iter()
        .map(|k| json!({ "id": format!("{k:?}"), "name": k.name(), "about": k.about() }))
        .collect();
    let lands: Vec<_> = mm2_course::LandKind::ALL
        .iter()
        .map(|l| json!({ "id": format!("{l:?}"), "name": l.name() }))
        .collect();
    json!({ "events": events, "jumps": jumps, "lands": lands }).to_string()
}

/// A quick look at the track: the ground every `every`th sample.
#[wasm_bindgen]
pub struct Preview {
    n: u32,
    step: f32,
    heights: Vec<f32>,
    colours: Vec<u8>,
    line: Vec<f32>,
    length: f32,
    gates: Vec<f32>,
    top: Vec<f32>,
    widths: Vec<f32>,
}

#[wasm_bindgen]
impl Preview {
    /// Samples a side.
    #[wasm_bindgen(getter)]
    pub fn n(&self) -> u32 {
        self.n
    }
    /// Feet between samples.
    #[wasm_bindgen(getter)]
    pub fn step(&self) -> f32 {
        self.step
    }
    /// Heights, ft, rows along +z.
    pub fn heights(&self) -> Vec<f32> {
        self.heights.clone()
    }
    /// The ground's colour, RGB per sample, as the bake paints it.
    pub fn colours(&self) -> Vec<u8> {
        self.colours.clone()
    }
    /// The smoothed racing line: x, z ft pairs.
    pub fn line(&self) -> Vec<f32> {
        self.line.clone()
    }
    /// The line's length, ft.
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> f32 {
        self.length
    }
    /// The track's surface height at each point of the line (its bed
    /// and jumps, without the berms), ft.
    pub fn top(&self) -> Vec<f32> {
        self.top.clone()
    }
    /// The track's width at each point of the line, ft.
    pub fn widths(&self) -> Vec<f32> {
        self.widths.clone()
    }
    /// A gate race's gates: x1, z1, x2, z2 for each.
    pub fn gates(&self) -> Vec<f32> {
        self.gates.clone()
    }
}

/// The track baked every `every`th sample (1 is full size, slow).
#[wasm_bindgen]
pub fn course_preview(text: &str, every: u32) -> Result<Preview, JsError> {
    let c = read(text)?;
    let every = every.max(1) as usize;
    let land = bake::land(&c, every);
    let field = bake::bake(&c, every, &land);
    let colours = field
        .surfaces
        .iter()
        .flat_map(|&z| {
            env::colour(mm2_course::Surface::from_zone(z))
                .map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8)
        })
        .collect();
    let path = c.path();
    let line = path.pts.iter().flatten().copied().collect();
    let length = path.s.last().copied().unwrap_or(0.0);
    let gates = if c.mode.laps() {
        Vec::new()
    } else {
        env::gates(&c, &path)
            .iter()
            .flat_map(|(a, b)| [a[0], a[1], b[0], b[1]])
            .collect()
    };
    let bed = bake::Bed::new(&c, &path);
    let top = path
        .s
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let mut h = bed.grade.get(i).copied().unwrap_or(0.0);
            for j in &c.jumps {
                let ds = path.ahead(j.at, s);
                if (0.0..=j.length).contains(&ds) {
                    h += j.height_at(ds);
                }
            }
            h
        })
        .collect();
    let widths = path.width.clone();
    Ok(Preview {
        n: field.n as u32,
        step: field.step,
        heights: field.heights,
        colours,
        line,
        length,
        gates,
        top,
        widths,
    })
}

/// The track baked into an `.ENV` for this port (the game in a browser or
/// on a computer, which also opens it in the full builder).
#[wasm_bindgen]
pub fn course_bake(text: &str) -> Result<Vec<u8>, JsError> {
    Ok(env::write(&read(text)?).env)
}

/// The track as an `.ENV` the original Motocross Madness 2 loads.
#[wasm_bindgen]
pub fn course_original(text: &str) -> Result<Vec<u8>, JsError> {
    env::write_original(&read(text)?, &env::Pictures::default()).map_err(err)
}

/// The file name the track saves under, without `.ENV` or `.course`.
#[wasm_bindgen]
pub fn course_stem(text: &str) -> Result<String, JsError> {
    Ok(read(text)?.stem())
}

/// The `.course` inside a builder `.ENV`, as JSON.
#[wasm_bindgen]
pub fn course_from_env(bytes: &[u8]) -> Option<String> {
    env::course_in(bytes).map(|c| c.to_json())
}
