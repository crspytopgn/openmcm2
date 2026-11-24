// AMD FidelityFX Super Resolution 1 (FSR 1): the scene drawn smaller than
// the screen stretched over it along its edges (EASU, edge-adaptive
// spatial upsampling) and then sharpened (RCAS, robust contrast-adaptive
// sharpening), in place of a plain bilinear stretch. Ported to WGSL from
// AMD's ffx_fsr1.h (v1.20210629), the 32-bit paths, with its texture
// gathers as loads (the same twelve taps), so it runs wherever WebGPU
// does.
//
// Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved.
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions: The above copyright notice and this
// permission notice shall be included in all copies or substantial
// portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT
// WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// The input is the finished picture (post.wgsl fs_composite) in gamma 2
// (its square root), as FSR wants it perceptual; RCAS squares it back.

struct Fsr {
    // The input's size and one over it.
    input: vec4<f32>,
    // The output's size and one over it.
    output: vec4<f32>,
    // x: RCAS's sharpness as AMD's constant (2^-stops, 1 the sharpest).
    sharp: vec4<f32>,
};

@group(0) @binding(0) var<uniform> fsr: Fsr;
@group(0) @binding(1) var src: texture_2d<f32>;

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}

fn tap(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(src, clamp(p, vec2<i32>(0), vec2<i32>(textureDimensions(src)) - 1), 0).rgb;
}

// Luma times 2, as FSR's.
fn luma2(c: vec3<f32>) -> f32 {
    return c.b * 0.5 + (c.r * 0.5 + c.g);
}

// FsrEasuSetF: the direction and length of the edge, for one of the four
// nearest's bilinear share `w`.
fn easu_set(dir: ptr<function, vec2<f32>>, len: ptr<function, f32>, w: f32, la: f32, lb: f32, lc: f32, ld: f32, le: f32) {
    let dc = ld - lc;
    let cb = lc - lb;
    var len_x = max(abs(dc), abs(cb));
    len_x = 1.0 / max(len_x, 1e-8);
    let dir_x = ld - lb;
    (*dir).x += dir_x * w;
    len_x = clamp(abs(dir_x) * len_x, 0.0, 1.0);
    len_x *= len_x;
    *len += len_x * w;
    let ec = le - lc;
    let ca = lc - la;
    var len_y = max(abs(ec), abs(ca));
    len_y = 1.0 / max(len_y, 1e-8);
    let dir_y = le - la;
    (*dir).y += dir_y * w;
    len_y = clamp(abs(dir_y) * len_y, 0.0, 1.0);
    len_y *= len_y;
    *len += len_y * w;
}

// FsrEasuTapF: one tap's share, by its place along and across the edge.
fn easu_tap(ac: ptr<function, vec3<f32>>, aw: ptr<function, f32>, off: vec2<f32>, dir: vec2<f32>, len: vec2<f32>, lob: f32, clp: f32, c: vec3<f32>) {
    var v = vec2<f32>(off.x * dir.x + off.y * dir.y, off.x * -dir.y + off.y * dir.x);
    v *= len;
    let d2 = min(dot(v, v), clp);
    var wb = 2.0 / 5.0 * d2 - 1.0;
    var wa = lob * d2 - 1.0;
    wb *= wb;
    wa *= wa;
    wb = 25.0 / 16.0 * wb - (25.0 / 16.0 - 1.0);
    let w = wb * wa;
    *ac += c * w;
    *aw += w;
}

