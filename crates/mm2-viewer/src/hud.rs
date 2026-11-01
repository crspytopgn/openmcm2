//! The in-race display as the game's `TrackOverlay.cpp` draws it
//! (`docs/physics.md` section 16): the speedometer at the bottom right,
//! the race panel at the top left and the radar at the top right, all in
//! the 640×480 screen's units.

use crate::race::Race;
use crate::ui::{Gfx, Ui};
use glam::Vec3;

/// ft/s to mph.
const MPH: f32 = 0.68182;

/// Where `instr1.tga` goes (`0x51ddf0`: the rect 511,351 to 639,479), and
/// its needle's centre and length.
const SPEEDO: (f32, f32) = (511.0, 351.0);
const NEEDLE_CENTRE: (f32, f32) = (64.0, 85.0);
const NEEDLE: f32 = 45.0;

/// The modernised HUD (a tweak, on unless `--original-race-display`):
/// the panel, radar and dial drawn at the window's resolution, the other
/// pictures enlarged smoothly and the lines smooth-edged.
pub fn sharp() -> bool {
    crate::tweaks::get().modern_hud
}

/// The modernised HUD's dial, drawn at the window's resolution in place
/// of `instr1.tga` (not the game's: it showed the 128-pixel picture): a
/// long mark each 10 mph and a short one between, out to `MARK_OUT` from
/// the needle's centre, and the numbers centred `NUMBER_OUT` out, all as
/// the picture has them.
const MARK_OUT: f32 = 52.0;
const MARK_LONG: f32 = 42.5;
const MARK_SHORT: f32 = 47.5;
const NUMBER_OUT: f32 = 34.0;
const NUMBER_PT: f32 = 15.0;
/// The picture's marks: light grey, as see-through as its face.
const MARK_COLOUR: [f32; 4] = [0.8, 0.8, 0.8, 0.8];
/// The dial's face: round about the needle's centre and cut flat below
/// it, as far as the picture's bracket went; and the hub over the
/// needle's end.
const DIAL_RADIUS: f32 = 57.0;
const DIAL_FLAT: f32 = 36.0;
const HUB: f32 = 5.0;
/// The modernised HUD's radar: a disc a little past the rim, with the
/// rim, a ring half way and a cross drawn in.
const RADAR_DISC: f32 = 58.0;
/// The player's arrow on it, wide and tall, pointing up.
const ARROW: [f32; 2] = [5.0, 6.0];

/// `BajaScore.tga` at the top left (`0x51eb30`), and its lines' top left
/// corners in the panel.
const PANEL: (f32, f32) = (0.0, 0.0);
const LINES: [(f32, f32); 6] = [
    (14.0, 20.0),
    (14.0, 36.0),
    (14.0, 53.0),
    (14.0, 69.0),
    (14.0, 87.0),
    (14.0, 105.0),
];
/// The panel's text height: a pixel under the game's 12, so its longer
/// lines stay in their boxes, and the room in a box for a line (the
/// panel's boxes end at 122); a line wider still is drawn smaller.
const TEXT: f32 = 11.0;
const LINE_ROOM: f32 = 106.0;
/// The modernised HUD's panel in place of `BajaScore.tga`: a rounded,
/// see-through grey box behind the lines, from the panel's top left, and
/// faint rules where the picture ruled the lines off.
const PANEL_BOX: [f32; 4] = [6.0, 12.0, 118.0, 112.0];
const RULES: [f32; 4] = [33.0, 49.5, 83.0, 101.0];
const RULE: [f32; 4] = [1.0, 1.0, 1.0, 0.16];
/// The live lap timing's box under the panel, from its top left.
const GAP_BOX: [f32; 4] = [6.0, 127.0, 118.0, 16.0];
/// The quick get-up's hint, across the middle near the bottom.
const GET_UP_Y: f32 = 400.0;

/// `radar128.tga` at the top right, its centre and rim (`0x520e90`).
const RADAR: (f32, f32) = (512.0, 1.0);
const RADAR_CENTRE: (f32, f32) = (572.0, 65.0);
const RADAR_RIM: f32 = 55.0;
/// The rim is 180 ft × zoom away; the game starts at 2.
const RADAR_FEET: f32 = 180.0;
/// The course line on the radar (0xedea5e) and the gate to make next
/// (0xffff5e).
const RADAR_COURSE: [f32; 4] = [0xed as f32 / 255.0, 0xea as f32 / 255.0, 0x5e as f32 / 255.0, 1.0];
const RADAR_GATE: [f32; 4] = [1.0, 1.0, 0x5e as f32 / 255.0, 1.0];

