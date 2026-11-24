//! Fixes to the Free Pack tracks' jumps found by riding them: the
//! computer riders' laps (the game's own physics and AI) checked with
//! [`crate::ridecheck`] by `mm2-tools freetune`, which writes this table.

use crate::free::Fix;

/// The fixes for the track `file` (e.g. "FPNAT01"), by jump number.
pub fn fixes(file: &str) -> &'static [(usize, Fix)] {
    TABLE.iter().find(|(f, _)| f.eq_ignore_ascii_case(file)).map_or(&[], |(_, v)| v)
}

/// Kept where the fixes cut the riders' crashes by 7 % or more with as
/// much air and as many big jumps (within a ride's noise) in 300 s rides
/// by six computer riders, and again in a separate ride by eight
/// (2026-11-24: crashes a lap 3.08 to 2.35, 3.69 to 3.25, 2.92 to 2.25).
const TABLE: &[(&str, &[(usize, Fix)])] = &[
    ("FPNAT03", &[(0, Fix::Long(40.0)), (1, Fix::Long(39.0)), (2, Fix::Long(30.0)), (3, Fix::Long(40.0)), (4, Fix::Long(34.0)), (5, Fix::Short(25.0)), (7, Fix::Soften), (8, Fix::Long(18.0)), (9, Fix::Long(40.0)), (10, Fix::Long(40.0)), (11, Fix::Long(40.0))]),
    ("FPNAT04", &[(0, Fix::Long(40.0)), (1, Fix::Short(25.0)), (2, Fix::Soften), (3, Fix::Short(10.0)), (4, Fix::Long(40.0)), (6, Fix::Soften), (7, Fix::Long(30.0)), (8, Fix::Long(40.0)), (9, Fix::Long(40.0))]),
    ("FPNAT10", &[(0, Fix::Soften), (1, Fix::Long(37.0)), (2, Fix::Soften), (3, Fix::Long(40.0)), (4, Fix::Soften), (1, Fix::Short(25.0)), (2, Fix::Soften), (3, Fix::Long(29.0))]),
];
