//! Ogg Vorbis sounds: the Free Pack's recordings, a tenth the size of the
//! same sound as a WAV. The original game's sounds are all WAVs ([`wav`]).
//!
//! [`wav`]: crate::wav

use crate::wav::Sound;
use crate::{Error, Result};
use lewton::inside_ogg::OggStreamReader;
use std::io::Cursor;

/// Whether `bytes` start like an Ogg stream (`OggS`).
pub fn is_ogg(bytes: &[u8]) -> bool {
    bytes.starts_with(b"OggS")
}

pub fn parse(bytes: &[u8]) -> Result<Sound> {
    if !is_ogg(bytes) {
        return Err(Error::BadMagic);
    }
    let mut r = OggStreamReader::new(Cursor::new(bytes)).map_err(|_| Error::Corrupt("bad ogg vorbis header"))?;
    let channels = r.ident_hdr.audio_channels as u16;
    let rate = r.ident_hdr.audio_sample_rate;
    if channels == 0 || rate == 0 {
        return Err(Error::Corrupt("ogg vorbis without channels"));
    }
    let mut samples: Vec<f32> = Vec::new();
    // Interleaved, as a WAV's are.
    loop {
        match r.read_dec_packet_generic::<lewton::samples::InterleavedSamples<f32>>() {
            Ok(Some(p)) => samples.extend_from_slice(&p.samples),
            Ok(None) => break,
            Err(_) => return Err(Error::Corrupt("bad ogg vorbis packet")),
        }
    }
    // The last block is decoded whole; the stream's last granule position
    // says where the sound really ends (an encoder pads it out), and a loop
    // with the padding left on would click at its seam.
    if let Some(end) = r.get_last_absgp() {
        let end = (end as usize).saturating_mul(channels as usize);
        if end > 0 && end < samples.len() {
            samples.truncate(end);
        }
    }
    Ok(Sound { rate, channels, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A quarter second (5513 samples, as ffmpeg rounds it) of 440 Hz at 22050 Hz, mono (made with
    /// `ffmpeg -f lavfi -i sine=frequency=440:sample_rate=22050:duration=0.25 -c:a libvorbis`).
    const TONE: &[u8] = include_bytes!("../tests/data/tone.ogg");

    #[test]
    fn decodes_every_sample() {
        let s = parse(TONE).unwrap();
        assert_eq!((s.rate, s.channels), (22050, 1));
        // Exactly as long as the tone: none lost or added at either end,
        // so a loop stays seamless.
        assert_eq!(s.frames(), 5513);
        let peak = s.samples.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "peak {peak}");
    }

    #[test]
    fn either_kind_decodes() {
        assert_eq!(crate::wav::decode(TONE).unwrap().frames(), 5513);
        assert!(parse(b"RIFF....WAVE").is_err());
        assert_eq!(crate::wav::other_name("Idle01.WAV").as_deref(), Some("Idle01.ogg"));
        assert_eq!(crate::wav::other_name("crowdloop.ogg").as_deref(), Some("crowdloop.wav"));
        assert_eq!(crate::wav::other_name("bands.ini"), None);
    }
}
