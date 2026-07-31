// Marine snow: SPEC section 12's *"marine snow — which is the actual detritus, not decoration"*.
//
// Every speck this draws is a grain out of `World::drift`, at the position the body that made it
// died at, holding what the ledger's `detritus` account says it holds. Nothing here is scattered
// by a random number, nothing is invented to fill the water, and a world in which nothing has yet
// died has no snow in it at all. `marine_snow_is_the_actual_detritus` is that claim as a test.
//
// ## Why it is drawn in the composite pass and not with the cells
//
// Because it must not bloom and it must not leave a trail. A grain is dead matter falling, not
// something making light: a halo round it would say it was alive, and a tail behind it would draw
// the eye to the one thing in the frame that nothing is happening to. So it goes on at the very
// end, over the tone-mapped picture, additively - which on an sRGB target is still an addition of
// light, because the card blends in linear and encodes on the way out.
//
// It is a second instanced draw call, and SPEC section 12's *"one instanced draw call for all
// cells"* is untouched by it: this draws no cells. `Frame::draws` counts the one that does.

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

@group(0) @binding(0) var<uniform> view: View;

struct Grain {
    @location(0) position: vec2<f32>,
    @location(1) energy: f32,
};

struct Speck {
    @builtin(position) clip: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) light: vec3<f32>,
};

// How much energy a grain holds when it is freshly dead, near enough.
//
// A cell of SPEC section 6's costs a few units to build and a corpse is shared out between its
// cells, so a new grain is around a unit and rots from there. This is only a scale: what it
// decides is how quickly a grain fades as it rots, and nothing depends on it being exact.
const FULL: f32 = 1.0;

// How wide a grain is drawn, in world units, when it is fresh.
const GRAIN: f32 = 1.1;

// The smallest a grain may be drawn, in pixels.
//
// Without this, zooming out to the whole world would shrink every grain below one pixel and the
// snow would flicker in and out as the camera moved - which is precisely the thing CLAUDE.md's
// visually-calm constraint forbids. Held at a pixel and a half so a grain is always a speck and
// never a hard dot.
const SMALLEST: f32 = 1.5;

// How bright a fresh grain is at its middle.
//
// ⚠️ Well under a tenth of a cell's. Snow is the texture of the water, not a population of small
// organisms, and snow bright enough to count is snow that makes the world look busier than it is.
const LIGHT: vec3<f32> = vec3<f32>(0.030, 0.034, 0.040);

@vertex
fn vertex(@builtin(vertex_index) vertex: u32, grain: Grain) -> Speck {
    // Twelve vertices, exactly as `cells.wgsl` uses: the grain, and the grain seen from the
    // other side of the world's seam. A grain sitting on the join is whole.
    var winding = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let copy = vertex / 6u;
    let numbered = winding[vertex % 6u];
    let corner = vec2<f32>(
        f32(numbered & 1u) * 2.0 - 1.0,
        f32(numbered >> 1u) * 2.0 - 1.0,
    );

    var x = grain.position.x;
    if copy == 1u {
        if x * 2.0 < view.world.x {
            x = x + view.world.x;
        } else {
            x = x - view.world.x;
        }
    }

    // How much world one pixel covers, so that a grain has a floor in pixels as well as a size
    // in world units.
    let scale = view.span.x / view.frame.x;
    let left = clamp(grain.energy / FULL, 0.0, 1.0);
    let reach = max(GRAIN * (0.45 + 0.55 * left), SMALLEST * scale);

    let at = vec2<f32>(x, grain.position.y) + corner * reach;
    let across = (at - view.origin) / view.span;

    var out: Speck;
    out.clip = vec4<f32>(across.x * 2.0 - 1.0, 1.0 - across.y * 2.0, 0.0, 1.0);
    out.local = corner;
    out.light = LIGHT * (0.25 + 0.75 * left);

    return out;
}

@fragment
fn fragment(speck: Speck) -> @location(0) vec4<f32> {
    let squared = dot(speck.local, speck.local);
    if squared >= 1.0 {
        discard;
    }

    // The same soft falloff the cells use, for the same reason: a speck with an edge on it is a
    // hard dot, and a frame full of hard dots is a frame that looks like static.
    let reached = 1.0 - squared;

    return vec4<f32>(speck.light * reached * reached * reached, 1.0);
}
