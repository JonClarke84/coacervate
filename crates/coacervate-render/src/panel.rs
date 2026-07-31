//! The chrome: `A1` to `A4`.
//!
//! One panel, read-only, sitting over the world, and the switch that takes it away.
//!
//! # ⭐ The register is a requirement here, not a preference
//!
//! SPEC section 12's last paragraph, in full:
//!
//! > `egui` panels sit over the world: translucent dark, thin borders, monospace numerics,
//! > recessive. **The simulation is the subject; the chrome should nearly disappear.**
//!
//! A default-styled egui window is the opposite of every word of that - it is light, it is
//! chunky, it has a title bar you can drag and a shadow under it, and it is the brightest thing
//! on a frame of nearly-black water. So [`recessive`] builds a style from the ground up and
//! [`Chrome::compose`] is the only place in the project that draws a widget.
//!
//! # ⚠️ Why there is a shader in this directory called `panel.wgsl`
//!
//! Because `egui-wgpu` cannot be used. The version matrix, checked before a line of this was
//! written:
//!
//! | Crate | Newest published | Wants |
//! | --- | --- | --- |
//! | `egui` | 0.35.0 | nothing about a graphics card at all |
//! | `egui-wgpu` | 0.35.0 | `wgpu ^29.0` |
//! | `egui-winit` | 0.35.0 | `winit 0.30.13` - which is what this workspace pins |
//! | `wgpu` | 30.0.0 | - and this renderer is written against it |
//!
//! `^29.0` excludes 30, so cargo would compile both and the types would not unify:
//! `egui_wgpu::Renderer::render` takes a wgpu-29 render pass and every pass in `frame.rs` is a
//! wgpu-30 one. That is a type error, not a warning. The alternative was to move `wgpu` back to
//! 29 underneath a renderer that works, which is a rewrite - `CurrentSurfaceTexture`,
//! `Queue::present`, `SurfaceColorSpace`, `multiview_mask` and `depth_slice` are all wgpu 30 -
//! for the sake of about two hundred lines. So [`Painter`] below is those two hundred lines.
//! **Delete it and take the dependency the day `egui-wgpu` names `wgpu 30`**; the seam is
//! exactly [`Painter`] and nothing else in this crate would move.
//!
//! What egui actually asks a backend for is small: a triangle list in points, a scissor
//! rectangle per batch, and one texture atlas that changes when a glyph is first drawn.
//!
//! # ⚠️ `egui-winit` is deliberately not here yet, and Group B is where it arrives
//!
//! Nothing in Group A is interactive - the panel reads and is never typed into or dragged - so
//! there is no input to translate. And there is a positive reason to wait, which is the same
//! one `frame.rs` gives about `F12` and `--dump-frame` going through one renderer: **a headless
//! dump has no winit window**, so an input path that only existed in the window would mean the
//! panel on a dumped frame and the panel on the screen were composed by two different routes,
//! and the frame would stop being evidence about the window. [`Chrome::compose`] is the one
//! route, and it builds its own [`egui::RawInput`] - which for a panel that answers to nothing
//! is a screen rectangle and a scale.
//!
//! The consequence to be honest about: dragging the pointer across the panel pans the camera
//! underneath it, because nothing is telling the camera that the pointer is over something.
//! Group B's sliders are the first thing that has to answer a pointer, and that is when
//! `egui-winit` earns its place.

use crate::census::{Census, millions_of_years};
use crate::gpu::Gpu;
use coacervate_sim::world::World;
use std::collections::BTreeMap;
use wgpu::util::DeviceExt as _;

/// How wide the panel is, in egui's points.
///
/// Fixed rather than fitted to its contents, because a panel that changed width as the
/// population went from 999 to 1000 would be a thing moving on the screen for a reason that is
/// not about the world - which is CLAUDE.md's *"nothing that pulls the eye"* exactly.
const WIDTH: f32 = 208.0;

/// How far the panel sits from the corner of the frame, in points.
const INSET: f32 = 12.0;

/// How many times [`Chrome::compose`] will ask egui for the same panel before giving up.
///
/// Two, and the second is only ever paid on the first composition of a run. See the comment in
/// [`Chrome::compose`], and `a_panel_appears_on_the_first_frame_it_is_asked_for` for the
/// measurement that put it there.
const SETTLES: usize = 2;

/// The size of every glyph in the chrome.
///
/// SPEC section 12 asks for *monospace numerics* and this crate takes that at its word: **every
/// character in the panel is monospace**, labels included, because a fixed-width label column
/// and a fixed-width value column line up with no layout machinery at all and a column of
/// figures that lines up is a column somebody can read at a glance from across a room.
const GLYPH: f32 = 11.0;

/// How many characters wide the unit column is.
///
/// The longest unit on the panel is `cells`. The column is given exactly this many monospace
/// characters of space whatever is written in it, so every numeral in the panel ends on the same
/// pixel however long its unit is - which is what SPEC section 12's *monospace numerics* is
/// actually for.
///
/// ⚠️ **The column is a reserved width and not a padded string, and the frame is why.** Padding
/// the unit to five characters with `{:<5}` looks right and is not: epaint trims the trailing
/// whitespace off `"Ma   "` and lays it out two characters wide, while `"     "` - which is
/// nothing but trailing whitespace - keeps all five. So every row lined up except `time`, which
/// sat three characters right of the rest. Visible on the dumped frame; invisible in the source.
const UNITS: u8 = 5;

