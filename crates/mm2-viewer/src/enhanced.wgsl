// The enhanced drawing's settings (enhance.rs EnhGlobals).
struct Enh {
    cascades: array<mat4x4<f32>, 3>,
    // Each cascade's reach, ft, and in w where the shadows end.
    splits: vec4<f32>,
    // x lighting, y shadows, z haze, w sky: 1 on.
    flags: vec4<f32>,
    // The haze's start, thickness per ft, and the far plane; w how
    // strongly the new ground's detail shows (0 to 1).
    haze: vec4<f32>,
    sun: vec4<f32>,
    to_sun: vec4<f32>,
    fog: vec4<f32>,
    // Each cascade's texel, ft; w soft shadows.
    texel: vec4<f32>,
    // The clock (s), wetness, rain, cloud cover.
    world: vec4<f32>,
    // The ruts' map: its corner (x, z), the ft it spans, on.
    ruts: vec4<f32>,
    // x the new ground, y grass, z the real sky, w the bikes' paint.
    more: vec4<f32>,
    // The photo materials' average colours (materials.rs).
    material_average: array<vec4<f32>, 9>,
    // x the screen's width over its height, y the bikes' mud, z cloud
    // drift, w exposure.
    sky: vec4<f32>,
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    // The smooth ground's patch: its corner (terrain units), cells a side,
    // on.
    fine_ground: vec4<f32>,
    // x the objects' photo materials, y swaying trees, z round bikes and
    // riders, w a stadium's floodlights.
    surfaces: vec4<f32>,
    // x the shadows ray traced.
    traced: vec4<f32>,
    // From the drawing to the screen, for the ray-traced shadows.
    view_proj: mat4x4<f32>,
    // The ambient light's colour, for what the reflections see.
    ambient: vec4<f32>,
    // From the drawing to the dust's shadow map, and x whether it is
    // drawn, y the least spread of a cloud's depth in it.
    dust_view: mat4x4<f32>,
    dust: vec4<f32>,
    // The terrain for the reflections: ft a terrain unit, and terrain
    // units the ground's colour map spans.
    terrain: vec4<f32>,
    // x how much it is night (0 to 1); yzw the track's own light on level
    // ground, which the ground's painted light is taken against at night.
    night: vec4<f32>,
    // Each bike's headlight, ray traced (rt.rs): x on, y how many glow.
    headlights: vec4<f32>,
    // Each glowing headlight's place and strength.
    lamps: array<vec4<f32>, 64>,
    // The light poles' baked light (flood.rs): its map's corner (x, z)
    // and the ft it spans across and down; 0 for none.
    flood: vec4<f32>,
    // The sky's light baked over the land (skylight.rs): samples a side,
    // ft between them, how strongly it shows (0 off), and w 1 when the
    // ground marks its water for the screen's reflections (post.wgsl).
    sky_light: vec4<f32>,
    // The baked map's heights: the lowest (ft) and the range over them.
    sky_height: vec4<f32>,
    // A stadium's own lights at night (night.rs): x how many, y how much
    // they light (0 off), z the lowest lamp's height, w the shadow groups.
    stadium: vec4<f32>,
    stadium_views: array<mat4x4<f32>, 3>,
    stadium_texels: vec4<f32>,
    // Each lamp: xyz where, w power; xyz its way, w its beam's edge plus
    // 10 times its shadow group.
    stadium_lamps: array<vec4<f32>, 24>,
    stadium_ways: array<vec4<f32>, 24>,
    // The sky's light all round (ibl.rs): x whether the physically based
    // materials take it, y the cube's roughest level, z how much of it the
    // game's own models take; w 1 for the sunlit air in the haze.
    ibl: vec4<f32>,
    // Its irradiance as harmonics, for the polynomials in irradiance().
    sh: array<vec4<f32>, 9>,
};
