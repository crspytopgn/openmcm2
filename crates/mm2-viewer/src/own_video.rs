//! The player's own intro video (the Extras' "Your own intro video",
//! `--intro-video=PATH`): an MP4 file on their computer, played in place
//! of the game's `UI/INTRO.AVI` (see `screens/intro.rs`). Nothing of it
//! comes with OpenMCM2: it is whatever file the player picks.
//!
//! The picture is H.264 (by far the commonest MP4 video), decoded with
//! Cisco's OpenH264 on a thread of its own; the sound is AAC, read whole
//! when it opens (Symphonia). A computer too slow for the picture's size
//! (a 4K video on a laptop) keeps to time by leaving out the frames
//! nothing else is drawn from, and if it falls well behind, by jumping to
//! the next key frame, so the picture never drifts from the sound.

use crate::audio::{Audio, Voice};
use crate::sprites::TextureId;
use crate::ui::Gfx;
use mm2_formats::wav::Sound;
use std::collections::VecDeque;
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::Arc;

/// Pictures decoded ahead of the one shown. Each is a whole frame
/// (33 MB at 4K), so only a few.
const AHEAD: usize = 3;

/// How far behind the picture may fall before it jumps to a key frame.
const JUMP_BEHIND: f64 = 0.75;

/// A decoded picture and when it shows, seconds from the start.
struct Frame {
    time: f64,
    size: (u32, u32),
    rgba: Vec<u8>,
}

pub struct OwnVideo {
    width: u32,
    height: u32,
    /// How long it runs, seconds.
    length: f64,
    frames: Receiver<Frame>,
    /// The next picture, waiting for its time.
    next: Option<Frame>,
    /// The decoder thread has ended (it sent its last picture).
    ended: bool,
    texture: Option<TextureId>,
    sound: Option<Arc<Sound>>,
    voice: Option<Voice>,
    audio: Option<Audio>,
    start: Option<f32>,
    /// The time played, for the decoder thread to keep up with (f32 bits;
    /// negative until it starts).
    clock: Arc<AtomicU32>,
    /// Tells the decoder thread to stop.
    quit: Arc<AtomicBool>,
}

/// The video track's samples, as the decoder thread reads them.
struct Track {
    mp4: mp4::Mp4Reader<BufReader<std::fs::File>>,
    id: u32,
    count: u32,
    timescale: f64,
    /// NAL length field's size in bytes (`avcC`).
    length_size: usize,
    /// The sequence and picture parameter sets, sent before each key
    /// frame (OpenH264 wants them in the stream).
    params: Vec<u8>,
    /// The key frames' sample numbers and times, for a jump.
    keys: Vec<(u32, f64)>,
}

