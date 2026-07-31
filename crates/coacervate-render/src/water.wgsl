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

struct View {
    world: vec2<f32>,
    origin: vec2<f32>,
    span: vec2<f32>,
    glow: f32,
    peak: f32,
    frame: vec2<f32>,
    phase: f32,
    unused: f32,
};

@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var lit: texture_2d<f32>;
@group(1) @binding(1) var lens: sampler;
@group(2) @binding(0) var halo: texture_2d<f32>;
@group(2) @binding(1) var halo_lens: sampler;

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

// The colour of the water where the light has not reached it.
//
// Not black. A frame of pure black is a frame with no depth in it and nothing for the deep
// colonies to sit against, and the sea is not black at four hundred metres either - it is the
// blue that is left when everything else has been absorbed.
const ABYSS: vec3<f32> = vec3<f32>(0.0020, 0.0038, 0.0075);

// How much light there is at the surface, over and above the abyss.
//
// Cool and green-blue, which is what the top of a water column looks like and is also the useful
// choice: SPEC section 6's cells are drawn across the whole colour wheel and a background biased
// to one end of it would flatter half of them and hide the other half. Its luminance is about
// 0.069 against a lone cell's 0.34, so a body in the brightest water is still five times the
// water it is in.
const SURFACE: vec3<f32> = vec3<f32>(0.0230, 0.0520, 0.0760);

// How sharply the water darkens with depth.
//
// SPEC section 4 has the light itself fall off with depth and `light.gradient` is 0.75, so the
// floor of the world receives a quarter of what the surface does. This is steeper than that on
// purpose: what is being drawn is not the light falling but the light *coming back*, which has
// crossed the depth twice. Group B's frame found depth to be the dominant axis of this world - the
// four shallow colonies several times the area of the four deep ones - and a background that says
// so is a background that explains the population.
const DEEPENS: f32 = 3.0;

// How bright the light shafts are at the surface.
//
// ⚠️ **A tenth of the surface water and no more.** CLAUDE.md: *"no flashing, no sudden camera
// moves, nothing that pulls the eye. When something dramatic happens in the simulation, the log
// says so - the screen does not shout."* A shaft is the easiest thing in this file to overdo, and
// an overdone one is a bright bar crossing the picture that the eye goes to every time instead of
// to the organisms. At this strength they are a texture in the water rather than an object in it.
const SHAFTS: vec3<f32> = vec3<f32>(0.0090, 0.0165, 0.0225);

// How far a shaft leans over as it goes down, as a fraction of the world's width per world's
// depth. Light entering the sea at an angle; a perfectly vertical shaft reads as a stripe.
const LEAN: f32 = 0.22;

// Where the tone map stops being the identity. See this file's header.
const KNEE: f32 = 0.75;

// How much of the blurred frame is added back over the top.
//
// ⚠️ Small, and for the same reason the shafts are faint. A bloom is what makes a bright thing
// look like it is *emitting* rather than being painted, and it is also the single easiest way to
// turn a calm picture into a glaring one. At a third the halo is visible as a softness around a
// body and around a colony, and no part of the frame that was dark becomes bright.
const BLOOM: f32 = 0.33;

// The water at a place in the world, before anything living is added to it.
fn water_at(place: vec2<f32>) -> vec3<f32> {
    // A frame taller than the world shows water that is not there - see `camera.rs` - and it is
    // drawn as the abyss, so the surface and the floor are where the world actually ends rather
    // than being wherever the window happens to stop.
    if place.y < 0.0 || place.y > view.world.y {
        return ABYSS;
    }

    let depth = clamp(place.y / view.world.y, 0.0, 1.0);
    let daylight = pow(1.0 - depth, DEEPENS);

    // ⭐ The shafts. The horizontal position is taken as a fraction of the way round the world
    // and every frequency below is a whole number, so the pattern joins up at the seam exactly as
    // the world does - a body swimming across the join does not pass through a discontinuity in
    // the water. `view.phase` is the world's own tick count, very slowly turned into a drift; see
    // `scene.rs` for why it is the world's clock and not a wall clock.
    let across = (place.x + place.y * LEAN) / view.world.x + view.phase;
    let turn = across * 6.2831855;
    let beams = sin(turn * 3.0) * 0.5 + sin(turn * 7.0 + 1.3) * 0.3 + sin(turn * 13.0 + 2.7) * 0.2;
    // Squared, so the pattern is a few soft beams with wide dark water between them rather than
    // a ripple over the whole frame.
    let shaft = max(beams, 0.0) * max(beams, 0.0);

    return ABYSS + SURFACE * daylight + SHAFTS * shaft * daylight;
}

// One channel, brought inside what a screen can show.
//
// The identity below the knee; above it, an exponential approach to one whose value and slope
// both match the identity at the knee, so nothing anywhere has an edge in it.
fn toned(value: f32) -> f32 {
    if value <= KNEE {
        return value;
    }

    let over = (value - KNEE) / (1.0 - KNEE);

    return KNEE + (1.0 - KNEE) * (1.0 - exp(-over));
}

@fragment
fn composite(covered: Covered) -> @location(0) vec4<f32> {
    let place = view.origin + covered.uv * view.span;

    let light = textureSampleLevel(lit, lens, covered.uv, 0.0).rgb;
    let bloom = textureSampleLevel(halo, halo_lens, covered.uv, 0.0).rgb;
    let total = water_at(place) + light + bloom * BLOOM;

    return vec4<f32>(toned(total.r), toned(total.g), toned(total.b), 1.0);
}
