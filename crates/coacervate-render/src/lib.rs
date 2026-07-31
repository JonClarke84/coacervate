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
//! The bloom, the HDR target, the motion trails, the depth gradient, the light shafts and the
//! marine snow are all Group D and none of them are here. That is not an omission: tuning a
//! falloff against a background that is about to be replaced would mean tuning it twice. The
//! panels, the sliders and the charts are Phase 6's, and this crate has no `egui` in it yet.
//!
//! # The pieces
//!
//! | Module | What it is |
//! | --- | --- |
//! | [`gpu`] | A device, a surface, and the two different ways of not having one |
//! | [`scene`] | The five things SPEC section 12 says a cell carries |
//! | [`camera`] | Where the world is on the frame, seam included, and the hands on it |
//! | [`frame`] | The pipeline, the offscreen target, the copy back, and the PNG |
//! | [`controls`] | What a person did, and what the camera does about it |
//! | [`window`] | The window, the event loop, `F12`, and a clean way to stop |
//!
//! `cells.wgsl` is the shader, and it is where SPEC section 12's *"most of the difference
//! between 'creature' and 'physics demo'"* actually lives.

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

/// Draw one frame of this world and write it out.
///
/// This is what `--dump-frame` is. It opens a device, builds a pipeline, draws once, copies the
/// result back and writes a PNG - and then everything it made is dropped. That is deliberately
/// wasteful and deliberately simple: a one-frame dump does the expensive setup exactly once and
/// there is nothing for it to be reused by.
///
/// # Errors
///
/// If there is no graphics adapter, or the file cannot be written. See [`DumpError`].
pub fn dump_frame(world: &World, path: &Path) -> Result<(), DumpError> {
    let gpu = gpu::Gpu::open().map_err(DumpError::NoGpu)?;
    let renderer = frame::Renderer::new(&gpu, DUMP_WIDTH, DUMP_HEIGHT);
    let drawn = renderer.render(&gpu, &scene::Scene::of(world));

    drawn.write_png(path).map_err(DumpError::Unwritable)
}
