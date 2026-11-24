//! A ride timed frame by frame (`MM2_FILM_BENCH=1` with `--ride S`, and
//! `--size WxH`): each frame of the ride drawn as on screen and timed, the
//! game's step, the scene's setting up, the recording of the draws and the
//! GPU's work each on its own, then the spread and the slowest frames
//! listed with where the camera was. For finding where frames come late
//! (near the scenery, say); off unless asked.

use crate::camera::Camera;
use crate::renderer::TerrainRenderer;
use glam::Vec3;

struct Frame {
    clock: f32,
    eye: Vec3,
    /// The game's step, the scene's setting up, the recording and the
    /// GPU's wait, ms.
    step: f64,
    show: f64,
    record: f64,
    gpu: f64,
}

pub struct Bench {
    target: Option<(wgpu::Texture, (u32, u32))>,
    frames: Vec<Frame>,
}

impl Bench {
    pub fn wanted() -> Option<Bench> {
        std::env::var_os("MM2_FILM_BENCH").map(|_| Bench { target: None, frames: Vec::new() })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut TerrainRenderer,
        size: (u32, u32),
        camera: &Camera,
        time: f32,
        clock: f32,
        step: f64,
        show: f64,
    ) {
        if self.target.as_ref().is_none_or(|t| t.1 != size) {
            self.target = Some((crate::offscreen(device, wgpu::TextureFormat::Rgba8UnormSrgb, size.0, size.1), size));
        }
        let view = self.target.as_ref().unwrap().0.create_view(&Default::default());
        let t = web_time::Instant::now();
        let draw = renderer.render(device, queue, &view, size, camera, time);
        let record = t.elapsed().as_secs_f64();
        let t = web_time::Instant::now();
        queue.submit([draw]);
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let gpu = t.elapsed().as_secs_f64();
        self.frames.push(Frame { clock, eye: camera.position, step: step * 1e3, show: show * 1e3, record: record * 1e3, gpu: gpu * 1e3 });
    }

    pub fn report(&self, device: &wgpu::Device, renderer: &mut TerrainRenderer) {
        // The first second settles (shaders, uploads): left out.
        let start = self.frames.first().map_or(0.0, |f| f.clock) + 1.0;
        let frames: Vec<&Frame> = self.frames.iter().filter(|f| f.clock >= start).collect();
        if frames.is_empty() {
            eprintln!("bench: no frames");
            return;
        }
        let line = |name: &str, get: &dyn Fn(&Frame) -> f64| {
            let mut v: Vec<f64> = frames.iter().map(|f| get(f)).collect();
            v.sort_by(f64::total_cmp);
            let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            eprintln!("bench {name:>7}: mean {mean:8.3}  p50 {:8.3}  p95 {:8.3}  p99 {:8.3}  max {:8.3} ms", at(0.5), at(0.95), at(0.99), at(1.0));
        };
        eprintln!("bench: {} frames", frames.len());
        line("step", &|f| f.step);
        line("show", &|f| f.show);
        line("record", &|f| f.record);
        line("gpu", &|f| f.gpu);
        line("cpu", &|f| f.step + f.show + f.record);
        line("total", &|f| f.step + f.show + f.record + f.gpu);
        let mut worst: Vec<&&Frame> = frames.iter().collect();
        worst.sort_by(|a, b| (b.step + b.show + b.record).total_cmp(&(a.step + a.show + a.record)));
        for f in worst.iter().take(5) {
            eprintln!(
                "bench slow cpu at {:6.2} s  eye {:7.0} {:6.0} {:7.0}: step {:.2} show {:.2} record {:.2} gpu {:.2}",
                f.clock, f.eye.x, f.eye.y, f.eye.z, f.step, f.show, f.record, f.gpu
            );
        }
        worst.sort_by(|a, b| b.gpu.total_cmp(&a.gpu));
        for f in worst.iter().take(5) {
            eprintln!(
                "bench slow gpu at {:6.2} s  eye {:7.0} {:6.0} {:7.0}: step {:.2} show {:.2} record {:.2} gpu {:.2}",
                f.clock, f.eye.x, f.eye.y, f.eye.z, f.step, f.show, f.record, f.gpu
            );
        }
        renderer.report_gpu_times(device);
    }
}
