//! Where the world is on the frame, and what it is drawn to look like.
//!
//! Three things live here. [`Camera`] is what the shader is told about *where* - an origin and a
//! span, and nothing that can move - [`Lens`] is Group C's addition, the same view with a
//! person's hands on it, and [`Look`] is Phase 6's: every number that decides what the picture
//! looks like, in one record on the card.
//!
//! # ⭐ Phase 6, `B5`: the look is a uniform and not a constant
//!
//! `docs/PHASE5.md`'s **Q26**, in full:
//!
//! > Phase 6 will want to move some of these numbers, and none of them can be moved at run time.
//! > Everything Group D tunes is a constant compiled into a shader or into `camera.rs`: the
//! > bloom's strength and radius, the trail's fade, the peak brightness, the tone map's knee,
//! > the water's colour and gradient, the shafts' strength. A panel with a slider on any of them
//! > needs those values in the `View` uniform instead - which is a straightforward change and
//! > should be made **once**, for all of them together, rather than one at a time as each slider
//! > is asked for.
//!
//! [`Look`] is that, done once. What it changed about the *shape* of things is worth saying,
//! because it is not quite what the question asked for: the numbers did not go into [`View`],
//! they went into a record beside it, and [`View`]'s two look-ish fields - `glow` and `peak` -
//! came **out** and joined them. So the split is now the honest one. [`View`] says where the
//! world is; [`Look`] says how it is drawn. A slider belongs to exactly one of them and there is
//! no third place for a number about the picture to live.
//!
//! ⚠️ **The guard tying `PEAK` and `TONE_KNEE` survived the move twice over.** It was a `const`
//! block - *"a peak of more than half the tone map's knee leaves no room for two cells to add up
//! to exactly twice one"* - and a `const` block cannot see a value somebody is dragging. So it is
//! now both: the same `const` assertion still holds over [`PEAK`] and [`crate::frame::TONE_KNEE`],
//! which are the **defaults**, and [`Look::compresses`] is the runtime form, asserted every time
//! a look reaches the card. See [`Look::sane`].
//!
//! # ⚠️ The camera moves only when it is moved
//!
//! SPEC section 12: *"Camera: smooth pan and zoom, user-driven only. **It must never move on
//! its own**"*, and CLAUDE.md's second-screen constraint is where that comes from - this runs
//! beside whatever you are actually doing, and a view that drifts is a view that keeps taking
//! your eye back.
//!
//! [`Lens`] holds that by construction rather than by care: **it has no clock in it and no
//! notion of a frame having passed.** Every method on it takes something a person did, and
//! there is no method that takes elapsed time. Nothing that only reads a `Lens` can make it
//! move, and `the_camera_pans_and_zooms_and_only_when_asked` states that as a test by drawing a
//! thousand frames from an untouched one and comparing every one of them.
//!
//! That is also why there is no easing. An eased camera keeps moving after the input that
//! started it has stopped, which is exactly the thing forbidden above; the smoothness comes
//! from the *steps* being small - a tenth of the view a wheel notch - rather than from a
//! decay applied afterwards.
//!
//! # At rest it fits the width, and the reason is the seam
//!
//! SPEC section 8 joins the world up sideways, and `cells.wgsl` draws every cell a second time
//! a world's width away so that a body standing on the join is whole. Two things follow, and
//! both are the same fact seen from different ends.
//!
//! **A window opens showing the whole width**, so its left and right edges are the same place
//! and the join is drawn across them. The depth is scaled by the same factor and centred, so
//! nothing is stretched: at SPEC section 3's shipped world - 2048 by 1152, exactly sixteen by
//! nine - a frame of any sixteen-by-nine size shows all of it and no water that is not there.
//!
//! **And zooming out stops there.** One extra copy of each cell covers any view up to one world
//! wide and no more, so a view wider than the world would show a stripe of nothing where the
//! world ought to repeat. Sideways there is otherwise no limit at all: [`Lens::pan`] wraps, so
//! dragging east far enough arrives back where it started rather than at a wall.

use bytemuck::{Pod, Zeroable};

/// How far a cell's light reaches, as a multiple of its own radius.
///
/// ⭐ **This is the number that decides whether cells merge**, and it is worth being plain
/// about why it is bigger than one. A cell of SPEC section 6's is between two and three and a
/// half units wide; `founding.rs` springs two of them together at a rest length of **eight**.
/// A glow that stopped at the cell's own edge would leave two units of black water between the
/// two halves of every founder, and SPEC section 12's sentence - *"neighbouring cells drawn
/// additively merge into a single organic silhouette rather than reading as a string of
/// beads"* - would be false of the plainest body in the world.
///
/// At 2.6 a photocyte's light reaches 7.8 units, so two cells eight apart overlap over almost
/// their whole separation and the pair reads as one shape with a slight waist.
/// `neighbouring_cells_merge_into_one_silhouette` measures that rather than assuming it.
pub const GLOW: f32 = 2.6;

/// How bright the very centre of one cell is, before anything is added to it.
///
/// Deliberately well below one. The whole technique is *additive*: two cells that overlap sum,
/// and a pair's midpoint comes out brighter than either centre alone. At one, every overlap
/// would clip to white and a body would be a flat blob with no interior at all - the sum would
/// be there and nothing would be able to show it.
///
/// ⭐ **Group D lowered this from a half to a third, and the reason is the tone map's knee.**
/// `water.wgsl` is the identity below `KNEE` and compresses smoothly above it, so a pair of cells
/// whose light sums to under the knee comes out of the composite at exactly twice one cell - the
/// property `neighbouring_cells_merge_into_one_silhouette` measures off the finished PNG. At a
/// half, `founding.rs`'s two-celled founder already summed past the knee and the measurement
/// would have had to accept a fudge factor; at a third, a founder sits just under it and only a
/// genuine crowd goes over. What is above the knee is where the HDR target earns its keep: four
/// bodies pressed together are no longer four bodies clipped to white.
pub const PEAK: f32 = 0.34;

