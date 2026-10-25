//! Sound: a small software mixer in the style of the game's DirectSound
//! buffers. Each voice plays one sound at a frequency, volume and pan the
//! game sets, looping or once.
//!
//! Without the `audio` feature (or without an output device) everything
//! here is silent but keeps working, so the game runs the same.

use mm2_formats::wav::Sound;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A playing sound, by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Voice(u64);

struct Playing {
    id: u64,
    sound: Arc<Sound>,
    /// Position in source frames.
    pos: f64,
    /// Source frames a second (the sound's own rate, or a new frequency).
    frequency: f32,
    /// The Doppler shift on top of the frequency (DirectSound 3D's).
    pitch: f32,
    volume: f32,
    /// -1 left to 1 right.
    pan: f32,
    looping: bool,
    /// What plays on, in turn and without a gap, when this sound ends.
    then: VecDeque<(Arc<Sound>, bool)>,
}

#[derive(Default)]
struct Mixer {
    voices: Vec<Playing>,
    next: u64,
    rate: f32,
    /// The EAX reverb on the whole mix, while a race asks for one.
    reverb: Option<crate::reverb::Tail>,
}

impl Mixer {
    /// Adds the voices into `out`, interleaved stereo.
    fn mix(&mut self, out: &mut [f32]) {
        out.fill(0.0);
        let rate = self.rate.max(1.0) as f64;
        self.voices.retain_mut(|v| {
            let frames = v.sound.frames();
            if frames == 0 {
                return false;
            }
            let step = (v.frequency * v.pitch) as f64 / rate;
            let (left, right) = ((1.0 - v.pan.max(0.0)), (1.0 + v.pan.min(0.0)));
            for frame in out.chunks_exact_mut(2) {
                if v.pos >= v.sound.frames() as f64 {
                    if let Some((next, looping)) = v.then.pop_front() {
                        v.pos -= v.sound.frames() as f64;
                        v.sound = next;
                        v.looping = looping;
                        if v.sound.frames() == 0 {
                            return false;
                        }
                    } else if !v.looping {
                        return false;
                    } else {
                        v.pos %= v.sound.frames() as f64;
                    }
                }
                let ch = v.sound.channels as usize;
                let i = v.pos as usize;
                let s = &v.sound.samples[i * ch..];
                let (l, r) = if ch == 1 { (s[0], s[0]) } else { (s[0], s[1]) };
                frame[0] += l * v.volume * left;
                frame[1] += r * v.volume * right;
                v.pos += step;
            }
            true
        });
        if let Some(tail) = &mut self.reverb {
            tail.apply(out);
        }
        for s in out {
            *s = s.clamp(-1.0, 1.0);
        }
    }
}

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    /// Keeps the output stream open.
    #[cfg(feature = "audio")]
    _stream: Option<cpal::Stream>,
}

impl Audio {
    /// Opens the default output device, or a silent mixer without one.
    pub fn new() -> Self {
        let mixer = Arc::new(Mutex::new(Mixer {
            rate: 44100.0,
            ..Default::default()
        }));
        #[cfg(feature = "audio")]
        {
            let stream = open(mixer.clone())
                .map_err(|e| eprintln!("no sound: {e}"))
                .ok();
            Audio {
                mixer,
                _stream: stream,
            }
        }
        #[cfg(not(feature = "audio"))]
        Audio { mixer }
    }

    /// A mixer with no output device, read back with `render`.
    pub fn offline(rate: f32) -> Self {
        Audio {
            mixer: Arc::new(Mutex::new(Mixer {
                rate,
                ..Default::default()
            })),
            #[cfg(feature = "audio")]
            _stream: None,
        }
    }

    /// Mixes the next `frames` frames, interleaved stereo.
    pub fn render(&self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        self.mixer.lock().unwrap().mix(&mut out);
        out
    }

    /// Another handle on the same mixer, for whatever must start or stop
    /// sounds where the first isn't to hand (the menus' movies).
    pub fn share(&self) -> Audio {
        Audio {
            mixer: self.mixer.clone(),
            #[cfg(feature = "audio")]
            _stream: None,
        }
    }

    /// Plays `sound` at its own rate.
    pub fn play(&self, sound: &Arc<Sound>, volume: f32, looping: bool) -> Voice {
        self.play_from(sound, 0.0, volume, looping)
    }

    /// Plays `sound` from `start` seconds in.
    pub fn play_from(&self, sound: &Arc<Sound>, start: f32, volume: f32, looping: bool) -> Voice {
        let mut m = self.mixer.lock().unwrap();
        m.next += 1;
        let id = m.next;
        m.voices.push(Playing {
            id,
            sound: sound.clone(),
            pos: (start.max(0.0) * sound.rate as f32) as f64,
            frequency: sound.rate as f32,
            pitch: 1.0,
            volume,
            pan: 0.0,
            looping,
            then: VecDeque::new(),
        });
        Voice(id)
    }