/// The panel's own background.
///
/// SPEC's *translucent dark*. Nearly black, slightly blue, and 85% opaque, so a sixth of
/// whatever is behind it comes through: over deep water that is nothing at all, and over a
/// colony it is a ghost of one. It is drawn over the *finished* picture, after the tone map, so
/// what shows through is the frame as it would otherwise have been.
///
/// ⚠️ **The alpha was two-thirds first and the frame is what changed it.** `docs/frames/` at
/// 170: the panel lands on the shallowest, brightest colony in this world and the magenta bodies
/// behind it are the *dominant* thing inside the panel's own rectangle - a label in the middle
/// of it is unreadable, and a person's eye goes to the chrome because the chrome is where two
/// pictures are fighting. That is the exact opposite of *recessive*, and it was invisible from
/// the colour values alone. At 217 the colony behind is still plainly there and is plainly
/// behind.
const FILL: egui::Color32 = egui::Color32::from_rgba_premultiplied(7, 10, 14, 217);

/// The panel's border.
///
/// SPEC's *thin borders*: one point, and a colour only a little above the fill. Enough that the
/// panel has an edge rather than fading into the water; not enough to be a line anybody looks
/// at.
const EDGE: egui::Color32 = egui::Color32::from_rgb(38, 50, 58);

/// What a row's name is written in.
///
/// Dimmer than its number on purpose. A label is read once and then known; the number beside it
/// is what changes, and it is the only thing on the panel that has any business drawing an eye.
const LABEL: egui::Color32 = egui::Color32::from_rgb(96, 112, 122);

/// What a row's number is written in.
///
/// ⚠️ Not white. A pale grey-blue, which on the water this renderer draws is legible at a glance
/// and is still darker than the core of a colony - so the brightest thing on the frame remains
/// something that is alive. `docs/PHASE5.md` measured four pixels in 2,073,600 within two bytes
/// of white and they are all organisms; the chrome does not join them.
const VALUE: egui::Color32 = egui::Color32::from_rgb(158, 176, 186);

/// The style the whole of the chrome is drawn in.
///
/// Built rather than adjusted. `egui::Style::default()` is a light theme with rounded chunky
/// widgets and a proportional font, and reaching that from here would be a list of overrides
/// long enough that the next person could not tell which of them were load-bearing.
///
/// ⚠️ **`animation_time` is nought, and that is two constraints at once.** CLAUDE.md's *"no
/// flashing, nothing that pulls the eye"* is one: a panel whose contents fade in is a panel that
/// moves. The other is that it is what makes a frame *reproducible* - an animating widget draws
/// differently on the second frame than the first, and
/// `screensaver_mode_hides_every_panel` compares two frames byte for byte.
#[must_use]
pub fn recessive() -> egui::Style {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = FILL;
    visuals.window_fill = FILL;
    visuals.window_stroke = egui::Stroke::new(1.0, EDGE);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, VALUE);
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, EDGE);

    // No shadow anywhere. A drop shadow is a soft dark halo, and this renderer already spends
    // five passes putting soft light halos around the things that are alive; a second kind of
    // halo around the thing that is not would be the loudest object on the frame.
    visuals.window_shadow = egui::Shadow::NONE;
    visuals.popup_shadow = egui::Shadow::NONE;

    let spacing = egui::style::Spacing {
        item_spacing: egui::vec2(6.0, 3.0),
        ..egui::style::Spacing::default()
    };

    egui::Style {
        // Every glyph in the chrome, in one line. SPEC section 12's monospace numerics.
        override_font_id: Some(egui::FontId::monospace(GLYPH)),
        visuals,
        spacing,
        animation_time: 0.0,
        ..egui::Style::default()
    }
}

/// The frame the panel is drawn in.
fn surround() -> egui::Frame {
    egui::Frame {
        inner_margin: egui::Margin::symmetric(10, 8),
        fill: FILL,
        stroke: egui::Stroke::new(1.0, EDGE),
        // Two points. Square corners read as a debug overlay; anything more reads as a widget.
        corner_radius: egui::CornerRadius::same(2),
        ..egui::Frame::default()
    }
}

/// ⭐ **A2.** What the panel says, as the lines it says it in.
///
/// A free function over a world, so that *what the panel reports* is checkable without a
/// graphics card, a window or a tessellator - see `the_panel_reports_what_the_world_is_doing`.
/// Everything after this is layout.
///
/// The first block is the run and its population; the second is SPEC section 5's five accounts,
/// in the order that section lists them. A blank name is the rule between the two.
///
/// ⚠️ **The five accounts are here because a population figure on its own cannot say why.** A
/// world at two thousand bodies and one at two thousand bodies with all its energy in
/// `dissipated` are the same number and different worlds, and the ledger is the only thing that
/// tells them apart. `main.rs`'s progress line has carried the same five columns since Phase 4
/// for the same reason.
#[must_use]
pub fn readings(world: &World) -> Vec<Reading> {
    let census = Census::of(world);
    let ledger = world.ledger();

    vec![
        // CLAUDE.md's deep time. `census::millions_of_years` is the one place this arithmetic
        // happens - the progress line and the window's title bar read it from there too.
        Reading::new("time", format!("{:.1}", millions_of_years(world)), "Ma"),
        Reading::new("alive", format!("{}", census.population), ""),
        Reading::new("body", format!("{:.2}", census.mean_cells), "cells"),
        Reading::new("genome", format!("{:.2}", census.mean_genes), "genes"),
        Reading::RULE,
        Reading::new("field", format!("{:.0}", world.grid().total_energy()), ""),
        Reading::new("biomass", format!("{:.0}", ledger.biomass()), ""),
        Reading::new("detritus", format!("{:.0}", ledger.detritus()), ""),
        Reading::new("dissipated", format!("{:.0}", ledger.dissipated()), ""),
        Reading::new("light", format!("{:.0}", ledger.influx_total()), ""),
    ]
}

/// One line of the panel: what it is, what it is, and what it is measured in.
///
/// ⚠️ **The unit is a third field rather than part of the number, and the frame is why.** With
/// `30.0 Ma`, `1713`, `1.98 cells` and `139886` all right-aligned as single strings, the *digits*
/// do not line up - a column of numerals ends at four different places - and SPEC section 12's
/// **monospace numerics** is asking for exactly the opposite of that. Kept apart, the unit gets a
/// fixed-width column of its own in the label's dim colour and every numeral in the panel ends on
/// the same pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// What the number is, in the left column.
    pub name: &'static str,

    /// The number itself, already formatted.
    pub value: String,

    /// What it is measured in, or nothing.
    pub unit: &'static str,
}