impl OwnVideo {
    /// Opens `path`, or says why it can't be played.
    pub fn open(path: &Path) -> Result<OwnVideo, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("can't open it ({e})"))?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let mp4 = mp4::Mp4Reader::read_header(BufReader::new(file), size).map_err(|e| format!("not an MP4 file this can read ({e})"))?;
        let track = mp4
            .tracks()
            .values()
            .find(|t| t.trak.mdia.minf.stbl.stsd.avc1.is_some())
            .ok_or("its video isn't H.264 (try an MP4 made with H.264, the commonest kind)")?;
        let avcc = &track.trak.mdia.minf.stbl.stsd.avc1.as_ref().unwrap().avcc;
        let mut params = Vec::new();
        for p in avcc.sequence_parameter_sets.iter().chain(&avcc.picture_parameter_sets) {
            params.extend([0, 0, 0, 1]);
            params.extend(&p.bytes);
        }
        let avc1 = track.trak.mdia.minf.stbl.stsd.avc1.as_ref().unwrap();
        let (width, height) = (avc1.width as u32, avc1.height as u32);
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err(format!("its picture is {width}x{height}, too big to show"));
        }
        let timescale = (track.timescale() as f64).max(1.0);
        let count = track.sample_count();
        let length = track.duration().as_secs_f64();
        let id = track.track_id();
        let length_size = avcc.length_size_minus_one as usize + 1;
        let stss = track.trak.mdia.minf.stbl.stss.as_ref().map(|s| s.entries.clone());
        let mut track = Track { mp4, id, count, timescale, length_size, params, keys: Vec::new() };
        // Key frames: the sync table, or every frame if it has none.
        if let Some(entries) = stss {
            for n in entries {
                if let Ok(Some(s)) = track.mp4.read_sample(id, n) {
                    track.keys.push((n, s.start_time as f64 / timescale));
                }
            }
        }
        let sound = read_sound(path);
        let clock = Arc::new(AtomicU32::new((-1.0f32).to_bits()));
        let quit = Arc::new(AtomicBool::new(false));
        let (send, frames) = std::sync::mpsc::sync_channel(AHEAD);
        let (c, q) = (clock.clone(), quit.clone());
        std::thread::Builder::new()
            .name("intro video".into())
            .spawn(move || decode(track, send, c, q))
            .map_err(|e| e.to_string())?;
        Ok(OwnVideo {
            width,
            height,
            length,
            frames,
            next: None,
            ended: false,
            texture: None,
            sound,
            voice: None,
            audio: None,
            start: None,
            clock,
            quit,
        })
    }

    /// Width over height.
    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Whether the first picture is ready (or the video turned out to
    /// have none), so it can start without a black moment.
    pub fn ready(&mut self) -> bool {
        self.pull();
        self.next.is_some() || self.ended
    }

    /// Whether it ended before showing anything: a file it couldn't decode.
    pub fn failed(&self) -> bool {
        self.ended && self.next.is_none() && self.texture.is_none()
    }

    pub fn play(&mut self, audio: Option<&Audio>, now: f32) {
        self.start = Some(now);
        self.clock.store(0f32.to_bits(), Ordering::Relaxed);
        if let (Some(a), Some(s)) = (audio, &self.sound) {
            self.voice = Some(a.play(s, 1.0, false));
            self.audio = Some(a.share());
        }
    }

    /// Whether it has played to its end.
    pub fn done(&mut self, now: f32) -> bool {
        let Some(start) = self.start else { return false };
        self.pull();
        let over = (now - start) as f64 >= self.length;
        // With the pictures all shown, or the time run out (a few more
        // than the track says are left behind).
        (self.ended && self.next.is_none() && over) || (now - start) as f64 >= self.length + 1.0
    }

    /// Takes the next decoded picture, if one is waiting and none is held.
    fn pull(&mut self) {
        if self.next.is_none() && !self.ended {
            match self.frames.try_recv() {
                Ok(f) => self.next = Some(f),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.ended = true,
            }
        }
    }

    /// Shows the newest picture due by `now` in `rect` (640x480 units).
    pub fn draw(&mut self, gfx: &mut Gfx, now: f32, rect: [f32; 4]) {
        let Some(start) = self.start else { return };
        let t = (now - start).max(0.0);
        self.clock.store(t.to_bits(), Ordering::Relaxed);
        let mut show = None;
        loop {
            self.pull();
            match &self.next {
                Some(f) if f.time <= t as f64 => show = self.next.take(),
                _ => break,
            }
        }
        if let Some(f) = show {
            match self.texture {
                Some(id) if (self.width, self.height) == f.size => gfx.sprites.update(gfx.queue, id, &f.rgba),
                old => {
                    if let Some(id) = old {
                        gfx.sprites.free(id);
                    }
                    (self.width, self.height) = f.size;
                    let image = mm2_formats::tga::Image { width: self.width, height: self.height, rgba: f.rgba };
                    self.texture = Some(gfx.sprites.upload(gfx.device, gfx.queue, &image));
                }
            }
        }
        if let Some(id) = self.texture {
            gfx.sprites.quad(id, [0.0, 0.0, 1.0, 1.0], rect, [1.0; 4]);
        }
    }
}

impl Drop for OwnVideo {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let (Some(a), Some(v)) = (&self.audio, self.voice.take()) {
            a.stop(v);
        }
        // The picture's texture (33 MB at 4K) goes at the next frame.
        if let Some(id) = self.texture.take() {
            crate::sprites::free_later(id);
        }
    }
}

