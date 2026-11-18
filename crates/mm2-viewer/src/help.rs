//! The Help buttons. The original opened its WinHelp file
//! (`MCM2HELP.HLP`), at the page for the screen it was pressed on; no
//! system opens WinHelp now, so Help opens our own help page on the
//! project's site instead (in a new tab in the browser version), at the
//! screen's part.

/// Our help page.
pub const PAGE: &str = "https://crspytopgn.github.io/openmcm2/help/";

/// The part of the help page for the dialog `pack` (a `UI/*.DAT` name).
pub fn topic(pack: &str) -> &'static str {
    match pack.to_ascii_uppercase().as_str() {
        "MAINMENU" | "GLOBSET" => "main-menu",
        "SPBASE" | "SPEVENT" => "single-player",
        "SPBIKER" => "bike-and-rider",
        "SPRINFO" => "race-info",
        "OPTBASE" | "OPTGAME" => "options",
        "OPTGRAPH" => "graphics",
        "OPTADVG" => "advanced-graphics",
        "OPTSOUND" => "sound",
        "OPTCONT" => "controls",
        "OPTMESS" => "messages",
        "OPTGAR" => "garage",
        "HISCORES" => "high-scores",
        "GRBASE" | "GHOSTFIL" | "REPLYFIL" | "VCR" => "replays-and-ghosts",
        p if p.starts_with("MP") || p == "HOSTJOIN" || p == "SERPOPUP" => "multiplayer",
        p if p.starts_with("PC") => "pro-circuit",
        _ => "",
    }
}

/// Opens the help page at `pack`'s part.
pub fn open(pack: &str) {
    let t = topic(pack);
    let url = if t.is_empty() { PAGE.to_string() } else { format!("{PAGE}#{t}") };
    crate::update_note::open_page(&url);
}

/// Whether a clicked control is one of the dialogs' Help buttons.
pub fn is_help(name: &str) -> bool {
    name.eq_ignore_ascii_case("help") || name.eq_ignore_ascii_case("menuhelp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_screen_has_its_part() {
        assert_eq!(topic("OptSound"), "sound");
        assert_eq!(topic("MPBASE"), "multiplayer");
        assert_eq!(topic("PCC"), "pro-circuit");
        assert_eq!(topic("SPBIKER"), "bike-and-rider");
        assert!(is_help("MenuHelp") && is_help("Help") && !is_help("Helper"));
    }
}