impl Reading {
    /// The rule between the population and the ledger, which is a line rather than a reading.
    pub const RULE: Self = Self {
        name: "",
        value: String::new(),
        unit: "",
    };

    /// One line of the panel.
    fn new(name: &'static str, value: String, unit: &'static str) -> Self {
        Self { name, value, unit }
    }

    /// Whether this is the rule rather than a number.
    #[must_use]
    pub const fn is_rule(&self) -> bool {
        self.name.is_empty()
    }
}

/// The panel, the switch that hides it, and what it takes to put it on a frame.
///
/// ⭐ **`hidden` is checked in exactly one place - the top of [`Chrome::compose`] - and that is
/// the whole design of `A3`.** `docs/PHASE5.md`'s **Q24** deferred screensaver mode from Group C
/// with the argument that chrome-hiding is far easier to keep working if it exists from the
/// first panel rather than being retrofitted after the fifth, and the way that promise is
/// actually kept is that **a panel added later cannot opt out of it**: every widget in this
/// program is built inside one closure, that closure does not run when the switch is on, and
/// nothing downstream has an `if` in it to forget. Group B's sliders and Group C's charts get
/// the mode for nothing.
#[derive(Debug)]
pub struct Chrome {
    context: egui::Context,
    painter: Painter,
    hidden: bool,

    /// The last composition, tessellated and waiting to be painted.
    jobs: Vec<egui::ClippedPrimitive>,

    /// Atlas changes egui made while composing, to be applied before painting.
    deltas: egui::TexturesDelta,

    /// Where the panel ended up, in pixels: left, top, width, height. Nothing at all when the
    /// chrome is hidden, which is what `egui_draws_over_the_world_without_clearing_it` uses to
    /// know which pixels of the frame it is entitled to demand are untouched.
    occupied: Option<[u32; 4]>,

    /// What one of egui's points is worth in pixels.
    scale: f32,
}

impl Chrome {
    /// Build the chrome, styled.
    #[must_use]
    pub fn new(gpu: &Gpu) -> Self {
        let context = egui::Context::default();
        context.all_styles_mut(|style| *style = recessive());

        Self {
            context,
            painter: Painter::new(gpu),
            hidden: false,
            jobs: Vec::new(),
            deltas: egui::TexturesDelta::default(),
            occupied: None,
            scale: 1.0,
        }
    }

    /// Whether screensaver mode is on: no panels, only the world.
    #[must_use]
    pub const fn hidden(&self) -> bool {
        self.hidden
    }

    /// Turn screensaver mode on or off.
    pub const fn hide(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// ⭐ **A3.** The switch, as a person uses it: press the key, the chrome goes.
    pub const fn toggle(&mut self) {
        self.hidden = !self.hidden;
    }

    /// Where the panel is on the frame, in pixels: left, top, width, height.
    ///
    /// Nothing at all when there is no panel - either because the chrome is hidden or because
    /// nothing has been composed yet.
    #[must_use]
    pub const fn occupies(&self) -> Option<[u32; 4]> {
        self.occupied
    }

    /// Lay the panel out over a frame of this size, reading this world.
    ///
    /// `scale` is what one of egui's points is worth in pixels - a window's own scale factor,
    /// and one for a dumped frame, whose pixels are its own.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height.
    pub fn compose(&mut self, world: &World, frame: (u32, u32), scale: f32) {
        // ⭐ **A3, and it is this line.** Everything below builds widgets; nothing below is
        // reached. A panel added in Group B is hidden by this without anybody remembering to
        // hide it, which is the whole of what Q24 asked for.
        if self.hidden {
            self.jobs.clear();
            self.occupied = None;
            return;
        }

        assert!(
            frame.0 > 0 && frame.1 > 0,
            "a frame cannot be {} by {} pixels",
            frame.0,
            frame.1
        );

        self.scale = scale;
        self.context.set_pixels_per_point(scale);

        let rows = readings(world);

        // ⚠️⚠️ **egui's very first pass over a fresh `Context` draws nothing at all**, and this
        // loop is the whole of the answer to it. Measured rather than guessed - see
        // `a_panel_appears_on_the_first_frame_it_is_asked_for`, which is what found it: on pass
        // nought egui creates its font atlas and hands back **zero shapes** and an area
        // rectangle at the wrong place and half the right height, because there are no glyphs
        // yet to lay the text out with. On pass one it hands back the panel, and on every pass
        // after that the same panel.
        //
        // So a composition that ran once and painted would produce a frame with no panel on it,
        // and the *window* would never have shown it - because a window composes sixty times a
        // second and only the first of them would have been empty. It is exactly the fault that
        // a headless dump makes visible and watching cannot.
        //
        // Twice is enough and the second is only ever paid once: the condition is **what came
        // out of the tessellator**, not a frame count, so once the fonts exist this runs one
        // pass like anything else. ⚠️ Not on `output.shapes` - egui hands back shapes on that
        // first pass and they tessellate to nothing at all, which is a difference that cost an
        // afternoon.
        let mut placed = None;
        let mut jobs = Vec::new();

        for _ in 0..SETTLES {
            let (output, at) = self.pass(&rows, frame, scale);

            self.deltas.append(output.textures_delta);
            placed = at;
            jobs = self
                .context
                .tessellate(output.shapes, output.pixels_per_point);

            if !jobs.is_empty() {
                break;
            }
        }

        self.occupied = placed.map(|rect| pixels_of(rect, scale, frame));
        self.jobs = jobs;
    }

    /// One pass of egui over this world, and where the panel came out.
    fn pass(
        &self,
        rows: &[Reading],
        frame: (u32, u32),
        scale: f32,
    ) -> (egui::FullOutput, Option<egui::Rect>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(points(frame.0) / scale, points(frame.1) / scale),
            )),
            max_texture_side: Some(self.painter.largest_texture),
            ..egui::RawInput::default()
        };

        let mut placed = None;
        let output = self.context.run_ui(input, |ui| {
            let context = ui.ctx().clone();

            placed = Some(
                egui::Area::new(egui::Id::new("coacervate readings"))
                    .fixed_pos(egui::pos2(INSET, INSET))
                    // Read-only, in all three of the ways egui means it. `movable` is what stops
                    // the panel being dragged around; `interactable` is what stops it taking a
                    // click at all; `fade_in` is what stops it being a different picture on its
                    // second frame than on its first, which the byte-for-byte comparison in
                    // `screensaver_mode_hides_every_panel` would otherwise fail on.
                    .movable(false)
                    .interactable(false)
                    .fade_in(false)
                    .show(&context, |ui| {
                        surround().show(ui, |ui| {
                            ui.set_width(WIDTH);
                            lay_out(ui, rows);
                        });
                    })
                    .response
                    .rect,
            );
        });

        (output, placed)
    }

    /// ⭐ **A1.** Draw whatever was last composed **over** the picture, without clearing it.
    ///
    /// The pass loads the target rather than clearing it, which is the whole of this step:
    /// `frame.rs`'s composite writes every pixel of the frame and `window.rs` presents
    /// immediately afterwards, so a chrome pass that cleared would be a black frame with a panel
    /// on it. See `egui_draws_over_the_world_without_clearing_it`.
    ///
    /// Nothing is submitted at all when there is nothing to draw. That is what makes screensaver
    /// mode **byte-identical** to a frame drawn by a program with no panels in it rather than
    /// merely similar to one: there is no pass, so there is no arithmetic to round differently.
    ///
    /// # Panics
    ///
    /// If the world holds more geometry than a frame's worth of buffer can address.
    pub fn paint(&mut self, gpu: &Gpu, target: &wgpu::TextureView, frame: (u32, u32)) {
        self.painter
            .paint(gpu, target, frame, self.scale, &mut self.deltas, &self.jobs);
    }
}

