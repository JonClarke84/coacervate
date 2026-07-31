// The water the world is in, and the one pass that turns everything into a picture.
//
// SPEC section 12, in one paragraph: *"Background: vertical gradient (bright at the surface,
// near-black at depth), slowly drifting light shafts… Render bodies into an HDR offscreen target,
// then a separable-Gaussian bloom pass, then composite with tone mapping."* This is the
// composite, and the background is computed here rather than drawn earlier for a reason worth
// stating: **the bloom must not bloom the water.** Everything upstream of this pass holds the
// light the organisms are making and nothing else, so the blur has nothing to spread but them.
//
// ## The tone map has a knee, and the knee is load-bearing
//
// Below `KNEE` this pass is the identity. Nothing is compressed, nothing is scaled, and the light
// that comes out is the light that went in - which is what keeps SPEC section 12's additive claim
// *"two overlapping cells are twice one"* true of the finished picture rather than only of an
// intermediate buffer nobody can look at. `neighbouring_cells_merge_into_one_silhouette` measures
// that ratio off the PNG and gets exactly two.
//
// Above the knee it bends smoothly - value and slope both continuous, so there is no ring where
// the two halves meet - and approaches one without ever reaching it. That is the whole of what
// the HDR target buys. Group B's honest criticism of the first frame was that *"the interiors are
// flat - a solid slab of colour with a soft rim, a paper cut-out lit from behind"*, and the cause
// was an 8-bit target: four bodies pressed together summed to well over one, every one of those
// sums clipped to white, and the sum that makes the silhouette existed and could not be seen.
// Here a crowd summing to 1.4 and a crowd summing to 2.0 are still two different colours.

//
// ## ⭐ Phase 6, B5: seven of this file's constants are now a uniform
//
// `docs/PHASE5.md`'s Q26 - the water's colour and gradient, the shafts' strength and lean, the
// bloom's strength and the tone map's knee were all compiled in here, so a slider on any of them
// meant editing a shader and rebuilding. They live in `camera.rs`'s `Look` now, which arrives at
// `@group(3)`. Nothing about the arithmetic changed; the numbers arrive by a different road.

struct View {
    world: vec2<f32>,
    origin: vec2<f32>,
    span: vec2<f32>,
    frame: vec2<f32>,
    phase: f32,
    // Three scalars rather than a `vec3<f32>`, which would be aligned to sixteen and would make
    // this a sixty-four-byte record against `camera.rs`'s forty-eight. See `cells.wgsl`.
    unused_a: f32,
    unused_b: f32,
    unused_c: f32,
};

// Everything about what the picture looks like. `camera.rs`'s `Look`, field for field; see
// `cells.wgsl`, which declares the same record and reads the two fields this one does not.
struct Look {
    glow: f32,
    peak: f32,
    // How much of the blurred frame is added back over the top.
    //
    // ⚠️ Small, and for the same reason the shafts are faint. A bloom is what makes a bright
    // thing look like it is *emitting* rather than being painted, and it is also the single
    // easiest way to turn a calm picture into a glaring one. At a third the halo is visible as a
    // softness around a body and around a colony, and no part of the frame that was dark
    // becomes bright.
    bloom: f32,
    // Where the tone map stops being the identity. See this file's header.
    knee: f32,
    // The colour of the water where the light has not reached it.
    abyss: vec3<f32>,
    // How sharply the water darkens with depth.
    deepens: f32,
    // How much light there is at the surface, over and above the abyss.
    surface: vec3<f32>,
    // How far a shaft leans over as it goes down, as a fraction of the world's width per world's
    // depth. Light entering the sea at an angle; a perfectly vertical shaft reads as a stripe.
    lean: f32,
    // How bright the light shafts are at the surface.
    shafts: vec3<f32>,
    // Read by `frame.rs`'s blend state and by nothing in any shader. Declared so that this
    // record and `camera.rs`'s are the same record.
    trail_fade: f32,
};

@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var lit: texture_2d<f32>;
@group(1) @binding(1) var lens: sampler;
@group(2) @binding(0) var halo: texture_2d<f32>;
@group(2) @binding(1) var halo_lens: sampler;
@group(3) @binding(0) var<uniform> look: Look;

struct Covered {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// One triangle covering the frame. See `post.wgsl`, which conjures the same one.
@vertex
fn cover(@builtin(vertex_index) vertex: u32) -> Covered {
    let corner = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));

    var out: Covered;
    out.clip = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);

    return out;
}

// The water at a place in the world, before anything living is added to it.
//
// ⚠️ `look.deepens` is what Group B's frame found: depth is the dominant axis of this world - the
// four shallow colonies several times the area of the four deep ones - and a background that says
// so is a background that explains the population.
fn water_at(place: vec2<f32>) -> vec3<f32> {
    // A frame taller than the world shows water that is not there - see `camera.rs` - and it is
    // drawn as the abyss, so the surface and the floor are where the world actually ends rather
    // than being wherever the window happens to stop.
    if place.y < 0.0 || place.y > view.world.y {
        return look.abyss;
    }

    let depth = clamp(place.y / view.world.y, 0.0, 1.0);
    // ⚠️ **The six bytes.** `B5` set out to make this a uniform without changing a pixel, and it
    // changed six of the 8,294,400 in the shipped frame, each by exactly one - and this is the
    // line. Written `pow(1.0 - depth, 3.0)` the compiler folds a literal exponent of three into
    // `x * x * x`; written against a uniform it has to stay `exp2(deepens * log2(x))`, which is
    // the same function to within a unit in the last place and not the same arithmetic. Measured
    // both ways: with the literal back, the frame is byte-for-byte the one Group D drew.
    // Everything else in this file moved for nothing at all.
    let daylight = pow(1.0 - depth, look.deepens);

    // ⭐ The shafts. The horizontal position is taken as a fraction of the way round the world
    // and every frequency below is a whole number, so the pattern joins up at the seam exactly as
    // the world does - a body swimming across the join does not pass through a discontinuity in
    // the water. `view.phase` is the world's own tick count, very slowly turned into a drift; see
    // `scene.rs` for why it is the world's clock and not a wall clock.
    let across = (place.x + place.y * look.lean) / view.world.x + view.phase;
    let turn = across * 6.2831855;
    let beams = sin(turn * 3.0) * 0.5 + sin(turn * 7.0 + 1.3) * 0.3 + sin(turn * 13.0 + 2.7) * 0.2;
    // Squared, so the pattern is a few soft beams with wide dark water between them rather than
    // a ripple over the whole frame.
    let shaft = max(beams, 0.0) * max(beams, 0.0);

    return look.abyss + look.surface * daylight + look.shafts * shaft * daylight;
}

// One channel, brought inside what a screen can show.
//
// The identity below the knee; above it, an exponential approach to one whose value and slope
// both match the identity at the knee, so nothing anywhere has an edge in it.
fn toned(value: f32) -> f32 {
    if value <= look.knee {
        return value;
    }

    let over = (value - look.knee) / (1.0 - look.knee);

    return look.knee + (1.0 - look.knee) * (1.0 - exp(-over));
}

@fragment
fn composite(covered: Covered) -> @location(0) vec4<f32> {
    let place = view.origin + covered.uv * view.span;

    let light = textureSampleLevel(lit, lens, covered.uv, 0.0).rgb;
    let bloom = textureSampleLevel(halo, halo_lens, covered.uv, 0.0).rgb;
    let total = water_at(place) + light + bloom * look.bloom;

    return vec4<f32>(toned(total.r), toned(total.g), toned(total.b), 1.0);
}
