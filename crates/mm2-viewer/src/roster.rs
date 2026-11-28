//! The bikes and rider outfits on offer, from `UI/UIRES.RES` `bikes.pb`
//! and `riders.pb` as the game reads them (0x49b7d0), and the player's
//! choice of them.

use mm2_formats::{rs2, scn};
use std::path::Path;

/// One bike of the Bike & Rider page.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BikeChoice {
    /// "Honda CR250": the maker and the bike (0x450630 shows `%s %s`).
    pub name: String,
    /// The race model (`honda.mcf`) and the one the menus turn
    /// (`uihonda.mcf`), in `RES/BIKE.RES`.
    pub model: String,
    pub ui_model: String,
    /// The paint, replacing the model's first texture.
    pub texture: String,
    pub cc: u32,
    pub four_stroke: bool,
    /// A paint from the game's `BIKES` folder (0x49bcfc, the game's own
    /// `BIKE01.TGA` among them): any engine size, picked on the page.
    pub own: bool,
    /// A Free Pack bike's engine sizes, picked in the page's engine size
    /// box (ours): the bike is listed once, and `cc` is the size picked.
    /// Empty for the game's bikes, which come in their own sizes.
    pub sizes: Vec<FreeSize>,
}

/// One engine size of a Free Pack bike.
#[derive(Debug, Clone, PartialEq)]
pub struct FreeSize {
    pub cc: u32,
    pub four_stroke: bool,
    /// The name it was listed and saved under ("OpenMX Ridgeback 250"):
    /// profiles keep it, so the size comes back with the bike.
    pub name: String,
}

impl BikeChoice {
    /// The name a profile keeps for the bike: a Free Pack bike's with its
    /// engine size.
    pub fn saved_name(&self) -> &str {
        self.sizes.iter().find(|s| s.cc == self.cc).map_or(&self.name, |s| &s.name)
    }

    /// Whether `name` (as a profile keeps it) is this bike, in any size.
    pub fn named(&self, name: &str) -> bool {
        self.name == name || self.sizes.iter().any(|s| s.name == name)
    }

    /// A Free Pack bike in the size nearest `cc`; the game's bikes keep
    /// their own. Whether the size changed.
    pub fn set_size(&mut self, cc: u32) -> bool {
        let Some(s) = self.sizes.iter().min_by_key(|s| s.cc.abs_diff(cc)) else {
            return false;
        };
        let changed = s.cc != self.cc;
        (self.cc, self.four_stroke) = (s.cc, s.four_stroke);
        changed
    }
}

/// "Ridgeback 250" -> "Ridgeback": a Free Pack bike's name without its
/// engine size, which the size box shows instead.
fn without_size(name: &str) -> &str {
    match name.rsplit_once(' ') {
        Some((bike, size)) if size.starts_with(|c: char| c.is_ascii_digit()) => bike,
        _ => name,
    }
}

/// One rider outfit.
#[derive(Debug, Clone, PartialEq)]
pub struct Outfit {
    pub name: String,
    pub file: String,
}