/// The part of the stretch `a`–`b` (radar pixels from the centre) inside
/// the rim (0x521b40), if any: both ends outside draws nothing.
fn clip_to_rim(a: [f32; 2], b: [f32; 2]) -> Option<([f32; 2], [f32; 2])> {
    let inside = |p: [f32; 2]| p[0] * p[0] + p[1] * p[1] < RADAR_RIM * RADAR_RIM;
    let (ia, ib) = (inside(a), inside(b));
    if ia && ib {
        return Some((a, b));
    }
    if !ia && !ib {
        return None;
    }
    let (from, to) = if ia { (a, b) } else { (b, a) };
    // from + t (to − from) on the rim, t in 0..1.
    let d = [to[0] - from[0], to[1] - from[1]];
    let (qa, qb) = (d[0] * d[0] + d[1] * d[1], 2.0 * (from[0] * d[0] + from[1] * d[1]));
    let qc = from[0] * from[0] + from[1] * from[1] - RADAR_RIM * RADAR_RIM;
    let t = (-qb + (qb * qb - 4.0 * qa * qc).max(0.0).sqrt()) / (2.0 * qa);
    let edge = [from[0] + d[0] * t, from[1] + d[1] * t];
    Some(if ia { (from, edge) } else { (edge, from) })
}

/// The text queue at the bottom left (`0x520970`, `0x520aa0`): its box,
/// text height, and how long each message stays (`0x497740`).
const MESSAGE: [f32; 4] = [10.0, 460.0, 246.0, 20.0];
const MESSAGE_TEXT: f32 = 17.0;
const MESSAGE_TIME: f32 = 3.25;
/// The riders' labels' text height.
const LABEL_TEXT: f32 = 14.0;
/// A label hangs this far over its rider's head (0x55d9fc), and fades in
/// or out over this long when a hill comes between it and the camera
/// (0x51e400's +0x144).
const LABEL_OVER_HEAD: f32 = 1.62;
const LABEL_FADE: f32 = 0.25;

/// A rider's label's fading (0x51e5b0): whether the camera saw it at the
/// last look, whether it is fading in or out, and how far through, s.
#[derive(Debug, Clone, Copy, Default)]
struct Label {
    hidden: bool,
    fading_in: bool,
    fading_out: bool,
    t: f32,
    alpha: f32,
}

impl Label {
    fn new() -> Self {
        Label {
            alpha: 1.0,
            ..Default::default()
        }
    }

    /// 0x51e760: after a look, a label the camera has just come to see
    /// fades in, and one it has lost fades out.
    fn saw(&mut self, seen: bool) {
        if seen && self.hidden {
            (self.fading_in, self.fading_out) = (true, false);
        } else if !seen && !self.hidden {
            (self.fading_in, self.fading_out) = (false, true);
        }
        self.hidden = !seen;
    }

    /// 0x51e5b0, each frame.
    fn step(&mut self, dt: f32) {
        if self.fading_in {
            self.t += dt;
            if self.t >= LABEL_FADE {
                (self.alpha, self.fading_in) = (1.0, false);
            }
        } else if self.fading_out {
            self.t -= dt;
            if self.t <= 0.0 {
                (self.t, self.alpha, self.fading_out) = (0.0, 0.0, false);
            }
        }
        if self.fading_in || self.fading_out {
            self.alpha = ((self.t / LABEL_FADE * 255.0 - 0.5).round() / 255.0).clamp(0.0, 1.0);
        }
    }
}

/// What the race shows: the Game and Graphics pages' switches, which the
/// F5 to F10 keys and Ctrl+N flip during a race (0x4e54c0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Show {
    /// The speedometer (Instruments, F7).
    pub gauges: bool,
    /// The race panel (Stats, F5).
    pub stats: bool,
    /// The radar (Radar Overlay, F6).
    pub radar: bool,
    /// Labels over the other riders: their names, their places, or both.
    pub names: bool,
    pub positions: bool,
    /// The sky (F8), the dirt (F9) and the shadows (F10).
    pub sky: bool,
    pub particles: bool,
    pub shadows: bool,
}

impl Default for Show {
    fn default() -> Self {
        Show::from_options(&Default::default(), &Default::default())
    }
}

impl Show {
    pub fn from_profile(p: &crate::profile::Profile) -> Self {
        Show::from_options(&p.game, &p.graphics)
    }

    fn from_options(g: &crate::profile::GameOptions, v: &crate::profile::GraphicsOptions) -> Self {
        Show {
            gauges: g.gauges,
            stats: g.race_stats,
            radar: g.overview,
            names: g.rider_names,
            positions: g.rider_position,
            sky: v.sky,
            particles: v.particles,
            shadows: v.shadows,
        }
    }

    /// Back into the profile, which keeps them.
    pub fn store(&self, p: &mut crate::profile::Profile) {
        let (g, v) = (&mut p.game, &mut p.graphics);
        g.gauges = self.gauges;
        g.race_stats = self.stats;
        g.overview = self.radar;
        g.rider_names = self.names;
        g.rider_position = self.positions;
        v.sky = self.sky;
        v.particles = self.particles;
        v.shadows = self.shadows;
    }