/// What the shader is told about **where the world is**.
///
/// `repr(C)` and `Pod` for the same reason [`crate::scene::Instance`] is - this becomes the
/// bytes of a uniform buffer, and the layout is `cells.wgsl`'s `View` struct field for field.
///
/// ⭐ Phase 6 took `glow` and `peak` out of here and put them in [`Look`], where everything else
/// about how the picture is drawn now lives. Nothing in this record is a matter of taste any
/// more: every field is a fact about the camera or the frame.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct View {
    /// How big the world is, in world units. Its width is where the seam is.
    pub world: [f32; 2],

    /// The world position drawn at the top-left corner of the frame.
    pub origin: [f32; 2],

    /// How much world the frame covers, in world units.
    pub span: [f32; 2],

    /// How big the frame is, in pixels.
    ///
    /// `snow.wgsl` is what wants it: a grain of detritus has a size in world units and a floor in
    /// *pixels*, so that zooming out to the whole world does not shrink the snow below one pixel
    /// and make it flicker as the camera moves.
    pub frame: [f32; 2],

    /// How far the light shafts have drifted, as a fraction of one full turn. See
    /// [`crate::scene::shaft_phase`].
    pub phase: f32,

    /// Nothing at all.
    ///
    /// A uniform buffer's contents have to be a whole number of sixteen-byte blocks, and the
    /// five fields above are thirty-six bytes. Named rather than left to `repr(C)`'s tail
    /// padding, because `bytemuck::Pod` will not derive for a struct with padding in it - and
    /// that refusal is the right one: padding is bytes nobody wrote being sent to the card.
    pub unused: [f32; 3],
}

/// ⭐⭐ **`B5`.** Every number that decides what the picture looks like, in one record.
///
/// See this module's header for **Q26**, which is the question this answers. Before Phase 6 all
/// ten of these were compile-time constants in three different files - two in `camera.rs`, one
/// in `frame.rs` and seven in `water.wgsl` - and a slider on any of them meant a separate small
/// change to the uniform, the shader and the pipeline layout. They moved together, once.
///
/// # The layout, which is written twice and has to match
///
/// `water.wgsl` and `cells.wgsl` both declare this struct. WGSL gives `vec3<f32>` an alignment of
/// sixteen bytes and a size of twelve, so a `vec3` followed by an `f32` occupies exactly one
/// sixteen-byte block with nothing between them - which is what `[f32; 3]` followed by `f32`
/// does in `repr(C)` as well. The three colours are therefore each paired with the scalar that
/// belongs beside them, and the record is sixty-four bytes with no padding anywhere.
/// `the_look_is_what_the_shaders_declare` measures that rather than trusting it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Look {
    /// How far a cell's light reaches, as a multiple of its own radius. Was [`GLOW`].
    pub glow: f32,

    /// How bright the very centre of one cell is, before anything is added to it. Was [`PEAK`].
    pub peak: f32,

    /// How much of the blurred frame is added back over the top. Was `water.wgsl`'s `BLOOM`.
    pub bloom: f32,

    /// Where the tone map stops being the identity. Was [`crate::frame::TONE_KNEE`], and it is
    /// still that as a default - see [`Look::sane`] for the guard that ties it to `peak`.
    pub knee: f32,

    /// The colour of the water where the light has not reached it. Was `water.wgsl`'s `ABYSS`.
    pub abyss: [f32; 3],

    /// How sharply the water darkens with depth. Was `water.wgsl`'s `DEEPENS`.
    pub deepens: f32,

    /// How much light there is at the surface, over and above the abyss. Was `SURFACE`.
    pub surface: [f32; 3],

    /// How far a shaft leans over as it goes down. Was `water.wgsl`'s `LEAN`.
    pub lean: f32,

    /// How bright the light shafts are at the surface. Was `water.wgsl`'s `SHAFTS`.
    pub shafts: [f32; 3],

    /// What is left of a motion trail after one frame.
    ///
    /// ⚠️ **The one field here no shader reads**, and it is here anyway. It is a blend constant
    /// rather than a number in an expression - `frame.rs` hands it to `set_blend_constant` - so
    /// the card gets it by a different road from the other nine. Keeping it in this record is
    /// the point of Q26's word *"together"*: there is one place a person changes what the
    /// picture looks like, and a tenth number kept somewhere else would be the one that got
    /// forgotten. The shaders declare it and do not use it, so the two declarations stay
    /// field-for-field identical.
    pub trail_fade: f32,
}