/// What the player rides.
#[derive(Debug, Clone, PartialEq)]
pub struct Ride {
    pub bike: BikeChoice,
    pub outfit: String,
    pub plate: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Roster {
    pub bikes: Vec<BikeChoice>,
    pub outfits: Vec<Outfit>,
    /// Where the Free Pack's bikes and outfits start in the lists (they
    /// come after the game's and the player's own): ours.
    pub free_from: (usize, usize),
}



/// Shown in the lists: `Availability` 2 and 3. 1 is kept from the player
/// (the amateurs' kit), and 10 is the award bike, shown once the career is
/// won (0x450630). The "all gear" extra shows the amateurs' kit too.
fn offered(availability: i32) -> bool {
    matches!(availability, 2 | 3) || (availability == 1 && (crate::tweaks::more().all_gear || crate::demo::active()))
}

/// The model a paint from the `BIKES` folder goes on: the game's template
/// (`BIKES/BIKE01.TGA`) is laid out for the Yamaha frame.
pub const OWN_MODEL: (&str, &str) = ("yamaha.mcf", "uiyamaha.mcf");

/// The maker name of the bike the `BIKES` folder's paints go on (0x573a2c).
pub const USER_MAP: &str = "(User Map)";

/// The maker name the free pack's bikes are listed under.
pub const FREE_MAKER: &str = "OpenMX";

/// The name a loose skin is found by in `BIKE.RES`, where
/// [`crate::addons`] puts it: `own-bikes-<file>` or `own-riders-<file>`
/// (no slash, as texture names lose their folders).
pub fn own_key(folder: &str, file: &str) -> String {
    format!("own-{}-{}", folder.to_ascii_lowercase(), file.to_ascii_lowercase())
}

/// The `.tga` files in the game folder's `folder` (`BIKES`, `RIDERS`),
/// sorted, as (file name, shown name). The game shows the name up to its
/// first dot (0x49be6a).
pub fn own_files(game: &Path, folder: &str) -> Vec<(String, String)> {
    let Some(dir) = crate::fs::read_dir(game)
        .ok()
        .and_then(|l| l.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(folder)))
        .map(|e| e.path())
    else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = crate::fs::read_dir(&dir)
        .map(|l| {
            l.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    let (_, ext) = name.rsplit_once('.')?;
                    let stem = name.split('.').next().unwrap_or_default().to_string();
                    ext.eq_ignore_ascii_case("tga").then(|| (name.clone(), stem))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(n, _)| n.to_ascii_lowercase());
    out
}

impl Roster {
    /// Reads the lists from the game's `UI/UIRES.RES`.
    pub fn load(game: &Path) -> Roster {
        let mut roster = Roster::default();
        if let Some(originals) = Roster::originals(game) {
            roster = originals;
            // The cut content extra's paints, for the player's lists only
            // (the computer riders keep the game's).
            if crate::tweaks::more().cut_content {
                add_cut(&mut roster, crate::demo::bike_files(game).unwrap_or_default());
            }
            roster.add_own(game);
        }
        roster.add_free();
        roster
    }

    /// The game's own bikes and outfits alone (for the demo, the few it
    /// really has), without the player's paints or the free pack's.
    pub fn originals(game: &Path) -> Option<Roster> {
        let bytes = crate::read_archive(&game.join("UI").join("UIRES.RES")).unwrap_or_default();
        let ar = rs2::Archive::parse(&bytes).ok()?;
        let text = |name: &str| {
            ar.get(name)
                .map(|b| String::from_utf8_lossy(b).into_owned())
        };
        let mut roster = Roster {
            bikes: text("bikes.pb").map(|t| bikes(&t)).unwrap_or_default(),
            outfits: text("riders.pb").map(|t| outfits(&t)).unwrap_or_default(),
            ..Roster::default()
        };
        // The demo's lists name the whole game's bikes and outfits, but it
        // has the models and paints of only a few: the rest are left out.
        if crate::demo::active() {
            if let Some(have) = crate::demo::bike_files(game) {
                let has = |f: &str| have.contains(&f.to_ascii_lowercase());
                roster.bikes.retain(|b| has(&b.model) && has(&b.ui_model) && (b.texture.is_empty() || has(&b.texture)));
                roster.outfits.retain(|o| has(&o.file));
            }
        }
        roster.free_from = (roster.bikes.len(), roster.outfits.len());
        Some(roster)
    }

    /// The free pack's bikes and outfits, after the game's. Each bike is
    /// listed once, its engine sizes picked in the size box (the pack lists
    /// one of each model and size).
    fn add_free(&mut self) {
        self.free_from = (self.bikes.len(), self.outfits.len());
        self.add_free_bikes(crate::freepack::listed_bikes());
        for o in crate::freepack::outfits() {
            self.outfits.push(Outfit { name: o.name.clone(), file: o.outfit.clone() });
        }
    }