@fragment
fn fs_easu(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    // Where this output pixel's middle falls on the input, from 'f'.
    let scale = fsr.input.xy * fsr.output.zw;
    var pp = floor(at.xy) * scale + (0.5 * scale - 0.5);
    let fp = floor(pp);
    pp -= fp;
    let f0 = vec2<i32>(fp);
    //    b c
    //  e f g h
    //  i j k l
    //    n o
    let b = tap(f0 + vec2<i32>(0, -1));
    let c = tap(f0 + vec2<i32>(1, -1));
    let e = tap(f0 + vec2<i32>(-1, 0));
    let f = tap(f0);
    let g = tap(f0 + vec2<i32>(1, 0));
    let h = tap(f0 + vec2<i32>(2, 0));
    let i = tap(f0 + vec2<i32>(-1, 1));
    let j = tap(f0 + vec2<i32>(0, 1));
    let k = tap(f0 + vec2<i32>(1, 1));
    let l = tap(f0 + vec2<i32>(2, 1));
    let n = tap(f0 + vec2<i32>(0, 2));
    let o = tap(f0 + vec2<i32>(1, 2));
    let bl = luma2(b);
    let cl = luma2(c);
    let el = luma2(e);
    let fl = luma2(f);
    let gl = luma2(g);
    let hl = luma2(h);
    let il = luma2(i);
    let jl = luma2(j);
    let kl = luma2(k);
    let ll = luma2(l);
    let nl = luma2(n);
    let ol = luma2(o);
    var dir = vec2<f32>(0.0);
    var len = 0.0;
    easu_set(&dir, &len, (1.0 - pp.x) * (1.0 - pp.y), bl, el, fl, gl, jl);
    easu_set(&dir, &len, pp.x * (1.0 - pp.y), cl, fl, gl, hl, kl);
    easu_set(&dir, &len, (1.0 - pp.x) * pp.y, fl, il, jl, kl, nl);
    easu_set(&dir, &len, pp.x * pp.y, gl, jl, kl, ll, ol);
    // Normalised, cleaned up close to zero.
    let dir2 = dir * dir;
    var dir_r = dir2.x + dir2.y;
    let zero = dir_r < 1.0 / 32768.0;
    dir_r = select(inverseSqrt(max(dir_r, 1e-12)), 1.0, zero);
    dir.x = select(dir.x, 1.0, zero);
    dir *= dir_r;
    len = len * 0.5;
    len *= len;
    let stretch = dot(dir, dir) / max(max(abs(dir.x), abs(dir.y)), 1e-8);
    let len2 = vec2<f32>(1.0 + (stretch - 1.0) * len, 1.0 - 0.5 * len);
    let lob = 0.5 + ((1.0 / 4.0 - 0.04) - 0.5) * len;
    let clp = 1.0 / lob;
    // Kept within the four nearest, so it rings nowhere.
    let min4 = min(min(f, g), min(j, k));
    let max4 = max(max(f, g), max(j, k));
    var ac = vec3<f32>(0.0);
    var aw = 0.0;
    easu_tap(&ac, &aw, vec2<f32>(0.0, -1.0) - pp, dir, len2, lob, clp, b);
    easu_tap(&ac, &aw, vec2<f32>(1.0, -1.0) - pp, dir, len2, lob, clp, c);
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 1.0) - pp, dir, len2, lob, clp, i);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 1.0) - pp, dir, len2, lob, clp, j);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 0.0) - pp, dir, len2, lob, clp, f);
    easu_tap(&ac, &aw, vec2<f32>(-1.0, 0.0) - pp, dir, len2, lob, clp, e);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 1.0) - pp, dir, len2, lob, clp, k);
    easu_tap(&ac, &aw, vec2<f32>(2.0, 1.0) - pp, dir, len2, lob, clp, l);
    easu_tap(&ac, &aw, vec2<f32>(2.0, 0.0) - pp, dir, len2, lob, clp, h);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 0.0) - pp, dir, len2, lob, clp, g);
    easu_tap(&ac, &aw, vec2<f32>(1.0, 2.0) - pp, dir, len2, lob, clp, o);
    easu_tap(&ac, &aw, vec2<f32>(0.0, 2.0) - pp, dir, len2, lob, clp, n);
    let pix = min(max4, max(min4, ac / max(aw, 1e-8)));
    return vec4<f32>(pix, 1.0);
}

// FSR_RCAS_LIMIT: the most negative lobe.
const RCAS_LIMIT: f32 = 0.25 - 1.0 / 16.0;

// RCAS (FsrRcasF, with its noise removal): sharpened as much as each
// pixel can be without leaving the range of the four round it; then out
// of gamma 2 back to linear for the screen.
@fragment
fn fs_rcas(@builtin(position) at: vec4<f32>) -> @location(0) vec4<f32> {
    let sp = vec2<i32>(at.xy);
    let b = tap(sp + vec2<i32>(0, -1));
    let d = tap(sp + vec2<i32>(-1, 0));
    let e = tap(sp);
    let f = tap(sp + vec2<i32>(1, 0));
    let h = tap(sp + vec2<i32>(0, 1));
    let bl = luma2(b);
    let dl = luma2(d);
    let el = luma2(e);
    let fl = luma2(f);
    let hl = luma2(h);
    // Noise detection.
    var nz = 0.25 * bl + 0.25 * dl + 0.25 * fl + 0.25 * hl - el;
    let range = max(max(max(bl, dl), max(el, fl)), hl) - min(min(min(bl, dl), min(el, fl)), hl);
    nz = clamp(abs(nz) / max(range, 1e-8), 0.0, 1.0);
    nz = -0.5 * nz + 1.0;
    // Min and max of the ring.
    let mn4 = min(min(b, d), min(f, h));
    let mx4 = max(max(b, d), max(f, h));
    let peak = vec2<f32>(1.0, -4.0);
    let hit_min = min(mn4, e) / max(4.0 * mx4, vec3<f32>(1e-8));
    let hit_max = (peak.x - max(mx4, e)) / min(4.0 * mn4 + peak.y, vec3<f32>(-1e-8));
    let lobe3 = max(-hit_min, hit_max);
    var lobe = max(-RCAS_LIMIT, min(max(max(lobe3.r, lobe3.g), lobe3.b), 0.0)) * fsr.sharp.x;
    lobe *= nz;
    let pix = (lobe * (b + d + h + f) + e) / (4.0 * lobe + 1.0);
    let lin = max(pix, vec3<f32>(0.0)) * max(pix, vec3<f32>(0.0));
    return vec4<f32>(lin, 1.0);
}