/// The decoder thread: reads samples in order, decodes them and sends the
/// pictures in the order they show, keeping up with `clock`.
fn decode(mut track: Track, send: SyncSender<Frame>, clock: Arc<AtomicU32>, quit: Arc<AtomicBool>) {
    use openh264::decoder::{Decoder, DecoderConfig, Flush};
    let new_decoder = || Decoder::with_api_config(openh264::OpenH264API::from_source(), DecoderConfig::new().flush_after_decode(Flush::NoFlush));
    let Ok(mut decoder) = new_decoder() else { return };
    // The times of the samples sent to the decoder and not yet shown, in
    // the order they show (the decoder hands pictures back in that order).
    let mut times: VecDeque<f64> = VecDeque::new();
    let mut packet = Vec::new();
    let mut n = 1;
    let played = || f32::from_bits(clock.load(Ordering::Relaxed)) as f64;
    let out = |yuv: openh264::decoder::DecodedYUV<'_>, times: &mut VecDeque<f64>| -> bool {
        use openh264::formats::YUVSource;
        let Some(time) = times.pop_front() else { return true };
        let (w, h) = yuv.dimensions();
        let mut rgba = vec![0; w * h * 4];
        yuv.write_rgba8(&mut rgba);
        !quit.load(Ordering::Relaxed) && send.send(Frame { time, size: (w as u32, h as u32), rgba }).is_ok()
    };
    while n <= track.count {
        if quit.load(Ordering::Relaxed) {
            return;
        }
        let Ok(Some(sample)) = track.mp4.read_sample(track.id, n) else {
            n += 1;
            continue;
        };
        let time = (sample.start_time as f64 + sample.rendering_offset as f64) / track.timescale;
        let behind = played() - time;
        // Well behind: on to the latest key frame that is due.
        if behind > JUMP_BEHIND {
            let now = played();
            if let Some(&(k, _)) = track.keys.iter().rev().find(|&&(k, t)| k > n && t <= now) {
                n = k;
                times.clear();
                match new_decoder() {
                    Ok(d) => decoder = d,
                    Err(_) => return,
                }
                continue;
            }
        }
        n += 1;
        packet.clear();
        if sample.is_sync {
            packet.extend(&track.params);
        }
        let mut b: &[u8] = &sample.bytes;
        let mut referenced = sample.is_sync;
        while b.len() > track.length_size {
            let len = b[..track.length_size].iter().fold(0usize, |l, &x| l << 8 | x as usize);
            b = &b[track.length_size..];
            let nal = &b[..len.min(b.len())];
            b = &b[nal.len()..];
            if let Some(&head) = nal.first() {
                // A slice other pictures are drawn from (nal_ref_idc).
                if matches!(head & 0x1f, 1 | 5) && head & 0x60 != 0 {
                    referenced = true;
                }
            }
            packet.extend([0, 0, 0, 1]);
            packet.extend(nal);
        }
        // Behind: leave out a picture nothing else is drawn from.
        if !referenced && behind > 0.0 {
            continue;
        }
        let at = times.partition_point(|&t| t < time);
        times.insert(at, time);
        match decoder.decode(&packet) {
            Ok(Some(yuv)) => {
                if !out(yuv, &mut times) {
                    return;
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("intro video: frame {}: {e}", n - 1),
        }
    }
    if let Ok(rest) = decoder.flush_remaining() {
        for yuv in rest {
            if !out(yuv, &mut times) {
                return;
            }
        }
    }
}

/// The video's sound (AAC), whole, or none if it has none this can read.
fn read_sound(path: &Path) -> Option<Arc<Sound>> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::io::MediaSourceStream;
    let file = std::fs::File::open(path).ok()?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = symphonia::core::probe::Hint::new();
    hint.with_extension("mp4");
    let mut format = symphonia::default::get_probe().format(&hint, stream, &Default::default(), &Default::default()).ok()?.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL)?.clone();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).ok()?;
    let (mut samples, mut rate, mut channels) = (Vec::new(), 0, 0);
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track.id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&packet) else { continue };
        let spec = *decoded.spec();
        (rate, channels) = (spec.rate, spec.channels.count());
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buffer.samples());
    }
    (rate > 0 && channels > 0 && !samples.is_empty()).then(|| Arc::new(Sound { rate, channels: channels as u16, samples }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A short H.264 + AAC MP4 (made with ffmpeg) plays to its end, its
    /// pictures in order, with its sound.
    #[test]
    fn plays_an_mp4() {
        let Ok(path) = std::env::var("MM2_TEST_INTRO_MP4") else {
            eprintln!("skipped: set MM2_TEST_INTRO_MP4 to an H.264 MP4");
            return;
        };
        let mut v = OwnVideo::open(Path::new(&path)).unwrap();
        assert!(v.sound.is_some());
        let mut last = -1.0;
        let mut count = 0;
        while let Ok(f) = v.frames.recv() {
            assert!(f.time > last, "{} after {last}", f.time);
            assert_eq!(f.size, (v.width, v.height));
            assert_eq!(f.rgba.len(), (v.width * v.height * 4) as usize);
            last = f.time;
            count += 1;
        }
        assert!(count > 10, "{count} pictures");
        assert!((last - v.length).abs() < 0.2, "last at {last} of {}", v.length);
    }
}