    /// The pack's bikes, one entry for each model and paint, in the order
    /// first listed, with the sizes it comes in; the smallest picked.
    fn add_free_bikes<'a>(&mut self, listed: impl Iterator<Item = &'a crate::freepack::Bike>) {
        let from = self.bikes.len();
        for b in listed {
            let size = FreeSize { cc: b.cc, four_stroke: b.four_stroke, name: format!("{FREE_MAKER} {}", b.name) };
            let model = format!("{}.mcf", b.shape);
            let name = format!("{FREE_MAKER} {}", without_size(&b.name));
            match self.bikes[from..].iter_mut().find(|c| c.model == model && c.texture == b.livery && c.name == name) {
                Some(c) => c.sizes.push(size),
                None => self.bikes.push(BikeChoice {
                    name,
                    model,
                    ui_model: format!("ui{}.mcf", b.shape),
                    texture: b.livery.clone(),
                    cc: b.cc,
                    four_stroke: b.four_stroke,
                    own: false,
                    sizes: vec![size],
                }),
            }
        }
        for c in &mut self.bikes[from..] {
            c.sizes.sort_by_key(|s| s.cc);
            let first = c.sizes[0].cc;
            c.set_size(first);
        }
    }

    /// The bike a profile kept (by [`BikeChoice::saved_name`]), in the size
    /// it kept.
    pub fn find_bike(&mut self, name: &str) -> Option<usize> {
        let i = self.bikes.iter().position(|b| b.named(name))?;
        let b = &mut self.bikes[i];
        if let Some(cc) = b.sizes.iter().find(|s| s.name == name).map(|s| s.cc) {
            b.set_size(cc);
        }
        Some(i)
    }

    /// The paints and outfits in the game's `BIKES` and `RIDERS` folders,
    /// after the lists' own, as the game always reads them (0x49bcfc,
    /// 0x49c2f9). Paints go on the "(User Map)" bike.
    fn add_own(&mut self, game: &Path) {
        for (file, name) in own_files(game, "BIKES") {
            self.bikes.push(BikeChoice {
                name: format!("{USER_MAP} {name}"),
                model: OWN_MODEL.0.into(),
                ui_model: OWN_MODEL.1.into(),
                texture: own_key("BIKES", &file),
                cc: 250,
                four_stroke: false,
                own: true,
                sizes: Vec::new(),
            });
        }
        for (file, name) in own_files(game, "RIDERS") {
            self.outfits.push(Outfit { name, file: own_key("RIDERS", &file) });
        }
    }

    /// Whether bike `i` is one of the Free Pack's.
    pub fn free_bike(&self, i: usize) -> bool {
        i >= self.free_from.0 && i < self.bikes.len()
    }

    /// Whether outfit `i` is one of the Free Pack's.
    pub fn free_outfit(&self, i: usize) -> bool {
        i >= self.free_from.1 && i < self.outfits.len()
    }

    /// The bikes' and outfits' names as the lists show them. The Free
    /// Pack's bikes go by their maker, OpenMX; with the game's own (or the
    /// demo's) outfits listed too, the pack's go by it as well ("OpenMX
    /// Yellow"), so they aren't taken for the game's.
    pub fn shown(&self) -> (Vec<String>, Vec<String>) {
        let bikes = self.bikes.iter().map(|c| c.name.clone()).collect();
        let o = self.free_from.1;
        let outfits = self
            .outfits
            .iter()
            .enumerate()
            .map(|(i, c)| if i >= o && o > 0 { format!("{FREE_MAKER} {}", c.name) } else { c.name.clone() })
            .collect();
        (bikes, outfits)
    }

    /// The bike and outfit the game starts a new profile on: the first of
    /// each, with our pick of plate (the game's run from 101 to 999).
    pub fn default_ride(&self) -> Option<Ride> {
        Some(Ride {
            bike: self.bikes.first()?.clone(),
            outfit: self.outfits.first()?.file.clone(),
            plate: 107,
        })
    }
}

