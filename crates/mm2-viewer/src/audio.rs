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
    /// Seconds each sound fades into the next over (0: straight on, as
    /// the game's own samples are cut to follow each other).
    blend: f32,
    /// The sound fading out under this one, where it is, and how far the
    /// fade has gone, 0 to 1.
    fading: Option<(Arc<Sound>, f64, bool)>,
    faded: f32,
}

/// One frame of `sound` at `pos`, left and right.
fn frame_at(sound: &Sound, pos: f64) -> (f32, f32) {
    let ch = sound.channels as usize;
    let i = (pos as usize).min(sound.frames().saturating_sub(1));
    let s = &sound.samples[i * ch..];
    if ch == 1 {
        (s[0], s[0])
    } else {
        (s[0], s[1])
    }
}

/// Without an output device, the most voices kept before the oldest
/// one-shot ones go: otherwise every sound played in a long race stays.
const SILENT_KEPT: usize = 256;

#[derive(Default)]
struct Mixer {
    voices: Vec<Playing>,
    next: u64,
    rate: f32,
    /// The EAX reverb on the whole mix, while a race asks for one.
    reverb: Option<crate::reverb::Tail>,
    /// No output device: nothing mixes, so no voice ever ends by itself
    /// and the one-shot sounds are let go as they pile up (see
    /// [`SILENT_KEPT`]).
    silent: bool,
    /// Silent, with the voices playing on (the mute-in-background extra).
    muted: bool,
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
            let blend_frames = (v.blend as f64 * rate).max(1.0);
            for frame in out.chunks_exact_mut(2) {
                // Fading into the next sound as this one nears its end.
                if v.blend > 0.0
                    && v.fading.is_none()
                    && !v.then.is_empty()
                    && v.sound.frames() as f64 - v.pos <= blend_frames * step
                {
                    let (next, looping) = v.then.pop_front().unwrap();
                    let old = std::mem::replace(&mut v.sound, next);
                    v.fading = Some((old, v.pos, false));
                    v.faded = 0.0;
                    v.pos = 0.0;
                    v.looping = looping;
                    if v.sound.frames() == 0 {
                        return false;
                    }
                }
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
                let (mut l, mut r) = frame_at(&v.sound, v.pos);
                if let Some((old, at, looping)) = &mut v.fading {
                    // Equal power: the new sound comes up as the old goes.
                    let t = v.faded * std::f32::consts::FRAC_PI_2;
                    let (gain_in, gain_out) = (t.sin(), t.cos());
                    l *= gain_in;
                    r *= gain_in;
                    if *looping && *at >= old.frames() as f64 {
                        *at %= old.frames() as f64;
                    }
                    if *at < old.frames() as f64 {
                        let (ol, or) = frame_at(old, *at);
                        l += ol * gain_out;
                        r += or * gain_out;
                    }
                    *at += step;
                    v.faded += (1.0 / blend_frames) as f32;
                    if v.faded >= 1.0 {
                        v.fading = None;
                    }
                }
                frame[0] += l * v.volume * left;
                frame[1] += r * v.volume * right;
                v.pos += step;
            }
            true
        });
        if let Some(tail) = &mut self.reverb {
            tail.apply(out);
        }
        if self.muted {
            out.fill(0.0);
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
    _stream: Option<Stream>,
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
            mixer.lock().unwrap().silent = stream.is_none();
            Audio {
                mixer,
                _stream: stream,
            }
        }
        #[cfg(not(feature = "audio"))]
        {
            mixer.lock().unwrap().silent = true;
            Audio { mixer }
        }
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
            blend: 0.0,
            fading: None,
            faded: 0.0,
        });
        if m.silent && m.voices.len() > SILENT_KEPT {
            // The oldest one-shot sounds would long have ended.
            let mut over = m.voices.len() - SILENT_KEPT / 2;
            m.voices.retain(|v| {
                let go = over > 0 && !v.looping && v.id != id;
                over -= go as usize;
                !go
            });
        }
        Voice(id)
    }

    /// Plays `sound` on `voice` from its start straight away, as the game
    /// rewrites a stream buffer from the play cursor on.
    pub fn switch(&self, voice: Voice, sound: &Arc<Sound>, looping: bool) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(v) = m.voices.iter_mut().find(|v| v.id == voice.0) {
            if v.blend > 0.0 {
                // The sound playing fades out under the new one.
                let old = std::mem::replace(&mut v.sound, sound.clone());
                v.fading = Some((old, v.pos, v.looping));
                v.faded = 0.0;
            }
            v.sound = sound.clone();
            v.pos = 0.0;
            v.frequency = sound.rate as f32;
            v.looping = looping;
            v.then.clear();
        }
    }

    /// Fade each sound on `voice` into the next over `seconds`, both when
    /// one runs on into the next and at a [`Audio::switch`]: for
    /// recordings not cut to follow each other.
    pub fn set_blend(&self, voice: Voice, seconds: f32) {
        let mut m = self.mixer.lock().unwrap();
        if let Some(v) = m.voices.iter_mut().find(|v| v.id == voice.0) {
            v.blend = seconds.max(0.0);
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

    /// Silences the mix, or lets it be heard again; the voices play on.
    pub fn set_muted(&self, muted: bool) {
        self.mixer.lock().unwrap().muted = muted;
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

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
type Stream = cpal::Stream;
#[cfg(all(feature = "audio", target_arch = "wasm32"))]
use web::{open, Stream};

#[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
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

/// Sound in a browser, through Web Audio: the mix, a short piece at a
/// time, queued on the sound card's clock a little ahead of it.
///
/// Not cpal's Web Audio output, which keeps two pieces in flight and
/// queues each where the last ended: when the page stalls for longer
/// than a piece (a busy start line, a frame that builds shaders or
/// loads pictures), those times fall behind the card's, and the pieces
/// then play over each other until it catches up, which crackles.
/// Here a piece is never queued in the past: after a stall the mix picks
/// up again at once, and keeps a little more in hand from then on.
#[cfg(all(feature = "audio", target_arch = "wasm32"))]
mod web {
    use super::Mixer;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{AudioContext, AudioContextState};

    /// Frames in each piece: 21 ms at 48 kHz.
    const PIECE: usize = 1024;
    /// Seconds of sound kept queued ahead of the card: at first, and at
    /// most, as it grows after each stall that ran it dry.
    const LEAD: (f64, f64) = (0.1, 0.25);
    /// How often the page tops the queue up, ms. A piece ending tops it
    /// up too, which goes on in a background tab where timers slow down.
    const TICK_MS: i32 = 10;

    struct Out {
        ctx: AudioContext,
        mixer: Arc<Mutex<Mixer>>,
        /// The card's time the next piece starts at (0: none queued).
        cursor: f64,
        lead: f64,
        stereo: Vec<f32>,
        channel: Vec<f32>,
        /// Tops the queue up as each piece ends.
        ended: Option<js_sys::Function>,
    }

    pub struct Stream {
        out: Rc<RefCell<Out>>,
        interval: i32,
        _tick: Closure<dyn FnMut()>,
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            if let Some(w) = web_sys::window() {
                w.clear_interval_with_handle(self.interval);
            }
            let out = self.out.borrow();
            let _ = out.ctx.close();
        }
    }

    fn top_up(out: &mut Out) {
        // Before the player's first tap or key the card is held, and
        // its clock with it: nothing is mixed until it runs.
        if out.ctx.state() != AudioContextState::Running {
            out.cursor = 0.0;
            return;
        }
        let now = out.ctx.current_time();
        let rate = out.ctx.sample_rate();
        if out.cursor < now + 0.002 {
            if out.cursor > 0.0 {
                // The queue ran dry: keep more in hand from now on.
                out.lead = (out.lead + 0.05).min(LEAD.1);
            }
            out.cursor = now + 0.01;
        }
        let piece = PIECE as f64 / rate as f64;
        while out.cursor < now + out.lead {
            out.stereo.resize(PIECE * 2, 0.0);
            out.mixer.lock().unwrap().mix(&mut out.stereo);
            let Ok(buffer) = out.ctx.create_buffer(2, PIECE as u32, rate) else {
                return;
            };
            for c in 0..2 {
                out.channel.clear();
                out.channel.extend(out.stereo.chunks_exact(2).map(|f| f[c]));
                let _ = buffer.copy_to_channel(&out.channel, c as i32);
            }
            let Ok(source) = out.ctx.create_buffer_source() else {
                return;
            };
            source.set_buffer(Some(&buffer));
            if source.connect_with_audio_node(&out.ctx.destination()).is_err() {
                return;
            }
            source.set_onended(out.ended.as_ref());
            if source.start_with_when(out.cursor).is_err() {
                return;
            }
            out.cursor += piece;
        }
    }

    pub fn open(mixer: Arc<Mutex<Mixer>>) -> Result<Stream, Box<dyn std::error::Error>> {
        let ctx = AudioContext::new().map_err(|e| format!("no Web Audio: {e:?}"))?;
        mixer.lock().unwrap().rate = ctx.sample_rate();
        let _ = ctx.resume();
        let out = Rc::new(RefCell::new(Out {
            ctx,
            mixer,
            cursor: 0.0,
            lead: LEAD.0,
            stereo: Vec::new(),
            channel: Vec::new(),
            ended: None,
        }));
        let weak = Rc::downgrade(&out);
        let tick = Closure::<dyn FnMut()>::new(move || {
            if let Some(out) = weak.upgrade() {
                if let Ok(mut out) = out.try_borrow_mut() {
                    top_up(&mut out);
                }
            }
        });
        let f: &js_sys::Function = tick.as_ref().unchecked_ref();
        out.borrow_mut().ended = Some(f.clone());
        let window = web_sys::window().ok_or("no window")?;
        let interval = window
            .set_interval_with_callback_and_timeout_and_arguments_0(f, TICK_MS)
            .map_err(|e| format!("{e:?}"))?;
        Ok(Stream { out, interval, _tick: tick })
    }
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
            blend: 0.0,
            fading: None,
            faded: 0.0,
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
            blend: 0.0,
            fading: None,
            faded: 0.0,
        });
        let mut out = [0.0; 6];
        m.mix(&mut out);
        assert_eq!(out, [0.5, 0.5, 0.25, 0.25, 0.25, 0.25]);
    }

    #[test]
    fn blends_into_the_next_sound() {
        let flat = |x, n| Arc::new(Sound { rate: 100, channels: 1, samples: vec![x; n] });
        let mut m = Mixer { rate: 100.0, ..Default::default() };
        m.voices.push(Playing {
            id: 1,
            sound: flat(1.0, 10),
            pos: 0.0,
            frequency: 100.0,
            pitch: 1.0,
            volume: 1.0,
            pan: 0.0,
            looping: false,
            then: VecDeque::from([(flat(0.5, 10), false)]),
            blend: 0.04,
            fading: None,
            faded: 0.0,
        });
        let mut out = [0.0; 28];
        m.mix(&mut out);
        let left: Vec<f32> = out.iter().step_by(2).copied().collect();
        // The first plays alone, then both are heard over four frames,
        // then the second alone: no gap and no jump.
        assert_eq!(left[0], 1.0);
        assert!(left[6..10].iter().all(|&x| x > 0.5 && x < 1.3), "{left:?}");
        assert_eq!(left[11], 0.5);
        assert!(left.iter().all(|&x| x > 0.0));
    }

    #[test]
    fn without_an_output_one_shot_sounds_do_not_pile_up() {
        let a = Audio::offline(100.0);
        a.mixer.lock().unwrap().silent = true;
        let s = Arc::new(Sound { rate: 100, channels: 1, samples: vec![0.5; 100] });
        let engine = a.play(&s, 1.0, true);
        for _ in 0..10_000 {
            a.play(&s, 1.0, false);
        }
        assert!(a.mixer.lock().unwrap().voices.len() <= SILENT_KEPT);
        assert!(a.playing(engine), "the looping engine plays on");
    }
}
