// The whole of SPEC section 12's Group B, in one vertex stage and one fragment stage.
//
//   "One instanced draw call for all cells. Per-instance: position, radius, hue, energy flow,
//    kind. Fragment shader does a soft radial falloff. Neighbouring cells drawn additively
//    merge into a single organic silhouette rather than reading as a string of beads. This one
//    technique is most of the difference between 'creature' and 'physics demo'."
//
// There is no vertex buffer of geometry here and there is no mesh. A cell is drawn as a pair of
// quads conjured out of `vertex_index`, and everything that makes it look like anything happens
// in `fragment`. What arrives on the card per frame is one array of 24-byte records and one
// draw call, whether the world holds two cells or a quarter of a million.
//
// ## The two quads, and why a cell is drawn twice
//
// SPEC section 8 joins the world up sideways. A cell standing on that join is half a cell into
// the world from the left and half a cell into it from the right, and drawing it once puts half
// of it against one edge of the frame and nothing against the other - so an organism swimming
// across the seam is sliced in two by the edge of the picture. The second quad is that cell seen
// from the other side. For a cell anywhere but near an edge it lands a whole world's width away
// and is thrown out by the clipper before a single fragment of it is shaded, so it costs six
// vertices and nothing else.
//
// ## The falloff, and why it is not a circle with an edge
//
// `(1 - d^2)^3` over the disc: one at the centre, nought at the rim, and - the part that matters
// - a *zero slope* at the rim as well. A falloff that stopped abruptly would draw a visible ring
// around every cell, and two overlapping rings read as two things. This one goes to nothing
// smoothly, so the sum of two overlapping cells has no seam in it anywhere and the pair is one
// shape. It is the cubic spike kernel that particle hydrodynamics uses, for the same reason.

// Where the world is on the frame, and how to draw light in it. `camera.rs` builds this and its
// fields are laid out to match that struct byte for byte.
struct View {
    // How big the world is, in world units. The seam is at `world.x`.
    world: vec2<f32>,
    // The world position drawn at the top-left corner of the frame.
    origin: vec2<f32>,
    // How much world the frame covers, in world units.
    span: vec2<f32>,
    // How far a cell's light reaches, as a multiple of its own radius.
    glow: f32,
    // How bright the very centre of one cell is, before anything is added to it.
    peak: f32,
};

@group(0) @binding(0) var<uniform> view: View;

// SPEC section 12's five, and `scene.rs`'s `Instance` field for field.
struct Cell {
    @location(0) position: vec2<f32>,
    @location(1) radius: f32,
    @location(2) hue: f32,
    @location(3) energy_flow: f32,
    @location(4) kind: u32,
};

struct Lit {
    @builtin(position) clip: vec4<f32>,
    // Where in the cell's own disc this fragment is: nought at the centre, one at the rim.
    @location(0) local: vec2<f32>,
    // The colour of this cell's light at its brightest, in linear units.
    @location(1) light: vec3<f32>,
};

// How much a cell's `energy_flow` may brighten or dim it.
//
// Small on purpose, and clamped at both ends. SPEC section 12 wants a well-fed organism to
// visibly glow and CLAUDE.md wants the screen not to shout; Group D's `D6` is where the two get
// balanced against a tone-mapped target. Until then this is enough that a feeding cell is
// distinguishable from a starving one and not enough to pull the eye.
const FED_REACH: f32 = 8.0;
const FED_BRIGHTEST: f32 = 0.55;
const FED_DIMMEST: f32 = -0.35;

// How saturated each of SPEC section 6's six kinds is drawn, in `scene.rs`'s numbering.
//
// SPEC section 12: "saturation and brightness modulated by cell kind and energy_flow". The
// shape of this is the shape of section 6's table rather than a decoration: a sclerocyte is
// structure with no metabolic function at all and is drawn nearly colourless, and a gonocyte -
// the cell a lineage's whole future is stored in - is the most saturated thing in the world.
fn saturation_of(kind: u32) -> f32 {
    switch kind {
        case 0u: { return 0.80; }  // photocyte  - harvests light
        case 1u: { return 0.90; }  // devorocyte - eats
        case 2u: { return 0.75; }  // myocyte    - contracts
        case 3u: { return 0.20; }  // sclerocyte - structure, and nothing else
        case 4u: { return 0.55; }  // sensocyte  - senses
        case 5u: { return 0.95; }  // gonocyte   - carries the lineage
        default: { return 0.80; }
    }
}

// A place on the colour wheel, as red, green and blue.
fn hue_to_rgb(hue: f32) -> vec3<f32> {
    let turn = fract(hue) * 6.0;

    return clamp(
        vec3<f32>(
            abs(turn - 3.0) - 1.0,
            2.0 - abs(turn - 2.0),
            2.0 - abs(turn - 4.0),
        ),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
}

@vertex
fn vertex(@builtin(vertex_index) vertex: u32, cell: Cell) -> Lit {
    // Twelve vertices a cell: two quads, six vertices each, as two triangles sharing an edge.
    // The quad's corners are numbered so that bit 0 is the horizontal and bit 1 the vertical,
    // which is what makes the arithmetic below a pair of bit tests rather than a second table.
    var winding = array<u32, 6>(0u, 1u, 2u, 2u, 1u, 3u);
    let copy = vertex / 6u;
    let numbered = winding[vertex % 6u];
    let corner = vec2<f32>(
        f32(numbered & 1u) * 2.0 - 1.0,
        f32(numbered >> 1u) * 2.0 - 1.0,
    );

    // The seam. Copy one is this cell seen from the other side of the join - to the right of
    // the world for a cell in its left-hand half, to the left of it for one in the right. Only
    // one of the two can ever be near enough to the frame to matter, because a world is very
    // much wider than a cell's light reaches.
    var x = cell.position.x;
    if copy == 1u {
        if x * 2.0 < view.world.x {
            x = x + view.world.x;
        } else {
            x = x - view.world.x;
        }
    }

    let reach = cell.radius * view.glow;
    let at = vec2<f32>(x, cell.position.y) + corner * reach;
    let across = (at - view.origin) / view.span;

    // Depth increases downwards - SPEC section 4 has the light fall off with `y` - so the
    // surface is the top of the frame and the vertical axis is flipped on its way to clip
    // space, where +1 is up.
    var out: Lit;
    out.clip = vec4<f32>(across.x * 2.0 - 1.0, 1.0 - across.y * 2.0, 0.0, 1.0);
    out.local = corner;

    let fed = clamp(cell.energy_flow * FED_REACH, FED_DIMMEST, FED_BRIGHTEST);
    let tint = mix(
        vec3<f32>(1.0),
        hue_to_rgb(cell.hue),
        saturation_of(cell.kind),
    );
    out.light = tint * view.peak * (1.0 + fed);

    return out;
}

@fragment
fn fragment(lit: Lit) -> @location(0) vec4<f32> {
    // The disc is inscribed in the quad, so the corners are outside it and are thrown away.
    let squared = dot(lit.local, lit.local);
    if squared >= 1.0 {
        discard;
    }

    // ⭐ The soft radial falloff. Everything else in this file is scaffolding for this line.
    let reached = 1.0 - squared;
    let falloff = reached * reached * reached;

    // The alpha is one wherever anything at all was drawn, and the blend adds it, so the frame
    // comes out opaque. Nothing here is transparent: the pipeline adds light to the water, it
    // does not lay anything over it.
    return vec4<f32>(lit.light * falloff, 1.0);
}