/// Paints in `RES/BIKE.RES` that `bikes.pb` and `riders.pb` never name, so
/// the original never offers them: the cut content extra lists them.
/// Each bike goes after the last of its maker's, on that maker's model
/// (the paints are laid out for it); the suit after the game's outfits.
/// (maker, bike name, paint, cc, four-stroke)
pub const CUT_BIKES: [(&str, &str, &str, u32, bool); 2] = [
    // KTM's enduro, with its own black and white flame graphics.
    ("KTM", "520EXC", "KTM520EXC_08.tga", 520, true),
    // The YZ426F's paint as it was for the 1999 YZ400F.
    ("Yamaha", "YZ400F", "YamahaYZ400F_08.tga", 400, true),
];

/// The rider suit no list names (yellow and blue, a red skull and
/// crossbones): (shown name, paint).
pub const CUT_OUTFITS: [(&str, &str); 1] = [("MPR", "MPR_08.tga")];

/// Adds the cut bikes and outfits whose paints `have` (the lowercase names
/// in `BIKE.RES`) holds.
fn add_cut(roster: &mut Roster, have: std::collections::HashSet<String>) {
    let has = |f: &str| have.contains(&f.to_ascii_lowercase());
    for (maker, bike, paint, cc, four_stroke) in CUT_BIKES {
        let prefix = format!("{maker} ");
        let Some(last) = roster.bikes.iter().rposition(|b| b.name.starts_with(&prefix) && !b.own) else {
            continue;
        };
        if !has(paint) || roster.bikes.iter().any(|b| b.texture.eq_ignore_ascii_case(paint)) {
            continue;
        }
        let like = roster.bikes[last].clone();
        roster.bikes.insert(
            last + 1,
            BikeChoice { name: format!("{prefix}{bike}"), texture: paint.into(), cc, four_stroke, ..like },
        );
    }
    for (name, paint) in CUT_OUTFITS {
        if has(paint) && !roster.outfits.iter().any(|o| o.file.eq_ignore_ascii_case(paint)) {
            roster.outfits.push(Outfit { name: name.into(), file: paint.into() });
        }
    }
}

fn int(s: &scn::Section, key: &str) -> Option<i32> {
    s.get(key)?.trim().parse().ok()
}

fn bikes(text: &str) -> Vec<BikeChoice> {
    let ini = scn::Ini::parse(text);
    let makers = ini
        .section("Information")
        .and_then(|s| int(s, "NumberOfManufacturers"))
        .unwrap_or(0);
    let mut out = Vec::new();
    for m in 1..=makers {
        let Some(s) = ini.section(&format!("Manufacturer_{m:02}")) else {
            continue;
        };
        if !offered(int(s, "Availability").unwrap_or(1)) {
            continue;
        }
        let maker = s.get("Name").unwrap_or("");
        for b in 1..=int(s, "NumberOfBikes").unwrap_or(0) {
            let key = |k: &str| s.get(&format!("{k}_{b:02}")).map(str::to_string);
            out.push(BikeChoice {
                name: format!("{maker} {}", key("BikeName").unwrap_or_default()),
                model: s.get("ModelFile").unwrap_or("").to_string(),
                ui_model: s.get("UIModelFile").unwrap_or("").to_string(),
                texture: key("BikeTexture").unwrap_or_default(),
                cc: key("BikeEngineSize")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(250),
                four_stroke: key("BikeFourStroke").is_some_and(|v| v.trim() == "1"),
                own: false,
                sizes: Vec::new(),
            });
        }
    }
    out
}