/// One row per reading: the name on the left, then the numeral, then its unit.
///
/// The unit is padded to a fixed width in a monospace font, which is what makes the numerals
/// line up despite the units being different lengths. See [`Reading`].
fn lay_out(ui: &mut egui::Ui, rows: &[Reading]) {
    let font = egui::FontId::monospace(GLYPH);
    let (column, line) = ui.fonts_mut(|fonts| {
        (
            // A monospace font, so any glyph's width is every glyph's width.
            fonts.glyph_width(&font, '0') * f32::from(UNITS),
            fonts.row_height(&font),
        )
    });

    for row in rows {
        if row.is_rule() {
            // The rule between the population and the ledger. Thin, and the same colour as the
            // border, so it reads as part of the frame rather than as a thing of its own.
            ui.add(egui::Separator::default().spacing(7.0));
            continue;
        }

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(row.name).color(LABEL));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // The unit column, reserved to its full width whether anything is written in it
                // or not. See `UNITS`.
                ui.allocate_ui_with_layout(
                    egui::vec2(column, line),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| ui.label(egui::RichText::new(row.unit).color(LABEL)),
                );
                ui.label(egui::RichText::new(&row.value).color(VALUE));
            });
        });
    }
}

/// A frame dimension, as the arithmetic wants it.
///
/// The same conversion and the same bound `frame.rs` uses: 65,535 is not a limit on anything
/// real, it is what lets the conversion be exact rather than a cast CLAUDE.md's lint table would
/// have to be argued out of.
fn points(count: u32) -> f32 {
    let count = u16::try_from(count).expect("a frame is not 65,536 pixels across");

    f32::from(count)
}

/// A rectangle in egui's points, as pixels on the frame, clamped to it.
fn pixels_of(rect: egui::Rect, scale: f32, frame: (u32, u32)) -> [u32; 4] {
    let left = whole(rect.min.x * scale).min(frame.0);
    let top = whole(rect.min.y * scale).min(frame.1);
    let right = whole(rect.max.x * scale).clamp(left, frame.0);
    let bottom = whole(rect.max.y * scale).clamp(top, frame.1);

    [left, top, right - left, bottom - top]
}

/// A coordinate in points, as a whole number of pixels.
///
/// Rounded outwards - down at the near edge, up at the far one, which is what the two callers
/// want between them: a scissor rectangle that covered less than the panel would clip its own
/// border away, and a rectangle that `egui_draws_over_the_world_without_clearing_it` excluded
/// less than the panel from would demand that a pixel the panel legitimately drew on be
/// untouched.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rectangle on a frame, in pixels. Every value reaching this is a position inside \
              a frame at most a few thousand across, which f32 holds exactly; the floor and the \
              negative are both taken before the conversion, so there is nothing left to \
              truncate and nothing left to lose a sign"
)]
fn whole(value: f32) -> u32 {
    value.max(0.0).round() as u32
}

/// What the card needs to know about the frame the chrome is being drawn on.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Screen {
    /// The frame in egui's points, which is what the vertices are in.
    size: [f32; 2],

    /// ⚠️ A named field rather than tail padding. `bytemuck::Pod` refuses to derive for a struct
    /// with padding in it, and that refusal is right: padding is bytes nobody wrote being sent
    /// to the card. `camera.rs`'s `View` carries the same note.
    pad: [f32; 2],
}

/// One texture egui has asked to be kept, and the bind group that reads it.
#[derive(Debug)]
struct Held {
    texture: wgpu::Texture,
    read: wgpu::BindGroup,
}