impl Look {
    /// The picture Group D of Phase 5 shipped, which is what these numbers were before they
    /// could be moved.
    ///
    /// ⭐ **Every value here is Group D's, to the digit**, and that is the whole regression guard
    /// on `B5`: a frame drawn through this is required to be byte-for-byte the frame the
    /// constants drew. Each one's reasoning is on the field that carries it or on the constant
    /// it came from; what follows is only where it used to live.
    pub const DEFAULT: Self = Self {
        glow: GLOW,
        peak: PEAK,

        // ⚠️ Small, and for the same reason the shafts are faint. A bloom is what makes a bright
        // thing look like it is *emitting* rather than being painted, and it is also the single
        // easiest way to turn a calm picture into a glaring one. At a third the halo is visible
        // as a softness around a body and around a colony, and no part of the frame that was
        // dark becomes bright.
        bloom: 0.33,
        knee: crate::frame::TONE_KNEE,

        // Not black. A frame of pure black is a frame with no depth in it and nothing for the
        // deep colonies to sit against, and the sea is not black at four hundred metres either -
        // it is the blue that is left when everything else has been absorbed.
        abyss: [0.0020, 0.0038, 0.0075],

        // SPEC section 4 has the light itself fall off with depth and `light.gradient` is 0.75,
        // so the floor of the world receives a quarter of what the surface does. This is steeper
        // than that on purpose: what is being drawn is not the light falling but the light
        // *coming back*, which has crossed the depth twice.
        deepens: 3.0,

        // Cool and green-blue, which is what the top of a water column looks like and is also
        // the useful choice: SPEC section 6's cells are drawn across the whole colour wheel and
        // a background biased to one end of it would flatter half of them and hide the other
        // half. Its luminance is about 0.069 against a lone cell's 0.34, so a body in the
        // brightest water is still five times the water it is in.
        surface: [0.0230, 0.0520, 0.0760],

        // Light entering the sea at an angle; a perfectly vertical shaft reads as a stripe.
        lean: 0.22,

        // ⚠️ **A tenth of the surface water and no more.** CLAUDE.md: *"nothing that pulls the
        // eye. When something dramatic happens in the simulation, the log says so - the screen
        // does not shout."* A shaft is the easiest thing here to overdo, and an overdone one is
        // a bright bar crossing the picture that the eye goes to every time instead of to the
        // organisms. At this strength they are a texture in the water rather than an object.
        shafts: [0.0090, 0.0165, 0.0225],

        // See `frame.rs`'s `TRAIL_FADE`, which is where the measurement that set this is
        // written down.
        trail_fade: crate::frame::TRAIL_FADE,
    };

    /// Whether a two-celled body would be compressed by the tone map.
    ///
    /// ⭐⭐ **This is the `const` assertion Group D wrote, in the form a slider left it in.**
    /// `water.wgsl` is the identity below the knee, which is what makes SPEC section 12's *"two
    /// overlapping cells are exactly twice one"* true of the **finished picture** and not merely
    /// of an intermediate buffer nobody can look at - and it is what
    /// `neighbouring_cells_merge_into_one_silhouette` measures off a PNG and gets exactly two
    /// from. Two cells at a peak above half the knee sum past it, the composite bends them, and
    /// that measurement becomes approximate.
    ///
    /// A `const` block could hold it while both numbers were constants. It cannot hold it while
    /// somebody is dragging one, so [`Look::sane`] is asserted instead, at the one place a look
    /// reaches the card. The `const` block still exists over the defaults - see
    /// `the_glow_reaches_past_the_cell_and_leaves_room_to_be_added_to`.
    #[must_use]
    pub fn compresses(&self) -> bool {
        self.peak * 2.0 > self.knee
    }

    /// The same claim, asserted.
    ///
    /// CLAUDE.md: *"invariants are asserted at runtime, not just in tests"*. `frame.rs` calls
    /// this on every look it is given, so there is no route to the card that does not pass it.
    ///
    /// # Panics
    ///
    /// If the peak is not positive, or if two cells at it would be compressed by the tone map.
    pub fn sane(&self) {
        assert!(
            self.peak > 0.0 && !self.compresses(),
            "a peak of {} against a tone-map knee of {} leaves no room for two cells to add up \
             to exactly twice one, which is the whole of SPEC section 12's additive technique \
             and the thing `neighbouring_cells_merge_into_one_silhouette` measures off a frame",
            self.peak,
            self.knee
        );
    }
}

impl Default for Look {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// How much closer one notch of the wheel brings the world.
///
/// A tenth, which is deliberately small. CLAUDE.md: *"no sudden camera moves, nothing that
/// pulls the eye"* - and a wheel notch that doubled the magnification would be exactly that.
/// Ten notches is a factor of two and a half, so crossing the whole range takes a deliberate
/// scroll rather than a flick.
const ZOOM_PER_NOTCH: f32 = 1.1;

/// The tightest the camera will go: the fewest world units it will show across a frame.
///
/// Sixty-four units is about twenty cells, or eight of `founding.rs`'s two-celled bodies laid
/// end to end. Closer than that and a single organism is a screenful of soft gradient, which is
/// a picture of the falloff rather than of the world.
pub const TIGHTEST: f32 = 64.0;

/// Where a frame is looking.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    origin: [f32; 2],
    span: [f32; 2],
}

impl Camera {
    /// A camera that shows the whole width of a world on a frame of this size.
    ///
    /// The depth shown follows from the frame's shape: the same number of world units per
    /// pixel in both directions, centred on the middle of the water. A frame shaped like the
    /// world shows all of it; a taller one shows dark water above and below rather than
    /// stretching what is there.
    ///
    /// This is [`Lens::at_rest`] with nobody having touched it, and it is written that way
    /// rather than duplicated: `--dump-frame` and a window nobody has dragged yet have to be
    /// looking at the same thing, or a frame dumped to check a shader is a picture of something
    /// the window never shows.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height, or is larger than 65,535 pixels in either
    /// direction. The last is not a real limit on anything - it is what lets the conversion to
    /// a floating-point number be exact rather than a cast that CLAUDE.md's lint table would
    /// have to be argued out of.
    #[must_use]
    pub fn showing_all_of(world: (f32, f32), frame: (u32, u32)) -> Self {
        Lens::at_rest(world, frame).camera()
    }

    /// What the shaders need, for a world of this size drawn on a frame of this size.
    #[must_use]
    pub const fn view(&self, world: (f32, f32), frame: (f32, f32), phase: f32) -> View {
        View {
            world: [world.0, world.1],
            origin: self.origin,
            span: self.span,
            frame: [frame.0, frame.1],
            phase,
            unused: [0.0, 0.0, 0.0],
        }
    }