    /// Ctrl+N: both, names, places, neither, and round again; the string
    /// the game says and whether it is on.
    pub fn cycle_labels(&mut self) -> (u32, bool) {
        match (self.names, self.positions) {
            (true, true) => {
                self.positions = false;
                (5138, true)
            }
            (true, false) => {
                (self.names, self.positions) = (false, true);
                (5139, true)
            }
            (false, false) => {
                (self.names, self.positions) = (true, true);
                (5140, true)
            }
            (false, true) => {
                self.positions = false;
                (5140, false)
            }
        }
    }
}

pub struct Hud {
    pub show: Show,
    /// The camera's projection and view, for the riders' labels.
    pub view_proj: glam::Mat4,
    /// Each rider's head, game coordinates, and each label's fading.
    pub heads: Vec<Option<Vec3>>,
    labels: Vec<Label>,
    /// Frames counted for the labels' looks, every fourth frame each.
    looks: usize,
    /// The speed the dial shows, mph; in the air it winds down.
    shown: f32,
    pub zoom: f32,
    /// The message showing and how long it has been up.
    message: Option<(String, f32)>,
    /// The player's finish has been announced.
    told: bool,
}

impl Default for Hud {
    fn default() -> Self {
        Hud {
            show: Show::default(),
            view_proj: glam::Mat4::IDENTITY,
            heads: Vec::new(),
            labels: Vec::new(),
            looks: 0,
            shown: 0.0,
            zoom: 2.0,
            message: None,
            told: false,
        }
    }
}

impl Hud {
    /// Shows a message in place of the one showing (`0x520b90`).
    pub fn say(&mut self, text: &str) {
        if !text.is_empty() {
            self.message = Some((text.to_string(), 0.0));
        }
    }

    /// Each rider's label looks for the camera from over the head, every
    /// fourth frame, the riders taking turns (0x51e680, 0x51e4b0's +0x130):
    /// hidden when the ground lies between them (0x50c600). `eye` is the
    /// camera, game coordinates.
    pub fn look(&mut self, terrain: &mm2_formats::trn::Terrain, eye: Vec3) {
        self.labels.resize(self.heads.len(), Label::new());
        let turn = self.looks % 4;
        self.looks = self.looks.wrapping_add(1);
        for (i, (label, head)) in self.labels.iter_mut().zip(&self.heads).enumerate() {
            let Some(head) = head else {
                continue;
            };
            if i % 4 == turn {
                label.saw(!ground_between(terrain, eye, *head + Vec3::Y * LABEL_OVER_HEAD));
            }
        }
    }

    /// Steps the radar's zoom (1 to 5).
    pub fn zoom_by(&mut self, step: f32) {
        self.zoom = (self.zoom + step).clamp(1.0, 5.0);
    }

    /// "Stats ON" and the like (0x4e5410).
    pub fn say_switch(&mut self, ui: &Ui, what: u32, on: bool) {
        let state = ui.lang(if on { 5127 } else { 5128 });
        self.say(&format!("{} {}", ui.lang(what), state));
    }

    pub fn draw(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race, dt: f32) {
        let me = &race.racers[race.player];
        self.draw_labels(ui, gfx, race, dt);
        if self.show.gauges {
            self.draw_gauges(ui, gfx, race, dt);
        }
        if self.show.stats {
            self.draw_panel(ui, gfx, race);
            self.draw_lap_gap(ui, gfx, race);
        }
        self.draw_get_up(ui, gfx, race);
        // The finish (`0x497740`): "You finished 2nd", or the
        // congratulations for a win.
        if me.finished.is_some() && !self.told {
            self.told = true;
            let place = race.position().clamp(1, 12) as u32;
            self.say(&ui.lang(5248 + place));
        }
        self.draw_message(ui, gfx, dt);
        if self.show.radar {
            self.draw_radar(ui, gfx, race);
        }
    }

