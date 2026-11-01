//! Options (`OptBase.dtm`, `0x4b8880`): a tabbed screen whose tabs are
//! Game, Graphics, Sound, Controls, Messages and Garage. Also here are
//! Advanced Graphics (`OptAdvG.dtm`, `0x4b6dd0`) and the box that puts the
//! default settings back (`GlobSet.dtm`, `0x4b9470`).
//!
//! As in the original the pages edit a copy of the profile's settings,
//! taken when the screen opens and given back (and saved) when Back
//! closes it; each page shows the copy when it opens and writes its
//! controls back into it when another page replaces it.

use super::keys;
use super::{popup, Cx, Go, MsgBox, Screen};
use crate::audio::Voice;
use crate::profile::{Controls, Profile, ACTIONS};
use crate::tweaks::{Tweaks, FOV};
use crate::ui::{Dlg, Event, Gfx, Ui};
use mm2_formats::rs2;
use std::path::Path;

/// The tabs, their text in `LANG.DLL`, and their pages.
const PAGES: [(&str, u32, &str); 6] = [
    ("GameSettings", 5085, "OPTGAME"),
    ("Graphics", 5086, "OPTGRAPH"),
    ("Sound", 5087, "OPTSOUND"),
    ("Controls", 5088, "OPTCONT"),
    ("Messages", 5089, "OPTMESS"),
    ("Garage", 5090, "OPTGAR"),
];

/// Where the pages go (`0x4b90b0`).
const PAGE_AT: (f32, f32) = (20.0, 35.0);

/// The engine sizes, one class each (`0x572bec`).
const ENGINE_SIZES: [u32; 5] = [125, 250, 400, 500, 600];

/// One engine class's power bands, from `UI/UIRES.RES` `presets.pb`
/// (`0x49b460`).
#[derive(Debug, Clone, Default)]
pub struct EngineClass {
    pub rpm_low: i32,
    pub rpm_high: i32,
    pub total: i32,
    pub least: i32,
    /// The highest band limit: the top of the sliders.
    pub most: i32,
    pub limits: [i32; 11],
    pub stock: [[i32; 11]; 3],
}

impl EngineClass {
    fn step(&self) -> i32 {
        (self.rpm_high - self.rpm_low) / 10
    }
}

/// The five classes, or the game's fallback when the file is missing.
pub fn engine_classes(game: &Path) -> [EngineClass; 5] {
    let text = crate::read_archive(&game.join("UI").join("UIRES.RES"))
        .ok()
        .and_then(|b| {
            let ar = rs2::Archive::parse(&b).ok()?;
            let e = ar
                .entries()
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case("presets.pb"))?;
            Some(String::from_utf8_lossy(ar.data(e)).into_owned())
        });
    let Some(text) = text else {
        return std::array::from_fn(|_| EngineClass {
            rpm_low: 5000,
            rpm_high: 10000,
            total: 100,
            least: 0,
            most: 40,
            limits: [40; 11],
            stock: [[10; 11]; 3],
        });
    };
    let ini = mm2_formats::scn::Ini::parse(&text);
    std::array::from_fn(|c| {
        let num = |sec: &str, key: &str, d: i32| {
            ini.section(sec)
                .and_then(|s| s.get(key))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(d)
        };
        let sec = format!("Category_{}", c + 1);
        let low = num(&sec, "RPMLowerLimit", 5000);
        let high = num(&sec, "RPMUpperLimit", 10000);
        let step = (high - low) / 10;
        let band = |sec: &str, b: usize| num(sec, &format!("RPM{:05}", low + b as i32 * step), 10);
        let limits: [i32; 11] = std::array::from_fn(|b| band(&sec, b));
        EngineClass {
            rpm_low: low,
            rpm_high: high,
            total: num(&sec, "HPSum", 400),
            least: num(&sec, "MinRange", 10),
            most: limits.iter().copied().max().unwrap_or(0),
            limits,
            stock: std::array::from_fn(|n| {
                let sec = format!("Category_{}Preset_{}", c + 1, n + 1);
                std::array::from_fn(|b| band(&sec, b))
            }),
        }
    })
}

/// The class of an engine size: those between two sizes count with the
/// lower (`0x4b7f70`).
/// The Bike & Rider page's bike under the player (0x450f00): a bike of
/// another class than the garage's puts that class's chosen curve under
/// the bike, with its revs; the same class leaves the curve as it was.
pub fn ride_bike(garage: &mut crate::profile::Garage, classes: &[EngineClass; 5], cc: u32) {
    let (was, now) = (engine_class(garage.cc), engine_class(cc));
    garage.cc = cc;
    if was == now {
        return;
    }
    let class = &classes[now];
    garage.ride_rpm = [class.rpm_low, class.rpm_high, class.step()];
    let curve = garage.curve[now];
    garage.ride_power = if curve < 3 {
        class.stock[curve]
    } else {
        garage.custom[now][curve - 3]
    };
}

pub fn engine_class(cc: u32) -> usize {
    if cc <= ENGINE_SIZES[0] {
        return 0;
    }
    if cc >= ENGINE_SIZES[4] {
        return 4;
    }
    (1..4)
        .find(|&i| cc > ENGINE_SIZES[i - 1] && cc < ENGINE_SIZES[i + 1])
        .unwrap_or(1)
}