    /// The world position drawn at the top-left corner of the frame.
    #[must_use]
    pub const fn origin(&self) -> [f32; 2] {
        self.origin
    }

    /// How much world the frame covers, in world units.
    #[must_use]
    pub const fn span(&self) -> [f32; 2] {
        self.span
    }
}

/// A [`Camera`] with a person's hands on it: what `C2` adds.
///
/// Held as a place and a magnification rather than as an origin and a span, because those are
/// the two things the two gestures change. A drag moves the place; the wheel changes the
/// magnification about whatever the pointer is over. [`Lens::camera`] turns the pair back into
/// the record the shader reads.
///
/// # What it will not do
///
/// - **It will not move on its own.** There is no clock here and no method that takes one.
/// - **It will not zoom out past the world's width.** The seam only joins up if the left and
///   right edges of the frame are the same place, and a view wider than the world would need
///   the world drawn three times rather than the two `cells.wgsl` draws. Zoomed out fully is
///   [`Lens::at_rest`], which is where it starts.
/// - **It will not wander off the top or the bottom.** The world does not join up that way, so
///   panning down past the floor would be an endless black nothing with no way back to the
///   world except by feel.
///
/// Sideways it will go as far as anybody likes, and that is the point: SPEC section 8 joins the
/// world up at `x = 0`, so panning past the seam has to arrive back where it started rather
/// than stopping at a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    /// How big the world is, in world units.
    world: [f32; 2],

    /// How big the frame is, in pixels.
    frame: [f32; 2],

    /// How much world one pixel covers. Smaller is closer in.
    scale: f32,

    /// The world position drawn at the top-left corner of the frame. Its `x` is kept inside
    /// the world; its `y` need not be, because a frame deeper than the world shows water above
    /// the surface and below the floor.
    origin: [f32; 2],
}

impl Lens {
    /// The view a window opens on: the whole width of the world, and nobody has touched it.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height, or is larger than 65,535 pixels in either
    /// direction.
    #[must_use]
    pub fn at_rest(world: (f32, f32), frame: (u32, u32)) -> Self {
        let frame = [pixels(frame.0), pixels(frame.1)];

        // One number for both directions. Two would stretch the world to the frame's shape,
        // which makes every organism in it the wrong shape as well.
        let scale = world.0 / frame[0];

        let mut lens = Self {
            world: [world.0, world.1],
            frame,
            scale,
            origin: [0.0, world.1.mul_add(0.5, -frame[1] * scale / 2.0)],
        };
        lens.settle();

        lens
    }

    /// The same view in a window that has been resized.
    ///
    /// ⭐ **What is kept is the piece of world on the screen, not the magnification.** A window
    /// dragged wider shows the same water at a larger size rather than more water at the same
    /// size, which is what "the same view" means to the person doing the dragging: they arranged
    /// the window to look at something and resizing it must not take that something away.
    ///
    /// ⚠️ **This is also a bug that was measured rather than imagined, and the measurement is
    /// worth keeping.** The other rule - keep the magnification, show more or less world - was
    /// written first and looks equally reasonable. What it does on Windows is this: a window
    /// asked for at 1280 by 720 is created, and winit reports **`Resized(2858, 1481)` followed
    /// immediately by `Resized(1280, 720)`**. Under "keep the magnification" the wide one
    /// clamps the scale to a world-width-per-2858-pixels and the narrow one that follows a
    /// frame later keeps it - so every window opened on this machine opened at **917 of the
    /// world's 2048 units across**, showing a quarter of the world and reporting nothing wrong.
    /// It was found by dumping a frame from a window and comparing it against the same tick
    /// dumped headlessly, which is exactly the check CLAUDE.md's `--dump-frame` exists for.
    ///
    /// Under this rule the same sequence keeps the whole width across both events and lands
    /// back where it started, which is `a_window_of_any_shape_shows_the_world_unstretched`'s
    /// last assertion.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height.
    pub fn resize(&mut self, frame: (u32, u32)) {
        let showing = self.span();
        let middle = self.origin[1] + showing[1] / 2.0;

        self.frame = [pixels(frame.0), pixels(frame.1)];
        self.scale = showing[0] / self.frame[0];

        // The depth is not kept - the frame's new shape decides how much of it there is room
        // for - so what is held fixed instead is the *middle* of what was being looked at.
        self.origin[1] = middle - self.span()[1] / 2.0;
        self.settle();
    }

    /// Drag the world by this many pixels.
    ///
    /// The world follows the pointer: a pointer dragged to the right takes what is under it to
    /// the right, which means the camera moves *left*. That is the mapping every map in the
    /// world uses and the opposite of the one a scrollbar uses, and getting it the wrong way
    /// round is immediately obvious to anybody holding a mouse and invisible in the arithmetic.
    pub fn pan(&mut self, across: f32, down: f32) {
        self.origin = [
            self.origin[0] - across * self.scale,
            self.origin[1] - down * self.scale,
        ];
        self.settle();
    }

    /// Turn the wheel by this many notches, with the pointer here.
    ///
    /// ⭐ **Anchored on the pointer.** Whatever world the pointer was over stays under the
    /// pointer, so zooming in on an organism keeps that organism where it was rather than
    /// sliding it off the frame - a camera that zoomed about the middle of the window would
    /// throw away the thing being looked at, which is both useless and a sudden camera move
    /// nobody asked for.
    pub fn zoom(&mut self, notches: f32, at: (f32, f32)) {
        let anchor = self.world_at(at);

        self.scale = self.held(self.scale / ZOOM_PER_NOTCH.powf(notches));
        self.origin = [anchor[0] - at.0 * self.scale, anchor[1] - at.1 * self.scale];
        self.settle();
    }