    /// Each other rider's label (0x523a40): its place, its name, or
    /// "place : name". 0x51e760 draws it at its size on the screen, its
    /// top left corner 1.62 ft over the rider's head, while that point is
    /// in view.
    fn draw_labels(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race, dt: f32) {
        for label in &mut self.labels {
            label.step(dt);
        }
        if !self.show.names && !self.show.positions {
            return;
        }
        let order = race.standings();
        for (i, r) in race.racers.iter().enumerate() {
            if i == race.player || Some(i) == race.ghost {
                continue;
            }
            let alpha = self.labels.get(i).map_or(1.0, |l| l.alpha);
            if alpha <= 0.0 {
                continue;
            }
            let head = self.heads.get(i).copied().flatten().unwrap_or(r.bike.position() + Vec3::Y * 4.5);
            let at = crate::renderer::from_game(head + Vec3::Y * LABEL_OVER_HEAD);
            let clip = self.view_proj * at.extend(1.0);
            if clip.w < 1.0 {
                continue;
            }
            let (x, y) = (clip.x / clip.w, clip.y / clip.w);
            if x.abs() > 1.0 || y.abs() > 1.0 || clip.z > clip.w {
                continue;
            }
            let place = order.iter().position(|&o| o == i).map_or(0, |p| p + 1);
            let place = if place < 1 { "--".to_string() } else { place.to_string() };
            let name = crate::rider_name(race, ui, i);
            let text = match (self.show.positions, self.show.names) {
                (true, true) => format!("{place} : {name}"),
                (true, false) => place,
                _ => name,
            };
            let Some(t) = ui.cell_text(gfx, &text, LABEL_TEXT, true) else {
                continue;
            };
            let (w, h) = gfx.sprites.size(t);
            let (w, h) = (w as f32, h as f32);
            let (sx, sy) = (((x + 1.0) * 320.0).trunc(), ((1.0 - y) * 240.0).trunc());
            gfx.sprites.in_view = true;
            for (d, colour) in [(1.0, [0.0, 0.0, 0.0, alpha]), (0.0, [1.0, 1.0, 1.0, alpha])] {
                gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [sx + d, sy + d, w, h], colour);
            }
            gfx.sprites.in_view = false;
        }
    }

    fn draw_gauges(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race, dt: f32) {
        let bike = &race.racers[race.player].bike;

        // Speedometer.
        if bike.airborne() {
            // 0.5 mph a frame at the game's 30 frames a second.
            self.shown = (self.shown - 0.5 * dt * 30.0).max(0.0);
        } else {
            self.shown = bike.speed().abs() * MPH;
        }
        let c = [SPEEDO.0 + NEEDLE_CENTRE.0, SPEEDO.1 + NEEDLE_CENTRE.1];
        let at = |mph: f32, out: f32| {
            let a = dial_angle(mph);
            [c[0] + a.sin() * out, c[1] - a.cos() * out]
        };
        if !sharp() {
            if let Some(t) = ui.overlay(gfx, "instr1.tga") {
                gfx.sprites.image(t, SPEEDO.0, SPEEDO.1);
            }
            ui.line(gfx, c, at(self.shown, NEEDLE), 1.5, [1.0, 0.0, 0.0, 1.0]);
            return;
        }
        let (r, flat) = (DIAL_RADIUS, DIAL_FLAT);
        let face = ui.made(gfx, "dial", || {
            painted(2.0 * r, r + flat, 0.58, |x, y| {
                let d = (x - r).hypot(y - r) - r;
                d.max(y - (r + flat))
            }, |_, _| 0.0)
        });
        gfx.sprites.quad(face, [0.0, 0.0, 1.0, 1.0], [c[0] - r, c[1] - r, 2.0 * r, r + flat], [1.0; 4]);
        for mph in (0..=100).step_by(5) {
            let from = if mph % 10 == 0 { MARK_LONG } else { MARK_SHORT };
            let (a, b) = (at(mph as f32, from), at(mph as f32, MARK_OUT));
            // Cut short of the face's flat bottom.
            let bottom = c[1] + flat - 2.0;
            if a[1] >= bottom {
                continue;
            }
            let t = if b[1] > bottom { (bottom - a[1]) / (b[1] - a[1]) } else { 1.0 };
            let b = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            ui.line(gfx, a, b, 1.25, MARK_COLOUR);
        }
        for mph in (0..=100).step_by(10) {
            if let Some(t) = ui.text(gfx, &mph.to_string(), NUMBER_PT, true) {
                let (w, h) = gfx.sprites.size(t);
                let (w, h) = (w as f32, h as f32);
                let [x, y] = at(mph as f32, NUMBER_OUT);
                gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [x - w / 2.0, y - h / 2.0, w, h], MARK_COLOUR);
            }
        }
        let tip = at(self.shown, NEEDLE);
        ui.line(gfx, c, tip, 1.5, [1.0, 0.0, 0.0, 1.0]);
        let hub = ui.made(gfx, "hub", || {
            painted(2.0 * HUB, 2.0 * HUB, 0.95, |x, y| (x - HUB).hypot(y - HUB) - HUB, |_, _| 0.0)
        });
        gfx.sprites.quad(hub, [0.0, 0.0, 1.0, 1.0], [c[0] - HUB, c[1] - HUB, 2.0 * HUB, 2.0 * HUB], [1.0; 4]);
    }

    fn draw_panel(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race) {
        let me = &race.racers[race.player];
        // Race panel.
        if sharp() {
            draw_box(ui, gfx, PANEL_BOX);
            for y in RULES {
                ui.fill(gfx, [PANEL.0 + 12.0, PANEL.1 + y, 106.0, 0.5], RULE);
            }
        } else if let Some(t) = ui.overlay(gfx, "BajaScore.tga") {
            gfx.sprites.image(t, PANEL.0, PANEL.1);
        }
        let clock = race.clock();
        let started = race.gate_down();
        let unset = "-:--.--".to_string();
        let lap = if race.laps >= 99 {
            format!("{} {}", ui.lang(2102), me.laps + 1)
        } else {
            format!(
                "{} {} / {}",
                ui.lang(2102),
                (me.laps + 1).min(race.laps),
                race.laps
            )
        };
        let mut lines = [
            format!(
                "{} {} / {}",
                ui.lang(2101),
                race.position(),
                race.racers.len() - race.ghost.is_some() as usize
            ),
            lap,
            format!(
                "{} {}",
                ui.lang(2103),
                if started {
                    clock_text(me.lap_time(clock))
                } else {
                    unset.clone()
                }
            ),
            format!(
                "{} {}",
                ui.lang(2104),
                me.best_lap().map_or(unset.clone(), clock_text)
            ),
            // Time behind the leader; the leader's is blank.
            format!(
                "{} {}",
                ui.lang(2105),
                if me.timing.behind_leader > 0.0 {
                    clock_text(me.timing.behind_leader)
                } else {
                    unset.clone()
                }
            ),
            format!(
                "{} {}",
                ui.lang(5048),
                if started {
                    clock_text(me.finished.unwrap_or(clock))
                } else {
                    unset.clone()
                }
            ),
        ];
        // The Stunt Quarry's panel (0x51f070): the time left, unless the
        // session has no end, then the riders by points, `%d) %s : %.0f`.
        if race.stunt_event {
            let order = race.standings();
            let names: Vec<String> = (0..race.racers.len())
                .map(|i| crate::rider_name(race, ui, i))
                .collect();
            let mut stunt: Vec<String> = Vec::new();
            if let Some(left) = race.time_left() {
                let s = left as u32;
                stunt.push(format!("{}{:02}:{:02}", ui.lang(2371), s / 60, s % 60));
            }
            let room = lines.len() - stunt.len();
            let me = order.iter().position(|&i| i == race.player).unwrap_or(0);
            for (place, &i) in order.iter().enumerate().take(room) {
                // The player keeps the last line when further down.
                let (place, i) = if place + 1 == room && me >= room {
                    (me, race.player)
                } else {
                    (place, i)
                };
                let name = names.get(i).cloned().unwrap_or_default();
                stunt.push(format!(
                    "{}) {name} : {:.0}",
                    place + 1,
                    race.racers[i].stunt_points
                ));
            }
            stunt.resize(lines.len(), String::new());
            lines = std::array::from_fn(|k| stunt[k].clone());
        }
        // Moto-Tag's panel (0x520090): the time left, the riders by points
        // in stunt mode (otherwise the player's place and times as IT), and
        // who is IT, or a loose ball.
        if let Some(t) = &race.tag {
            let mut tag: Vec<String> = Vec::new();
            let s = race.time_left().unwrap_or(0.0) as u32;
            tag.push(format!("{}{:02}:{:02}", ui.lang(2371), s / 60, s % 60));
            if t.stunts {
                let order = race.standings();
                for (place, &i) in order.iter().enumerate().take(4) {
                    tag.push(format!(
                        "{}) {} : {:.0}",
                        place + 1,
                        crate::rider_name(race, ui, i),
                        race.racers[i].stunt_points
                    ));
                }
            } else {
                tag.push(format!(
                    "{} {} / {}",
                    ui.lang(2101),
                    race.position(),
                    race.racers.len()
                ));
                tag.push(format!(
                    "{} {}",
                    ui.lang(2361),
                    clock_text(t.time_it(race.player))
                ));
                tag.push(format!(
                    "{} {}",
                    ui.lang(5163),
                    clock_text(t.longest_it(race.player))
                ));
            }
            tag.resize(lines.len() - 1, String::new());
            tag.push(match t.it {
                Some(i) => format!("{} {}", crate::rider_name(race, ui, i), ui.lang(5304)),
                None if t.ball.is_some() => ui.lang(5303),
                None => String::new(),
            });
            lines = std::array::from_fn(|k| tag[k].clone());
        }
        // A gate race's panel (0x51fbb0) ends on the gate being ridden for
        // instead of the race time.
        if !race.gates.is_empty() && race.tag.is_none() {
            let count = race.gates.len();
            let gate = if me.finished.is_some() {
                count
            } else {
                me.timing.gate + 1
            };
            lines[5] = format!("{} {gate} / {count}", ui.lang(5037));
        }
        for (text, (x, y)) in lines.iter().zip(LINES) {
            if let Some(t) = ui.cell_text(gfx, text, TEXT, false) {
                let (w, h) = gfx.sprites.size(t);
                let (w, h) = (w as f32, h as f32);
                let fit = (LINE_ROOM / w).min(1.0);
                gfx.sprites.quad(
                    t,
                    [0.0, 0.0, 1.0, 1.0],
                    [PANEL.0 + x, PANEL.1 + y - 1.0 + h * (1.0 - fit) / 2.0, w * fit, h * fit],
                    [1.0; 4],
                );
            }
        }

    }

    /// The fork's live lap timing, in a box under the panel: the gap to
    /// the best lap, green ahead and red behind, and each lap's time and
    /// gap for a while as it ends.
    fn draw_lap_gap(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race) {
        if !crate::lapgap::on() || race.stunt_event || race.tag.is_some() {
            return;
        }
        let me = &race.racers[race.player];
        let Some(shown) = me.lap_gap.shown(race.clock()) else {
            return;
        };
        let signed = |g: f32| format!("{g:+.2}");
        let (text, gap) = match shown {
            crate::lapgap::Shown::Lap(time, gap) => (
                match gap {
                    Some(g) => format!("Lap {}  {}", clock_text(time), signed(g)),
                    None => format!("Lap {}", clock_text(time)),
                },
                gap,
            ),
            crate::lapgap::Shown::Gap(g) => (format!("Gap to best {}", signed(g)), Some(g)),
        };
        let colour = match gap {
            Some(g) if g < 0.0 => [0.45, 1.0, 0.45, 1.0],
            Some(g) if g > 0.0 => [1.0, 0.5, 0.45, 1.0],
            _ => [1.0; 4],
        };
        let [x, y, w, h] = GAP_BOX;
        if sharp() {
            draw_box(ui, gfx, GAP_BOX);
        } else {
            ui.fill(gfx, [PANEL.0 + x, PANEL.1 + y, w, h], [0.0, 0.0, 0.0, 0.55]);
        }
        let Some(t) = ui.cell_text(gfx, &text, TEXT, false) else {
            return;
        };
        let (tw, th) = gfx.sprites.size(t);
        let (tw, th) = (tw as f32, th as f32);
        let fit = ((w - 12.0) / tw).min(1.0);
        let at = [PANEL.0 + x + 6.0, PANEL.1 + y + (h - th * fit) / 2.0];
        for (d, c) in [(1.0, [0.0, 0.0, 0.0, 1.0]), (0.0, colour)] {
            gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [at[0] + d, at[1] + d, tw * fit, th * fit], c);
        }
    }

    /// While the player is down: which buttons put them straight back on
    /// the bike (the fork's quick get-up).
    fn draw_get_up(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race) {
        let me = &race.racers[race.player];
        if !race.quick_get_up || !race.gate_down() || me.finished.is_some() || !me.bike.crashed() {
            return;
        }
        let Some(t) = ui.cell_text(gfx, "G or Back: get back on", TEXT, false) else {
            return;
        };
        let (w, h) = gfx.sprites.size(t);
        let (w, h) = (w as f32, h as f32);
        let at = [(320.0 - w / 2.0).round(), GET_UP_Y];
        for (d, c) in [(1.0, [0.0, 0.0, 0.0, 1.0]), (0.0, [1.0, 1.0, 1.0, 0.9])] {
            gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [at[0] + d, at[1] + d, w, h], c);
        }
    }

    fn draw_radar(&mut self, ui: &mut Ui, gfx: &mut Gfx, race: &Race) {
        let bike = &race.racers[race.player].bike;
        // Radar, heading up.
        if sharp() {
            let r = RADAR_DISC;
            let disc = ui.made(gfx, "radar", || {
                painted(2.0 * r, 2.0 * r, 0.58, |x, y| (x - r).hypot(y - r) - r, |x, y| {
                    let (dx, dy) = (x - r, y - r);
                    let d = dx.hypot(dy);
                    let ring = (d - RADAR_RIM).abs().min((d - RADAR_RIM / 2.0).abs());
                    let cross = if d < RADAR_RIM { dx.abs().min(dy.abs()) } else { f32::MAX };
                    // Lines half a pixel wide.
                    (1.5 - ring.min(cross) * 4.0).clamp(0.0, 1.0)
                })
            });
            let (cx, cy) = RADAR_CENTRE;
            gfx.sprites.quad(disc, [0.0, 0.0, 1.0, 1.0], [cx - r, cy - r, 2.0 * r, 2.0 * r], [1.0; 4]);
            // The player's arrow in the middle, which the picture had.
            let [w, h] = ARROW;
            let arrow = ui.made(gfx, "arrow", arrow);
            gfx.sprites.quad(arrow, [0.0, 0.0, 1.0, 1.0], [cx - w / 2.0, cy - h / 2.0, w, h], [1.0; 4]);
        } else if let Some(t) = ui.overlay(gfx, "radar128.tga") {
            gfx.sprites.image(t, RADAR.0, RADAR.1);
        }
        let at = bike.position();
        let forward = bike.forward();
        let right = Vec3::new(forward.z, 0.0, -forward.x);
        let scale = RADAR_RIM / (RADAR_FEET * self.zoom);
        for (i, r) in race.racers.iter().enumerate() {
            if i == race.player {
                continue;
            }
            let rel = r.bike.position() - at;
            let (mut x, mut y) = (rel.dot(right) * scale, -rel.dot(forward) * scale);
            let d = (x * x + y * y).sqrt();
            let colour = if d > RADAR_RIM {
                (x, y) = (x / d * RADAR_RIM, y / d * RADAR_RIM);
                [
                    0x28 as f32 / 255.0,
                    0xaa as f32 / 255.0,
                    0x28 as f32 / 255.0,
                    1.0,
                ]
            } else {
                [
                    0x50 as f32 / 255.0,
                    0xe6 as f32 / 255.0,
                    0x50 as f32 / 255.0,
                    1.0,
                ]
            };
            // A 3 pixel dash, two rows high.
            let (cx, cy) = (RADAR_CENTRE.0 + x, RADAR_CENTRE.1 + y);
            ui.fill(gfx, [cx - 1.5, cy - 1.0, 3.0, 2.0], colour);
        }
        // Whole pixels from the centre (0x5219b0), but not with the
        // modernised HUD: on a big window those step the lines in blocks.
        let whole = |v: f32| if sharp() { v } else { v.trunc() };
        let spot = |p: Vec3| {
            let rel = p - at;
            [whole(rel.dot(right) * scale), whole(-rel.dot(forward) * scale)]
        };
        // The original's lines are one screen pixel wide at any size.
        let width = 1.0 / gfx.sprites.scale.max(1.0);
        let line = |ui: &mut Ui, gfx: &mut Gfx, a: [f32; 2], b: [f32; 2], colour: [f32; 4]| {
            if let Some((a, b)) = clip_to_rim(a, b) {
                let at = |p: [f32; 2]| [RADAR_CENTRE.0 + p[0], RADAR_CENTRE.1 + p[1]];
                ui.line(gfx, at(a), at(b), width, colour);
            }
        };
        if !race.gates.is_empty() {
            // Baja and Enduro (0x522170): the gate the player heads for in
            // yellow and the one after it in white, each post to post, when
            // its first post is on the radar.
            let next = race.racers[race.player].timing.gate;
            for (k, colour) in [(0, RADAR_GATE), (1, [1.0; 4])] {
                let g = &race.gates[(next + k) % race.gates.len()];
                let (a, b) = (spot(g.centre - g.half_width), spot(g.centre + g.half_width));
                if (a[0] * a[0] + a[1] * a[1]).sqrt() < RADAR_RIM {
                    line(ui, gfx, a, b, colour);
                }
            }
        } else if let (Some(course), false) = (&race.course, race.stunt_event || race.tag.is_some()) {
            // Nationals and Supercross (0x521d70): both sides of the
            // course, cut at the rim.
            for (a, b) in course.side_segments() {
                line(ui, gfx, spot(a), spot(b), RADAR_COURSE);
            }
        }
    }
}