/// The Game page's check boxes and what they set.
fn game_checks(p: &mut Profile) -> [(&'static str, &mut bool); 11] {
    let g = &mut p.game;
    [
        ("RiderPositionCheckBox", &mut g.rider_position),
        ("RaceStatsCheckBox", &mut g.race_stats),
        ("GuagesCheckBox", &mut g.gauges),
        ("ChkUIAnims", &mut g.ui_anims),
        ("OverviewCheckBox", &mut g.overview),
        ("RiderNamesCheckBox", &mut g.rider_names),
        ("ChkPodiums", &mut g.podiums),
        ("ChkIntroMovie", &mut g.intro_movie),
        ("ChkPlayerIndicator", &mut g.player_indicator),
        ("ChkToolTips", &mut g.tooltips),
        ("ChkWreckResetOnTrack", &mut g.wreck_reset),
    ]
}

fn sound_checks(p: &mut Profile) -> [(&'static str, &mut bool); 5] {
    let s = &mut p.sound;
    [
        ("InGameSoundEffectsCheck", &mut s.effects),
        ("SoundHardwareCheckBox", &mut s.hardware),
        ("UserInterfaceSoundsCheckBox", &mut s.ui_sounds),
        ("ChkEax", &mut s.eax),
        ("ChkWaypointChime", &mut s.waypoint_chime),
    ]
}

fn sound_sliders(p: &mut Profile) -> [(&'static str, &mut u32); 2] {
    let s = &mut p.sound;
    [
        ("UserInterfaceVolumeSlider", &mut s.ui_volume),
        ("InGameVolumeSlider", &mut s.game_volume),
    ]
}

fn graphics_checks(p: &mut Profile) -> [(&'static str, &mut bool); 3] {
    let g = &mut p.graphics;
    [
        ("ShadowsCheckBox", &mut g.shadows),
        ("ParticlesCheckBox", &mut g.particles),
        ("SkyCheckBox", &mut g.sky),
    ]
}

/// The launcher's pad vibration (`--vibration=N`, 0 to 100, or `off`
/// as `Some(None)`), in place of the profile's Force Feedback.
pub fn vibration() -> Option<Option<u32>> {
    crate::tweaks::get().vibration
}

fn control_sliders(p: &mut Profile) -> [(&'static str, &mut u32); 8] {
    let c = &mut p.controls;
    [
        ("SteeringSlider", &mut c.steering),
        ("CrossupSlider", &mut c.cross_up),
        ("PitchSlider", &mut c.pitch),
        ("PressThrottleSlider", &mut c.press_throttle),
        ("PressBrakesSlider", &mut c.press_brakes),
        ("ReleaseThrottleSlider", &mut c.release_throttle),
        ("ReleaseBrakesSlider", &mut c.release_brakes),
        ("FeedbackSlider", &mut c.feedback),
    ]
}

fn control_checks(p: &mut Profile) -> [(&'static str, &mut bool); 4] {
    let c = &mut p.controls;
    [
        ("ForceFeedCheck", &mut c.force_feedback),
        ("ChkGasGyro", &mut c.gas_gyro),
        ("ChkBrakeGyro", &mut c.brake_gyro),
        ("ChkAnalogAxes", &mut c.analog_axes),
    ]
}

const SUSPENSION: [&str; 6] = [
    "SldFrontComp",
    "SldFrontDecomp",
    "SldRearComp",
    "SldRearDecomp",
    "SldFrontDamp",
    "SldRearDamp",
];

/// The chat box modes, in the order the box steps through them, with
/// their text.
const CHAT_MODES: [u32; 3] = [5131, 5130, 5132];

/// The advanced screen's quality sliders, after the video memory box.
pub(crate) const DETAILS: [&str; 4] = ["SldEcosystem", "SldObjects", "SldTerrain", "SldVisibility"];

/// Where the Tweaks button sits on the screen.
const TWEAKS_BUTTON: [i32; 4] = [206, 416, 117, 33];

/// One of the Tweaks box's check boxes: its name, text, where it sits,
/// what it says when pointed at, and what it sets.
struct Check {
    name: &'static str,
    text: &'static str,
    rect: [i32; 4],
    about: &'static str,
    field: fn(&mut Tweaks) -> &mut bool,
}

const TWEAK_CHECKS: [Check; 10] = [
    Check {
        name: "ChkAirSteer",
        text: "Air steering",
        rect: [24, 110, 329, 20],
        about: "Steering in the air bends your flight a little and turns the bike with it, so you land in line. Off is the original, where it only turns the bike slightly.",
        field: |t| &mut t.air_steer,
    },
    Check {
        name: "ChkAirBrake",
        text: "Air braking",
        rect: [24, 132, 329, 20],
        about: "Holding the brake in the air takes off a little speed, so you can come up short of a landing. The brake still tips the nose down if Brake Gyro is on.",
        field: |t| &mut t.air_brake,
    },
    Check {
        name: "ChkWideCourse",
        text: "Wider course",
        rect: [24, 154, 329, 20],
        about: "The course counts 20 ft wider on each side before the off-course timer starts, on every difficulty. Off is the original.",
        field: |t| &mut t.wide_course,
    },
    Check {
        name: "ChkQuickGetUp",
        text: "Quick get-up",
        rect: [24, 176, 329, 20],
        about: "After a crash, G on the keyboard or Back on a pad puts you straight back on the bike instead of waiting for the crash to play out. Not in network races.",
        field: |t| &mut t.quick_get_up,
    },
    Check {
        name: "ChkModernHud",
        text: "Modernised HUD",
        rect: [24, 226, 329, 20],
        about: "The race panel, radar and speedometer as clean see-through shapes, crisp at your screen's resolution. Off is the original game's pictures.",
        field: |t| &mut t.modern_hud,
    },
    Check {
        name: "ChkCentred",
        text: "Race display nearer the middle",
        rect: [24, 248, 329, 20],
        about: "On screens wider than 16:9: the speedometer, map and race info where they sit on a 16:9 screen, instead of out at the edges.",
        field: |t| &mut t.centred,
    },
    Check {
        name: "ChkLapGap",
        text: "Live gap to your best lap",
        rect: [24, 270, 329, 20],
        about: "Under the race panel: how far ahead (green) or behind (red) your best lap you are, and each lap's time as it ends. Comparing starts from lap 2.",
        field: |t| &mut t.lap_gap,
    },
    Check {
        name: "ChkSteady",
        text: "Steadier camera",
        rect: [381, 140, 207, 20],
        about: "The chase camera rises and dips with the bike more gently, so bumps don't shake the view. It still turns with the bike as the original does.",
        field: |t| &mut t.steady_camera,
    },
    Check {
        name: "ChkVsync",
        text: "VSync",
        rect: [366, 318, 248, 20],
        about: "Draws a picture each time the screen refreshes: smooth, with no tearing. Off draws as fast as the computer can.",
        field: |t| &mut t.vsync,
    },
    Check {
        name: "ChkShowFps",
        text: "Show frame rate",
        rect: [366, 342, 248, 20],
        about: "The frames a second, small at the bottom left of the screen, in the menus and in races. F2 shows or hides it too.",
        field: |t| &mut t.show_fps,
    },
];

/// The vibration check box, which sets a strength rather than a flag.
const VIBRATION_CHECK: (&str, &str, [i32; 4], &str) = (
    "ChkVibration",
    "Controller vibration",
    [381, 180, 207, 20],
    "The pad thumps when you land, knocks over sharp bumps and jolts when you crash, on pads that can vibrate.",
);

/// The Tweaks box's sliders (the advanced screen's) and what they say
/// when pointed at.
const FOV_SLIDER: (&str, &str) = (
    "SldEcosystem",
    "How wide the riding cameras see. 85° is the original game's; wider shows more round the bike.",
);
const VIBRATION_SLIDER: (&str, &str) = ("SldVisibility", "How hard the pad vibrates.");

/// The Tweaks box's Difficulty button, where it sits and what it says
/// when pointed at.
const DIFFICULTY_BUTTON: [i32; 4] = [24, 70, 117, 33];
const DIFFICULTY_BUTTON_ABOUT: &str =
    "Easy, Normal and Hard, each set from 1 to 10: the other riders' speed and skill, how easily you crash, and their tricks in the Stunt Quarry.";

/// One row of the Difficulty box's table: its text, what it says when
/// pointed at, and what a step stands for, `#` its value from
/// [`mm2_settings::DIFFICULTY_STEPS`].
struct Row {
    text: &'static str,
    about: &'static str,
    shows: &'static str,
}

/// The table's rows, in [`mm2_settings::DIFFICULTY`]'s order. Higher is
/// harder on each.
const DIFFICULTY_ROWS: [Row; 4] = [
    Row {
        text: "Rivals' speed",
        about: "How hard the other riders' engines pull. Higher gets them out of slow corners quicker; their top speed hardly changes, as they already have more than they use.",
        shows: "their engines at #% of the game's Normal",
    },
    Row {
        text: "Rivals' skill",
        about: "How big a hit the other riders can take before they fall. Higher and they crash less.",
        shows: "they crash at #% of the game's Normal",
    },
    Row {
        text: "How easily you crash",
        about: "How small a hit throws you off your bike. Higher and you crash sooner.",
        shows: "you take #% of the game's Normal hit",
    },
    Row {
        text: "Rivals' tricks (Stunt Quarry)",
        about: "In the Stunt Quarry, how often the other riders do tricks. Higher and they wait less between them.",
        shows: "they wait # s between tricks",
    },
];

/// What the Difficulty box says when the pointer is on none of it.
const DIFFICULTY_ABOUT: &str =
    "Point at a row to see what it does. Each is 1 to 10, higher is harder; the original game's Easy, Normal and Hard are 3, 5 and 7. The single event's Difficulty Level, or a Pro Circuit's or network game's, picks the column.";
const ORIGINAL_ABOUT: &str = "Puts back the original game's numbers.";

/// Where the table's first arrow, the heading row and the first row sit,
/// and the spacing of its columns and rows.
const TABLE: (i32, i32, i32, i32, i32) = (368, 62, 100, 82, 40);
const ORIGINAL_BUTTON: [i32; 4] = [472, 416, 117, 33];

/// The pieces of the advanced screen the Difficulty box doesn't use.
const NOT_DIFFICULTY: [&str; 29] = [
    "SldVisibility",
    "UnusedSlide03Text",
    "Less03",
    "More03",
    "rbwLabel1",
    "rbwLabel3",
    "rbwLabel4",
    "SldTerrain",
    "SldObjects",
    "rbwLabel5",
    "rbwLabel6",
    "rbwLabel7",
    "SldEcosystem",
    "rbwLabel8",
    "rbwLabel9",
    "rbwLabel10",
    "rbwLabel11",
    "StartupCardSelectionText",
    "ChkShowPick3D",
    "Txt3DCard",
    "ChkForceVideoMem",
    "rbwLabel2",
    "RadTerrSingle",
    "RadTerrDual",
    "RadTerrNone",
    "rbwLabel12",
    "ChkFilterInput",
    "ChkShowPickController",
    "Help",
];

/// A table cell's arrow, from its name (`DifLess21`): which way, its row
/// and its level.
fn difficulty_arrow(name: &str) -> Option<(i32, usize, usize)> {
    let (by, rest) = match name.get(..7) {
        Some("DifLess") => (-1, &name[7..]),
        Some("DifMore") => (1, &name[7..]),
        _ => return None,
    };
    let d: Vec<usize> = rest.chars().map(|c| c.to_digit(10).map(|d| d as usize)).collect::<Option<_>>()?;
    match d[..] {
        [r, l] if r < 4 && l < 3 => Some((by, r, l)),
        _ => None,
    }
}

/// `v` an arrow's step up or down, from 1 to 10.
fn step_difficulty(v: u32, by: i32) -> u32 {
    v.saturating_add_signed(by).clamp(1, mm2_settings::MOST_STEP)
}

/// What the Tweaks box says when the pointer is on none of them.
const TWEAKS_ABOUT: &str =
    "Point at a tweak to see what it does. Changes count from the next race, and the launcher keeps them too.";

/// The pieces of the advanced screen the Tweaks box doesn't use.
const NOT_TWEAKS: [&str; 20] = [
    "ChkForceVideoMem",
    "rbwLabel2",
    "RadTerrSingle",
    "RadTerrDual",
    "RadTerrNone",
    "rbwLabel12",
    "ChkFilterInput",
    "CardSelectionBox",
    "StartupCardSelectionText",
    "ChkShowPick3D",
    "Txt3DCard",
    "ChkShowPickController",
    "SldObjects",
    "rbwLabel5",
    "rbwLabel6",
    "rbwLabel7",
    "SldTerrain",
    "rbwLabel4",
    "rbwLabel1",
    "rbwLabel3",
];

#[allow(clippy::large_enum_variant)]
enum Sub {
    Advanced(Dlg),
    /// The fork's Additional Tweaks, and the tweaks as the box opened.
    Tweaks(Dlg, Tweaks),
    /// The fork's difficulty table, as it is now.
    Difficulty(Dlg, [[u32; 3]; 4]),
    Restore { dlg: Dlg, confirm: Option<MsgBox> },
}

pub struct Options {
    base: Dlg,
    page: Option<(usize, Dlg)>,
    sub: Option<Sub>,
    /// The copy the pages edit.
    p: Profile,
    classes: [EngineClass; 5],
    /// The garage page's bands as shown, and the power they use.
    bands: [i32; 11],
    used: i32,
    /// The controls page's action waiting for a key.
    listening: Option<usize>,
    ratchet: Option<Voice>,
    ratchet_at: f32,
}

impl Options {
    /// The screen, on page `page` (0 the game's; the single player screen
    /// opens it on 3 for the controls and 5 for the garage, `0x44df50`).
    pub fn open(cx: &mut Cx, page: usize) -> Option<Options> {
        let mut base = Dlg::open(cx.ui, "OPTBASE", (0.0, 0.0))?;
        for (tab, id, _) in PAGES {
            base.set_text(tab, cx.ui.lang(id));
        }
        let mut o = Options {
            base,
            page: None,
            sub: None,
            p: cx.user.profile.clone(),
            classes: engine_classes(cx.game()),
            bands: [0; 11],
            used: 0,
            listening: None,
            ratchet: None,
            ratchet_at: f32::MIN,
        };
        // The fork's Tweaks box, between Help and Advanced.
        o.base.add_copy("Restore", "Tweaks", TWEAKS_BUTTON);
        o.base.set_text("Tweaks", "Tweaks...");
        o.show_page(cx, page.min(5));
        // Without a profile there is nothing to restore (`0x527300`).
        if cx.user.store.list().is_empty() {
            o.base.show("Restore", false);
        }
        Some(o)
    }

    /// Opens page `page` in place of the one showing (`0x4b90b0`).
    fn show_page(&mut self, cx: &mut Cx, page: usize) {
        if self.page.as_ref().is_some_and(|(i, _)| *i == page) {
            return;
        }
        self.close_page(cx);
        for (i, (tab, _, _)) in PAGES.iter().enumerate() {
            self.base.set_on(tab, i == page);
        }
        let Some(dlg) = Dlg::open(cx.ui, PAGES[page].2, PAGE_AT) else {
            return;
        };
        self.page = Some((page, dlg));
        self.fill(cx);
    }

    fn close_page(&mut self, cx: &mut Cx) {
        if self.page.is_some() {
            self.commit(cx);
        }
        self.page = None;
        self.listening = None;
    }

    fn dlg(&mut self) -> Option<&mut Dlg> {
        self.page.as_mut().map(|(_, d)| d)
    }

    /// The copy into the page's controls (the handlers' open, and slot 31
    /// with 0).
    fn fill(&mut self, cx: &mut Cx) {
        let Some((page, d)) = self.page.as_mut() else {
            return;
        };
        let p = &mut self.p;
        match *page {
            0 => {
                for (name, v) in game_checks(p) {
                    d.set_on(name, *v);
                }
                let mode = p.game.chat_mode.min(2) as usize;
                d.set_text("MultChatMode", cx.ui.lang(CHAT_MODES[mode]));
                // With no product ID from Setup the game shows a blank one.
                let pid = cx
                    .user
                    .settings
                    .get("PID")
                    .filter(|p| !p.is_empty())
                    .unwrap_or("00000-000-0000000-00000")
                    .to_string();
                d.set_text("TxtPID", format!("{}: {pid}", cx.ui.lang(5047)));
            }
            1 => {
                // Not the game's 16-bit modes of the time: full screen or
                // a window of a size that fits, changed at once.
                let modes = crate::display::modes();
                let now = crate::display::current();
                let at = modes.iter().position(|m| *m == now).unwrap_or(0);
                let modes = modes.iter().map(|m| m.text()).collect();
                d.set_items("DisplayResolutionDropDown", modes, at);
                // 0x4b6ac0: white and centred.
                d.set_color("DisplayResolutionDropDown", 0xffffff);
                d.set_align("DisplayResolutionDropDown", 0x12);
                d.set_slider("TerrainQualitySliderBar", p.graphics.terrain, None);
                for (name, v) in graphics_checks(p) {
                    d.set_on(name, *v);
                }
            }
            2 => {
                for (name, v) in sound_checks(p) {
                    d.set_on(name, *v);
                }
                for (name, v) in sound_sliders(p) {
                    d.set_slider(name, *v, None);
                }
                d.set_on("Rad16Bit", p.sound.sixteen_bit);
                d.set_on("Rad8Bit", !p.sound.sixteen_bit);
            }
            3 => {
                for (name, v) in control_sliders(p) {
                    d.set_slider(name, *v, None);
                }
                for (name, v) in control_checks(p) {
                    d.set_on(name, *v);
                }
                // The analog axes box is hidden (`0x4b74e0`), and so is
                // force feedback without a pad that can vibrate, as the
                // original shows it with no joystick that has it.
                d.show("ChkAnalogAxes", false);
                // The launcher's vibration setting stands in for it.
                let rumble = keys::pad().is_some_and(|p| p.rumble) && vibration().is_none();
                for name in [
                    "ForceFeedCheck",
                    "FeedbackSlider",
                    "WeakerText",
                    "StrongerText",
                ] {
                    d.show(name, rumble);
                }
                let actions = (0..ACTIONS as u32).map(|i| cx.ui.lang(402 + i)).collect();
                d.set_items("ActionListBox", actions, 0);
                // The pad plugged in by its own name in place of its kind.
                let pad = keys::pad();
                let devices = (0..8)
                    .map(|i| match &pad {
                        Some(p) if p.device == i && !p.name.is_empty() => p.name.clone(),
                        _ => cx.ui.lang(460 + i as u32),
                    })
                    .collect();
                d.set_items("InputDeviceDDL", devices, p.controls.device as usize);
                d.set_selectable("ActionListBox", true);
                d.set_selectable("MapKeyListBox", true);
                Self::fill_keys(cx.ui, d, p);
            }
            4 => {
                for i in 0..10 {
                    let name = format!("Message{i}");
                    d.set_text(&name, p.messages[i].clone());
                    if let Some(c) = d.ctl(&name) {
                        c.max_len = c.max_len.min(127);
                    }
                }
            }
            _ => {
                self.fill_curves(cx.ui);
                self.show_bands();
            }
        }
    }

    /// The controls page's keys on the device shown (`0x4b7af0`).
    fn fill_keys(ui: &Ui, d: &mut Dlg, p: &Profile) {
        let device = p.controls.device as usize;
        let keys = (0..ACTIONS)
            .map(|a| {
                let (kind, code) = p.keys.get(device, a);
                keys::input_name(kind, code, |id| ui.lang(id))
            })
            .collect();
        let at = d.selected("MapKeyListBox");
        d.set_items("MapKeyListBox", keys, at);
    }

    /// The page's controls into the copy (slot 31 with 1, and the bound
    /// controls as the page closes).
    fn commit(&mut self, cx: &mut Cx) {
        let Some((page, d)) = self.page.as_mut() else {
            return;
        };
        let p = &mut self.p;
        match *page {
            0 => {
                for (name, v) in game_checks(p) {
                    *v = d.get(name).is_some_and(|c| c.on);
                }
            }
            1 => {
                p.graphics.terrain = d.slider("TerrainQualitySliderBar");
                for (name, v) in graphics_checks(p) {
                    *v = d.get(name).is_some_and(|c| c.on);
                }
            }
            2 => self.commit_sound(cx.ui),
            3 => {
                for (name, v) in control_sliders(p) {
                    *v = d.slider(name);
                }
                for (name, v) in control_checks(p) {
                    *v = d.get(name).is_some_and(|c| c.on);
                }
                p.controls.device = d.selected("InputDeviceDDL") as u32;
                p.keys.current = p.controls.device;
            }
            4 => {
                for i in 0..10 {
                    if let Some(t) = d.get(&format!("Message{i}")).and_then(|c| c.text.clone()) {
                        p.messages[i] = t;
                    }
                }
            }
            _ => {
                for (i, name) in SUSPENSION.iter().enumerate() {
                    p.garage.suspension[i] = d.slider_at(name, 100) as f32 * 0.01;
                }
            }
        }
    }

    /// The sound page into the copy, and into the menus' volume at once
    /// (`0x4b7420`).
    fn commit_sound(&mut self, ui: &mut Ui) {
        let Some((2, d)) = self.page.as_mut() else {
            return;
        };
        let p = &mut self.p;
        for (name, v) in sound_checks(p) {
            *v = d.get(name).is_some_and(|c| c.on);
        }
        for (name, v) in sound_sliders(p) {
            *v = d.slider(name).min(100);
        }
        p.sound.sixteen_bit = d.get("Rad16Bit").is_some_and(|c| c.on);
        ui.volume = p.sound.ui_gain();
    }

    // ---- The garage page (`0x4b7be0`) ----

    /// DDLCurves: the class's three stock curves and the player's three
    /// (`0x4b7f70`).
    fn fill_curves(&mut self, ui: &mut Ui) {
        let cc = self.p.garage.cc;
        let cls = engine_class(cc);
        let size = ui.lang(5226).replace("%d", &ENGINE_SIZES[cls].to_string());
        let (stock, custom) = (ui.lang(5227), ui.lang(5228));
        let lines = (0..6)
            .map(|i| {
                if i < 3 {
                    format!("{cc}cc ({size}) {stock} #{}", i + 1)
                } else {
                    format!("{cc}cc ({size}) {custom} #{}", i - 2)
                }
            })
            .collect();
        let curve = self.p.garage.curve[cls];
        if let Some(d) = self.dlg() {
            d.set_items("DDLCurves", lines, curve);
        }
        self.choose_curve(curve);
    }

    fn choose_curve(&mut self, curve: usize) {
        let cls = engine_class(self.p.garage.cc);
        self.p.garage.curve[cls] = curve;
        self.bands = if curve < 3 {
            self.classes[cls].stock[curve]
        } else {
            self.p.garage.custom[cls][curve - 3]
        };
    }

    /// Fills in each rpm band's slider and figure. The stock curves show
    /// greyed out; the player's own are editable (`0x4b8240`).
    fn show_bands(&mut self) {
        let cls = engine_class(self.p.garage.cc);
        let class = self.classes[cls].clone();
        let curve = self.p.garage.curve[cls];
        let suspension = self.p.garage.suspension;
        let Some(d) = self.page.as_mut().map(|(_, d)| d) else {
            return;
        };
        d.show("TxtUnused", curve >= 3);
        d.show("HPUnused", curve >= 3);
        let span = (class.most - class.least).max(0) as u32;
        self.used = 0;
        for b in 0..11 {
            let power = self.bands[b];
            let rpm = (b as i32 * class.step() + class.rpm_low) as f32 * 0.001;
            d.set_text(&format!("Txt{}", b + 1), format!("{rpm:2.1}k"));
            let slider = format!("SldEQ{}", b + 1);
            if let Some(c) = d.ctl(&slider) {
                c.reversed = true;
            }
            d.set_steps(&slider, span + 1);
            d.set_slider(&slider, (power - class.least).max(0) as u32, Some(span));
            d.enable(&slider, curve >= 3);
            self.used += power;
            d.set_text(&format!("HP{}", b + 1), power.to_string());
        }
        d.set_text("HPUnused", (class.total - self.used).to_string());
        for (i, name) in SUSPENSION.iter().enumerate() {
            d.set_slider(name, (suspension[i] * 100.0) as u32, Some(100));
        }
    }

    /// Called when the player drags one band's slider. The new figure is
    /// clamped to that band's cap and to the unspent power, then stored in
    /// the selected curve (`0x4b8120`).
    fn slide_band(&mut self, band: usize) {
        let cls = engine_class(self.p.garage.cc);
        let class = &self.classes[cls];
        let span = (class.most - class.least).max(0) as u32;
        let Some(d) = self.page.as_ref().map(|(_, d)| d) else {
            return;
        };
        let value = d.slider_at(&format!("SldEQ{}", band + 1), span) as i32 + class.least;
        let most = value.min(class.limits[band]);
        let power = (self.bands[band] + (class.total - self.used)).min(most);
        self.bands[band] = power;
        let curve = self.p.garage.curve[cls];
        if curve >= 3 {
            self.p.garage.custom[cls][curve - 3][band] = power;
        }
        // The suspension is read back before the bands redraw it.
        if let Some((_, d)) = self.page.as_ref() {
            for (i, name) in SUSPENSION.iter().enumerate() {
                self.p.garage.suspension[i] = d.slider_at(name, 100) as f32 * 0.01;
            }
        }
        self.show_bands();
    }

    // ---- The controls page (`0x4b74e0`) ----

    /// An action picked: "press any key" in its place until one is
    /// (`0x4b74e0` event 2).
    fn listen(&mut self, ui: &Ui, line: usize) {
        if line >= ACTIONS {
            return;
        }
        self.listening = Some(line);
        if let Some(d) = self.dlg() {
            let mut keys = d
                .get("MapKeyListBox")
                .map(|c| c.items.clone())
                .unwrap_or_default();
            if let Some(k) = keys.get_mut(line) {
                *k = ui.lang(450);
            }
            d.set_items("MapKeyListBox", keys, line);
            d.mark("MapKeyListBox", Some(line));
            d.mark("ActionListBox", Some(line));
        }
    }

    /// Binds the key just let go to the waiting action, unbinding it from
    /// whichever action held it before. Escape cancels and leaves the
    /// binding alone (`0x4b9c00`).
    fn take_key(&mut self, ui: &Ui, code: u32) {
        if code == 0x01 {
            self.listening = None;
            self.refill_keys(ui);
        } else {
            self.take_input(ui, (0, code as i32));
        }
    }

    /// Binds a key or joystick input to the waiting action. The game's
    /// page took only keys for the steering and lean (`0x4b9b70`) and only
    /// buttons from a joystick; here a stick pushed binds its axis to the
    /// steering or lean (both sides at once, as the factory map has
    /// them), and Accelerate also takes a stick's throttle lever (the Z
    /// axis), which the game's map can hold (0x449e20).
    fn take_input(&mut self, ui: &Ui, input: (i32, i32)) {
        let Some(action) = self.listening else {
            return;
        };
        let pair = match action {
            0 | 1 => Some([0, 1]),
            2 | 3 => Some([2, 3]),
            _ => None,
        };
        match input {
            (2, code) if code < 0 && pair.is_none() && !(action == crate::input::ACCELERATE && code == -7) => {
                return
            }
            _ => {}
        }
        self.listening = None;
        // A joystick's input binds on the joystick's own map: shown the
        // keyboard or mouse, the page turns to the pad (as the race reads it).
        if input.0 == 2 && self.p.controls.device as usize >= crate::input::KEYBOARD {
            let pad = keys::pad().map_or(3, |p| p.device);
            self.p.controls.device = pad as u32;
            if let Some(d) = self.dlg() {
                d.select("InputDeviceDDL", pad);
            }
        }
        let device = self.p.controls.device as usize;
        if let (Some(pair), (2, code)) = (pair, input) {
            if code < 0 {
                for a in 0..ACTIONS {
                    if !pair.contains(&a) && self.p.keys.get(device, a) == input {
                        self.p.keys.set(device, a, (-1, 0));
                    }
                }
                for a in pair {
                    self.p.keys.set(device, a, input);
                }
                self.refill_keys(ui);
                return;
            }
        }
        for a in 0..ACTIONS {
            if a != action && self.p.keys.get(device, a) == input {
                self.p.keys.set(device, a, (-1, 0));
            }
        }
        self.p.keys.set(device, action, input);
        self.refill_keys(ui);
    }

    fn refill_keys(&mut self, ui: &Ui) {
        if let Some((_, d)) = self.page.as_mut() {
            Self::fill_keys(ui, d, &self.p);
            d.mark("MapKeyListBox", None);
            d.mark("ActionListBox", None);
        }
    }

    /// The ratchet sound while a slider moves (`0x4b6ac0`).
    fn ratchet(&mut self, cx: &mut Cx, on: bool) {
        let Some(audio) = cx.audio else {
            return;
        };
        // Not again while it is still clicking.
        if on && self.ratchet.is_some() && cx.now - self.ratchet_at < 0.15 {
            return;
        }
        self.ratchet_at = cx.now;
        if let Some(v) = self.ratchet.take() {
            audio.stop(v);
        }
        if on && cx.ui.volume > 0.0 {
            let pack = self
                .page
                .as_ref()
                .map_or("OPTBASE".to_string(), |(_, d)| d.pack.clone());
            if let Some(s) = cx.ui.sound(&pack, "Ratchet03") {
                self.ratchet = Some(audio.play(&s, cx.ui.volume, false));
            }
        }
    }

    fn page_event(&mut self, cx: &mut Cx, e: Event) {
        let Some(page) = self.page.as_ref().map(|(i, _)| *i) else {
            return;
        };
        match (page, e) {
            (0, Event::Clicked(n)) if n.eq_ignore_ascii_case("MultChatMode") => {
                self.p.game.chat_mode = (self.p.game.chat_mode + 1) % 3;
                let text = cx.ui.lang(CHAT_MODES[self.p.game.chat_mode as usize]);
                if let Some(d) = self.dlg() {
                    d.set_text("MultChatMode", text);
                }
            }
            (1, Event::Changed(n)) => {
                self.ratchet(cx, true);
                if n.eq_ignore_ascii_case("TerrainQualitySliderBar") {
                    // The overall quality sets all four details at once.
                    let q = self
                        .dlg()
                        .map_or(0, |d| d.slider("TerrainQualitySliderBar"));
                    self.p.graphics.terrain = q;
                    for key in DETAILS {
                        cx.user.settings.set(key, &q.to_string());
                    }
                }
            }
            (1, Event::Selected(n, line)) if n.eq_ignore_ascii_case("DisplayResolutionDropDown") => {
                if let Some(&m) = crate::display::modes().get(line) {
                    crate::display::request(m);
                }
            }
            (2, Event::Clicked(_)) => self.commit_sound(cx.ui),
            (2, Event::Changed(_)) => {
                self.commit_sound(cx.ui);
                self.ratchet(cx, true);
            }
            (3, Event::Selected(n, line))
                if n.eq_ignore_ascii_case("ActionListBox")
                    || n.eq_ignore_ascii_case("MapKeyListBox") =>
            {
                if self.listening.is_none() {
                    self.listen(cx.ui, line);
                }
            }
            (3, Event::Selected(n, line)) if n.eq_ignore_ascii_case("InputDeviceDDL") => {
                self.p.controls.device = line as u32;
                let ui = &*cx.ui;
                if let Some((_, d)) = self.page.as_mut() {
                    Self::fill_keys(ui, d, &self.p);
                }
            }
            (3, Event::Changed(n)) if !n.eq_ignore_ascii_case("ListboxScroll") => {
                self.ratchet(cx, true)
            }
            (5, Event::Clicked(n)) => {
                let n = n.to_ascii_lowercase();
                if n == "curveleft" || n == "curveright" {
                    let cls = engine_class(self.p.garage.cc);
                    let by = if n == "curveleft" { 5 } else { 1 };
                    let curve = (self.p.garage.curve[cls] + by) % 6;
                    if let Some(d) = self.dlg() {
                        d.select("DDLCurves", curve);
                    }
                    self.choose_curve(curve);
                    self.show_bands();
                }
            }
            (5, Event::Selected(n, line)) if n.eq_ignore_ascii_case("DDLCurves") => {
                self.choose_curve(line.min(5));
                self.show_bands();
            }
            (5, Event::Changed(n)) | (5, Event::Released(n)) => {
                self.ratchet(cx, true);
                let band = n
                    .get(..5)
                    .filter(|p| p.eq_ignore_ascii_case("SldEQ"))
                    .and_then(|_| n[5..].parse::<usize>().ok());
                if let Some(b) = band.filter(|b| (1..=11).contains(b)) {
                    self.slide_band(b - 1);
                }
            }
            (_, Event::Released(_)) => self.ratchet(cx, false),
            _ => {}
        }
    }

    // ---- Advanced (`0x4b6dd0`) ----

    fn open_advanced(&mut self, cx: &mut Cx) {
        if self.page.as_ref().is_some_and(|(i, _)| *i == 1) {
            self.commit(cx);
        }
        let Some(mut d) = Dlg::open(cx.ui, "OPTADVG", (0.0, 0.0)) else {
            return;
        };
        let s = &cx.user.settings;
        d.set_on("ChkForceVideoMem", s.number("ForceVideoMemory", 0) != 0);
        for key in DETAILS {
            d.set_slider(
                key,
                s.number(key, self.p.graphics.terrain as i64).clamp(0, 9) as u32,
                None,
            );
        }
        // This renderer is never the software one.
        d.set_text("Txt3DCard", "Hardware (wgpu)");
        d.set_on("ChkShowPick3D", s.number("UseLastVideoCard", 1) == 0);
        let method = s.number("TerrainDetailTextureMethod", -1);
        let radio = match method {
            0 => "RadTerrNone",
            1 => "RadTerrDual",
            _ => "RadTerrSingle",
        };
        d.set_on(radio, true);
        d.set_on("ChkFilterInput", s.number("JoystickFilter", 0) != 0);
        d.set_on(
            "ChkShowPickController",
            s.number("UseLastController", 0) == 0,
        );
        self.sub = Some(Sub::Advanced(d));
    }

    /// Closing the advanced screen keeps its choices, and the overall
    /// quality becomes the four details' mean.
    fn close_advanced(&mut self, cx: &mut Cx) {
        let Some(Sub::Advanced(d)) = self.sub.take() else {
            return;
        };
        let s = &mut cx.user.settings;
        let on = |n: &str| d.get(n).is_some_and(|c| c.on);
        s.set(
            "ForceVideoMemory",
            &(on("ChkForceVideoMem") as u32).to_string(),
        );
        let mut sum = 0;
        for key in DETAILS {
            let v = d.slider(key);
            sum += v;
            s.set(key, &v.to_string());
        }
        s.set(
            "UseLastVideoCard",
            &(!on("ChkShowPick3D") as u32).to_string(),
        );
        let method = if on("RadTerrSingle") {
            2
        } else {
            on("RadTerrDual") as u32
        };
        s.set("TerrainDetailTextureMethod", &method.to_string());
        s.set("JoystickFilter", &(on("ChkFilterInput") as u32).to_string());
        s.set(
            "UseLastController",
            &(!on("ChkShowPickController") as u32).to_string(),
        );
        cx.user.save_settings();
        self.p.graphics.terrain = sum / 4;
        if let Some((1, _)) = &self.page {
            self.fill(cx);
        }
    }

    // ---- Tweaks (the fork's) ----

    /// The Additional Tweaks, in a box made from the advanced screen's
    /// pieces: riding and the race display on the left, the camera and
    /// pad on the plate, and what the tweak under the pointer does below.
    fn open_tweaks(&mut self, cx: &mut Cx) {
        let Some(mut d) = Dlg::open(cx.ui, "OPTADVG", (0.0, 0.0)) else {
            return;
        };
        let mut t = crate::tweaks::get();
        d.set_text("AdvancedGraphicsTitle", "Additional Tweaks");
        d.add_copy("rbwLabel12", "TwkDisplay", [24, 200, 249, 25]);
        d.set_text("TwkDisplay", "Race Display:");
        d.add_copy("rbwLabel12", "TwkScreen", [366, 290, 249, 25]);
        d.set_text("TwkScreen", "Screen:");
        // In the frame the background has there.
        d.add_copy("StartupCardSelectionText", "TwkAbout", [33, 303, 294, 80]);
        d.set_text("TwkAbout", TWEAKS_ABOUT);
        if let Some(c) = d.ctl("TwkAbout") {
            c.wrap = true;
        }
        d.add_from(&self.base, "Restore", "Difficulty", DIFFICULTY_BUTTON);
        d.set_text("Difficulty", "Difficulty...");
        for c in &TWEAK_CHECKS {
            d.add_copy("ChkForceVideoMem", c.name, c.rect);
            d.set_text(c.name, c.text);
            d.set_on(c.name, *(c.field)(&mut t));
        }
        let (name, text, rect, _) = VIBRATION_CHECK;
        d.add_copy("ChkForceVideoMem", name, rect);
        d.set_text(name, text);
        for name in NOT_TWEAKS {
            d.show(name, false);
        }
        d.set_text("rbwLabel11", "Camera and Controller");
        let fov = FOV_SLIDER.0;
        if let Some(c) = d.ctl(fov) {
            c.range = FOV.1 - FOV.0;
        }
        d.set_steps(fov, (FOV.1 - FOV.0) / 5 + 1);
        d.set_slider(fov, t.fov - FOV.0, None);
        let vib = VIBRATION_SLIDER.0;
        if let Some(c) = d.ctl(vib) {
            c.range = 100;
        }
        d.set_steps(vib, 21);
        // Without the launcher's setting, the profile's Force Feedback.
        let c = &self.p.controls;
        let (on, strength) = match t.vibration {
            Some(v) => (v.is_some(), v.unwrap_or(c.feedback)),
            None => (c.force_feedback, c.feedback),
        };
        d.set_on(VIBRATION_CHECK.0, on);
        d.set_slider(vib, strength, None);
        Self::label_tweaks(&mut d, t.fov, strength, on);
        self.sub = Some(Sub::Tweaks(d, t));
    }

    /// The sliders' headings, with their values.
    fn label_tweaks(d: &mut Dlg, fov: u32, strength: u32, vibration: bool) {
        d.set_text("rbwLabel8", format!("Field of view: {fov}°"));
        d.set_text("UnusedSlide03Text", format!("Vibration strength: {strength}%"));
        d.enable(VIBRATION_SLIDER.0, vibration);
    }

    /// The box's tweaks, and its vibration strength and whether it is on.
    fn read_tweaks(d: &Dlg, was: Tweaks) -> (Tweaks, u32, bool) {
        let mut t = was;
        let on = |n: &str| d.get(n).is_some_and(|c| c.on);
        for c in &TWEAK_CHECKS {
            *(c.field)(&mut t) = on(c.name);
        }
        t.fov = FOV.0 + d.slider(FOV_SLIDER.0);
        (t, d.slider(VIBRATION_SLIDER.0), on(VIBRATION_CHECK.0))
    }

    /// Closing keeps the tweaks, saved for the launcher too.
    fn close_tweaks(&mut self) {
        let Some(Sub::Tweaks(d, was)) = self.sub.take() else {
            return;
        };
        let (mut t, strength, on) = Self::read_tweaks(&d, was);
        let vibration = on.then_some(strength);
        // Left to the profile while the box shows the profile's.
        let c = &self.p.controls;
        if was.vibration.is_some() || vibration != c.force_feedback.then_some(c.feedback) {
            t.vibration = Some(vibration);
        }
        if t != was {
            crate::tweaks::set(t);
        }
    }

    fn tweaks_event(&mut self, cx: &mut Cx, e: Event) {
        let Some(Sub::Tweaks(d, was)) = &mut self.sub else {
            return;
        };
        match e {
            Event::Clicked(n) if n.eq_ignore_ascii_case("Back") => self.close_tweaks(),
            Event::Clicked(n) if n.eq_ignore_ascii_case("Difficulty") => {
                self.close_tweaks();
                self.open_difficulty(cx);
            }
            Event::Clicked(_) | Event::Changed(_) | Event::Released(_) => {
                let (t, strength, on) = Self::read_tweaks(d, *was);
                Self::label_tweaks(d, t.fov, strength, on);
            }
            _ => {}
        }
    }

    /// What the tweak under the pointer does.
    fn describe_tweak(&mut self) {
        let Some(Sub::Tweaks(d, _)) = &mut self.sub else {
            return;
        };
        let about = d.hovered().and_then(|h| {
            let is = |n: &str| n.eq_ignore_ascii_case(h);
            let check = TWEAK_CHECKS.iter().find(|c| is(c.name)).map(|c| c.about);
            let vibration = is(VIBRATION_CHECK.0).then_some(VIBRATION_CHECK.3);
            let vibration = vibration.or(is("Difficulty").then_some(DIFFICULTY_BUTTON_ABOUT));
            let slider = [FOV_SLIDER, VIBRATION_SLIDER].into_iter().find(|s| is(s.0)).map(|s| s.1);
            check.or(vibration).or(slider)
        });
        let about = about.unwrap_or(TWEAKS_ABOUT);
        d.set_text("TwkAbout", about);
    }

    // ---- The fork's Difficulty box ----

    /// The advanced screen's pieces again, with the single event page's
    /// arrows and number boxes for a table of Easy, Normal and Hard.
    fn open_difficulty(&mut self, cx: &mut Cx) {
        let (Some(mut d), Some(event)) = (
            Dlg::open(cx.ui, "OPTADVG", (0.0, 0.0)),
            Dlg::open(cx.ui, "SPEVENT", (0.0, 0.0)),
        ) else {
            return;
        };
        let (x, head, top, across, down) = TABLE;
        d.set_text("AdvancedGraphicsTitle", "Difficulty");
        for (l, name) in ["Easy", "Normal", "Hard"].into_iter().enumerate() {
            let at = x + l as i32 * across;
            let id = format!("DifHead{l}");
            d.add_copy("rbwLabel11", &id, [at - 6, head, 89, 21]);
            d.set_text(&id, name);
        }
        for (r, row) in DIFFICULTY_ROWS.iter().enumerate() {
            let y = top + r as i32 * down;
            let id = format!("DifRow{r}");
            d.add_copy("rbwLabel2", &id, [24, y, 330, 21]);
            d.set_text(&id, row.text);
            for l in 0..3 {
                let at = x + l as i32 * across;
                d.add_from(&event, "OpponentsLeftarrow", &format!("DifLess{r}{l}"), [at, y, 21, 21]);
                d.add_from(&event, "OpponentsListBox", &format!("DifBox{r}{l}"), [at + 22, y, 33, 21]);
                d.add_from(&event, "OpponentsrightArrow", &format!("DifMore{r}{l}"), [at + 56, y, 21, 21]);
            }
        }
        d.add_copy("StartupCardSelectionText", "DifAbout", [33, 303, 294, 80]);
        d.set_text("DifAbout", DIFFICULTY_ABOUT);
        if let Some(c) = d.ctl("DifAbout") {
            c.wrap = true;
        }
        d.add_from(&self.base, "Restore", "DifOriginal", ORIGINAL_BUTTON);
        d.set_text("DifOriginal", "Original");
        for name in NOT_DIFFICULTY {
            d.show(name, false);
        }
        let table = crate::tweaks::get().difficulty;
        Self::show_difficulty(&mut d, &table);
        self.sub = Some(Sub::Difficulty(d, table));
    }

    /// The table's numbers into their boxes.
    fn show_difficulty(d: &mut Dlg, table: &[[u32; 3]; 4]) {
        for (r, row) in table.iter().enumerate() {
            for (l, v) in row.iter().enumerate() {
                d.set_items(&format!("DifBox{r}{l}"), vec![v.to_string()], 0);
            }
        }
    }

    /// Closing keeps the table, saved for the launcher too, and goes back
    /// to the Tweaks box.
    fn close_difficulty(&mut self, cx: &mut Cx) {
        let Some(Sub::Difficulty(_, table)) = self.sub.take() else {
            return;
        };
        let mut t = crate::tweaks::get();
        if t.difficulty != table {
            t.difficulty = table;
            crate::tweaks::set(t);
        }
        self.open_tweaks(cx);
    }

    fn difficulty_event(&mut self, cx: &mut Cx, e: Event) {
        let Some(Sub::Difficulty(d, table)) = &mut self.sub else {
            return;
        };
        let Event::Clicked(n) = e else {
            return;
        };
        if n.eq_ignore_ascii_case("Back") {
            return self.close_difficulty(cx);
        }
        if n.eq_ignore_ascii_case("DifOriginal") {
            *table = mm2_settings::DIFFICULTY;
        } else if let Some((by, r, l)) = difficulty_arrow(&n) {
            table[r][l] = step_difficulty(table[r][l], by);
        } else {
            return;
        }
        Self::show_difficulty(d, table);
    }

    /// What the row under the pointer does, and on a cell what its step
    /// stands for.
    fn describe_difficulty(&mut self) {
        let Some(Sub::Difficulty(d, table)) = &mut self.sub else {
            return;
        };
        let original = d.hovered().is_some_and(|h| h.eq_ignore_ascii_case("DifOriginal"));
        // Anywhere along a row, its text included.
        let (left, head, first, across, down) = TABLE;
        let (x, y) = d.pointer();
        let row = (y - first as f32 + (down as f32 - 21.0) / 2.0) / down as f32;
        let row = (x > 16.0 && x < 620.0 && row >= 0.0 && y > head as f32 + 24.0)
            .then_some(row as usize)
            .filter(|&r| r < DIFFICULTY_ROWS.len());
        let column = (x >= left as f32 - 6.0)
            .then(|| ((x - left as f32 + 6.0) / across as f32) as usize)
            .filter(|&l| l < 3);
        let about = match (original, row, column) {
            (true, ..) => ORIGINAL_ABOUT.to_string(),
            (_, Some(r), Some(l)) => {
                let row = &DIFFICULTY_ROWS[r];
                let step = table[r][l];
                let value = mm2_settings::difficulty_value(r, step).to_string();
                let level = ["Easy", "Normal", "Hard"][l];
                format!("{} {level} at {step} of 10: {}.", row.about, row.shows.replace('#', &value))
            }
            (_, Some(r), None) => DIFFICULTY_ROWS[r].about.to_string(),
            _ => DIFFICULTY_ABOUT.to_string(),
        };
        d.set_text("DifAbout", &about);
    }

    // ---- Restore (`0x4b9470`) ----

    fn open_restore(&mut self, cx: &mut Cx) {
        let Some(mut d) = popup(cx.ui, "GLOBSET") else {
            return;
        };
        // The other players' profiles (`0x4b97a0`).
        let others: Vec<String> = cx
            .user
            .store
            .list()
            .into_iter()
            .filter(|n| *n != self.p.name)
            .collect();
        if others.is_empty() {
            d.enable("ButCopyUser", false);
        }
        d.set_items("LstProfiles", others, 0);
        d.set_selectable("LstProfiles", true);
        d.mark("LstProfiles", Some(0));
        d.show("LstProfiles", false);
        d.show("ScrollProfiles", false);
        d.set_on("ButFactory", true);
        self.sub = Some(Sub::Restore {
            dlg: d,
            confirm: None,
        });
    }

    fn restore_event(&mut self, cx: &mut Cx, e: Event) {
        let Some(Sub::Restore { dlg, confirm }) = &mut self.sub else {
            return;
        };
        if confirm.is_some() {
            let Event::Clicked(n) = e else { return };
            if n.eq_ignore_ascii_case("ButLeft") {
                // Confirmed (0x60): the factory settings, or another
                // player's, under this player's name (`0x4b9a40`).
                let factory = dlg.get("ButFactory").is_some_and(|c| c.on);
                let other = dlg
                    .get("LstProfiles")
                    .and_then(|c| c.mark.and_then(|m| c.items.get(m).cloned()));
                let messages = super::default_messages(cx.ui);
                let preset = Controls::preset(cx.game());
                if factory {
                    self.p.defaults(messages, preset);
                } else if let Some(o) = other {
                    if let Some(mut q) = cx.user.store.load(&o, messages, preset) {
                        q.name = self.p.name.clone();
                        q.bike = self.p.bike.clone();
                        q.outfit = self.p.outfit.clone();
                        q.plate = self.p.plate;
                        self.p = q;
                    }
                }
                self.sub = None;
                self.fill(cx);
                if let Some((2, _)) = &self.page {
                    self.commit_sound(cx.ui);
                }
            } else if n.eq_ignore_ascii_case("ButRight") {
                self.sub = None;
            }
            return;
        }
        let Event::Clicked(n) = e else { return };
        match n.to_ascii_lowercase().as_str() {
            "butfactory" => {
                dlg.show("LstProfiles", false);
                dlg.show("ScrollProfiles", false);
            }
            "butcopyuser" => {
                dlg.show("LstProfiles", true);
                dlg.show("ScrollProfiles", true);
            }
            "butcancel" => self.sub = None,
            "butload" => {
                let (t, p) = (cx.ui.lang(5321), cx.ui.lang(5322));
                if let Some(mut b) = MsgBox::ask(cx.ui, &t, &p, [5097, 0, 5084]) {
                    b.dlg.set_wrap("TxtPrompt", true);
                    *confirm = Some(b);
                }
            }
            _ => {}
        }
    }
}

impl Screen for Options {
    fn layers(&mut self) -> Vec<&mut Dlg> {
        match &mut self.sub {
            Some(Sub::Advanced(d)) | Some(Sub::Tweaks(d, _)) | Some(Sub::Difficulty(d, _)) => vec![d],
            Some(Sub::Restore { dlg, confirm }) => {
                let mut v = vec![&mut self.base];
                if let Some((_, p)) = &mut self.page {
                    v.push(p);
                }
                v.push(dlg);
                if let Some(b) = confirm {
                    v.push(&mut b.dlg);
                }
                v
            }
            None => {
                let mut v = vec![&mut self.base];
                if let Some((_, p)) = &mut self.page {
                    v.push(p);
                }
                v
            }
        }
    }

    fn modal(&self) -> bool {
        self.sub.is_some()
    }

    fn event(&mut self, cx: &mut Cx, e: Event) -> Go {
        match &self.sub {
            Some(Sub::Advanced(_)) => {
                if let Event::Clicked(n) = &e {
                    if n.eq_ignore_ascii_case("Back") {
                        self.close_advanced(cx);
                    }
                }
                return Go::Stay;
            }
            Some(Sub::Restore { .. }) => {
                self.restore_event(cx, e);
                return Go::Stay;
            }
            Some(Sub::Tweaks(..)) => {
                self.tweaks_event(cx, e);
                return Go::Stay;
            }
            Some(Sub::Difficulty(..)) => {
                self.difficulty_event(cx, e);
                return Go::Stay;
            }
            None => {}
        }
        if let Event::Clicked(n) = &e {
            let lower = n.to_ascii_lowercase();
            if let Some(i) = PAGES.iter().position(|(t, _, _)| t.eq_ignore_ascii_case(n)) {
                self.show_page(cx, i);
                return Go::Stay;
            }
            match lower.as_str() {
                "back" => {
                    // Leaving the screen hands the edited copy back (`0x4b8880` close).
                    self.close_page(cx);
                    cx.user.profile = self.p.clone();
                    cx.ui.volume = self.p.sound.ui_gain();
                    cx.user.save();
                    self.ratchet(cx, false);
                    return Go::Back;
                }
                "advanced" => {
                    self.open_advanced(cx);
                    return Go::Stay;
                }
                "tweaks" => {
                    self.open_tweaks(cx);
                    return Go::Stay;
                }
                "restore" => {
                    self.open_restore(cx);
                    return Go::Stay;
                }
                // The help file is WinHelp, which no system opens now.
                "help" => return Go::Stay,
                _ => {}
            }
        }
        self.page_event(cx, e);
        Go::Stay
    }

    fn escape(&mut self, cx: &mut Cx) -> Go {
        match &mut self.sub {
            Some(Sub::Advanced(_)) => self.close_advanced(cx),
            Some(Sub::Tweaks(..)) => self.close_tweaks(),
            Some(Sub::Difficulty(..)) => self.close_difficulty(cx),
            Some(Sub::Restore { confirm, .. }) => {
                if confirm.is_some() {
                    *confirm = None;
                } else {
                    self.sub = None;
                }
            }
            None if self.listening.is_some() => self.take_key(cx.ui, 0x01),
            None => return self.event(cx, Event::Clicked("Back".into())),
        }
        Go::Stay
    }

    fn key(&mut self, cx: &mut Cx, code: u32) -> Go {
        if self.sub.is_none() && self.listening.is_some() {
            self.take_key(cx.ui, code);
        }
        Go::Stay
    }

    fn wants_pad(&self) -> bool {
        self.sub.is_none() && self.listening.is_some()
    }

    fn pad(&mut self, cx: &mut Cx, input: (i32, i32)) -> Go {
        if self.sub.is_none() && self.listening.is_some() {
            self.take_input(cx.ui, input);
        }
        Go::Stay
    }

    fn tick(&mut self, _cx: &mut Cx) -> Go {
        self.describe_tweak();
        self.describe_difficulty();
        Go::Stay
    }

    fn draw(&mut self, ui: &mut Ui, gfx: &mut Gfx, now: f32) {
        for d in self.layers() {
            d.tick(now);
            d.draw(ui, gfx, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_arrows_step_from_1_to_10() {
        assert_eq!(difficulty_arrow("DifMore21"), Some((1, 2, 1)));
        assert_eq!(difficulty_arrow("DifLess30"), Some((-1, 3, 0)));
        assert_eq!(difficulty_arrow("DifBox21"), None);
        assert_eq!(step_difficulty(5, 1), 6);
        assert_eq!(step_difficulty(5, -1), 4);
        assert_eq!(step_difficulty(1, -1), 1);
        assert_eq!(step_difficulty(10, 1), 10);
    }

    #[test]
    fn classes() {
        assert_eq!(engine_class(125), 0);
        assert_eq!(engine_class(250), 1);
        assert_eq!(engine_class(300), 1);
        assert_eq!(engine_class(400), 2);
        assert_eq!(engine_class(450), 2);
        assert_eq!(engine_class(500), 3);
        assert_eq!(engine_class(650), 4);
    }

    #[test]
    fn presets() {
        let Some(game) = std::env::var_os("MM2_DATA") else {
            return;
        };
        let c = engine_classes(Path::new(&game));
        assert_eq!(
            (c[1].rpm_low, c[1].rpm_high, c[1].total),
            (5000, 10000, 380)
        );
        assert_eq!(c[1].least, 20);
        assert_eq!(c[1].most, 45);
        assert_eq!(c[1].stock[0][0], 24);
        assert_eq!(c[0].limits[10], 20);
    }
}