    /// The world position under a pixel of the frame.
    #[must_use]
    pub fn world_at(&self, pixel: (f32, f32)) -> [f32; 2] {
        [
            self.scale.mul_add(pixel.0, self.origin[0]),
            self.scale.mul_add(pixel.1, self.origin[1]),
        ]
    }

    /// What the shader is drawn through.
    #[must_use]
    pub const fn camera(&self) -> Camera {
        Camera {
            origin: self.origin,
            span: self.span(),
        }
    }

    /// How much world the frame covers, in world units.
    #[must_use]
    pub const fn span(&self) -> [f32; 2] {
        [self.frame[0] * self.scale, self.frame[1] * self.scale]
    }

    /// How much world one pixel covers.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }

    /// A magnification, brought inside what the renderer can draw.
    fn held(&self, scale: f32) -> f32 {
        // The widest is the whole width of the world - see the struct's documentation for why
        // there is nothing beyond it. `min` rather than a bare pair of bounds because a frame
        // narrower than [`TIGHTEST`] pixels would otherwise ask for a floor above its ceiling.
        let widest = self.world[0] / self.frame[0];
        let tightest = (TIGHTEST / self.frame[0]).min(widest);

        scale.clamp(tightest, widest)
    }

    /// Bring the view back inside the world after something has moved it.
    fn settle(&mut self) {
        self.scale = self.held(self.scale);

        let span = self.span();
        self.origin = [
            // ⭐ Sideways the world joins up, so this is a wrap and not a clamp. See
            // `panning_past_the_seam_comes_back_round`.
            wrapped(self.origin[0], self.world[0]),
            if span[1] >= self.world[1] {
                // A frame deeper than the world: the water is centred and there is nothing to
                // pan to, so the depth simply is what it is.
                self.world[1].mul_add(0.5, -span[1] / 2.0)
            } else {
                self.origin[1].clamp(0.0, self.world[1] - span[1])
            },
        ];
    }
}

/// A position brought back inside a world that joins up at `width`.
///
/// ⚠️ **The second line is not defensive and it is not dead.** `f32::rem_euclid` of a value a
/// hair below nought returns the modulus itself once the subtraction rounds - and Group A
/// measured exactly such a value, `x = -2.8e-16`, coming out of `organism::body_centre` without
/// its own wrap. A camera origin of exactly the world's width is a position outside the world,
/// which is the thing `docs/PHASE4.md` and `docs/PHASE5.md` both record as impossible.
fn wrapped(position: f32, width: f32) -> f32 {
    let inside = position.rem_euclid(width);

    if inside >= width { 0.0 } else { inside }
}

/// A frame dimension, as a number the arithmetic can use.
fn pixels(count: u32) -> f32 {
    assert!(count > 0, "a frame cannot be nought pixels across");

    let count = u16::try_from(count).expect("a frame is not 65,536 pixels across");

    f32::from(count)
}

#[cfg(test)]
mod tests {
    use super::{Camera, GLOW, Lens, Look, PEAK, TIGHTEST, View};

    /// SPEC section 3's world, which every test below is set in.
    const WORLD: (f32, f32) = (2048.0, 1152.0);

    /// A frame half the world's width, so that one pixel is exactly two world units and the
    /// arithmetic below is exact rather than nearly right.
    const FRAME: (u32, u32) = (1024, 576);

    /// How far apart two world positions may be and still be the same place.
    ///
    /// A thousandth of a world unit, which is a three-hundredth of the smallest cell in SPEC
    /// section 6 and far below anything a frame could show. Everything measured here is a
    /// distance in world units, so one tolerance serves for all of it.
    const CLOSE: f32 = 0.001;

    /// The two positions are the same place, to within [`CLOSE`].
    #[track_caller]
    fn near(measured: f32, expected: f32, what: &str) {
        assert!(
            (measured - expected).abs() < CLOSE,
            "{what}: measured {measured} against {expected}"
        );
    }