impl Hud {
    /// Green text with a black shadow, held for its time.
    fn draw_message(&mut self, ui: &mut Ui, gfx: &mut Gfx, dt: f32) {
        let Some((text, age)) = &mut self.message else {
            return;
        };
        *age += dt;
        if *age > MESSAGE_TIME {
            self.message = None;
            return;
        }
        let Some(t) = ui.cell_text(gfx, text, MESSAGE_TEXT, true) else {
            return;
        };
        let (w, h) = gfx.sprites.size(t);
        let (w, h) = (w as f32, h as f32);
        let y = (MESSAGE[1] + (MESSAGE[3] - h) / 2.0).round();
        for (dx, colour) in [(0.0, [0.0, 0.0, 0.0, 1.0]), (-1.0, [0.0, 1.0, 0.0, 1.0])] {
            gfx.sprites.quad(
                t,
                [0.0, 0.0, 1.0, 1.0],
                [MESSAGE[0] + dx, y + dx, w, h],
                colour,
            );
        }
    }
}

/// The modernised HUD's rounded box at `rect` in the panel.
fn draw_box(ui: &mut Ui, gfx: &mut Gfx, rect: [f32; 4]) {
    let [x, y, w, h] = rect;
    let t = ui.made(gfx, &format!("box{w}x{h}"), || rounded_box(w, h));
    gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], [PANEL.0 + x, PANEL.1 + y, w, h], [1.0; 4]);
}

