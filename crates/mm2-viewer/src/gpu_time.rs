//! How long each part of a frame takes on the GPU (`MM2_GPU_TIMES=1`, with
//! `MM2_BENCH`): timestamps written between the frame's passes and inside
//! the scene's, read back after each frame and averaged, the bench then
//! listing each part. For finding what to make faster; off unless asked.

use std::collections::HashMap;

/// The most marks in a frame.
const MOST: u32 = 64;

/// Whether the timings are asked for.
pub fn wanted() -> bool {
    std::env::var_os("MM2_GPU_TIMES").is_some()
}

/// The device features they need.
pub fn features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES
}

pub struct GpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    /// Nanoseconds a tick.
    period: f64,
    /// This frame's marks: each is the end of the part it names.
    labels: Vec<&'static str>,
    /// Each part's total, in the order first seen, and frames counted.
    totals: Vec<(&'static str, f64)>,
    index: HashMap<&'static str, usize>,
    frames: u32,
    pending: Option<usize>,
}

impl GpuTimer {
    /// A timer, if asked for and the device can.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<GpuTimer> {
        if !wanted() || !device.features().contains(features()) {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("gpu times"), ty: wgpu::QueryType::Timestamp, count: MOST });
        let size = MOST as u64 * 8;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu times"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu times read"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(GpuTimer {
            set,
            resolve,
            read,
            period: queue.get_timestamp_period() as f64,
            labels: Vec::new(),
            totals: Vec::new(),
            index: HashMap::new(),
            frames: 0,
            pending: None,
        })
    }

    /// Reads the last frame's times (waiting for them), and starts a frame
    /// with its first mark.
    pub fn begin(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        self.collect(device);
        self.labels.clear();
        self.mark(encoder, "start");
    }

    /// Marks, between passes, the end of the part `label`.
    pub fn mark(&mut self, encoder: &mut wgpu::CommandEncoder, label: &'static str) {
        if (self.labels.len() as u32) < MOST {
            encoder.write_timestamp(&self.set, self.labels.len() as u32);
            self.labels.push(label);
        }
    }

    /// Marks, inside a render pass, the end of the part `label`.
    pub fn mark_pass(&mut self, pass: &mut wgpu::RenderPass<'_>, label: &'static str) {
        if (self.labels.len() as u32) < MOST {
            pass.write_timestamp(&self.set, self.labels.len() as u32);
            self.labels.push(label);
        }
    }

    /// Ends the frame: its marks copied out to be read at the next.
    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let n = self.labels.len() as u32;
        if n < 2 {
            return;
        }
        encoder.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, n as u64 * 8);
        self.pending = Some(n as usize);
    }

    fn collect(&mut self, device: &wgpu::Device) {
        let Some(n) = self.pending.take() else { return };
        let slice = self.read.slice(0..n as u64 * 8);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let Ok(view) = slice.get_mapped_range() else { return };
        let ticks: Vec<u64> = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        self.read.unmap();
        for k in 1..n.min(self.labels.len()) {
            let ms = ticks[k].saturating_sub(ticks[k - 1]) as f64 * self.period / 1e6;
            let label = self.labels[k];
            let i = *self.index.entry(label).or_insert_with(|| {
                self.totals.push((label, 0.0));
                self.totals.len() - 1
            });
            self.totals[i].1 += ms;
        }
        self.frames += 1;
    }

    /// Prints each part's average, and starts counting afresh.
    pub fn report(&mut self, device: &wgpu::Device) {
        self.collect(device);
        let frames = self.frames.max(1) as f64;
        let sum: f64 = self.totals.iter().map(|t| t.1).sum::<f64>() / frames;
        eprintln!("gpu times over {} frames ({sum:.2} ms marked):", self.frames);
        for (label, total) in &self.totals {
            eprintln!("  {:<22} {:6.3} ms", label, total / frames);
        }
        self.totals.clear();
        self.index.clear();
        self.frames = 0;
    }
}