/// egui's output, on a graphics card.
///
/// See this module's documentation for why this is here rather than being `egui-wgpu`. It does
/// three things and no more: keep the texture atlas in step with what egui says it should be,
/// put a frame's triangles in a buffer, and draw them in batches with a scissor rectangle
/// apiece.
#[derive(Debug)]
struct Painter {
    pipeline: wgpu::RenderPipeline,
    screen: wgpu::Buffer,
    screen_read: wgpu::BindGroup,
    atlas_layout: wgpu::BindGroupLayout,

    /// ⚠️ Nearest, not linear, and clamped. egui rasterises its glyphs at the size they will be
    /// drawn at and hands over an atlas that is already the right size, so a filtered sample of
    /// it is a blurred glyph rather than a smooth one. Clamped because a sample that ran off the
    /// edge of the atlas would pick up whatever glyph was on the other side of it.
    lens: wgpu::Sampler,

    /// Whatever egui has asked to be kept. One entry, in practice: the font atlas. A `BTreeMap`
    /// rather than a `HashMap`, which `clippy.toml` bans workspace-wide.
    textures: BTreeMap<egui::TextureId, Held>,

    /// The largest texture this card will take, which egui needs to know before it decides how
    /// big to make its atlas.
    largest_texture: usize,
}

impl Painter {
    /// Everything the chrome needs to be drawn with, built once.
    fn new(gpu: &Gpu) -> Self {
        let device = gpu.device();
        let shader = device.create_shader_module(wgpu::include_wgsl!("panel.wgsl"));

        let screen = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coacervate chrome screen"),
            size: u64::try_from(size_of::<Screen>()).expect("the screen record is sixteen bytes"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("coacervate chrome screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let screen_read = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("coacervate chrome screen"),
            layout: &screen_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen.as_entire_binding(),
            }],
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("coacervate chrome atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let lens = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("coacervate chrome lens"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..wgpu::SamplerDescriptor::default()
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate chrome"),
            bind_group_layouts: &[Some(&screen_layout), Some(&atlas_layout)],
            ..wgpu::PipelineLayoutDescriptor::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("coacervate chrome"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    // Two points, two texture coordinates and a packed colour: what
                    // `epaint::Vertex` is, and the layout the whole of egui is written around.
                    array_stride: 5 * 4,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x2, 1 => Float32x2, 2 => Uint32
                    ],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: crate::frame::Renderer::FORMAT,
                    // ⚠️ Ordinary alpha compositing, and it is the one blend state in this crate
                    // that is not additive. Everything else here draws light, which adds; a
                    // panel is an object in front of the picture, which covers. egui's own
                    // backend uses exactly this pair, and the alpha half of it is what keeps a
                    // partly-covered pixel's alpha correct on a target that has one.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let largest_texture = usize::try_from(device.limits().max_texture_dimension_2d)
            .expect("a texture bound is a size");

        Self {
            pipeline,
            screen,
            screen_read,
            atlas_layout,
            lens,
            textures: BTreeMap::new(),
            largest_texture,
        }
    }

    /// Bring the atlas up to date, then draw a composition over whatever is on the target.
    fn paint(
        &mut self,
        gpu: &Gpu,
        target: &wgpu::TextureView,
        frame: (u32, u32),
        scale: f32,
        deltas: &mut egui::TexturesDelta,
        jobs: &[egui::ClippedPrimitive],
    ) {
        // ⭐ Nothing composed means nothing submitted - not an empty pass. See `Chrome::paint`.
        if jobs.is_empty() {
            deltas.clear();
            return;
        }

        for (id, delta) in std::mem::take(&mut deltas.set) {
            self.upload(gpu, id, &delta);
        }

        gpu.queue().write_buffer(
            &self.screen,
            0,
            bytemuck::bytes_of(&Screen {
                size: [points(frame.0) / scale, points(frame.1) / scale],
                pad: [0.0, 0.0],
            }),
        );

        // One vertex buffer and one index buffer for the whole composition, with each batch
        // drawing its own stretch of them. `frame.rs` builds its cell buffer the same way and
        // for the same reason: a buffer per batch would be a dozen allocations a frame for a
        // panel that is two hundred triangles.
        let mut vertices: Vec<egui::epaint::Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut batches = Vec::new();

        for job in jobs {
            let egui::epaint::Primitive::Mesh(mesh) = &job.primitive else {
                // A paint callback: something drawing its own geometry inside a panel. Nothing
                // in this program makes one, and there is no sensible thing to draw instead.
                continue;
            };

            let first = u32::try_from(indices.len()).expect("a panel is not four billion indices");
            let base = i32::try_from(vertices.len()).expect("a panel is not two billion vertices");

            indices.extend_from_slice(&mesh.indices);
            vertices.extend_from_slice(&mesh.vertices);
            batches.push((
                job.clip_rect,
                mesh.texture_id,
                first..u32::try_from(indices.len()).expect("a panel is not four billion indices"),
                base,
            ));
        }

        if batches.is_empty() {
            return;
        }

        let device = gpu.device();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coacervate chrome vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("coacervate chrome indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("coacervate chrome"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("coacervate chrome"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // ⭐⭐ **A1, and it is this one word.** The picture is already on this
                        // target - `frame.rs`'s composite wrote every pixel of it and a window
                        // is about to present it. A clear here would throw the world away and
                        // leave a panel on black water.
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..wgpu::RenderPassDescriptor::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.screen_read, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            for (clip, id, range, base) in batches {
                let [left, top, width, height] = pixels_of(clip, scale, frame);
                if width == 0 || height == 0 {
                    continue;
                }

                let Some(held) = self.textures.get(&id) else {
                    // egui named a texture it never handed over. Nothing can be drawn for it and
                    // drawing it with the wrong one would be worse than leaving a hole.
                    continue;
                };

                pass.set_scissor_rect(left, top, width, height);
                pass.set_bind_group(1, &held.read, &[]);
                pass.draw_indexed(range, base, 0..1);
            }
        }

        gpu.queue().submit([encoder.finish()]);

        for id in std::mem::take(&mut deltas.free) {
            self.textures.remove(&id);
        }
    }

    /// Put one of egui's images on the card, whole or in part.
    fn upload(&mut self, gpu: &Gpu, id: egui::TextureId, delta: &egui::epaint::ImageDelta) {
        let egui::ImageData::Color(image) = &delta.image;
        let width = u32::try_from(image.size[0]).expect("an atlas is not four billion across");
        let height = u32::try_from(image.size[1]).expect("an atlas is not four billion deep");
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        // ⚠️ `Rgba8Unorm` and **not** the sRGB form. egui's colours are already sRGB values and
        // `panel.wgsl` converts them; a texture that also converted would do it twice.
        let origin = match delta.pos {
            Some([x, y]) => wgpu::Origin3d {
                x: u32::try_from(x).expect("an atlas patch is not four billion across"),
                y: u32::try_from(y).expect("an atlas patch is not four billion down"),
                z: 0,
            },
            None => {
                let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
                    label: Some("coacervate chrome atlas"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                let read = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("coacervate chrome atlas"),
                    layout: &self.atlas_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.lens),
                        },
                    ],
                });