/// A rounded, see-through grey box `w` by `h` (640×480 pixels).
fn rounded_box(w: f32, h: f32) -> mm2_formats::tga::Image {
    const RADIUS: f32 = 5.0;
    let outside = |x: f32, y: f32| {
        let qx = (x - w / 2.0).abs() - (w / 2.0 - RADIUS);
        let qy = (y - h / 2.0).abs() - (h / 2.0 - RADIUS);
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - RADIUS
    };
    painted(w, h, 0.58, outside, |_, _| 0.0)
}

/// One of the modernised HUD's see-through grey pieces, `w` by `h`
/// (640×480 pixels) at four texels a pixel: `outside(x, y)` is how far a
/// point (pixels from the top left) lies outside the shape, and
/// `lines(x, y)` how much a faint drawn line covers it, 0 to 1. Shaded a
/// little darker towards the bottom, with a faint light rim; `opacity`
/// inside.
fn painted(
    w: f32,
    h: f32,
    opacity: f32,
    outside: impl Fn(f32, f32) -> f32,
    lines: impl Fn(f32, f32) -> f32,
) -> mm2_formats::tga::Image {
    const K: f32 = 4.0;
    let (tw, th) = ((w * K).round() as u32, (h * K).round() as u32);
    let mut rgba = Vec::with_capacity((tw * th * 4) as usize);
    for y in 0..th {
        for x in 0..tw {
            let (px, py) = ((x as f32 + 0.5) / K, (y as f32 + 0.5) / K);
            // How far outside the edge, texels.
            let d = outside(px, py) * K;
            let inside = (0.5 - d).clamp(0.0, 1.0);
            let rim = (1.0 - (d + 2.0).abs() / 1.5).clamp(0.0, 1.0);
            let line = lines(px, py).clamp(0.0, 1.0);
            let shade = 0.17 - 0.07 * (y as f32 / th as f32);
            let grey = shade + (0.8 - shade) * rim.max(line * 0.7);
            let alpha = inside * (opacity + 0.12 * rim + 0.1 * line).min(1.0);
            let v = (grey * 255.0).round() as u8;
            rgba.extend_from_slice(&[v, v, v, (alpha * 255.0).round() as u8]);
        }
    }
    mm2_formats::tga::Image { width: tw, height: th, rgba }
}

