//! The Course Builder's link on the main menu's chain: the empty roller
//! above Help gets an engraved jump and shovel, lit blue under the
//! pointer like the game's own links. On unless the player turns it off
//! (Extras, `--no-builder-menu`).
//!
//! The glyph is our own art (`assets/menu/builder_*.png`, drawn by
//! `tools/art/builder_link.py`); each state's picture is the menu's own
//! roller with the glyph laid over it, made when the menu opens.

use mm2_formats::dtm::{Control, Kind, Props};
use mm2_formats::tga::Image;

/// The button's name, as the menu's clicks see it.
pub const NAME: &str = "MenuCourseBuilder";
/// The empty roller above Help, on the 640×480 menu.
const RECT: [i32; 4] = [527, 281, 71, 47];
/// The pictures' names: the dialog's own never start so.
const PREFIX: &str = "openmcm2:builder_";
const TIP: &str = "Course Builder";

const UP: &[u8] = include_bytes!("../assets/menu/builder_up.png");
const LIT: &[u8] = include_bytes!("../assets/menu/builder_lit.png");
const GREY: &[u8] = include_bytes!("../assets/menu/builder_grey.png");

/// Whether menu `pack` gets the link: the main menu, with the extra on,
/// and the game's own art behind it (`background`, its picture's file):
/// a free pack's menu picture has its rollers elsewhere, and the pack
/// adds a button of its own.
pub fn wanted(pack: &str, background: Option<&str>) -> bool {
    pack.eq_ignore_ascii_case("MAINMENU")
        && crate::tweaks::more().builder_menu
        && background.is_none_or(|bg| crate::freepack::ui_image(pack, bg).is_none())
}

/// The button, as the dialog's own are.
pub fn control() -> Control {
    let props = vec![
        ("AnimNorm".to_string(), format!("{PREFIX}up")),
        ("AnimFocus".to_string(), format!("{PREFIX}lit")),
        ("AnimPush".to_string(), format!("{PREFIX}lit")),
        ("AnimDisable".to_string(), format!("{PREFIX}grey")),
        ("ToolTipText".to_string(), TIP.to_string()),
    ];
    Control { name: NAME.to_string(), kind: Kind::Button, rect: RECT, anchor: None, props: Props(props) }
}

/// The state a picture name is for (`up`, `lit`, `grey`), if it's one of
/// the link's.
pub fn state(name: &str) -> Option<&str> {
    let lower = name.get(..PREFIX.len())?;
    lower.eq_ignore_ascii_case(PREFIX).then(|| &name[PREFIX.len()..])
}

/// The link's picture in `state`: the roller cut from the menu's
/// `background`, the glyph over it.
pub fn picture(background: &Image, state: &str) -> Option<Image> {
    let png = match state.to_ascii_lowercase().as_str() {
        "lit" => LIT,
        "grey" => GREY,
        _ => UP,
    };
    let (w, h, glyph) = crate::hires::decode_png(png).ok()?;
    let [x0, y0, ..] = RECT;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (bx, by) = (x0 as u32 + x, y0 as u32 + y);
            if bx >= background.width || by >= background.height {
                continue;
            }
            let b = ((by * background.width + bx) * 4) as usize;
            let g = ((y * w + x) * 4) as usize;
            let a = glyph[g + 3] as f32 / 255.0;
            for c in 0..3 {
                let v = background.rgba[b + c] as f32 * (1.0 - a) + glyph[g + c] as f32 * a;
                rgba[g + c] = v.round() as u8;
            }
            rgba[g + 3] = 255;
        }
    }
    Some(Image { width: w, height: h, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_cover_the_roller() {
        let bg = Image { width: 640, height: 480, rgba: vec![40; 640 * 480 * 4] };
        for s in ["up", "lit", "grey"] {
            let p = picture(&bg, s).expect("picture");
            assert_eq!((p.width as i32, p.height as i32), (RECT[2], RECT[3]));
        }
        assert_eq!(state("OpenMCM2:builder_lit"), Some("lit"));
        assert_eq!(state("SingleUp"), None);
    }
}
