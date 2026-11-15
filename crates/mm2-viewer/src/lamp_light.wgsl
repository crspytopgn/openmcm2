// The bikes' headlights and the light poles' lamps: how far each reaches
// and how its beam falls off. rt_shadow.wgsl traces them each frame;
// flood.wgsl bakes the poles' light over the ground once, by the same
// beams.

// A headlight's colour and strength (a ft away), how wide its beam is
// (the cosines of where it is full and where it ends), and how far it
// reaches, ft.
const LAMP: vec3<f32> = vec3<f32>(1.0, 0.92, 0.78) * 800.0;
// Close to the lamp its light grows no more than this.
const LAMP_MOST: f32 = 2.5;
const BEAM_FULL: f32 = 0.95;
const BEAM_EDGE: f32 = 0.62;
const LAMP_REACH: f32 = 350.0;
// A light pole: its colour and strength (a ft away), its wide downward
// beam (the cosines of full and edge) and how far it reaches, ft.
const POLE: vec3<f32> = vec3<f32>(0.95, 0.97, 1.0) * 3400.0;
const POLE_FULL: f32 = 0.95;
const POLE_EDGE: f32 = 0.2;
const POLE_REACH: f32 = 260.0;
// A lamp lighting a place less than this is left untraced there.
const LAMP_FAINT: f32 = 0.015;

// The light of a lamp at `at` shining along `way` (a pole's if `pole`)
// reaching `world`, were nothing in the way, before how squarely it meets
// the surface there: none past its reach or outside its beam.
fn lamp_beam(at: vec3<f32>, way: vec3<f32>, pole: bool, world: vec3<f32>) -> vec3<f32> {
    let to = at - world;
    let d = length(to);
    let reach = select(LAMP_REACH, POLE_REACH, pole);
    // Not a headlight's own bike, just behind the lamp.
    if (d > reach || d < 2.0) {
        return vec3<f32>(0.0);
    }
    let l = to / d;
    // A pole's pool fades out slowly to its edge, brightest under it.
    let into = select(smoothstep(BEAM_EDGE, BEAM_FULL, dot(-l, way)), pow(smoothstep(POLE_EDGE, POLE_FULL, dot(-l, way)), 1.5), pole);
    if (into <= 0.0) {
        return vec3<f32>(0.0);
    }
    let fade = 1.0 - smoothstep(reach * 0.7, reach, d);
    // A headlight's beam carries further than the square of how far
    // would let it, as a focused beam does.
    let falloff = select(pow(d, 1.5) + 12.0, d * d + 30.0, pole);
    return select(LAMP, POLE, pole) * into * into * fade / falloff;
}