/// The radar's red arrow, [`ARROW`] at four texels a pixel, with smooth
/// edges.
fn arrow() -> mm2_formats::tga::Image {
    const K: f32 = 4.0;
    let [w, h] = ARROW.map(|v| v * K);
    let (tw, th) = (w as u32, h as u32);
    // Each side as a line `a·p + c` that is negative inside, in texels.
    let side = |(x0, y0): (f32, f32), (x1, y1): (f32, f32)| {
        let (nx, ny) = (y1 - y0, x0 - x1);
        let l = nx.hypot(ny);
        move |x: f32, y: f32| ((x - x0) * nx + (y - y0) * ny) / l
    };
    let (top, left, right) = ((w / 2.0, 0.0), (0.0, h), (w, h));
    let sides = [side(top, right), side(right, left), side(left, top)];
    let mut rgba = Vec::with_capacity((tw * th * 4) as usize);
    for y in 0..th {
        for x in 0..tw {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = sides.iter().map(|s| s(px, py)).fold(f32::MIN, f32::max);
            let a = (0.5 - d).clamp(0.0, 1.0);
            rgba.extend_from_slice(&[235, 30, 30, (a * 255.0).round() as u8]);
        }
    }
    mm2_formats::tga::Image { width: tw, height: th, rgba }
}

