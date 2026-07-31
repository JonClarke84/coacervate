// The two passes that happen between the cells being drawn and the picture being made: the
// motion trail, and the bloom.
//
// Everything here reads one texture and writes another, over the whole frame, so there is no
// geometry: `cover` conjures a single triangle big enough to hang off all four edges of the
// screen. One triangle rather than two, because a quad's diagonal is a seam that every fragment
// along it is shaded twice at.
//
// ## D2 - the accumulation buffer, and why it is a maximum rather than a sum
//
// SPEC section 12 asks for *"an accumulation buffer with a slow fade"*. The obvious arrangement
// is `trail = scene + trail × fade`, which is a sum, and a sum of a thing that is not moving
// converges on `scene / (1 - fade)` - at a fade of 0.965 that is **twenty-eight times** as bright
// as the cell that is standing there. Every colony in the world would be a white slab within a
// couple of seconds of the run starting, and that is exactly `docs/PHASE5.md`'s warning about
// smearing into mush.
//
// So it is `trail = max(scene, trail × fade)`. A cell can never make the frame brighter than
// itself, whatever it does; what it leaves behind it is a tail that decays geometrically to
// nothing, and a body standing still simply looks like a body standing still. Group D's
// `an_accumulation_buffer_leaves_motion_trails` is that pair of claims stated as measurements.
//
// It takes two draws into the same attachment and neither reads it, which is what makes one
// texture enough where the arithmetic would otherwise want two:
//
// 1. `fade`, whose blend state is `source × 0 + destination × constant`, so what it writes is
//    irrelevant and what is already there is multiplied by the fade. The constant is set on the
//    pass by `frame.rs`.
// 2. `accumulate`, whose blend operation is `max`, over the top.
//
// ## D1 - the bloom, and why it is separable
//
// A two-dimensional Gaussian of radius `r` costs `(2r + 1)²` samples a pixel taken directly, and
// `2 × (2r + 1)` taken as a horizontal pass followed by a vertical one, because a Gaussian is the
// product of two one-dimensional Gaussians. At the thirteen taps below that is 26 samples instead
// of 169. SPEC section 12 asks for the separable one by name.
//
// It is done at **half the frame's resolution**, which halves the taps again and costs nothing
// visible: the whole point of a blur is that it has no detail in it. Sampling a full-resolution
// texture at half-resolution texel centres lands exactly on the corner between four texels, so
// the hardware's linear filter averages all four - the downsample is a proper box filter rather
// than a decimation that would alias.

struct Covered {
    @builtin(position) clip: vec4<f32>,
    // Where on the source texture this fragment is: nought at the top left, one at the
    // bottom right.
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var lens: sampler;

// One triangle covering the frame, out of nothing but the vertex number.
//
// Vertex 0 lands at the bottom left of the screen, vertex 1 a screen's width to the right of the
// bottom right, and vertex 2 a screen's height above the top left. The part of it that is on the
// screen is the whole screen.
@vertex
fn cover(@builtin(vertex_index) vertex: u32) -> Covered {
    let corner = vec2<f32>(f32((vertex << 1u) & 2u), f32(vertex & 2u));

    var out: Covered;
    out.clip = vec4<f32>(corner * 2.0 - 1.0, 0.0, 1.0);
    // Clip space has +1 at the top and a texture has nought there, so the vertical axis turns
    // over on the way across.
    out.uv = vec2<f32>(corner.x, 1.0 - corner.y);

    return out;
}

// Dim whatever is already in the trail buffer.
//
// The value returned is thrown away - the blend state multiplies it by nought - and what the
// draw actually does is multiply the destination by the pass's blend constant. See this file's
// header.
@fragment
fn fade() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// Put this frame's light into the trail buffer, wherever it is brighter than what is there.
@fragment
fn accumulate(covered: Covered) -> @location(0) vec4<f32> {
    return textureSampleLevel(source, lens, covered.uv, 0.0);
}

// How far the blur reaches, in texels of the half-resolution buffer it runs in.
//
// Six either side of the middle, which is twelve full-resolution pixels. Group C measured a
// two-celled body at about eight pixels across at 1280 and twelve at 1920, so the halo around one
// body is about one body wide: enough to give a flat interior somewhere to fall off to, and not
// enough to join two bodies that are not touching into one.
const REACH: i32 = 6;

// Blur along one axis.
//
// `stride` is how many of the *source's* texels one of the destination's is, because the two
// passes are not reading the same size of thing: the horizontal one reads the full-resolution
// trail and writes half of it, and the vertical one reads and writes at half. A step measured in
// the source's texels would make the first blur half as wide as the second and the result would
// be an oval rather than a circle.
//
// The weights are a Gaussian of standard deviation three texels, normalised, from the middle
// outwards. Written out rather than computed because they are constants and this runs twice for
// every pixel of every frame. **They sum to one**, which is what makes the blur a redistribution
// of light rather than a gain, and is what lets `frame.rs` state how much brighter the bloom
// makes anything.
fn blurred(uv: vec2<f32>, axis: vec2<f32>, stride: f32) -> vec4<f32> {
    var weights = array<f32, 7>(0.13702, 0.12962, 0.10971, 0.08310, 0.05633, 0.03417, 0.01854);

    let step = axis * stride / vec2<f32>(textureDimensions(source, 0));

    var total = textureSampleLevel(source, lens, uv, 0.0) * weights[0];
    for (var tap = 1; tap <= REACH; tap++) {
        let away = f32(tap) * step;
        total += (textureSampleLevel(source, lens, uv + away, 0.0)
            + textureSampleLevel(source, lens, uv - away, 0.0)) * weights[tap];
    }

    return total;
}

// Reads the frame at full resolution and writes it at half, so one of its own texels is two of
// the source's.
@fragment
fn blur_across(covered: Covered) -> @location(0) vec4<f32> {
    return blurred(covered.uv, vec2<f32>(1.0, 0.0), 2.0);
}

// Reads and writes at half resolution.
@fragment
fn blur_down(covered: Covered) -> @location(0) vec4<f32> {
    return blurred(covered.uv, vec2<f32>(0.0, 1.0), 1.0);
}