                self.textures.insert(id, Held { texture, read });
                wgpu::Origin3d::ZERO
            }
        };

        let Some(held) = self.textures.get(&id) else {
            // A patch for a texture that was never allocated. egui does not do this, and there
            // is nothing to write it into if it did.
            return;
        };

        gpu.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &held.texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&image.pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Chrome, readings, recessive};
    use crate::census::{Census, millions_of_years};
    use crate::frame::Renderer;
    use crate::gpu::testing::shared;
    use crate::scene::{Instance, Scene, kind_number};
    use coacervate_sim::cell::CellKind;
    use coacervate_sim::config::spec_defaults;
    use coacervate_sim::world::World;

    /// How big the frames below are drawn. Big enough that the panel is a corner of one rather
    /// than the whole of it, and small enough that six of them cost a fraction of a second.
    const FRAME: (u32, u32) = (512, 288);

    /// A world with a few ticks on it, so the ledger has moved and the clock is not nought.
    fn ticked(ticks: u64) -> World {
        let config = spec_defaults()
            .validate()
            .expect("SPEC section 3's defaults are a world");
        let mut world = World::new(&config);

        for _ in 0..ticks {
            world.tick();
        }

        world
    }

    /// A world with light all over it, including underneath where the panel goes.
    ///
    /// ⚠️ Built rather than taken from `Scene::of(world)`, and the reason is what would otherwise
    /// be measured. A world four hundred ticks old is still in `founding.rs`'s dawn and has
    /// nothing alive in it at all, so the frame would be water - and *"the panel did not erase
    /// the world"* is a claim worth nothing if there is no world on the frame to erase. This
    /// spreads cells over the whole of it, the top-left corner included.
    fn lit() -> Scene {
        let mut cells = Vec::new();
        for row in 0_u8..8 {
            for column in 0_u8..12 {
                cells.push(Instance {
                    position: [
                        f32::from(column) * 40.0 + 20.0,
                        f32::from(row) * 32.0 + 16.0,
                    ],
                    radius: CellKind::Photocyte.radius(),
                    hue: f32::from(column) / 12.0,
                    energy_flow: 0.0,
                    kind: kind_number(CellKind::Photocyte),
                });
            }
        }

        Scene {
            cells,
            snow: Vec::new(),
            width: 512.0,
            height: 288.0,
            phase: 0.0,
        }
    }

    /// Everything the panel says, run together, so a test can look for a number in it.
    fn said(world: &World) -> String {
        readings(world)
            .iter()
            .map(|row| format!("{} {} {}", row.name, row.value, row.unit))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// ⭐ **A2.** The panel reports what the world is doing, and every figure on it is the
    /// figure the rest of the program would give for the same world.
    ///
    /// The claim worth stating is not that the panel prints *something* - it is that it prints
    /// **the same thing** as `Census::of` and the ledger. A panel that walked the population
    /// itself would be a second implementation of six numbers, and the day the two disagreed
    /// would be a day somebody spent believing the wrong one. `census.rs`'s own opening
    /// paragraph is about exactly this, and it is why that module moved into this crate rather
    /// than being copied.
    #[test]
    fn the_panel_reports_what_the_world_is_doing() {
        let world = ticked(400);
        let census = Census::of(&world);
        let ledger = world.ledger();
        let panel = said(&world);

        // CLAUDE.md's deep time: a tick count is never shown as a tick count.
        assert!(
            panel.contains(&format!("{:.1} Ma", millions_of_years(&world))),
            "the panel does not read in millions of years:\n{panel}"
        );
        assert!(
            !panel.contains(&format!("{}", world.ticks())),
            "the panel shows the raw tick count ({}), and CLAUDE.md's deep time is that a long \
             run reads as Earth's history rather than as a number going up:\n{panel}",
            world.ticks()
        );

        // The population, the body and the genome, from the one walk of the world there is.
        for (what, wanted) in [
            ("alive", format!("{}", census.population)),
            ("body", format!("{:.2}", census.mean_cells)),
            ("genome", format!("{:.2}", census.mean_genes)),
        ] {
            assert!(
                panel.contains(&wanted),
                "the panel's {what} is not Census::of(world)'s {wanted}:\n{panel}"
            );
        }

        // ⭐ SPEC section 5's five accounts, all of them. A population figure on its own cannot
        // say whether a world is short of light or short of room.
        for (account, amount) in [
            ("field", world.grid().total_energy()),
            ("biomass", ledger.biomass()),
            ("detritus", ledger.detritus()),
            ("dissipated", ledger.dissipated()),
            ("light", ledger.influx_total()),
        ] {
            let row = readings(&world)
                .into_iter()
                .find(|row| row.name == account)
                .unwrap_or_else(|| panic!("the panel has no {account} row at all:\n{panel}"));

            assert_eq!(
                row.value,
                format!("{amount:.0}"),
                "the panel's {account} is not the ledger's"
            );
        }

        // And the rule between the two blocks, which is what makes them read as two blocks.
        assert_eq!(
            readings(&world).iter().filter(|row| row.is_rule()).count(),
            1,
            "the panel is one undivided column of ten numbers"
        );

        // ⚠️ Every unit fits the column the numerals are aligned against. A unit longer than
        // `UNITS` would push its own numeral left of every other one on the panel, which is
        // the single thing SPEC section 12's "monospace numerics" is asking not to happen.
        for row in readings(&world) {
            assert!(
                row.unit.len() <= usize::from(super::UNITS),
                "\"{}\" is {} characters and the unit column is {}, so the {} row's numeral \
                 would be pushed left of every other numeral on the panel",
                row.unit,
                row.unit.len(),
                super::UNITS,
                row.name
            );
        }
    }

    /// ⭐ **A3, at the near end.** Screensaver mode composes nothing at all - not a smaller
    /// panel, not a transparent one.
    ///
    /// This is the structural half of the claim and it is the half that keeps working as panels
    /// are added. `Chrome::compose` checks the switch before it builds a single widget, so a
    /// panel written in Group B is hidden by a line written in Group A. The other half - that
    /// the *frame* is then indistinguishable from one drawn by a program with no panels in it -
    /// is `screensaver_mode_hides_every_panel`, which needs a graphics card.
    #[test]
    fn screensaver_mode_composes_nothing() {
        let Some(gpu) = shared() else {
            return;
        };

        let world = ticked(400);
        let mut chrome = Chrome::new(gpu);

        chrome.compose(&world, (1280, 720), 1.0);
        let panel = chrome
            .occupies()
            .expect("the chrome is not hidden, so there is a panel somewhere");
        assert!(
            panel[2] > 0 && panel[3] > 0,
            "the panel occupies no pixels: {panel:?}"
        );

        chrome.toggle();
        assert!(chrome.hidden(), "the switch did not go on");
        chrome.compose(&world, (1280, 720), 1.0);
        assert_eq!(
            chrome.occupies(),
            None,
            "screensaver mode left a panel on the frame at {panel:?}"
        );

        // And back again, because a mode that could only be entered would be a mode nobody
        // would ever use.
        chrome.toggle();
        chrome.compose(&world, (1280, 720), 1.0);
        assert!(
            chrome.occupies().is_some(),
            "the chrome did not come back when the switch went off"
        );
    }

    /// ⚠️⚠️ **A panel appears on the very first frame it is asked for**, and that is not free.
    ///
    /// ⭐ **This one found a real fault rather than pinning a decision.** egui's first pass over a
    /// fresh `Context` hands back **no shapes at all**: the font atlas is created during that
    /// pass, so there are no glyphs to lay the text out with, and the area's rectangle comes back
    /// at the wrong place and half the right height. Measured, on the first version of
    /// `Chrome::compose` written:
    ///
    /// | Pass | Area rectangle | Shapes | Atlas changes |
    /// | --- | --- | --- | --- |
    /// | 0 | `[0, 0] - [230, 47]` | **0** | 1 |
    /// | 1 | `[12, 12] - [242, 59]` | 474 indices | 0 |
    /// | 2 | `[12, 12] - [242, 59]` | 474 indices | 0 |
    ///
    /// A composition that ran once and painted therefore drew nothing, and
    /// `egui_draws_over_the_world_without_clearing_it` failed with *"the panel drew on 0 of the
    /// 38,870 pixels of its own rectangle"* - which is the fault named exactly.
    ///
    /// ⚠️ **A window would never have shown this**, and that is the point worth keeping. A window
    /// composes sixty times a second and only the first of those would have been empty, so the
    /// panel would have appeared instantly and correctly on the screen while every single frame
    /// this project dumps to disk - the one instrument CLAUDE.md provides for judging visual
    /// work - came out with no chrome on it at all.
    #[test]
    fn a_panel_appears_on_the_first_frame_it_is_asked_for() {
        let Some(gpu) = shared() else {
            return;
        };

        let world = ticked(400);
        let mut chrome = Chrome::new(gpu);
        chrome.compose(&world, FRAME, 1.0);

        let first = chrome
            .occupies()
            .expect("the very first composition produced no panel");

        // Where it was asked to be: `INSET` points in from the corner, at `WIDTH` plus its own
        // margins and border. A panel at the origin is egui's unsettled first pass showing
        // through.
        let inset = super::whole(super::INSET);
        assert_eq!(
            [first[0], first[1]],
            [inset, inset],
            "the panel is at the corner of the frame rather than inset from it, which is what \
             egui's unsettled first pass reports"
        );

        // And it does not move afterwards. A panel that settled into a different shape on its
        // second frame would be a thing changing size on the screen for no reason in the world.
        for again in 1..4 {
            chrome.compose(&world, FRAME, 1.0);
            assert_eq!(
                chrome.occupies(),
                Some(first),
                "the panel was {first:?} on its first frame and moved on frame {again}"
            );
        }
    }

    /// ⭐⭐ **A1, stated as a measurement.** The chrome is drawn *over* the world and touches
    /// nothing outside itself.
    ///
    /// Group D left `Renderer::paint` clearing its target and writing every pixel of it, and
    /// `window.rs` presenting immediately afterwards - so a chrome pass that cleared would
    /// produce a black frame with a panel on it, and one that got its blending wrong would
    /// silently tint the whole picture. Both are caught here, and they are caught the strong
    /// way: **every pixel outside the panel's own rectangle is required to be byte-identical**
    /// to the same frame drawn with no chrome at all. Not nearly the same - the same byte.
    ///
    /// The second half is what stops the first from being vacuous. A `Chrome` that composed
    /// nothing, or a pass that drew nothing, would satisfy "changed nothing outside the panel"
    /// perfectly, so the panel's own rectangle is required to have actually changed.
    #[test]
    fn egui_draws_over_the_world_without_clearing_it() {
        let Some(gpu) = shared() else {
            return;
        };

        let world = ticked(400);
        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu);

        // ⚠️ `forget` before each, because a motion trail carried from one frame into the next
        // would make two renders of one scene two different pictures - which is `frame.rs`'s
        // own rule and would otherwise be measured here as a fault in the chrome.
        renderer.forget();
        let bare = renderer.render_through(gpu, &scene, &showing_all());

        chrome.compose(&world, FRAME, 1.0);
        let panel = chrome
            .occupies()
            .expect("the chrome is not hidden, so there is a panel somewhere");
        renderer.forget();
        let over = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        let mut changed_outside = Vec::new();
        let mut changed_inside = 0_u32;
        for y in 0..FRAME.1 {
            for x in 0..FRAME.0 {
                if bare.pixel(x, y) == over.pixel(x, y) {
                    continue;
                }

                if inside(panel, x, y) {
                    changed_inside += 1;
                } else if changed_outside.len() < 8 {
                    changed_outside.push((x, y, bare.pixel(x, y), over.pixel(x, y)));
                }
            }
        }

        assert!(
            changed_outside.is_empty(),
            "the chrome changed the frame outside its own panel at {}, {panel:?} - so the egui \
             pass is not loading the picture it is drawing over, and the world it is supposed to \
             be sitting on top of is being erased or tinted",
            changed_outside
                .iter()
                .map(|(x, y, was, now)| format!("({x}, {y}): {was:?} became {now:?}"))
                .collect::<Vec<_>>()
                .join("; ")
        );

        // The panel really is there, and really covers most of its rectangle. Anything much
        // below this would be a border with nothing in it.
        let area = panel[2] * panel[3];
        assert!(
            changed_inside * 2 > area,
            "the panel drew on {changed_inside} of the {area} pixels of its own rectangle \
             {panel:?}, so the frame above is nearly a frame with no panel on it and proves \
             nothing"
        );
    }

    /// ⭐⭐ **A3, stated as a measurement, and it is the strong form of it.**
    ///
    /// CLAUDE.md: *"A screensaver mode that hides all UI and shows only the world."* The weak way
    /// to check that is to look for a panel and not find one. The strong way is the claim a
    /// person watching would actually make - that what is on the screen is **exactly** what a
    /// program with no panels in it would have drawn - and that is a byte-for-byte comparison
    /// against a frame rendered with no `Chrome` anywhere near it.
    ///
    /// It is measured on a chrome that has *already drawn a panel*, which is the case that
    /// matters: screensaver mode is a key somebody presses part-way through a run, so the
    /// question is whether the mode leaves anything behind, not whether it can be started in.
    #[test]
    fn screensaver_mode_hides_every_panel() {
        let Some(gpu) = shared() else {
            return;
        };

        let world = ticked(400);
        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu);

        renderer.forget();
        let no_chrome_at_all = renderer.render_through(gpu, &scene, &showing_all());

        chrome.compose(&world, FRAME, 1.0);
        renderer.forget();
        let shown = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);
        assert_ne!(
            shown.pixels(),
            no_chrome_at_all.pixels(),
            "the panel is not on the frame at all, so hiding it proves nothing"
        );

        // The key. Everything after this is what a person watching would see.
        chrome.toggle();
        chrome.compose(&world, FRAME, 1.0);
        renderer.forget();
        let screensaver = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        let differing = screensaver
            .pixels()
            .iter()
            .zip(no_chrome_at_all.pixels())
            .filter(|(saver, plain)| saver != plain)
            .count();

        assert_eq!(
            differing,
            0,
            "screensaver mode left {differing} bytes of {} different from a frame drawn with no \
             chrome in the program at all, so the mode dims or ghosts the interface rather than \
             removing it",
            screensaver.pixels().len()
        );
    }

    /// The camera every frame above is drawn through: the whole world, at the frame's size.
    fn showing_all() -> crate::camera::Camera {
        crate::camera::Camera::showing_all_of((512.0, 288.0), FRAME)
    }

    /// Whether a pixel is inside a rectangle given as left, top, width and height.
    const fn inside(rect: [u32; 4], x: u32, y: u32) -> bool {
        x >= rect[0] && x < rect[0] + rect[2] && y >= rect[1] && y < rect[1] + rect[3]
    }

    /// The style is the one SPEC section 12's last paragraph asks for.
    ///
    /// Four claims, and every one of them is a word from that sentence. Held as a test rather
    /// than left to the frame, because they are the properties a later change would break
    /// silently: a default-styled egui panel is light, opaque, proportional and animated, and
    /// every one of those is one line away.
    #[test]
    fn the_chrome_is_translucent_dark_and_monospace() {
        let style = recessive();

        let font = style
            .override_font_id
            .as_ref()
            .expect("SPEC section 12 asks for monospace numerics and nothing is overridden");
        assert_eq!(
            font.family,
            egui::FontFamily::Monospace,
            "the panel is drawn in a proportional font"
        );

        assert!(
            super::FILL.a() < 235,
            "SPEC section 12 asks for translucent panels and this one is {} of 255 opaque, so \
             the world stops at its edge instead of passing under it",
            super::FILL.a()
        );
        assert!(
            u32::from(super::FILL.r()) + u32::from(super::FILL.g()) + u32::from(super::FILL.b())
                < 90,
            "SPEC section 12 asks for dark panels and this one is {:?}",
            super::FILL
        );
        assert!(
            super::VALUE.r() < 230 && super::VALUE.g() < 230 && super::VALUE.b() < 230,
            "the panel's numbers are near enough white to be the brightest thing on a frame, \
             and the brightest thing on a frame of this program is supposed to be alive"
        );

        assert!(
            style.animation_time <= f32::EPSILON,
            "the chrome animates, which is both CLAUDE.md's \"nothing that pulls the eye\" and \
             the reason two frames of an unchanged world would not be the same picture"
        );
    }
}
