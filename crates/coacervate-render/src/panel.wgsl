// The chrome, drawn over the finished picture. `panel.rs` explains why this shader exists in
// this repository at all rather than coming out of `egui-wgpu`: the newest `egui-wgpu` names
// `wgpu ^29`, this renderer is written against `wgpu 30`, and the two do not unify.
//
// egui hands over a triangle list in *points* with the origin at the top left, a texture
// coordinate and a packed colour. Everything below is the smallest thing that draws that.

struct Screen {
    /// The frame, in egui's points - which is pixels divided by `points_per_pixel`.
    size: vec2<f32>,
    /// Written so the record is a whole number of sixteen-byte blocks. `bytemuck::Pod` refuses
    /// to derive for a struct with padding in it, and that refusal is right: padding is bytes
    /// nobody wrote being sent to the card.
    pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var lens: sampler;

struct Shaded {
    @builtin(position) place: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

/// A `Color32` is four bytes in memory and arrives here as one little-endian word.
fn unpack(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        f32(packed & 255u),
        f32((packed >> 8u) & 255u),
        f32((packed >> 16u) & 255u),
        f32((packed >> 24u) & 255u),
    ) / 255.0;
}

/// 0-1 linear light from 0-1 sRGB.
///
/// ⚠️ **This is the one thing about drawing egui that is easy to get silently wrong.** egui
/// works in sRGB values throughout - a `Color32` of 128 means half-*bright*, not half the
/// light - and `frame.rs`'s target is `Rgba8UnormSrgb`, which means the card encodes linear
/// light into sRGB on the way in and blends in linear. So a colour handed straight through
/// would be encoded twice and every panel would come out washed pale. Undoing the curve here
/// is what makes the byte that lands in the PNG the byte egui asked for.
fn linear_from_srgb(srgb: vec3<f32>) -> vec3<f32> {
    let low = srgb / 12.92;
    let high = pow((srgb + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));

    return select(high, low, srgb < vec3<f32>(0.04045));
}

@vertex
fn vertex(
    @location(0) at: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) packed: u32,
) -> Shaded {
    var out: Shaded;
    out.uv = uv;
    out.tint = unpack(packed);
    out.place = vec4<f32>(
        2.0 * at.x / screen.size.x - 1.0,
        1.0 - 2.0 * at.y / screen.size.y,
        0.0,
        1.0,
    );

    return out;
}

@fragment
fn fragment(in: Shaded) -> @location(0) vec4<f32> {
    // The atlas is `Rgba8Unorm` - not sRGB - so what comes back is the byte egui wrote, which
    // is a coverage value for a glyph and a flat white for everything else. Multiplying in sRGB
    // space is what egui's own backend does and what its anti-aliasing was tuned against.
    let shade = in.tint * textureSample(atlas, lens, in.uv);

    return vec4<f32>(linear_from_srgb(shade.rgb), shade.a);
}
