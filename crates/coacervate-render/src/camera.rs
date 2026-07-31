//! Where the world is on the frame.
//!
//! Group B's camera does exactly one thing: it shows the whole width of the world. It cannot
//! pan and it cannot zoom, and that is not a gap - it is Group C's `C2`, which SPEC section 12
//! and CLAUDE.md's second-screen constraint both say must be *user-driven only*. A camera with
//! no controls cannot move on its own, which is the property that matters and the only one
//! Group B needs.
//!
//! # It fits to the width, and the reason is the seam
//!
//! SPEC section 8 joins the world up sideways. A view that showed part of the width would have
//! two edges that are not edges of anything - the world carries on past both of them - and a
//! body swimming off the right of the frame would simply vanish rather than appearing on the
//! left. Showing the whole width makes the frame's left and right edges *the same place*, which
//! is what the wrap in `cells.wgsl` then draws across.
//!
//! The depth is scaled by the same factor and centred, so nothing is stretched. At SPEC
//! section 3's shipped world - 2048 by 1152, which is exactly sixteen by nine - a frame of any
//! sixteen-by-nine size shows all of it and no water that is not there.

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
/// Group D owns brightness properly - `D1`'s HDR target and tone mapping are what make this a
/// choice rather than a ceiling. Until then this is a plain 8-bit target and half is what
/// leaves room for the additions to be visible.
pub const PEAK: f32 = 0.5;

/// What the shader is told: where the world is, and how to draw light in it.
///
/// `repr(C)` and `Pod` for the same reason [`crate::scene::Instance`] is - this becomes the
/// bytes of a uniform buffer, and the layout is `cells.wgsl`'s `View` struct field for field.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct View {
    /// How big the world is, in world units. Its width is where the seam is.
    pub world: [f32; 2],

    /// The world position drawn at the top-left corner of the frame.
    pub origin: [f32; 2],

    /// How much world the frame covers, in world units.
    pub span: [f32; 2],

    /// [`GLOW`], handed on.
    pub glow: f32,

    /// [`PEAK`], handed on.
    pub peak: f32,
}

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
    /// # Panics
    ///
    /// If the frame has no width or no height, or is larger than 65,535 pixels in either
    /// direction. The last is not a real limit on anything - it is what lets the conversion to
    /// a floating-point number be exact rather than a cast that CLAUDE.md's lint table would
    /// have to be argued out of.
    #[must_use]
    pub fn showing_all_of(world: (f32, f32), frame: (u32, u32)) -> Self {
        let across = pixels(frame.0);
        let down = pixels(frame.1);
        let (width, height) = world;

        // One number, used for both directions. Two would stretch the world to the frame's
        // shape, which makes every organism in it the wrong shape as well.
        let units_per_pixel = width / across;
        let depth_shown = down * units_per_pixel;

        Self {
            origin: [0.0, height.mul_add(0.5, -depth_shown / 2.0)],
            span: [width, depth_shown],
        }
    }

    /// What the shader needs, for a world of this size.
    #[must_use]
    pub const fn view(&self, world: (f32, f32)) -> View {
        View {
            world: [world.0, world.1],
            origin: self.origin,
            span: self.span,
            glow: GLOW,
            peak: PEAK,
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

/// A frame dimension, as a number the arithmetic can use.
fn pixels(count: u32) -> f32 {
    assert!(count > 0, "a frame cannot be nought pixels across");

    let count = u16::try_from(count).expect("a frame is not 65,536 pixels across");

    f32::from(count)
}

#[cfg(test)]
mod tests {
    use super::{Camera, GLOW, PEAK, View};

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
            camera.view((2048.0, 1152.0)),
            View {
                world: [2048.0, 1152.0],
                origin: [0.0, 0.0],
                span: [2048.0, 1152.0],
                glow: GLOW,
                peak: PEAK,
            }
        );

        // `cells.wgsl` declares this struct too, and a uniform buffer has to be a whole
        // number of sixteen-byte blocks.
        assert_eq!(size_of::<View>(), 32);
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

        // In a const block, so that a peak edited past a half stops the build rather than one
        // test. Everything the renderer draws is added to something, and there is nowhere
        // above one for the additions to go.
        const {
            assert!(
                PEAK > 0.0 && PEAK <= 0.5,
                "a peak of nought or of more than a half leaves no room for two cells to add \
                 up to something brighter, which is the whole of SPEC section 12's technique"
            );
        }
    }
}
