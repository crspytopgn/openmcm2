//! English stand-ins for the game text the race screens use, for when
//! the game's `LANG.DLL` isn't there (playing with the Free Pack alone).
//! Keyed by the same string ids, so the HUD's labels, messages and the
//! computer riders' names still show. The names are our own.

/// The stand-in for `LANG.DLL` string `id`, if the engine uses it.
pub fn lang(id: u32) -> Option<&'static str> {
    if (5000..5036).contains(&id) {
        return Some(NAMES[(id - 5000) as usize]);
    }
    if (5249..=5259).contains(&id) {
        return Some(PLACES[(id - 5249) as usize]);
    }
    Some(match id {
        2101 => "Position:",
        2102 => "Lap:",
        2103 => "Lap Time:",
        2104 => "Best Lap:",
        2105 => "TBL:",
        2356 => "Name",
        2357 => "Place",
        2358 => "Best Lap Time",
        2359 => "Score",
        2360 => "Win Points",
        2361 => "Total It Time",
        2371 => "Time Remaining: ",
        4080 => "incomplete",
        5037 => "Gate: ",
        5046 => "Visual Cues",
        5048 => "TRT",
        5091 => "Event",
        5092 => "Bike & Rider",
        5093 => "Race Info",
        5127 => "ON",
        5128 => "OFF",
        5133 => "Gas Gyro",
        5135 => "Particles",
        5136 => "Shadows",
        5137 => "Sky",
        5138 => "Names",
        5139 => "Positions",
        5140 => "Names and Positions",
        5141 => "Stats",
        5142 => "Instruments",
        5161 => "Sounds",
        5162 => "Single Stunt",
        5163 => "Best It Time",
        5165 => "Brake Gyro",
        5184 => "Save this ghost lap?",
        5185 => "Radar Overlay",
        5260 => "Keep trying",
        5303 => "FREE BALL!",
        5304 => "is IT!",
        5312 => "Crowd Sounds Reduced",
        5313 => "Crowd Sounds On",
        5314 => "Crowd Sounds Off",
        5315 => "Reverb",
        5316 => "Graphics Quality",
        5319 => "Record a new ghost race",
        5320 => "Ghost Race",
        _ => return None,
    })
}

const PLACES: [&str; 11] = [
    "Congratulations! You finished 1st!",
    "You finished 2nd",
    "You finished 3rd",
    "You finished 4th",
    "You finished 5th",
    "You finished 6th",
    "You finished 7th",
    "You finished 8th",
    "You finished 9th",
    "You finished 10th",
    "You finished 11th",
];

/// The computer riders' names (our own, not the game's).
const NAMES: [&str; 36] = [
    "DUSTY", "KOJI", "MARLO", "RIPCORD", "BEN", "SANTI", "HOLLIS", "REESE", "MAREN", "OLEG", "PIKE",
    "JT", "NOOR", "MUDCAT", "PIPER", "SKIDS", "TALON", "FINN", "CODY", "TORCH", "ZEPHYR", "BRAMBLE",
    "KNOX", "GRIT", "MAGNUS", "VIPER", "RIVET", "HARLAN", "QUINN", "DEACON", "ROWAN", "TESS",
    "RHINO", "ROCKET", "FROSTY", "JUNO",
];

#[cfg(test)]
mod tests {
    #[test]
    fn has_every_name_and_place() {
        for id in (5000..5036).chain(5249..=5260).chain([2101, 2102, 5127, 5128]) {
            assert!(super::lang(id).is_some_and(|s| !s.is_empty()), "{id}");
        }
    }
}