    /// Plays `sound` on `voice` from its start straight away, as the game
    /// rewrites a stream buffer from the play cursor on.
    pub fn switch(&self, voice: Voice, sound: &Arc<Sound>, looping: bool) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(v) = m.voices.iter_mut().find(|v| v.id == voice.0) {
            v.sound = sound.clone();
            v.pos = 0.0;
            v.frequency = sound.rate as f32;
            v.looping = looping;
            v.then.clear();
        }
    }

    /// Queues `sound` to follow on `voice` after what it plays and has
    /// queued.
    pub fn then(&self, voice: Voice, sound: &Arc<Sound>, looping: bool) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(v) = m.voices.iter_mut().find(|v| v.id == voice.0) {
            v.then.push_back((sound.clone(), looping));
        }
    }

    /// Places a playing voice as DirectSound 3D hears it: its volume, pan
    /// and Doppler shift.
    pub fn place(&self, voice: Voice, volume: f32, pan: f32, pitch: f32) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(v) = m.voices.iter_mut().find(|v| v.id == voice.0) {
            v.volume = volume.max(0.0);
            v.pan = pan.clamp(-1.0, 1.0);
            v.pitch = pitch.clamp(0.25, 4.0);
        }
    }

    /// Whether a voice is still playing.
    pub fn playing(&self, voice: Voice) -> bool {
        self.mixer.lock().unwrap().voices.iter().any(|v| v.id == voice.0)
    }

    pub fn stop(&self, voice: Voice) {
        self.mixer
            .lock()
            .unwrap()
            .voices
            .retain(|v| v.id != voice.0);
    }

    /// Sets the listener's EAX environment, or none.
    pub fn set_reverb(&self, reverb: Option<crate::reverb::Reverb>) {
        let mut m = self.mixer.lock().unwrap();
        if m.reverb.as_ref().map(|t| t.reverb) != reverb {
            let rate = m.rate;
            m.reverb = reverb.map(|r| crate::reverb::Tail::new(r, rate));
        }
    }

    pub fn stop_all(&self) {
        self.mixer.lock().unwrap().voices.clear();
    }
}

impl Default for Audio {
    fn default() -> Self {
        Audio::new()
    }
}

#[cfg(feature = "audio")]
fn open(mixer: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, Box<dyn std::error::Error>> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no output device")?;
    let config = device.default_output_config()?;
    let channels = config.channels() as usize;
    mixer.lock().unwrap().rate = config.sample_rate().0 as f32;
    let mut stereo = Vec::new();
    let err = |e| eprintln!("sound: {e}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let frames = out.len() / channels;
                stereo.resize(frames * 2, 0.0);
                mixer.lock().unwrap().mix(&mut stereo);
                for (o, s) in out.chunks_exact_mut(channels).zip(stereo.chunks_exact(2)) {
                    for (c, v) in o.iter_mut().enumerate() {
                        *v = s[c.min(1)];
                    }
                }
            },
            err,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |out: &mut [i16], _: &cpal::OutputCallbackInfo| {
                let frames = out.len() / channels;
                stereo.resize(frames * 2, 0.0);
                mixer.lock().unwrap().mix(&mut stereo);
                for (o, s) in out.chunks_exact_mut(channels).zip(stereo.chunks_exact(2)) {
                    for (c, v) in o.iter_mut().enumerate() {
                        *v = (s[c.min(1)] * 32767.0) as i16;
                    }
                }
            },
            err,
            None,
        )?,
        other => return Err(format!("unsupported sample format {other:?}").into()),
    };
    stream.play()?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_at_the_set_frequency() {
        let sound = Arc::new(Sound {
            rate: 100,
            channels: 1,
            samples: vec![0.5, 0.25],
        });
        let mut m = Mixer {
            rate: 100.0,
            ..Default::default()
        };
        m.voices.push(Playing {
            id: 1,
            sound,
            pos: 0.0,
            frequency: 100.0,
            pitch: 1.0,
            volume: 1.0,
            pan: 0.0,
            looping: false,
            then: VecDeque::new(),
        });
        let mut out = [0.0; 6];
        m.mix(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.25, 0.25, 0.0, 0.0]);
        assert!(m.voices.is_empty());
    }

    #[test]
    fn plays_on_into_the_next_sound() {
        let one = |x| Arc::new(Sound { rate: 100, channels: 1, samples: vec![x] });
        let mut m = Mixer { rate: 100.0, ..Default::default() };
        m.voices.push(Playing {
            id: 1,
            sound: one(0.5),
            pos: 0.0,
            frequency: 100.0,
            pitch: 1.0,
            volume: 1.0,
            pan: 0.0,
            looping: false,
            then: VecDeque::from([(one(0.25), true)]),
        });
        let mut out = [0.0; 6];
        m.mix(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.25, 0.25, 0.25, 0.25]);
    }
}
