//! Drawing the world. Nothing in here is allowed to change it.
//!
//! CLAUDE.md's architecture puts `wgpu` on this side of the wall and keeps `coacervate-sim`
//! ignorant of it, and the enforcement is in the manifests rather than in anybody's care:
//! `coacervate-sim` names exactly `rand` and `serde`, so a `use wgpu::…` written there does not
//! compile. Everything here reads the accessors Group A added - the living cells, whose slot
//! each belongs to, the world's size - and nothing here is called from a tick.
//!
//! # What is here, and what deliberately is not
//!
//! Group B was *one frame, on disk*: cells, merged into bodies, on plain dark water, seen whole.
//! It is the phase's done-criterion because it is what makes every later visual decision
//! checkable - CLAUDE.md: *"A UI change is not complete until a frame has been dumped and
//! looked at."* Group C adds the window over the top of it, drawing through exactly the same
//! pipeline: a window and a frame dump differ in where the pixels go and in nothing else.
//!
//! Group D is the rest of SPEC section 12: a floating-point target, a separable-Gaussian bloom, a
//! tone-mapped composite, an accumulation buffer, the depth gradient, the light shafts and the
//! marine snow. The panels, the sliders and the charts are Phase 6's, and this crate has no
//! `egui` in it yet.
//!
//! # The pieces
//!
//! | Module | What it is |
//! | --- | --- |
//! | [`gpu`] | A device, a surface, and the two different ways of not having one |
//! | [`scene`] | The five things SPEC section 12 says a cell carries, and the drift |
//! | [`camera`] | Where the world is on the frame, seam included, and the hands on it |
//! | [`frame`] | The five passes, the offscreen targets, the copy back, and the PNG |
//! | [`controls`] | What a person did, and what the camera does about it |
//! | [`window`] | The window, the event loop, `F12`, and a clean way to stop |
//!
//! | Shader | What it draws |
//! | --- | --- |
//! | `cells.wgsl` | The cells, additively, into a floating-point buffer. SPEC section 12's *"most of the difference between 'creature' and 'physics demo'"* |
//! | `post.wgsl` | The motion trail, and the two halves of the separable Gaussian |
//! | `water.wgsl` | The depth gradient, the light shafts, the bloom composite and the tone map |
//! | `snow.wgsl` | The marine snow, which is the actual detritus |
//!
//! # ⚠️ One thing about colour that is *not* in this crate
//!
//! An organism's hue is its lineage's, and a lineage's is **inherited rather than computed** -
//! SPEC section 11. The number lives on the organism, in `coacervate-sim`, where it is a marker a
//! lineage carries and passes on with a small error rather than a colour; [`scene`] is the one
//! place in the project that reads it as a place on the colour wheel. Group B computed it here,
//! from the genome fingerprint, and the frame that produced -
//! `docs/frames/phase5-groupb.png` - is why it does not any more.

#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    reason = "Group C's event loop has to decide how much of a frame's time the simulation may \
              have, and that is a wall-clock question - see window.rs. Written as a crate-root \
              allow and NOT as a package-level [lints] table, because a package table REPLACES \
              the workspace one and would silently take the five cast lints with it. clippy.toml \
              carries the same warning, and main.rs makes the same allowance for the same reason."
)]

pub mod camera;
pub mod controls;
pub mod frame;
pub mod gpu;
pub mod scene;
pub mod window;

use coacervate_sim::world::World;
use std::path::Path;

/// How wide a dumped frame is, in pixels.
///
/// SPEC section 3's world is 2048 by 1152 - exactly sixteen by nine - so a sixteen-by-nine
/// frame shows all of it and no water that is not there. At this size a world unit is a little
/// under a pixel, which is about as small as an organism can be drawn and still have a shape.
pub const DUMP_WIDTH: u32 = 1920;

/// How tall a dumped frame is, in pixels.
pub const DUMP_HEIGHT: u32 = 1080;

/// Why a frame could not be dumped.
#[derive(Debug)]
pub enum DumpError {
    /// There is nothing on this machine to draw with, or what there is refused.
    NoGpu(gpu::Unavailable),

    /// The file could not be written.
    Unwritable(png::EncodingError),
}

impl std::fmt::Display for DumpError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpu(why) => write!(out, "no frame could be drawn: {why}"),
            Self::Unwritable(why) => write!(out, "the frame could not be written: {why}"),
        }
    }
}

/// How many ticks pass between two of the moments a dumped frame's motion trail is built from.
///
/// Group C measured a watched run at about 650 ticks a second at 60 frames a second, which is
/// eleven ticks a frame. A dump that sampled the world more finely than a window does would show
/// a shorter, denser tail than the window shows; more coarsely, a dotted one.
pub const TRAIL_STRIDE: u64 = 11;