/// The panel's times: `%2d:%05.2f`.
/// Where the needle points at `mph`, radians clockwise from straight up:
/// 0 mph at 230°, 50 straight up and 100 at 130°.
fn dial_angle(mph: f32) -> f32 {
    let v = mph.clamp(0.0, 100.0);
    if v < 50.0 {
        230.0 + 130.0 * v / 50.0
    } else {
        130.0 * (v - 50.0) / 50.0
    }
    .to_radians()
}

pub fn clock_text(t: f32) -> String {
    let m = (t / 60.0).floor();
    format!("{:2}:{:05.2}", m as u32, t - m * 60.0)
}

/// Whether the ground rises between `from` and `to` (0x50c600's test along
/// the line), checked every 2 ft.
fn ground_between(terrain: &mm2_formats::trn::Terrain, from: Vec3, to: Vec3) -> bool {
    let steps = ((to - from).length() / 2.0).ceil().clamp(1.0, 1000.0) as usize;
    (1..steps).any(|k| {
        let p = from.lerp(to, k as f32 / steps as f32);
        crate::ride::ground(terrain, p.x, p.z) > p.y
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_box_is_clear_at_the_corners_and_see_through_inside() {
        let img = rounded_box(20.0, 10.0);
        assert_eq!((img.width, img.height), (80, 40));
        assert_eq!(img.pixel(0, 0)[3], 0);
        let middle = img.pixel(40, 20);
        assert!(middle[3] > 100 && middle[3] < 200, "{middle:?}");
    }

    #[test]
    fn the_arrow_points_up() {
        let img = arrow();
        assert_eq!(img.pixel(10, 22)[3], 255);
        assert_eq!(img.pixel(1, 1)[3], 0);
        assert_eq!(img.pixel(18, 1)[3], 0);
    }

    #[test]
    fn the_dial_runs_from_230_to_130_degrees() {
        let deg = |mph: f32| dial_angle(mph).to_degrees().rem_euclid(360.0);
        assert!((deg(0.0) - 230.0).abs() < 1e-3);
        assert!(deg(50.0).abs() < 1e-3);
        assert!((deg(100.0) - 130.0).abs() < 1e-3);
        assert_eq!(dial_angle(150.0), dial_angle(100.0));
    }
}
