// cargo run --example ogg_check -- a.wav a.ogg ...: length, level and loop-seam checks.
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    for pair in a.chunks(2) {
        let w = mm2_formats::wav::decode(&std::fs::read(&pair[0]).unwrap()).unwrap();
        let o = mm2_formats::wav::decode(&std::fs::read(&pair[1]).unwrap()).unwrap();
        let n = w.samples.len().min(o.samples.len());
        let err: f32 = (0..n).map(|i| (w.samples[i] - o.samples[i]).powi(2)).sum::<f32>() / n as f32;
        let sig: f32 = w.samples.iter().map(|v| v * v).sum::<f32>() / w.samples.len() as f32;
        // Loop seam: jump from the last sample to the first, vs the clip's typical step.
        let seam = |s: &[f32]| (s[0] - s[s.len() - 1]).abs();
        let step: f32 = w.samples.windows(2).map(|p| (p[1] - p[0]).abs()).sum::<f32>() / n as f32;
        println!("{} wav {} ogg {} snr {:.1} dB seam wav {:.4} ogg {:.4} step {:.4}", pair[0], w.samples.len(), o.samples.len(),
            10.0 * (sig / err.max(1e-12)).log10(), seam(&w.samples), seam(&o.samples), step);
    }
}