fn outfits(text: &str) -> Vec<Outfit> {
    let ini = scn::Ini::parse(text);
    let count = ini
        .section("Information")
        .and_then(|s| int(s, "NumberOfRiders"))
        .unwrap_or(0);
    (1..=count)
        .filter_map(|r| ini.section(&format!("Rider_{r:02}")))
        .filter(|s| offered(int(s, "Availability").unwrap_or(1)))
        .map(|s| Outfit {
            name: s.get("Name").unwrap_or("").to_string(),
            file: s.get("File").unwrap_or("").to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_lists() {
        let bikes = bikes(
            "[Information]\nNumberOfManufacturers = 2\n[Manufacturer_01]\nName = \"Honda\"\n\
             NumberOfBikes = 1\nUIModelFile = uihonda.mcf\nModelFile = honda.mcf\nAvailability = 3\n\
             BikeName_01 = \"CR250\"\nBikeTexture_01 = HondaCR250_08.tga\nBikeEngineSize_01 = 250\n\
             BikeFourStroke_01 = 0\n[Manufacturer_02]\nName = \"Generic\"\nNumberOfBikes = 1\n\
             Availability = 1\nBikeName_01 = \"125\"\n",
        );
        assert_eq!(bikes.len(), 1);
        assert_eq!(bikes[0].name, "Honda CR250");
        assert_eq!((bikes[0].cc, bikes[0].four_stroke), (250, false));
        assert_eq!(bikes[0].texture, "HondaCR250_08.tga");
        let outfits = outfits(
            "[Information]\nNumberOfRiders=2\n[Rider_01]\nFile=A.tga\nName=\"A\"\nAvailability = 3\n\
             [Rider_02]\nFile=B.tga\nName=\"B\"\nAvailability = 1\n",
        );
        assert_eq!(
            outfits,
            [Outfit {
                name: "A".into(),
                file: "A.tga".into()
            }]
        );
    }

    /// With the game's files beside the engine, the lists fill.
    #[test]
    fn game_lists() {
        let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../GAME");
        if !crate::fs::is_file(game.join("UI/UIRES.RES")) {
            return;
        }
        let r = Roster::load(&game);
        assert!(r.bikes.len() > 10, "{}", r.bikes.len());
        assert!(r.outfits.len() > 40, "{}", r.outfits.len());
        assert_eq!(r.bikes[0].name, "Honda CR125");
    }

    /// The cut paints go in beside their makers' bikes, once each, and
    /// only when the game's files hold them.
    #[test]
    fn cut_paints_join_the_lists() {
        let b = |name: &str, model: &str, paint: &str| BikeChoice {
            name: name.into(),
            model: model.into(),
            ui_model: format!("ui{model}"),
            texture: paint.into(),
            cc: 250,
            four_stroke: false,
            own: false,
            sizes: Vec::new(),
        };
        let mut r = Roster {
            bikes: vec![
                b("Yamaha YZ426F", "yamaha.mcf", "YamahaYZ426F_08.tga"),
                b("KTM 520SX", "ktm.mcf", "KTM520SX_08.tga"),
                b("Generic 125", "yamaha.mcf", "x.tga"),
            ],
            outfits: vec![Outfit { name: "A".into(), file: "A.tga".into() }],
            free_from: (0, 0),
        };
        let have: std::collections::HashSet<String> =
            ["ktm520exc_08.tga", "yamahayz400f_08.tga", "mpr_08.tga"].map(String::from).into();
        add_cut(&mut r, have.clone());
        add_cut(&mut r, have);
        let names: Vec<_> = r.bikes.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Yamaha YZ426F", "Yamaha YZ400F", "KTM 520SX", "KTM 520EXC", "Generic 125"]);
        assert_eq!((r.bikes[3].model.as_str(), r.bikes[3].cc, r.bikes[3].four_stroke), ("ktm.mcf", 520, true));
        assert_eq!(r.outfits.len(), 2);
        assert_eq!(r.outfits[1].file, "MPR_08.tga");
        // Without the paints, nothing.
        let mut r2 = Roster { bikes: vec![b("KTM 520SX", "ktm.mcf", "KTM520SX_08.tga")], ..Roster::default() };
        add_cut(&mut r2, Default::default());
        assert_eq!(r2.bikes.len(), 1);
        assert!(r2.outfits.is_empty());
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;

    fn bike(name: &str) -> BikeChoice {
        BikeChoice {
            name: name.into(),
            model: String::new(),
            ui_model: String::new(),
            texture: String::new(),
            cc: 250,
            four_stroke: false,
            own: false,
            sizes: Vec::new(),
        }
    }

    #[test]
    fn free_ones_are_marked_beside_the_games() {
        let outfit = |n: &str| Outfit { name: n.into(), file: String::new() };
        let mut r = Roster {
            bikes: vec![bike("Suzuki Kazuyuki 250"), bike("OpenMX Red 250")],
            outfits: vec![outfit("Jeans & Flannel"), outfit("Red")],
            free_from: (1, 1),
        };
        let (b, o) = r.shown();
        assert_eq!(b, ["Suzuki Kazuyuki 250", "OpenMX Red 250"]);
        assert_eq!(o, ["Jeans & Flannel", "OpenMX Red"]);
        assert!(!r.free_bike(0) && r.free_bike(1) && r.free_outfit(1));
        // The free version alone: nothing to tell apart.
        r.bikes.remove(0);
        r.outfits.remove(0);
        r.free_from = (0, 0);
        assert_eq!(r.shown(), (vec!["OpenMX Red 250".to_string()], vec!["Red".to_string()]));
    }

    #[test]
    fn free_bikes_are_listed_once_with_their_sizes() {
        let pack = |shape: &str, cc: u32, name: &str| crate::freepack::Bike {
            shape: shape.into(),
            livery: format!("{shape}_red"),
            cc,
            four_stroke: name.ends_with('F'),
            name: name.into(),
            listed: true,
        };
        let listed = [
            pack("mx2", 250, "Ridgeback 250"),
            pack("mx2", 125, "Ridgeback 125"),
            pack("mx2", 600, "Ridgeback 600F"),
            pack("two", 250, "Kestrel 250"),
        ];
        let mut r = Roster { bikes: vec![bike("Suzuki Kazuyuki 250")], ..Roster::default() };
        r.add_free_bikes(listed.iter());
        let names: Vec<&str> = r.bikes.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Suzuki Kazuyuki 250", "OpenMX Ridgeback", "OpenMX Kestrel"]);
        let ridgeback = &r.bikes[1];
        assert_eq!(ridgeback.sizes.iter().map(|s| s.cc).collect::<Vec<_>>(), [125, 250, 600]);
        assert_eq!((ridgeback.cc, ridgeback.saved_name()), (125, "OpenMX Ridgeback 125"));
        // A profile's saved size comes back, and the box picks the nearest.
        assert_eq!(r.find_bike("OpenMX Ridgeback 600F"), Some(1));
        assert_eq!((r.bikes[1].cc, r.bikes[1].four_stroke), (600, true));
        assert!(r.bikes[1].set_size(400));
        assert_eq!(r.bikes[1].cc, 250);
        assert!(!r.bikes[1].set_size(250));
        assert_eq!(r.bikes[1].saved_name(), "OpenMX Ridgeback 250");
        assert_eq!(r.find_bike("OpenMX Ridgeback"), Some(1));
        assert_eq!(r.find_bike("Suzuki Kazuyuki 250"), Some(0));
        assert!(!r.bikes[0].set_size(600));
    }
}

#[cfg(test)]
mod own_tests {
    #[test]
    fn finds_loose_skins() {
        let dir = std::env::temp_dir().join(format!("mm2-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Bikes")).unwrap();
        std::fs::write(dir.join("Bikes/Red_One.TGA"), b"x").unwrap();
        std::fs::write(dir.join("Bikes/a.tga"), b"x").unwrap();
        std::fs::write(dir.join("Bikes/notes.txt"), b"x").unwrap();
        let f = super::own_files(&dir, "BIKES");
        assert_eq!(f, [("a.tga".to_string(), "a".to_string()), ("Red_One.TGA".into(), "Red_One".into())]);
        assert!(super::own_files(&dir, "RIDERS").is_empty());
        assert_eq!(super::own_key("BIKES", "Red_One.TGA"), "own-bikes-red_one.tga");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