    /// ⭐ **C2, first half.** The camera pans and zooms, and both do what a hand expects.
    ///
    /// **And the last claim is the one CLAUDE.md actually asks for**: a lens nobody touches
    /// hands out the same view a thousand times. SPEC section 12 - *"user-driven only, it must
    /// never move on its own"* - is a claim about what happens when *nothing* happens, and the
    /// only way to state it is to let a great many frames go by and check that none of them
    /// moved. A `Lens` has no clock in it and no method that takes one, so this passes by
    /// construction; it is here because the day somebody adds an eased zoom it stops passing.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the world is 2048 wide and the frame 1024 across, so a pixel is exactly two \
                  world units and every product below is exact in binary; a tolerance here \
                  would hide a camera that had moved by a whole pixel"
    )]
    fn the_camera_pans_and_zooms_and_only_when_asked() {
        // Where a window opens: the whole width, which is what `--dump-frame` draws too.
        let rest = Lens::at_rest(WORLD, FRAME);
        assert_eq!(rest.camera(), Camera::showing_all_of(WORLD, FRAME));
        assert_eq!(rest.span(), [2048.0, 1152.0]);
        assert_eq!(rest.scale(), 2.0);

        // Nothing moves on its own. A thousand frames of nobody touching anything.
        let still = rest;
        for frame in 0..1_000 {
            assert_eq!(
                still.camera(),
                rest.camera(),
                "the camera moved by itself between frame 0 and frame {frame}"
            );
        }

        // The wheel. In shows less world, out shows more, and out stops at the whole width.
        let mut lens = rest;
        lens.zoom(4.0, (512.0, 288.0));
        assert!(
            lens.span()[0] < 2048.0 * 0.7,
            "four notches of the wheel changed the view from 2048 units across to {}",
            lens.span()[0]
        );
        assert_eq!(
            lens.span()[0] / lens.span()[1],
            2048.0 / 1152.0,
            "zooming changed the shape of what is shown, so the world is being stretched"
        );

        lens.zoom(-4.0, (512.0, 288.0));
        near(
            lens.span()[0],
            2048.0,
            "four notches back out did not return the view it came from",
        );

        // The drag. The world follows the pointer, so a pointer dragged right takes the world
        // right with it and the camera moves left - not the other way round.
        let mut lens = Lens::at_rest(WORLD, FRAME);
        lens.zoom(6.0, (512.0, 288.0));
        let before = lens.world_at((512.0, 288.0));
        lens.pan(100.0, 0.0);
        let after = lens.world_at((612.0, 288.0));
        near(
            after[0],
            before[0],
            "the world did not follow the pointer: what was under the middle of the frame is \
             not under the pointer after dragging it a hundred pixels right",
        );
    }

    /// ⭐⭐ **C2, and the property Group B's seam exists for.** Panning sideways never meets a
    /// wall: the world joins up, so a camera dragged far enough arrives back where it started.
    ///
    /// SPEC section 8 joins the world at `x = 0` and `cells.wgsl` draws every cell a second
    /// time a world's width away so that a body standing on the join is whole. None of that is
    /// worth anything if the *camera* stops at the edge - a person dragging east would hit a
    /// wall at a place where the world visibly carries on.
    ///
    /// Three claims. The origin is always inside the world, so the second copy `cells.wgsl`
    /// draws is always the one on the near side. A drag of exactly one world's width is a drag
    /// of nothing at all. And the view is never wider than the world, because a wider one would
    /// need a third copy that nothing draws.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a whole world's width of panning at two units to the pixel is exact in \
                  binary, and this test's whole subject is whether it lands exactly back where \
                  it started"
    )]
    fn panning_past_the_seam_comes_back_round() {
        let mut lens = Lens::at_rest(WORLD, FRAME);
        lens.zoom(8.0, (512.0, 288.0));

        let start = lens.camera().origin()[0];
        let a_world = WORLD.0 / lens.scale();

        // Left first, which is where a naive camera goes negative.
        lens.pan(-1.0, 0.0);
        let stepped = lens.camera().origin()[0];
        assert!(
            (0.0..WORLD.0).contains(&stepped),
            "one pixel of panning left put the camera at x = {stepped}, which is not a place in \
             a world 2048 units wide"
        );
        lens.pan(1.0, 0.0);

        // And then all the way round, in one drag and in a hundred.
        lens.pan(a_world, 0.0);
        assert_eq!(
            lens.camera().origin()[0],
            start,
            "dragging the camera the whole width of the world did not bring it back to where \
             it started, so the seam is a wall"
        );

        for _ in 0..100 {
            lens.pan(-a_world / 100.0, 0.0);
        }
        // A twentieth of a unit rather than [`CLOSE`]: a hundred separate additions of a
        // hundredth of a world accumulate about a thousandth of a unit of rounding, which is a
        // fortieth of a pixel at this magnification and not a camera that failed to come back.
        let round_trip = lens.camera().origin()[0];
        assert!(
            (round_trip - start).abs() < 0.05,
            "a hundred small drags the other way round the world arrived at {round_trip} \
             rather than back at {start}"
        );

        // Every one of a full turn's worth of positions is a position in the world, and the
        // view is never wider than the world at any of them.
        for step in 0..360 {
            lens.pan(a_world / 360.0, 0.0);

            let origin = lens.camera().origin()[0];
            assert!(
                (0.0..WORLD.0).contains(&origin),
                "step {step} of a turn round the world put the camera at x = {origin}"
            );
            assert!(
                lens.span()[0] <= WORLD.0,
                "the view is {} units across in a world {} wide, so `cells.wgsl`'s one extra \
                 copy of each cell cannot cover it",
                lens.span()[0],
                WORLD.0
            );
        }
    }

    /// ⭐⭐ **C2, and the thing that makes zooming usable rather than infuriating.** The world
    /// under the pointer stays under the pointer.
    ///
    /// A camera that zoomed about the middle of the window would throw away whatever was being
    /// looked at the moment somebody leant in on it, and it would do so as a *jump* - which
    /// CLAUDE.md's "no sudden camera moves" forbids on its own terms, quite apart from being
    /// useless.
    ///
    /// The anchor is deliberately **not** the middle of the frame: a camera zooming about its
    /// own centre keeps the centre fixed too, so an anchor tested there would pass on exactly
    /// the implementation this test exists to reject. The second half is the other side of
    /// that - a pixel somewhere else has to have moved, or "nothing changed" would pass.
    #[test]
    fn zooming_is_anchored_on_the_pointer() {
        let pointer = (768.0, 144.0);

        let mut lens = Lens::at_rest(WORLD, FRAME);
        lens.zoom(6.0, (512.0, 288.0));

        let under = lens.world_at(pointer);
        let elsewhere = lens.world_at((64.0, 500.0));

        lens.zoom(3.0, pointer);

        let still_under = lens.world_at(pointer);
        near(
            still_under[0],
            under[0],
            "zooming in moved the world sideways under the pointer",
        );
        near(
            still_under[1],
            under[1],
            "zooming in moved the world up or down under the pointer",
        );

        assert!(
            (lens.world_at((64.0, 500.0))[0] - elsewhere[0]).abs() > 1.0,
            "nothing at all moved, so the anchor above proves nothing"
        );

        // And out again, anchored on somewhere else entirely.
        let corner = (0.0, 0.0);
        let under_corner = lens.world_at(corner);
        lens.zoom(-2.0, corner);
        near(
            lens.world_at(corner)[0],
            under_corner[0],
            "zooming back out moved the world sideways under the pointer",
        );
    }

    /// The camera stays where there is a world to see, in every direction the world has an end.
    ///
    /// Sideways there is no end - `panning_past_the_seam_comes_back_round` is the other half of
    /// this - but the water has a surface and a floor, and a camera that could be dragged below
    /// the floor would be a black frame with no landmark to find the way back by. The
    /// magnification has ends for two quite different reasons: out, because a view wider than
    /// the world needs a copy of each cell that nothing draws; in, because a cell filling the
    /// frame is a picture of the falloff.
    #[test]
    fn the_camera_stays_where_there_is_a_world_to_see() {
        let mut lens = Lens::at_rest(WORLD, FRAME);
        lens.zoom(10.0, (512.0, 288.0));

        // Straight up, hard, for a long time.
        for _ in 0..100 {
            lens.pan(0.0, 400.0);
        }
        near(
            lens.camera().origin()[1],
            0.0,
            "the camera was dragged up past the surface of the water",
        );

        // And straight down.
        for _ in 0..100 {
            lens.pan(0.0, -400.0);
        }
        near(
            lens.camera().origin()[1] + lens.span()[1],
            WORLD.1,
            "the camera was dragged down past the floor",
        );

        // Out, past the whole world.
        for _ in 0..100 {
            lens.zoom(-1.0, (512.0, 288.0));
        }
        near(
            lens.span()[0],
            WORLD.0,
            "the camera zoomed out past the width of the world, which `cells.wgsl` cannot draw",
        );
        near(
            lens.camera().origin()[0] + lens.span()[0] - lens.camera().origin()[0],
            WORLD.0,
            "the view is not the whole width after zooming all the way out",
        );

        // In, past what is worth looking at.
        for _ in 0..200 {
            lens.zoom(1.0, (512.0, 288.0));
        }
        near(
            lens.span()[0],
            TIGHTEST,
            "the camera zoomed in past the tightest view it allows",
        );
    }

    /// A window of any shape shows the world unstretched, and a very tall one shows water that
    /// is not there rather than a world pulled about.
    ///
    /// CLAUDE.md: *"Resizable window that behaves itself at any aspect ratio."* The two odd
    /// shapes below are the ones a person actually makes by dragging a window edge - a wide
    /// letterbox and a tall column - and the claim in both is the same: one world unit is the
    /// same number of pixels across as it is down.
    #[test]
    fn a_window_of_any_shape_shows_the_world_unstretched() {
        for frame in [(1920, 1080), (3000, 400), (400, 3000), (1, 1), (17, 913)] {
            let lens = Lens::at_rest(WORLD, frame);
            let span = lens.span();
            let across = span[0] / super::pixels(frame.0);
            let down = span[1] / super::pixels(frame.1);

            // Within a millionth of itself rather than exactly: both numbers are a scale
            // multiplied by a frame dimension and divided by it again, and the two roundings
            // need not land on the same bit. A stretched world would be out by a factor, not
            // by an ulp.
            assert!(
                (across - down).abs() < down * 1e-6,
                "a {} by {} window shows {across} world units to the pixel across and {down} \
                 down, so the world is stretched",
                frame.0,
                frame.1
            );
        }

        // A window resized keeps the piece of world that is on the screen. The person dragging
        // the edge arranged the view to look at something; making the window bigger must not
        // take that something away.
        let mut lens = Lens::at_rest(WORLD, (1024, 576));
        lens.zoom(5.0, (512.0, 288.0));
        let showing = lens.span()[0];
        let middle = lens.camera().origin()[0] + showing / 2.0;

        lens.resize((1536, 864));
        near(
            lens.span()[0],
            showing,
            "resizing the window changed how much world is on the screen",
        );
        near(
            lens.camera().origin()[0] + lens.span()[0] / 2.0,
            middle,
            "resizing the window moved what is in the middle of it",
        );

        // ⚠️ And the sequence a real window actually produces. Measured on Windows 11: a window
        // asked for at 1280 by 720 is created and winit immediately reports a resize to 2858 by
        // 1481 and then one back to 1280 by 720, a frame apart and before anybody has touched
        // anything. A camera that kept its *magnification* across those two would come to rest
        // showing 917 units of a 2048-unit world - which is what the first frame ever dumped
        // from this window actually showed, and what comparing it against the same tick dumped
        // headlessly is what caught.
        let mut opened = Lens::at_rest(WORLD, (1280, 720));
        let rest = opened.camera();

        opened.resize((2858, 1481));
        opened.resize((1280, 720));

        for (part, was, is) in [
            ("left edge", rest.origin()[0], opened.camera().origin()[0]),
            ("top edge", rest.origin()[1], opened.camera().origin()[1]),
            ("width", rest.span()[0], opened.span()[0]),
            ("depth", rest.span()[1], opened.span()[1]),
        ] {
            near(
                is,
                was,
                &format!(
                    "a window resized twice on the way to the size it asked for came back with \
                     a different {part}"
                ),
            );
        }
        near(
            opened.span()[0],
            WORLD.0,
            "a window nobody has touched is not showing the whole width of the world",
        );
    }

    /// A camera shows the whole width, does not stretch what it shows, and hands the shader a
    /// record the shader's own struct can read.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the world and the frame are both powers-of-two-friendly whole numbers here, \
                  so every division below is exact and a tolerance would only hide a mistake"
    )]
    fn the_camera_shows_the_whole_width_without_stretching_it() {
        // SPEC section 3's world is exactly sixteen by nine, and so is this frame, so the
        // whole world fits with nothing left over.
        let camera = Camera::showing_all_of((2048.0, 1152.0), (1920, 1080));
        assert_eq!(camera.origin(), [0.0, 0.0]);
        assert_eq!(camera.span(), [2048.0, 1152.0]);

        // A frame twice as tall for its width shows twice the depth, centred - so there is
        // dark water above the surface and below the floor rather than a world stretched to
        // fit.
        let tall = Camera::showing_all_of((2048.0, 1152.0), (1920, 2160));
        assert_eq!(tall.span(), [2048.0, 2304.0]);
        assert_eq!(
            tall.origin(),
            [0.0, -576.0],
            "the extra depth is not shared equally above and below the world"
        );

        assert_eq!(
            camera.view((2048.0, 1152.0), (1920.0, 1080.0), 0.25),
            View {
                world: [2048.0, 1152.0],
                origin: [0.0, 0.0],
                span: [2048.0, 1152.0],
                frame: [1920.0, 1080.0],
                phase: 0.25,
                unused: [0.0, 0.0, 0.0],
            }
        );

        // `cells.wgsl`, `water.wgsl` and `snow.wgsl` all declare this struct, and a uniform
        // buffer has to be a whole number of sixteen-byte blocks.
        assert_eq!(size_of::<View>(), 48);
    }

    /// ⭐ **`B5`.** The look is one sixty-four-byte record, and it is the one the shaders declare.
    ///
    /// A uniform's layout is written out twice - once in Rust and once in WGSL - and nothing
    /// checks that the two agree. `wgpu` will happily bind a buffer of the right *size* whose
    /// fields land in different places, and the result is a picture drawn with the bloom
    /// strength in the tone-map knee: wrong, plausible, and impossible to find from either file
    /// alone.
    ///
    /// So the size is pinned, and it is pinned at the number WGSL's own alignment rules give.
    /// `vec3<f32>` has an alignment of sixteen and a size of twelve, so each of the three
    /// colours takes a block with its neighbouring scalar tucked into the last four bytes of it.
    /// Four scalars, then three of those blocks: 16 + 48. A field inserted anywhere but at the
    /// end of one of those groups changes this number.
    #[test]
    fn the_look_is_what_the_shaders_declare() {
        assert_eq!(size_of::<Look>(), 64);

        // ⚠️ And it has no padding in it, which `bytemuck::Pod` already refuses to derive for -
        // this says it out loud, because the reason is worth keeping: padding is bytes nobody
        // wrote being sent to the card.
        assert_eq!(
            size_of::<Look>(),
            4 * 4 + 3 * (3 * 4 + 4),
            "the look has padding in it somewhere"
        );

        // The defaults are Group D's picture, and `B5`'s whole regression guard is that a frame
        // drawn through them is the frame the constants drew.
        let look = Look::DEFAULT;
        look.sane();
        assert!(
            (look.glow - GLOW).abs() < f32::EPSILON && (look.peak - PEAK).abs() < f32::EPSILON,
            "the default look is not the one `camera.rs`'s constants describe"
        );
        assert!(
            (look.knee - crate::frame::TONE_KNEE).abs() < f32::EPSILON,
            "the default knee is not `frame.rs`'s TONE_KNEE, so the tests that measure the tone \
             map off a frame are measuring a different curve from the one being drawn"
        );
    }

    /// ⭐⭐ **The guard that had to survive `B5`**, now that both of its numbers can be dragged.
    ///
    /// Group D's `const` block still holds over the defaults - see
    /// `the_glow_reaches_past_the_cell_and_leaves_room_to_be_added_to` below - and this is the
    /// half a `const` block cannot do. A peak past half the knee is a peak at which two
    /// overlapping cells no longer come out of the composite at exactly twice one, which is the
    /// measurement SPEC section 12's additive technique is stated as.
    #[test]
    #[should_panic(expected = "leaves no room for two cells")]
    fn a_peak_past_half_the_knee_stops_the_frame_rather_than_quietly_compressing_it() {
        Look {
            peak: crate::frame::TONE_KNEE * 0.5 + 0.01,
            ..Look::DEFAULT
        }
        .sane();
    }

    /// ⭐ The glow reaches beyond the cell, and one cell alone does not fill the range.
    ///
    /// Both halves are what make `neighbouring_cells_merge_into_one_silhouette` possible at
    /// all, and both are easy to undo by tuning. A glow of one would stop at the cell's own
    /// edge and `founding.rs`'s two cells, eight units apart, would never touch. A peak of one
    /// would clip every overlap to white, so the sum that produces the silhouette would exist
    /// and be invisible.
    #[test]
    fn the_glow_reaches_past_the_cell_and_leaves_room_to_be_added_to() {
        use coacervate_sim::cell::CellKind;

        let founder_spring = 8.0_f32;
        let reach = CellKind::Photocyte.radius() * GLOW;
        assert!(
            reach * 2.0 > founder_spring,
            "a photocyte's light reaches {reach} units and `founding.rs` springs two of them \
             {founder_spring} apart, so the plainest body in the world is drawn as two beads"
        );

        // In a const block, so that a peak edited past the tone map's knee stops the build
        // rather than one test. `water.wgsl` is the identity below the knee, which is what makes
        // "two cells are twice one" true of the finished picture and not only of a buffer; two
        // cells at a peak above half the knee sum past it and the claim becomes approximate.
        const {
            assert!(
                PEAK > 0.0 && PEAK * 2.0 <= crate::frame::TONE_KNEE,
                "a peak of nought, or of more than half the tone map's knee, leaves no room for \
                 two cells to add up to exactly twice one - which is the whole of SPEC section \
                 12's additive technique and the thing B4 measures off the frame"
            );
        }
    }
}
