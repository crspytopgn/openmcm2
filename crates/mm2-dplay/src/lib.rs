//! DirectPlay 4 over TCP/IP, enough of it for OpenMCM2 to host and join
//! games with the original Motocross Madness 2, built from Microsoft's
//! open specifications of the protocol ([MC-DPL4CS] and [MC-DPL4R]) and
//! checked against the original game playing itself.

pub mod game;
pub mod rel;
pub mod session;
pub mod wire;

pub use session::{Event, Found, Session};