/// How many of those moments go into a dumped frame's trail.
///
/// `frame.rs`'s fade leaves three per cent of a frame after a hundred of them, which is below what
/// an 8-bit picture can show, so a hundred is the whole of the tail and any more would be
/// arithmetic nobody can see. See [`Dump::watching_from`].
///
/// ⚠️ **This is also what keeps a dumped frame honest about how many organisms are alive.** A
/// trail holds every body that stood anywhere over its own length, including the ones that have
/// since died - and this world turns over about a third of its population every 1,600 ticks, so a
/// trail twice this long visibly *thickens* every colony with the recently dead. Measured by
/// dumping the same tick with the trail switched off and comparing: at this length the trail
/// changes 18% of the frame's pixels and 5.7% of them by more than eight bytes, which is a tail
/// behind each body and not a second population.
pub const TRAIL_FRAMES: u64 = 100;

/// A device, a renderer, and a motion trail being built up.
///
/// ⭐ **This exists because a trail is a record of several moments and `--dump-frame` renders
/// one.** Without it, D2 - the one thing Group D adds that is about *movement* - would be the one
/// thing that could never be checked by the means CLAUDE.md provides for checking visual work:
/// *"Claude reads those PNGs directly to see what it has built."* So a headless dump watches the
/// closing stretch of the run go by, exactly as a window would have watched it, and the picture
/// at the end has the same tails on it a window would have shown.
///
/// It is also why the device is opened *before* the run rather than after it. A machine with no
/// graphics card now says so in the first second rather than after twenty-three seconds of
/// simulation.
#[derive(Debug)]
pub struct Dump {
    gpu: gpu::Gpu,
    renderer: frame::Renderer,
}

impl Dump {
    /// Open a device and build everything a dumped frame needs.
    ///
    /// # Errors
    ///
    /// If there is no graphics adapter. See [`DumpError`].
    pub fn open() -> Result<Self, DumpError> {
        let gpu = gpu::Gpu::open().map_err(DumpError::NoGpu)?;
        let renderer = frame::Renderer::new(&gpu, DUMP_WIDTH, DUMP_HEIGHT);

        Ok(Self { gpu, renderer })
    }

    /// Which tick a run has to start showing this a world on for the trail to be full.
    ///
    /// A run bounded at `until` calls [`Dump::watch`] from here onwards, every [`TRAIL_STRIDE`]
    /// ticks. Saturating, because a run of fewer ticks than a trail is long simply gets a shorter
    /// tail rather than an underflow.
    #[must_use]
    pub const fn watching_from(until: u64) -> u64 {
        until.saturating_sub(TRAIL_STRIDE * TRAIL_FRAMES)
    }

    /// Take one more moment of this world into the trail.
    ///
    /// Nothing is drawn where anybody can see it and nothing is read back; this is the cheap half
    /// of a frame. The camera is the same one [`Dump::write`] will use, which is what stops the
    /// trail being thrown away between one moment and the next.
    pub fn watch(&mut self, world: &World) {
        let scene = scene::Scene::of(world);
        let camera = camera::Camera::showing_all_of((scene.width, scene.height), self.size());

        self.renderer.watch(&self.gpu, &scene, &camera);
    }

    /// Draw this world with whatever trail has been built up behind it, and write it out.
    ///
    /// # Errors
    ///
    /// If the file cannot be written. See [`DumpError`].
    pub fn write(&mut self, world: &World, path: &Path) -> Result<(), DumpError> {
        let scene = scene::Scene::of(world);
        let camera = camera::Camera::showing_all_of((scene.width, scene.height), self.size());
        let drawn = self.renderer.render_through(&self.gpu, &scene, &camera);

        drawn.write_png(path).map_err(DumpError::Unwritable)
    }

    /// How big a dumped frame is.
    const fn size(&self) -> (u32, u32) {
        let _ = self;

        (DUMP_WIDTH, DUMP_HEIGHT)
    }
}

/// Draw one frame of this world, on its own, and write it out.
///
/// One moment with nothing behind it: no motion trail, because there is no motion in a single
/// frame to leave one. [`Dump`] is what `--dump-frame` actually uses, and this is what everything
/// else does - a caller with a world and a path and no run to watch.
///
/// # Errors
///
/// If there is no graphics adapter, or the file cannot be written. See [`DumpError`].
pub fn dump_frame(world: &World, path: &Path) -> Result<(), DumpError> {
    Dump::open()?.write(world, path)
}
