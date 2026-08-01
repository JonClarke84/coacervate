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
//! # ⭐⭐ `Q27`, answered: the input goes *into* the `RawInput`, and `egui-winit` is still absent
//!
//! Group A deliberately took no input at all, and gave a reason that Group B had to keep:
//!
//! > A headless dump has no winit window, so an input path that only existed in the window would
//! > mean the panel on a dumped frame and the panel on the screen were composed by two different
//! > routes, and the frame would stop being evidence about the window.
//!
//! A slider is the first thing in this program that has to answer a pointer, so the input path
//! arrives here - and it arrives in the shape that argument demands. [`Chrome::compose`] is still
//! **the one composition route**, it still builds its own [`egui::RawInput`], and what changed is
//! only that the `events` field of that input is no longer empty: [`Chrome::feels`] pushes onto a
//! queue and `compose` drains it into the input it was already building. There is no second path
//! and no branch anywhere on whether there is a window.
//!
//! ⭐ **The evidence that this is true is that the input is exercisable with no window at all.**
//! `a_slider_answers_a_pointer_with_no_window_anywhere_near_it` drives a slider from end to end
//! by pushing three `egui::Event`s and composing - the same three a window pushes, through the
//! same call - and asserts the setting moved. A test that could not exist if the input lived in
//! `window.rs`.
//!
//! ⚠️ **`egui-winit` is still not taken, and this is now the settled reason rather than a
//! deferral.** Its whole job is `State::on_window_event`, which takes a `&winit::Window` - the
//! one object a headless dump has not got. Taking it would mean the window fed egui through
//! `egui-winit` and a dump fed it through something else, which is exactly the two routes the
//! paragraph above forbids. What it does that this does not is IME, clipboard, accessibility and
//! touch, none of which a panel of sliders over a simulation has any use for.
//! `controls.rs`'s `felt` is the forty lines that stand in for it.
//!
//! # ⚠️ A pointer over a panel does not reach the camera
//!
//! The other half of `Q27`, and it is in `controls.rs` rather than here, because it is a
//! decision about the *camera*. This side of it is [`Chrome::wants_pointer`], which is egui's own
//! answer read off the last composition. `controls.rs`'s `Controls::apply` takes it and refuses a
//! grab or a wheel that starts over the chrome - and only a grab that *starts* there, so a drag
//! begun on open water goes on panning when the pointer crosses the panel rather than stopping
//! dead half way.

use crate::camera::Look;
use crate::census::{Census, millions_of_years};
use crate::controls::Ask;
use crate::gpu::Gpu;
use crate::series::{Sample, Series};
use crate::settings::{DIALS, Dial, Dials};
use coacervate_sim::chronicle::Chronicle;
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

/// The most of a frame's height **the whole chrome** is ever allowed to take.
///
/// ⚠️ **A fraction of the frame and not what is left over, and a window is what found it.**
/// Bounded by the leftover room the controls would simply grow until they ran out, and on a window
/// somebody had dragged small that is nearly all of the picture - the exact opposite of SPEC
/// section 12's *recessive*.
///
/// ⭐ **Group C moved it from the controls to the column.** Group B's ceiling bounded the scroll
/// area and nothing else, which was right while the controls were the last thing on the frame and
/// is wrong now that the charts are under them: three separate bounds that each hold say nothing
/// about their sum. So the readings, the controls and the charts share one ceiling, and the
/// controls get what is left of it - which makes the chrome's share of the frame something that
/// can be *stated* rather than measured and hoped for. See
/// `the_chrome_is_a_small_part_of_whatever_it_is_drawn_into`, which multiplies this by the panel's
/// width and asserts the product.
///
/// At eleven-twentieths, a person who opens every fold at once gets a panel that scrolls rather
/// than a panel that eats the world. On the dumped frame this project judges itself by it binds on
/// nothing: the shipped arrangement is about 480 points of the 594 it is allowed.
const CEILING: f32 = 0.55;

/// ⭐⭐ **`Q29`.** How large one of egui's points is, in pixels of the frame it is drawn on.
///
/// # The fault this replaces
///
/// Group B handed egui the **display's** scale factor, which is what a normal application does and
/// is wrong for this one. A point was 1.5 pixels on this machine whatever it was drawn into, so a
/// panel of a fixed number of points was a fixed number of *pixels* - and a fixed number of pixels
/// is a small part of a large frame and a large part of a small one. Measured: **4.9% of a 1920 by
/// 1080 dumped frame and 22.1% of the 1280 by 720 window this program opens.** Same panel, same
/// code, same style; a fifth of the picture.
///
/// # The rule
///
/// **A point is a pixel of the frame this project judges itself on** - `DUMP_WIDTH` by
/// `DUMP_HEIGHT`, the size every measurement in `frame.rs` and every note in `docs/PHASE5.md` is
/// taken at. On a smaller frame a point is proportionally smaller, on a larger one proportionally
/// larger, so **the chrome is the same fraction of whatever it is drawn into** - which is the
/// whole of what `Q29` asked for. The lesser of the two ratios is taken, so a window dragged into
/// a tall thin shape shrinks the chrome by its width rather than growing it by its height.
///
/// # And the two bounds, because a bare ratio is wrong at both ends
///
/// - ⚠️ **It never goes below [`SMALLEST`].** Below about nine pixels a monospace numeral stops
///   being something a person can read from across a room, and SPEC section 12's *monospace
///   numerics* is a promise about reading. A panel too small to read is a worse answer than a
///   panel too large, because at least the large one can be turned off with `S`.
/// - ⚠️ **It never goes above the display's own scale.** A point larger than the desktop's point
///   is chrome that is physically bigger than every other window on the screen - which is what
///   Group B shipped, and is the fault. On a 3840 by 2160 window on an unscaled display the ratio
///   asks for 2.0 and the display says a point is a pixel, so a pixel it is.
///
/// At 1920 by 1080 with an unscaled display this is exactly 1.0, which is what the whole of Group
/// A and Group B was looked at through: **every frame this project has dumped is unchanged.**
fn chrome_scale(frame: (u32, u32), display: f32) -> f32 {
    let across = points(frame.0) / points(crate::DUMP_WIDTH);
    let down = points(frame.1) / points(crate::DUMP_HEIGHT);

    across.min(down).min(display).max(SMALLEST)
}

/// The smallest a point is ever allowed to be, in pixels.
///
/// ⚠️ **Legibility, and it was arrived at by looking.** [`GLYPH`] is eleven points, so this is a
/// numeral about nine pixels tall - which on the 1280 by 720 window this program opens is small
/// and is readable, and is what the fourth dump-look-adjust round of Group C settled on. The bare
/// ratio there is 0.667, which gives a seven-pixel glyph: still legible on a magnified crop and
/// not legible on a screen.
///
/// It is the one place the chrome is allowed to be a larger fraction of a small frame than of a
/// large one, and it is deliberately the *only* one.
const SMALLEST: f32 = 0.8;

/// How tall one chart is, in points.
///
/// ⚠️ **A sparkline and not a graph, and that is the whole of how `C2` stays inside SPEC section
/// 12's register.** A chart with an axis on it has tick marks, numbers along the bottom and a
/// legend, which between them are half a dozen bright small things in the corner of a picture that
/// is supposed to nearly disappear. At twenty points beside a seven-character name, a chart is the
/// same shape as a row of readings - a label on the left and something to look at on the right -
/// and the panel gains a third block of the same rhythm rather than a dashboard.
///
/// What it costs is that no chart here can be read as a *quantity*. That is deliberate: the
/// readings panel directly above prints every one of these numbers as a figure, and a second copy
/// of a number is the thing `census.rs` exists to argue against. A chart is for the shape.
const TRACE: f32 = 20.0;

/// How many monospace characters the charts' name column is given.
///
/// `biomass` is the longest of the three. The same device as [`UNITS`]: a reserved width rather
/// than a padded string, so the three traces begin on the same pixel.
const CHART_NAME: u8 = 7;

/// How tall the charts panel is, in points - contents, margins and border together.
///
/// ⚠️ **Known before it is laid out, because the controls above it are bounded by what is left of
/// [`CEILING`]'s column once the charts have had their share.** A height that could only be
/// measured after the fact would mean the ScrollArea above being sized by last frame's charts,
/// which is exactly the one-frame lag Group B's settle loop exists to remove.
///
/// Three traces, the two gaps between them, [`surround`]'s eight points of padding above and below
/// and its one-point border on each edge.
const CHARTS: f32 = 3.0 * TRACE + 2.0 * SPACING + 18.0;

/// ⭐ **Phase 7, Group C.** How tall the log panel is, in points - contents, margins and border
/// together.
///
/// ⚠️ **A reserved height and not a measured one**, for [`CHARTS`]'s reason: the controls above it
/// are bounded by what is left of [`CEILING`]'s column once everything below has had its share, so
/// a height that could only be known after the layout would mean the scroll area being sized by
/// last frame's log. And an event is a *sentence*, so unlike a chart there is no arithmetic that
/// could predict how tall it wants to be - which is why what goes in here is a box with a fixed
/// height that the newest line is kept at the bottom of, rather than a column of labels.
///
/// Six rows of [`GLYPH`] with a little leading, plus [`surround`]'s eight points of padding above
/// and below and its one-point border on each edge. At [`ENTRIES`] events of one wrapped sentence
/// each, the newest is always whole and the ones above it are as much as fits.
const NOTES: f32 = 6.0 * (GLYPH + 2.0) + 18.0;

/// ⭐ **Phase 7, Group C.** How many events the panel is handed.
///
/// More than fit in [`NOTES`], deliberately: the box is scrolled to its bottom, so what is handed
/// over is *at least* enough to fill it however short the sentences happen to be. Four is what a
/// person glancing at a second screen can take in, and the rest of the log is the chronicle's.
const ENTRIES: usize = 4;

/// The vertical gap between two rows of the chrome, in points.
///
/// Named because [`CHARTS`] has to do arithmetic with it, and a panel whose reserved height and
/// actual height were computed from two different numbers would be a panel that slowly grew past
/// its own ceiling.
const SPACING: f32 = 3.0;

/// How far the controls sit below the readings, in points.
///
/// Small, and smaller than [`INSET`] on purpose: the two panels are one column down the left-hand
/// edge of the frame rather than two objects on it, and a gap as wide as the inset would make
/// them read as the latter.
const GAP: f32 = 6.0;

/// How many times [`Chrome::compose`] will ask egui for the same panel before giving up.
///
/// ⚠️ **Group A said two and Group B measured four**, and the extra two are the honest cost of a
/// panel that is more than a list of numbers. What `a_panel_appears_on_the_first_frame_it_is_asked_for`
/// reports on the very first composition of a run:
///
/// | Pass | What comes back | Why |
/// | --- | --- | --- |
/// | 0 | `[0, 0] - [230, 169]`, **nothing tessellated** | egui creates its font atlas during this pass, so there are no glyphs to lay anything out with |
/// | 1 | `[12, 12] - [242, 279]` | laid out, but the scroll area has not measured its own contents yet |
/// | 2 | `[12, 12] - [242, 288]` | it has now |
/// | 3 | the same | settled |
///
/// The second and third are new in Group B and the second is the interesting one: an
/// `egui::ScrollArea` sizes itself from **what it held last time**, so a panel with one in it is
/// nine points shorter on its first frame than on its second. In a window that is one frame at
/// sixty a second and nobody would ever see it. On a *dumped* frame it is the whole picture -
/// which is Group A's finding about the font atlas happening a second time, for a second reason,
/// and is why the loop's condition is now that the panel came out **where it came out last
/// time** rather than merely that something came out at all.
///
/// ⭐ **Eight is a bound and not a cost.** The loop stops the moment the chrome comes out where
/// it came out last *frame*, so a panel nobody has touched settles on its first pass and pays
/// nothing; the four above are the opening composition of a run, and a fold being opened costs
/// two or three. The number is high enough that a panel with every fold open still arrives
/// settled, which was measured: with all six open it takes three.
const SETTLES: usize = 8;

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

/// The rail a slider's handle runs along, and the fill behind a value.
///
/// ⚠️ **Barely above the panel it is drawn on**, and that is the whole of how twenty-one sliders
/// stay recessive. egui's own dark theme draws a slider as a light grey rail with a pale round
/// grab on it, which on a frame of near-black water is the brightest object in the picture -
/// twenty-one times over. At this value a rail reads as a groove in the panel rather than as a
/// thing sitting on it, and the only part of a slider anybody's eye goes to is the number beside
/// it, which is where CLAUDE.md's *"nothing that pulls the eye"* wants it.
const TRACK: egui::Color32 = egui::Color32::from_rgb(30, 40, 47);

/// The part of the rail below the handle, and the handle itself.
///
/// Between the rail and the numerals: enough that the filled part of a slider says at a glance
/// where in its range a setting is - which is the one thing a slider is better at than a number -
/// and not enough to be brighter than the figures on the panel above.
const LEVEL: egui::Color32 = egui::Color32::from_rgb(74, 92, 102);

/// ⭐ **`C2`.** What a chart's own trace is filled with.
///
/// ⚠️ **Below [`TRACK`], and the frame is what put it there.** Round one used the rail's own value
/// on the grounds that a chart should be a groove in the panel like a slider is - but a rail is
/// four points tall and seventy-four wide, and a trace is *twenty* points tall and a hundred and
/// fifty wide. The same colour over eight times the area is not the same weight, and at 3× the
/// `alive` chart read as a solid block with a line on it rather than as a line with a shadow under
/// it. Here the body of a trace is barely above the panel and the edge does the work.
const CHART_FILL: egui::Color32 = egui::Color32::from_rgb(20, 27, 32);

/// The line along the top of a trace.
///
/// One step above the fill, so that a quantity which has not changed for an hour is still a *line*
/// rather than a rectangle - and so that the eye can follow the shape without the fill having to
/// be bright enough to see on its own.
const CHART_EDGE: egui::Color32 = LEVEL;

/// ⭐⭐ **The ledger chart, bottom to top**, in the order [`readings`] lists the accounts.
///
/// # ⚠️ Four bands and not five, and the fifth is not missing
///
/// SPEC section 5 has five accounts and the readings panel prints five rows, but `light` is not a
/// place energy *is* - it is where the energy in the other four came from, and it is already
/// inside them. A fifth band would count every joule twice. What it does instead is grow the whole
/// stack: the chart is drawn as shares of the total, so the total rising is the light arriving.
///
/// # ⚠️ Why the shares and not the amounts
///
/// Because the amounts cannot share an axis. Over the shipped 30,000-tick run the field holds
/// 139,886, `detritus` holds 3,713 and `dissipated` holds 270,506 - so a chart scaled to the
/// largest of them draws detritus as a line along the bottom and biomass as the same line. As
/// shares of a conserved total they are four bands that fill the box, and what the chart says is
/// **where the world's energy is**, which is the question SPEC section 5 exists to answer.
///
/// # ⭐ What is filled is what the world still has, and the frame is what decided that
///
/// Round one drew all four bands at values a hair apart, on the reasoning that four large dark
/// regions would be quieter than two. Magnified 4× it was a box with a dotted line in it: the
/// composition was there and no eye could find it, which is a chart that costs twenty points of
/// panel and says nothing.
///
/// So one band carries the picture and the others get out of its way. **`field` is filled and
/// `dissipated` is not**, which makes the shaded part of the box *the energy the world still has*
/// (a region that starts nearly full and drains over the run). The other way round works equally
/// well as arithmetic and reads backwards: a growing grey block that means *gone*.
///
/// # ⚠️ And the brightest band is the smallest one
///
/// `biomass` is about a twentieth of the total, so it is a hairline sitting on top of the water -
/// and it is the line that says where what is alive ends and what has been spent begins. Giving it
/// [`LEVEL`] is the same decision `frame.rs` makes about the picture underneath: the brightest
/// thing is the thing that is alive.
const ACCOUNTS: [egui::Color32; 4] = [
    // field - the water the world still has, and the one band that carries the picture
    egui::Color32::from_rgb(28, 37, 44),
    // biomass - what is alive, and the brightest hairline on the chrome
    LEVEL,
    // detritus - what was alive
    egui::Color32::from_rgb(46, 60, 69),
    // dissipated - spent for good, and drawn as the absence it is
    egui::Color32::from_rgb(12, 16, 20),
];

/// How wide the rail of a slider is, in points.
///
/// The panel is [`WIDTH`] and a setting's name takes most of it, so the rail is what is left. A
/// name is read once; where the handle is, is what changes.
const RAIL: f32 = 74.0;

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

    // ⭐ **Group B: what a slider is allowed to look like.** Every widget state is written out,
    // because the default dark theme's are the wrong end of the scale in every one of them - a
    // pale rail, a paler grab, a bright hover, rounded corners and a stroke round each. Twenty-one
    // of those over a picture of near-black water is a control surface with a simulation behind
    // it, which is precisely the thing SPEC section 12's *recessive* is asking not to happen.
    for state in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        state.bg_fill = TRACK;
        state.bg_stroke = egui::Stroke::NONE;
        // ⚠️ **This is the slider's handle, and it is deliberately not [`VALUE`].** egui outlines
        // a handle in the widget's foreground stroke, and at the readings' own brightness a
        // column of twelve of them is twelve bright ticks - the brightest things on the panel,
        // marking nothing a person is reading. At [`LEVEL`] a handle is the same weight as the
        // filled part of the rail it sits on, which is what it is: a position, not a figure.
        // The figures keep their own colour, through `override_text_color` below.
        state.fg_stroke = egui::Stroke::new(1.0, LEVEL);
        state.corner_radius = egui::CornerRadius::same(1);
        state.expansion = 0.0;
        // ⚠️ **The box a slider's own number sits in, and the frame is what changed it.** egui
        // draws that number as a `DragValue`, which has a filled rectangle behind it - and at
        // anything visible, twelve of those stacked down the panel are twelve pale chips, easily
        // the loudest thing in the picture and brighter than the readings panel above them. See
        // the round-one note in `docs/PHASE6.md`. Drawn as nothing at all, the box disappears
        // and what is left is a numeral in a column, which is exactly what Group A's typography
        // already does with the readings.
        //
        // ⚠️ Transparent and **not** [`FILL`], which is what round two tried. `FILL` is 85%
        // opaque and the panel it is drawn over is the same colour at the same opacity, so a box
        // painted in it comes out *darker* than the panel: the chip is still there, and it is a
        // hole rather than a tile.
        state.weak_bg_fill = egui::Color32::TRANSPARENT;
    }
    // The one place a widget is allowed to brighten: while a hand is actually on it. Nothing
    // moves on its own here, so this is only ever visible under a pointer that is being held -
    // and it is what tells somebody the number beside a slider can be dragged too.
    visuals.widgets.hovered.weak_bg_fill = TRACK;
    visuals.widgets.active.weak_bg_fill = TRACK;
    visuals.widgets.hovered.bg_fill = LEVEL;
    visuals.widgets.active.bg_fill = LEVEL;
    visuals.selection.bg_fill = LEVEL;
    visuals.selection.stroke = egui::Stroke::new(1.0, VALUE);

    // ⭐ Every figure a widget writes, in the same colour as every figure the readings panel
    // writes. Without it a slider's own number would be its handle's colour, and the handle is
    // deliberately dim - see above. Anything set with `RichText::color` overrides this, which is
    // how the labels stay dimmer than their values.
    visuals.override_text_color = Some(VALUE);

    // ⭐ The filled half of a rail. This is what makes a column of sliders readable at a glance
    // without reading any of the numbers: how far along each one is, as a bar. egui draws it in
    // `selection.bg_fill`, which is why that is [`LEVEL`] above.
    visuals.slider_trailing_fill = true;
    visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.4 };

    let spacing = egui::style::Spacing {
        item_spacing: egui::vec2(6.0, SPACING),
        slider_width: RAIL,
        // A slider's own number is drawn in a drag box, and the default is wide enough for six
        // digits and a sign. Four places after the point is the most any dial here shows.
        interact_size: egui::vec2(40.0, 14.0),
        // The triangle a fold opens with. Small, because it is the only thing on the panel that
        // is a *control* rather than a reading, and it should read as a bullet.
        icon_width: 10.0,
        icon_width_inner: 5.0,
        icon_spacing: 4.0,
        indent: 8.0,
        // ⚠️ **A solid bar rather than egui's floating one, and the frame is why.** With every
        // fold open the controls are taller than the room the panel is given, so the last row
        // visible is cut in half - which with a bar that only appears while something is being
        // scrolled reads as a rendering fault rather than as *there is more below this*. Four
        // points wide and drawn in the rail's own colour: a groove at the panel's edge.
        scroll: egui::style::ScrollStyle {
            bar_width: 4.0,
            floating: false,
            ..egui::style::ScrollStyle::solid()
        },
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

/// ⭐ **`C2`.** What the charts show, as the shapes they are drawn as.
///
/// A free function over a [`Series`], for the same reason [`readings`] is a free function over a
/// world: *what the charts say* is then checkable without a graphics card or a tessellator, and
/// everything after this is layout. See
/// `the_charts_show_population_biomass_and_the_ledger_over_time`.
///
/// Three charts, and the third is a different kind of thing from the first two - see [`ACCOUNTS`].
/// A series with nothing in it produces three charts with no readings in them rather than no
/// charts, because a run that has not reached its first hundredth tick should show three empty
/// boxes and not a panel that grows a third block a second and a half in.
#[must_use]
pub fn charts(history: &Series) -> Vec<Chart> {
    let samples = history.samples();

    vec![
        // The population. `Census::of`'s own figure, as it was every hundred ticks.
        trace(
            "alive",
            samples.iter().map(|sample| counted(sample.population)),
        ),
        // Energy in living organisms, which is what a population figure cannot say on its own: a
        // world of four thousand starving bodies and a world of four thousand fed ones are the
        // same count and different worlds.
        trace("biomass", samples.iter().map(|sample| sample.biomass)),
        stack(samples),
    ]
}

/// ⭐ **Phase 7, Group C.** What the log says, as the lines the panel shows it in.
///
/// A free function over a [`Chronicle`], for the reason [`readings`] and [`charts`] are free
/// functions over a world and a series: *what the panel says* is then checkable with no graphics
/// card and no tessellator anywhere near it, and everything after this is layout.
///
/// # ⚠️ The newest last, and the first sentence only
///
/// Newest last because that is the direction a log is read in and the direction the box scrolls -
/// a person watching sees a line appear at the bottom, which is the calmest possible way for
/// something to arrive on a screen that CLAUDE.md asks never to shout.
///
/// The first sentence only, because the column is [`WIDTH`] points across and that is about
/// twenty-eight monospace characters: a two-sentence event is nine lines of chrome, and three of
/// them would be a panel taller than the readings, the controls and the charts together. The
/// first sentence of every event this project writes is the one that says what happened. The rest
/// is the detail, and the detail is what the chronicle is for.
#[must_use]
pub fn notes(log: &Chronicle) -> Vec<String> {
    log.latest(ENTRIES)
        .map(|event| format!("{:.1} Ma  {}", event.ma, event.headline()))
        .collect()
}

/// One chart on the panel: what it is, and the bands stacked up its height.
#[derive(Debug, Clone, PartialEq)]
pub struct Chart {
    /// What it is, in the left column - the same place a reading's name is.
    pub name: &'static str,

    /// The bands, bottom first. A chart with one band is a trace; a chart with several is a
    /// composition that fills the box.
    pub bands: Vec<Band>,
}

/// One region of a chart, from the band below it up to its own edge.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// What it is filled with.
    pub fill: egui::Color32,

    /// Its top edge at each reading, oldest first, as a fraction of the chart's own height.
    ///
    /// ⚠️ **Cumulative**: it includes every band below it, so a band is drawn as the region
    /// between this and the one before rather than as a height that has to be added up at draw
    /// time. It is what makes a stacked chart and a single trace the same drawing code.
    pub top: Vec<f32>,
}

/// One quantity over the run, as a single band scaled to its own greatest reading.
///
/// ⚠️ **Each chart has its own scale and there is no number saying what it is.** That is the
/// honest shape for a sparkline: what it can say is *how this went*, and what it cannot say is how
/// big it got - which the readings panel directly above prints as a figure. A shared scale would
/// be worse in both directions, because a population of two thousand and a biomass of a hundred
/// and forty thousand do not go on one axis.
fn trace(name: &'static str, values: impl Iterator<Item = f32>) -> Chart {
    let values: Vec<f32> = values.collect();
    let peak = values.iter().copied().fold(0.0_f32, f32::max);

    let top = values
        .iter()
        .map(|value| {
            if peak > 0.0 {
                (value / peak).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect();

    Chart {
        name,
        bands: vec![Band {
            fill: CHART_FILL,
            top,
        }],
    }
}

/// SPEC section 5's accounts as shares of the energy the world is holding. See [`ACCOUNTS`].
fn stack(samples: &[Sample]) -> Chart {
    let mut bands = ACCOUNTS.map(|fill| Band {
        fill,
        top: Vec::with_capacity(samples.len()),
    });

    for sample in samples {
        let total = sample.total();
        let mut running = 0.0_f32;

        for (band, held) in bands.iter_mut().zip([
            sample.field,
            sample.biomass,
            sample.detritus,
            sample.dissipated,
        ]) {
            // A world whose books have not been opened holds nothing at all, and nought over
            // nought is not a share. Four empty bands is the right picture of it.
            if total > 0.0 {
                running += held / total;
            }

            band.top.push(running.clamp(0.0, 1.0));
        }
    }

    Chart {
        name: "energy",
        bands: bands.into(),
    }
}

/// A population, as a height on a chart.
#[expect(
    clippy::cast_precision_loss,
    reason = "a count of organisms, bounded by CLAUDE.md's hundred-thousand arena, on its way to \
              being a fraction of a box twenty points tall. A 32-bit float holds every whole \
              number up to sixteen million exactly, so nothing is lost at all"
)]
fn counted(value: u32) -> f32 {
    value as f32
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

    /// ⭐ **`B1`, `B2` and `B3`.** The settings, and the gate in front of them. See
    /// [`crate::settings`], which is where every argument about them is written down.
    dials: Dials,

    /// ⭐ **`B5`.** What the picture is drawn to look like. See [`crate::camera::Look`].
    look: Look,

    /// Whether the run is paused, so the panel can say so. Set by whoever owns the pacing -
    /// `window.rs` - because the panel reports the run rather than being it.
    paused: bool,

    /// ⭐ **`Q27`.** What a person has done that egui has not been told about yet.
    ///
    /// Drained into the `events` field of the [`egui::RawInput`] that [`Chrome::compose`] was
    /// already building. That is the whole of the input path: there is no second route, and a
    /// headless composition simply finds this empty.
    pending: Vec<egui::Event>,

    /// Whether the last composition wanted the pointer: egui's own answer, kept so
    /// `controls.rs` can ask it without reaching into a context.
    wants_pointer: bool,

    /// What the last composition's buttons asked the window to go and do.
    asked: Vec<Ask>,

    /// The last composition, tessellated and waiting to be painted.
    jobs: Vec<egui::ClippedPrimitive>,

    /// Atlas changes egui made while composing, to be applied before painting.
    deltas: egui::TexturesDelta,

    /// Where the panel ended up, in pixels: left, top, width, height. Nothing at all when the
    /// chrome is hidden, which is what `egui_draws_over_the_world_without_clearing_it` uses to
    /// know which pixels of the frame it is entitled to demand are untouched.
    occupied: Option<[u32; 4]>,

    /// The same, in points, as the last composition left it.
    ///
    /// What [`SETTLES`] is compared against: a composition is done when the chrome comes out
    /// where it came out last time, which for a panel nothing has changed is true on the first
    /// pass.
    settled: Option<egui::Rect>,

    /// What one of egui's points is worth in pixels.
    scale: f32,
}

impl Chrome {
    /// Build the chrome, styled, over these settings.
    ///
    /// The dials are handed in rather than made here because they are **this run's**: a run
    /// started with `--config` is running under a document of its own, and a panel that showed
    /// SPEC's defaults instead would be a panel lying about the world underneath it. `main.rs`
    /// builds them from the same document `args.rs` validated.
    #[must_use]
    pub fn new(gpu: &Gpu, dials: Dials) -> Self {
        let context = egui::Context::default();
        context.all_styles_mut(|style| *style = recessive());

        Self {
            context,
            painter: Painter::new(gpu),
            hidden: false,
            dials,
            look: Look::DEFAULT,
            paused: false,
            pending: Vec::new(),
            wants_pointer: false,
            asked: Vec::new(),
            jobs: Vec::new(),
            deltas: egui::TexturesDelta::default(),
            occupied: None,
            settled: None,
            scale: 1.0,
        }
    }

    /// The settings as the panel now has them, checked.
    #[must_use]
    pub const fn dials(&self) -> &Dials {
        &self.dials
    }

    /// What the picture is to be drawn to look like.
    #[must_use]
    pub const fn look(&self) -> Look {
        self.look
    }

    /// Tell the panel whether the run is paused, so it can say so.
    pub const fn pausing(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// ⭐ **`Q27`.** Something a person did, for egui to be told about at the next composition.
    ///
    /// This is the whole of the input path. It is a queue rather than a call into egui because
    /// [`Chrome::compose`] is the only place an [`egui::RawInput`] is built, and an event handed
    /// straight to a context would be a second way in.
    pub fn feels(&mut self, event: egui::Event) {
        self.pending.push(event);
    }

    /// Whether the pointer is over something the chrome owns.
    ///
    /// egui's own answer, taken off the last composition: it is true while the pointer is inside
    /// a panel's rectangle and while a widget is being dragged, wherever the pointer has got to.
    /// `controls.rs` is what does something with it. Always false in screensaver mode, because
    /// there is nothing there.
    #[must_use]
    pub const fn wants_pointer(&self) -> bool {
        self.wants_pointer
    }

    /// What the panel's buttons asked for since this was last called, and clear the list.
    pub fn asked(&mut self) -> Vec<Ask> {
        std::mem::take(&mut self.asked)
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
    /// ⭐ **`display` is the *display's* scale factor and not the chrome's**, which is `Q29`. A
    /// window hands over what winit reports and a headless dump hands over one; what a point is
    /// actually worth on this frame is [`chrome_scale`]'s answer, and it is a fraction of the
    /// frame rather than a fact about the monitor.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height.
    pub fn compose(
        &mut self,
        world: &World,
        history: &Series,
        log: &Chronicle,
        frame: (u32, u32),
        display: f32,
    ) {
        // ⭐ **A3, and it is this line.** Everything below builds widgets; nothing below is
        // reached. A panel added in Group B is hidden by this without anybody remembering to
        // hide it, which is the whole of what Q24 asked for.
        if self.hidden {
            self.jobs.clear();
            self.occupied = None;
            self.settled = None;
            // ⚠️ And the pointer is nobody's but the camera's while the chrome is away. Without
            // this line, screensaver mode would leave whatever the last composition happened to
            // want, and a person who pressed `S` while the pointer was over where the panel had
            // been would find the camera would not drag there.
            self.wants_pointer = false;
            self.pending.clear();
            return;
        }

        assert!(
            frame.0 > 0 && frame.1 > 0,
            "a frame cannot be {} by {} pixels",
            frame.0,
            frame.1
        );

        // ⭐⭐ **`Q29`, and it is this line.** What a point is worth is decided by the frame the
        // chrome is being drawn into, bounded by the display's own idea of one.
        let scale = chrome_scale(frame, display);
        self.scale = scale;
        self.context.set_pixels_per_point(scale);

        let rows = readings(world);

        // ⭐ **`C2`.** Worked out once and handed to every settle pass, like the readings: a
        // composition asked for again is the same picture, and re-scaling four thousand readings
        // per pass would be the one expensive thing on this panel.
        let traces = charts(history);

        // ⭐ **Phase 7, Group C.** And the same for the log, for the same reason.
        let lines = notes(log);

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
        // ⭐ **`Q27`.** The queue goes into the input this loop was already building, and nowhere
        // else.
        //
        // ⚠️⚠️ **Only the first pass gets it, and that is load-bearing.** A settle pass is the
        // *same* composition asked for again, so an event handed to two of them happens twice -
        // and the two things on this panel that answer a click both break in ways nobody would
        // look for. A fold would open and close again in one frame, so a person clicking `light`
        // would see nothing happen at all. A button would ask twice, so the pause key on the
        // panel would toggle the run back to where it was. Neither shows up in the source and
        // both are one line away.
        let mut felt = std::mem::take(&mut self.pending);

        // Cleared once and then accumulated, for the same reason. Only the pass that was given
        // the events can produce an ask, so there is nothing to double up and nothing to lose.
        self.asked.clear();

        // ⭐ Where the chrome came out **last frame**, so that a steady panel settles on its
        // first pass and only a panel that has actually changed shape costs a second. See
        // [`SETTLES`].
        let mut placed = self.settled;
        let mut jobs = Vec::new();

        for _ in 0..SETTLES {
            let (output, at) = self.pass(
                &rows,
                &traces,
                &lines,
                frame,
                scale,
                std::mem::take(&mut felt),
            );

            self.deltas.append(output.textures_delta);
            let drawn = self
                .context
                .tessellate(output.shapes, output.pixels_per_point);
            let settled = !drawn.is_empty() && at == placed;

            placed = at;
            jobs = drawn;

            if settled {
                break;
            }
        }

        self.settled = placed;

        self.wants_pointer =
            self.context.is_pointer_over_egui() || self.context.egui_wants_pointer_input();
        self.occupied = placed.map(|rect| pixels_of(rect, scale, frame));
        self.jobs = jobs;
    }

    /// One pass of egui over this world, and where the chrome came out.
    fn pass(
        &mut self,
        rows: &[Reading],
        traces: &[Chart],
        lines: &[String],
        frame: (u32, u32),
        scale: f32,
        felt: Vec<egui::Event>,
    ) -> (egui::FullOutput, Option<egui::Rect>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(points(frame.0) / scale, points(frame.1) / scale),
            )),
            max_texture_side: Some(self.painter.largest_texture),
            // ⭐⭐ **`Q27` in one line.** Everything a person did, in the input `Chrome::compose`
            // has built since Group A. A dumped frame's is empty and nothing else differs.
            events: felt,
            ..egui::RawInput::default()
        };

        // How tall the frame is in egui's points, and the most of it the **whole column** may
        // have. See [`CEILING`], which a window measured and Group C moved off the controls.
        let tall = points(frame.1) / scale;
        let column = (tall * CEILING).min(tall - INSET * 2.0);

        // The closure below writes into three of this struct's fields while `run_ui` is reading
        // another, so `self` is taken apart into the fields rather than passed whole. The
        // context is an `Arc` handle and cloning one costs a reference count.
        let Self {
            context: held,
            dials,
            look,
            paused,
            asked,
            ..
        } = self;
        let paused = *paused;
        asked.clear();
        let held = held.clone();

        let mut placed = None;
        let output = held.run_ui(input, |ui| {
            let context = ui.ctx().clone();

            let readings = egui::Area::new(egui::Id::new("coacervate readings"))
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
                .rect;

            // ⭐ Group B's panel, directly under Group A's and the same width - one column down
            // the left-hand edge rather than two objects on the frame. `interactable` is the one
            // word that differs between the two areas, and it is `B1` to `B5` in a nutshell:
            // the readings are looked at and the controls are touched.
            let controls = egui::Area::new(egui::Id::new("coacervate controls"))
                .fixed_pos(egui::pos2(INSET, readings.max.y + GAP))
                .movable(false)
                .interactable(true)
                .fade_in(false)
                .show(&context, |ui| {
                    surround().show(ui, |ui| {
                        ui.set_width(WIDTH);
                        // ⚠️ Bounded by the frame rather than by the contents. Every fold open
                        // at once is taller than a small window, and a panel that ran off the
                        // bottom would put the settings a person cannot see out of reach with
                        // no indication that they were there.
                        egui::ScrollArea::vertical()
                            .max_height(
                                // ⭐ **Group C, and Phase 7's log under it.** What is left of the
                                // column once the readings above and the charts and the log below
                                // have had their share. See [`CEILING`], [`CHARTS`] and [`NOTES`].
                                (column - readings.height() - GAP - CHARTS - GAP - NOTES - GAP)
                                    .max(GLYPH * 4.0),
                            )
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                lay_out_controls(ui, dials, look, paused, asked);
                            });
                    });
                })
                .response
                .rect;

            // ⭐ **`C2`.** Group C's panel, under Group B's and the same width again - so the
            // chrome is still one column down the left-hand edge of the frame rather than three
            // objects on it. `interactable` is false: the charts are looked at, like the readings.
            let charts = egui::Area::new(egui::Id::new("coacervate charts"))
                .fixed_pos(egui::pos2(INSET, controls.max.y + GAP))
                .movable(false)
                .interactable(false)
                .fade_in(false)
                .show(&context, |ui| {
                    surround().show(ui, |ui| {
                        ui.set_width(WIDTH);
                        lay_out_charts(ui, traces);
                    });
                })
                .response
                .rect;

            // ⭐⭐ **Phase 7, Group C. The event log, where a person watching can see it.**
            //
            // `docs/PHASE7.md`: a run grew serially repeated bodies at tick 2.8 million and it
            // was found by eye, in a screenshot, hours later. This is the panel that would have
            // said so at the time. Under the charts, the same width again, `interactable` false
            // like the readings and the charts - it is looked at, and the world is the subject.
            let notes = egui::Area::new(egui::Id::new("coacervate log"))
                .fixed_pos(egui::pos2(INSET, charts.max.y + GAP))
                .movable(false)
                .interactable(false)
                .fade_in(false)
                .show(&context, |ui| {
                    surround().show(ui, |ui| {
                        ui.set_width(WIDTH);
                        lay_out_notes(ui, lines);
                    });
                })
                .response
                .rect;

            placed = Some(readings.union(controls).union(charts).union(notes));
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

/// ⭐ **`C2`.** One row per chart: the name on the left, the trace on the right.
///
/// The same grammar as [`lay_out`] one panel above, deliberately - a dim name in a reserved column
/// and the thing that changes to the right of it. What sits in the right-hand column is a shape
/// instead of a numeral, and nothing else about the row is different.
fn lay_out_charts(ui: &mut egui::Ui, charts: &[Chart]) {
    let font = egui::FontId::monospace(GLYPH);
    let column = ui.fonts_mut(|fonts| fonts.glyph_width(&font, '0')) * f32::from(CHART_NAME);

    for chart in charts {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(column, TRACE),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(egui::RichText::new(chart.name).color(LABEL));
                },
            );

            // ⚠️ `allocate_space` and not `allocate_exact_size`, because a chart senses nothing.
            // A widget that answered a hover would put egui's own tooltip machinery over a panel
            // whose whole argument is that nothing on it moves.
            let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), TRACE));
            draw_chart(ui.painter(), rect, chart);
        });
    }
}

/// ⭐ **Phase 7, Group C.** The last few things that happened, newest at the bottom.
///
/// # ⚠️ A box of a fixed height, filled from the bottom with whole events
///
/// An event is a *sentence*, and a sentence in a column twenty-eight characters wide is two to
/// five lines depending on what happened. So unlike the readings and the charts there is no
/// arithmetic that says in advance how tall this wants to be - and a block that took whatever
/// height its contents asked for would be a panel that changed size every time something occurred.
/// On a screen CLAUDE.md asks to be *"visually calm"*, with *"nothing that pulls the eye"*, that is
/// the worst possible behaviour: the whole chrome would jump at the moment there was news.
///
/// So the box is [`NOTES`] tall whatever is in it, the newest event sits on its floor, and older
/// ones stack up above it **for as long as one fits whole**. Nothing moves except the text.
///
/// ⚠️ **Whole, and a dumped frame is what asked for that.** The first version was a scroll area
/// held at its end, which is the obvious answer and which slices the line at the top of the box
/// through the middle of its letters. A half-line of text reads as a rendering fault rather than
/// as *there is more above*; measuring each event and dropping the one that will not fit costs
/// eight lines and leaves a block with nothing broken in it. What is above is the chronicle's, and
/// the log has to be readable at a glance rather than complete.
fn lay_out_notes(ui: &mut egui::Ui, lines: &[String]) {
    let font = egui::FontId::monospace(GLYPH);
    let wrap = ui.available_width();
    let (row, gap) = (
        ui.fonts_mut(|fonts| fonts.row_height(&font)),
        ui.spacing().item_spacing.y,
    );

    // A whole number of rows, so that an empty box and a full one are the same height and the
    // block does not shrink by a fraction of a line when a run has said nothing for an hour.
    let box_height = (((NOTES - 18.0) / row).floor() * row).max(row);
    let mut left = box_height;

    // ⚠️ `allocate_ui_with_layout` and not `with_layout`, which was the first version: a bottom-up
    // layout inside an `Area` takes the height that is *available*, which is the rest of the
    // frame - so the panel came out four hundred pixels tall with the events sitting at the
    // bottom of an empty box. This asks for exactly the box.
    ui.allocate_ui_with_layout(
        egui::vec2(wrap, box_height),
        egui::Layout::bottom_up(egui::Align::LEFT),
        |ui| {
            for line in lines.iter().rev() {
                let laid =
                    ui.fonts_mut(|fonts| fonts.layout(line.clone(), font.clone(), VALUE, wrap));

                let wants = laid.size().y + gap;
                if wants > left {
                    break;
                }

                left -= wants;
                ui.label(laid);
            }
        },
    );
}

/// Paint one chart's bands into its box.
///
/// # ⚠️ The readings are decimated to the width of the box before anything is drawn
///
/// A series holds up to `series::CAPACITY` readings and a chart is about a hundred and fifty
/// points wide, so at the far end of a long run there are twenty-seven readings behind every
/// column. Handing all of them to the tessellator would be four thousand line segments per chart
/// per frame for a picture that cannot show more than one point per column.
///
/// ⚠️ **The reading nearest the column is taken, rather than the mean of the readings behind it**,
/// which is `series.rs`'s rule about thinning applied to drawing: a mean of two readings is a
/// third reading that no tick of the world ever produced. Every point on every chart in this
/// program is a reading the world actually gave.
fn draw_chart(painter: &egui::Painter, rect: egui::Rect, chart: &Chart) {
    let Some(first) = chart.bands.first() else {
        return;
    };

    let readings = first.top.len();
    if readings < 2 {
        // One reading is a dot and no readings is nothing. Either way there is no shape yet, and
        // an empty box is the honest picture of a run that has not been going long enough.
        return;
    }

    let columns = columns_of(rect.width(), readings);
    let across =
        |column: u16| rect.left() + rect.width() * f32::from(column) / f32::from(columns - 1);
    let reading = |column: u16| usize::from(column) * (readings - 1) / usize::from(columns - 1);

    // ⚠️⚠️ **A band that holds anything at all is drawn at least [`THIN`] tall, and a *window*
    // is what found it.** `biomass` is about a twentieth of the world's energy, which in a box
    // sixteen pixels high is four fifths of a pixel - and a mesh is handed to the card as
    // triangles rather than through epaint's tessellator, so it gets no feathering at all and a
    // sub-pixel band is filled only in the columns where it happens to cross a pixel's centre.
    // On the 1920 by 1080 dump it came out as a line; in the 1280 by 720 window, where a point
    // is 0.8 pixels, it came out as a row of **dashes** - which reads as a rendering fault
    // rather than as the thing that is alive.
    //
    // The cost, stated rather than hidden: a band forced up to a pixel is drawn larger than it
    // is, and in the worst case - three slivers at once - three points of a twenty-point box go
    // on saying *there is something here*. That is the right trade for this chart, because a
    // sliver that rounds away to nothing is the one reading it must not give.
    //
    // ⚠️ Only for a stack. A single trace is drawn through epaint's own line tessellator, which
    // does feather, and a floor there would lift a population of nought off the bottom of its
    // box - a chart of an extinct world showing a line above the floor.
    let floor = chart.bands.len() > 1;

    // Where the band below this one got to. The first band stands on the floor of the box.
    let mut lower: Vec<f32> = (0..columns).map(|_| rect.bottom()).collect();

    // ⚠️⚠️ **And what it *would* have got to, unfloored - which is a second list because the first
    // one is a lie the moment [`THIN`] has lifted anything.** Written without this and measured:
    // once `biomass` was floored, `detritus`'s own level was still computed from the true
    // cumulative share and came out **below** the band beneath it, so its quad was inside out -
    // and `dissipated`, whose top is always the top of the box, then painted from there down over
    // everything the floor had just made room for. The picture on the frame was a `biomass` line
    // that appeared and disappeared along its length, which is exactly what the fault looks like
    // and is nothing like what it is. Whether a band holds anything is a question about the
    // **readings**; where it is drawn is a question about the box.
    let mut beneath: Vec<f32> = (0..columns).map(|_| 0.0).collect();

    for band in &chart.bands {
        let mut upper = Vec::with_capacity(usize::from(columns));
        let mut shares = Vec::with_capacity(usize::from(columns));

        for column in 0..columns {
            let at = usize::from(column);
            let share = band.top[reading(column)];
            let under = lower[at];
            let mut level = rect.bottom() - share * rect.height();

            if floor && share > beneath[at] {
                level = level.min(under - THIN);
            }

            // ⚠️ Never below the band beneath it and never outside the box, which between them
            // are what make a quad a quad.
            upper.push(level.clamp(rect.top(), under));
            shares.push(share);
        }

        let mut mesh = egui::Mesh::default();
        for column in 0..columns - 1 {
            let (near, far) = (usize::from(column), usize::from(column) + 1);
            let base =
                u32::try_from(mesh.vertices.len()).expect("a chart is not four billion vertices");

            mesh.colored_vertex(egui::pos2(across(column), upper[near]), band.fill);
            mesh.colored_vertex(egui::pos2(across(column + 1), upper[far]), band.fill);
            mesh.colored_vertex(egui::pos2(across(column + 1), lower[far]), band.fill);
            mesh.colored_vertex(egui::pos2(across(column), lower[near]), band.fill);
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }

        painter.add(egui::Shape::mesh(mesh));

        // The line along the top of a trace. Only for a chart that is one band: on a stack it
        // would be three rules across the box, which is the gridlines this design does not have.
        if chart.bands.len() == 1 {
            painter.add(egui::Shape::line(
                (0..columns)
                    .map(|column| egui::pos2(across(column), upper[usize::from(column)]))
                    .collect(),
                egui::Stroke::new(1.0, CHART_EDGE),
            ));
        }

        lower = upper;
        beneath = shares;
    }
}

/// The least a band of a stacked chart is drawn at, in points.
///
/// One and a half, which at [`SMALLEST`] - the smallest a point is ever worth - is a whole pixel
/// and a fifth. See [`draw_chart`], where a window found what a dumped frame could not.
const THIN: f32 = 1.5;

/// How many columns a chart this wide is drawn in.
///
/// One per point of width, and never more than there are readings to draw - so a run three
/// readings old is three steps rather than a hundred and fifty copies of three readings.
fn columns_of(width: f32, readings: usize) -> u16 {
    let across = u16::try_from(up(width)).unwrap_or(u16::MAX);
    let held = u16::try_from(readings).unwrap_or(u16::MAX);

    across.clamp(2, held.max(2))
}

/// ⭐ **`B1`, `B2` and `B4`: the whole of the controls panel.**
///
/// # ⚠️ What is open when nobody has touched it, and why that is the whole register argument
///
/// CLAUDE.md: *"`egui` panels sit over the world: translucent dark, thin borders, monospace
/// numerics, **recessive**."* SPEC section 3 asks for twenty-one live settings and ten locked
/// ones, and thirty-one rows of widgets over a picture of water is a control surface with a
/// simulation behind it - the exact opposite of that sentence, and it would be no less true for
/// every individual row being quiet.
///
/// So the default is **the run's own controls and six closed folds**: eight rows, about a fifth
/// of what is here, and the six labels are SPEC section 3's own table names so that anybody who
/// has read the configuration file already knows which one to open.
///
/// `[light]` is the exception and opens by itself, for the reason SPEC section 3 gives about it
/// in as many words: *"`influx` is the single most consequential slider"*. If somebody opens this
/// panel to change one thing, that is the thing - and a fold that has to be opened before the
/// most likely change can be made is a fold in the way.
///
/// # Why the run's controls are not behind a fold
///
/// Because pausing is not a setting. `B4` is the pair of things a person reaches for while
/// *watching* - stop, and one more tick - and something that answers a question about the moment
/// cannot be two clicks away. It is three widgets and it stays at the top.
fn lay_out_controls(
    ui: &mut egui::Ui,
    dials: &mut Dials,
    look: &mut Look,
    paused: bool,
    asked: &mut Vec<Ask>,
) {
    // ⭐ **`B4`.** Pause, and one tick at a time.
    ui.horizontal(|ui| {
        if ui
            .add(pressable(if paused { "run" } else { "pause" }))
            .clicked()
        {
            asked.push(Ask::Pause);
        }

        // A step is only a step while the run is stopped. Enabled during a run it would be a
        // button that does nothing anybody could see, because the next tick was coming anyway.
        if ui.add_enabled(paused, pressable("step")).clicked() {
            asked.push(Ask::Step);
        }

        // ⚠️ The word, and not a light on the button. A person glancing at this from across a
        // room needs to know whether the world is moving, and the button says what pressing it
        // would *do* rather than what the run is doing - which are opposites.
        ui.label(egui::RichText::new(if paused { "stopped" } else { "" }).color(LABEL));
    });

    // ⭐ **`B4`.** `max_ticks_per_second`, which SPEC section 3 says is what the `slow` profile
    // is made of. Nought is its way of writing *uncapped*, so the far end of this dial is "as
    // fast as the machine goes" and every other position is a real slowing.
    for dial in DIALS.iter().filter(|dial| dial.table == "run") {
        slider(ui, dials, dial);
    }

    ui.add(egui::Separator::default().spacing(7.0));

    // ⭐ **`B1`.** SPEC section 3's live tables, one fold each, in the order that section writes
    // them.
    for (table, open) in [
        ("light", true),
        ("physics", false),
        ("metabolism", false),
        ("mutation", false),
    ] {
        fold(ui, table, open, |ui| {
            for dial in DIALS.iter().filter(|dial| dial.table == table) {
                slider(ui, dials, dial);
            }
        });
    }

    // ⭐ **`B5`.** The four numbers of `camera.rs`'s `Look` that are worth a hand on them, which
    // is what Q26 was asking for the day it was written. The tone map's knee is deliberately
    // *not* one of them - see [`peak_dial`].
    fold(ui, "view", false, |ui| {
        look_sliders(ui, look);
    });

    // ⭐ **`B2`.** Shown, and not editable. See `settings::locked`.
    fold(ui, "locked", false, |ui| {
        let rows: Vec<Reading> = crate::settings::locked(dials.config())
            .into_iter()
            .map(|(name, value, unit)| Reading { name, value, unit })
            .collect();

        lay_out(ui, &rows);
    });

    // ⚠️ **`B3`, as a person would meet it.** The gate refuses in a sentence naming the setting,
    // and this is that sentence, printed. It cannot be reached by dragging - every dial's range
    // is inside what validation accepts, which `every_dial_reaches_both_of_its_ends` holds it to
    // - so a line here means the two have come apart, and the useful thing is to say which
    // setting and what about it rather than to spring back silently.
    if let Some(refused) = dials.refused() {
        ui.add(egui::Separator::default().spacing(7.0));
        ui.label(
            egui::RichText::new(refused.to_string())
                .color(LABEL)
                .size(GLYPH - 1.0),
        );
    }
}

/// A button, in the register.
///
/// ⚠️ **The only widget in the program with a fill on it, and it is the darkest one that still
/// reads as a box.** Everything else on this panel is text or a rail; a button has to look like
/// something a finger goes on or nobody will ever press it, and the way to say that quietly is a
/// hairline in the panel's own border colour rather than a bright face.
fn pressable(said: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(said).color(VALUE))
        .fill(TRACK)
        .stroke(egui::Stroke::new(1.0, EDGE))
        .corner_radius(egui::CornerRadius::same(1))
}

/// One of SPEC section 3's tables, as a fold.
///
/// Closed unless it is the one somebody most likely came for. See [`lay_out_controls`].
fn fold(ui: &mut egui::Ui, table: &str, open: bool, contents: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(table).color(LABEL))
        .id_salt(table)
        .default_open(open)
        .show_unindented(ui, contents);
}

/// ⭐⭐ **`B3`, at the near end.** One live setting, as a slider, with the gate behind it.
///
/// The two lines that matter are the last two. A slider hands back a number, and that number goes
/// to [`Dials::set`] - which writes it into a *copy* of the document, puts the copy through
/// `RawConfig::validate`, and keeps it only if the gate accepted. **There is no assignment here
/// that goes anywhere near the running world.**
///
/// The refusal is dropped on the floor deliberately: `dials` keeps it, and [`lay_out_controls`]
/// prints it. Handling it here would put the sentence beside the slider, which is a line of text
/// appearing in the middle of a column of them and pushing everything below it down - a thing
/// moving on the screen, for CLAUDE.md's purposes.
fn slider(ui: &mut egui::Ui, dials: &mut Dials, dial: &'static Dial) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(short(dial.label)).color(LABEL));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut value = dials.value(dial);
            let places = dial.places;
            let widget = egui::Slider::new(&mut value, dial.least..=dial.most)
                .text("")
                .trailing_fill(true)
                .custom_formatter(move |value, _| figure(value, places))
                .custom_parser(|written| written.trim().parse().ok());
            let widget = match places {
                Some(places) => widget.fixed_decimals(places),
                None => widget.integer(),
            };

            if ui.add(widget).changed() {
                // ⭐ The gate. What comes back is a refusal or nothing, and `dials` keeps it
                // either way - see the note above.
                drop(dials.set(dial, value));
            }
        });
    })
    .response
    // The shortened names have their full paths here, so nothing is actually lost by shortening
    // them. It costs nothing on a frame nobody is hovering over, which is every frame this
    // project dumps.
    .on_hover_text(egui::RichText::new(dial.field()).color(LABEL));
}

/// How many characters wide a setting's value is written.
///
/// ⚠️ **Every numeral on the controls panel ends on the same pixel, and this is what does it** -
/// the same property Group A's third dump-look-adjust round bought for the readings, arrived at
/// by the opposite route. There the unit was given a reserved column and the number right-aligned
/// against it; here egui draws a slider's value in a box it sizes to the contents and **centres
/// the text inside**, so `0` and `0.0010` come out at two different places even though the
/// sliders beside them line up exactly. Padding to a fixed width in a monospace font puts them
/// back in a column.
///
/// Six is the longest any dial writes: `0.0010` for the light's influx, `0.0200` for the genome
/// duplication rate, `400.0` for the collision stiffness. ⚠️ It is *leading* whitespace, which
/// epaint keeps - Group A's note is about **trailing** whitespace, which it trims.
const FIGURE: usize = 6;

/// One dial's value, padded into that column.
fn figure(value: f64, places: Option<usize>) -> String {
    match places {
        Some(places) => format!("{value:>FIGURE$.places$}"),
        None => format!("{value:>FIGURE$.0}"),
    }
}

/// A setting's name, short enough for a panel [`WIDTH`] points wide.
///
/// ⚠️ **Twelve characters is what is left**, and the frame is what measured it. The panel is
/// [`WIDTH`] points, of which the rail takes [`RAIL`], the numeral column takes [`FIGURE`]
/// monospace characters and the two gaps take twelve - which leaves about twelve characters for a
/// name. Nine of SPEC section 3's are longer than that, and a name that overran did not wrap or
/// clip: it pushed its own numeral right, so `offspring_share0.45` came out as one word with a
/// number stuck to it. Visible on the dumped frame with every fold open; invisible in the source.
///
/// Where a word is dropped it is the one the *fold* already says - `duplication_rate` inside a
/// fold called `mutation` is a rate, and `spring_damping` inside `physics` is a spring's. The
/// full path is on every row's tooltip, so nothing is actually lost.
fn short(label: &str) -> &str {
    match label {
        "collision_stiffness" => "stiffness",
        "spring_damping" => "damping",
        "movement_cost" => "movement",
        "reproduction_threshold" => "threshold",
        "offspring_share" => "offspring",
        "duplication_rate" => "duplication",
        "deletion_rate" => "deletion",
        "insertion_rate" => "insertion",
        "reorder_rate" => "reorder",
        "genome_duplication_rate" => "genome_dup",
        "max_ticks_per_second" => "ticks/s",
        other => other,
    }
}

/// ⭐ **`B5`.** The four of [`Look`]'s ten worth a hand on them.
///
/// Not all ten, and the ones left out are left out for reasons rather than for room. The water's
/// three colours and its gradient are a *palette* - three numbers each that have to move together
/// to mean anything, which is a colour picker rather than a slider, and a colour picker is a
/// large bright rectangle in the corner of a frame that is supposed to be recessive. And the tone
/// map's knee is left out because moving it is how the guard below gets broken; see the peak.
fn look_sliders(ui: &mut egui::Ui, look: &mut Look) {
    // ⚠️ **The peak stops at half the knee**, which is `camera.rs`'s `Look::sane` written as a
    // range rather than as a panic. A peak past it is a peak at which two overlapping cells no
    // longer come out of the composite at exactly twice one - SPEC section 12's whole additive
    // technique - and `Renderer::looks` asserts it. A slider that could reach it would be a
    // slider that stops the program, which is not what a slider is for.
    let ceiling = look.knee * 0.5;

    for (label, value, range, places) in [
        ("glow", &mut look.glow, 1.0..=4.0, 2),
        ("peak", &mut look.peak, 0.05..=f64::from(ceiling), 3),
        ("bloom", &mut look.bloom, 0.0..=1.0, 2),
        ("trail", &mut look.trail_fade, 0.0..=0.995, 3),
    ] {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(LABEL));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut held = f64::from(*value);
                let widget = egui::Slider::new(&mut held, range)
                    .text("")
                    .trailing_fill(true)
                    .fixed_decimals(places)
                    .custom_formatter(move |value, _| figure(value, Some(places)))
                    .custom_parser(|written| written.trim().parse().ok());

                if ui.add(widget).changed() {
                    *value = narrowed(held);
                }
            });
        });
    }
}

/// One of the look's numbers, as the card holds it.
///
/// Every value reaching this came out of a slider whose range is a handful of small decimals, all
/// of which a 32-bit float holds to seven digits - which is more than the three any of them is
/// shown to. `config.rs`'s `narrow` is the same conversion where it matters, with a refusal
/// attached; nothing here decides anything about a *world*.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a number between nought and four, off a slider, on its way to a uniform buffer the \
              card reads as 32-bit floats. There is nothing here for a wider type to carry"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
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
///
/// ⚠️ **Rounded outwards, and grown by [`FEATHER`] on every side.** Both are about the same thing:
/// the pixels the chrome is *entitled* to have touched. A scissor rectangle smaller than the panel
/// clips the panel's own border away, and a rectangle that
/// `egui_draws_over_the_world_without_clearing_it` excluded less than the panel from would demand
/// that a pixel the panel legitimately drew on come back unchanged.
///
/// ⚠️ **`Q29` is what found this.** Until Group C every position here was a whole number of points
/// at a scale of one, so a rounded conversion and an outward one were the same conversion and the
/// doc comment saying *outward* was describing something the code did not do. At a scale of 0.8
/// the panel's corner lands on pixel 9.6, rounding puts it at 10, and the row of pixels at 9 -
/// which the panel's own border is drawn across - is outside the rectangle the chrome claims. The
/// test failed the moment the scale stopped being one, which is the useful kind of latent fault.
fn pixels_of(rect: egui::Rect, scale: f32, frame: (u32, u32)) -> [u32; 4] {
    let left = down(rect.min.x * scale - FEATHER).min(frame.0);
    let top = down(rect.min.y * scale - FEATHER).min(frame.1);
    let right = up(rect.max.x * scale + FEATHER).clamp(left, frame.0);
    let bottom = up(rect.max.y * scale + FEATHER).clamp(top, frame.1);

    [left, top, right - left, bottom - top]
}

/// How far past its own edge a stroke is allowed to have reached, in pixels.
///
/// epaint antialiases by feathering a shape outwards over about a pixel, so the border of a panel
/// whose rectangle ends at 9.6 has put some ink on the pixel at 8. One pixel of slack on each side
/// of a panel two hundred across costs the A1 measurement nothing and is the difference between a
/// claim that is true and a claim that is nearly true.
const FEATHER: f32 = 1.0;

/// A coordinate in points, as the whole pixel at or below it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rectangle on a frame, in pixels. Every value reaching this is a position inside \
              a frame at most a few thousand across, which f32 holds exactly; the floor and the \
              negative are both taken before the conversion, so there is nothing left to \
              truncate and nothing left to lose a sign"
)]
fn down(value: f32) -> u32 {
    value.max(0.0).floor() as u32
}

/// A coordinate in points, as the whole pixel at or above it.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the same conversion as `down`, at the far edge of the same rectangle"
)]
fn up(value: f32) -> u32 {
    value.max(0.0).ceil() as u32
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
    use super::{Chrome, Chronicle, charts, notes, readings, recessive};
    use crate::census::{Census, millions_of_years};
    use crate::controls::Ask;
    use crate::frame::Renderer;
    use crate::gpu::testing::shared;
    use crate::scene::{Instance, Scene, kind_number};
    use crate::series::Series;
    use crate::settings::{DIALS, Dial, Dials};
    use coacervate_sim::cell::CellKind;
    use coacervate_sim::config::spec_defaults;
    use coacervate_sim::world::World;

    /// The dials of a run started from SPEC section 3's own document.
    fn shipped() -> Dials {
        Dials::new(spec_defaults()).expect("SPEC section 3's defaults are a world")
    }

    /// How big the frames below are drawn. Big enough that the panel is a corner of one rather
    /// than the whole of it, and small enough that six of them cost a fraction of a second.
    const FRAME: (u32, u32) = (512, 288);

    /// A world with a few ticks on it and something alive in it, and the series taken as it ran.
    ///
    /// ⚠️ **It has a *population* since Group C, and that is not a detail.** Until the charts
    /// landed this was `World::new` and some ticks, which is empty water: every figure the panel
    /// prints was nought, so `the_panel_reports_what_the_world_is_doing` was comparing nought
    /// against nought and the `alive` chart would have been a flat line at the bottom of its box.
    /// `series::testing` is where the seeding lives, because `coacervate-app`'s `founding.rs`
    /// cannot be reached from this crate.
    ///
    /// ⭐ **Since Phase 7 it hands over the event log as well**, taken over the same ticks by the
    /// same three observers `run.rs` drives, so the panel's fourth block is composed over a log a
    /// run actually produced rather than one written here.
    fn ticked(ticks: u64) -> (World, Series, Chronicle) {
        crate::series::testing::living(ticks)
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
        let (world, _, _) = ticked(400);
        let census = Census::of(&world);
        let ledger = world.ledger();
        let panel = said(&world);

        // ⚠️ And there is something alive to report on, which until Group C there was not: every
        // claim below is satisfied by a panel of noughts over empty water.
        assert!(
            census.population > 0,
            "the world this panel is reporting on has nothing alive in it, so every figure \
             compared below is nought against nought"
        );

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

    /// ⭐⭐ **Phase 7, Group C.** The panel shows the most recent events, and screensaver mode
    /// hides them with the rest.
    ///
    /// Three claims, and the third is the one `docs/PHASE7.md` actually asked for.
    ///
    /// **What the lines say** is checkable with no graphics card: [`notes`] is a free function
    /// over a [`Chronicle`], so the newest-last order, the deep-time prefix and the one-sentence
    /// trim are all claims about a function rather than about a picture.
    ///
    /// **That the frame changes** is what stops the first half being a picture of a panel: the
    /// same world is composed twice, once with the run's log behind it and once with an empty
    /// one, and the two frames have to differ.
    ///
    /// **And that the switch still hides everything.** `panel.rs`'s screensaver check is one line
    /// at the top of [`Chrome::compose`] and Group A's whole argument for putting it there was
    /// that a panel written by a later phase would be hidden by it *without anybody remembering
    /// to hide it*. This is the phase that tests the claim rather than making it.
    #[test]
    fn the_panel_shows_the_most_recent_events() {
        let (world, series, log) = ticked(400);

        assert!(
            log.events().len() >= 2,
            "the run behind this panel produced {} events, which is not a log",
            log.events().len()
        );

        let lines = notes(&log);
        assert!(
            lines.len() <= super::ENTRIES,
            "the panel was handed {} events and it has room for {}",
            lines.len(),
            super::ENTRIES
        );

        // Newest last, which is the direction a log is read in and the direction the box scrolls.
        let newest = log
            .events()
            .last()
            .expect("the log has something in it")
            .clone();
        assert!(
            lines
                .last()
                .expect("there is a line")
                .contains(newest.headline()),
            "the last line of the panel is not the most recent event:\n{lines:#?}"
        );

        // CLAUDE.md's deep time on every line, and never a raw tick count.
        for line in &lines {
            assert!(
                line.contains(" Ma  "),
                "a line of the log does not say when it happened: {line}"
            );
        }

        // ⚠️ One sentence per event. The column is 208 points across, which is about
        // twenty-eight monospace characters, so a two-sentence event is nine lines of chrome.
        for (line, event) in lines.iter().zip(log.latest(super::ENTRIES)) {
            assert!(
                line.len() <= event.said.len() + 16,
                "a line of the log is the whole event rather than its first sentence: {line}"
            );
        }

        let Some(gpu) = shared() else {
            return;
        };

        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu, shipped());
        let empty = Chronicle::new(world.config());

        chrome.compose(&world, &series, &empty, FRAME, 1.0);
        renderer.forget();
        let silent = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        chrome.compose(&world, &series, &log, FRAME, 1.0);
        renderer.forget();
        let told = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        assert_ne!(
            told.pixels(),
            silent.pixels(),
            "a panel with a run's own event log behind it draws exactly what a panel with an \
             empty one draws, so the log is not on the frame at all"
        );

        // ⭐ And the one line at the top of `compose` still hides it, with no `if` of its own.
        chrome.toggle();
        chrome.compose(&world, &series, &log, FRAME, 1.0);
        assert_eq!(
            chrome.occupies(),
            None,
            "screensaver mode left the event log on the frame"
        );
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

        let (world, series, log) = ticked(400);
        let mut chrome = Chrome::new(gpu, shipped());

        chrome.compose(&world, &series, &log, (1280, 720), 1.0);
        let panel = chrome
            .occupies()
            .expect("the chrome is not hidden, so there is a panel somewhere");
        assert!(
            panel[2] > 0 && panel[3] > 0,
            "the panel occupies no pixels: {panel:?}"
        );

        chrome.toggle();
        assert!(chrome.hidden(), "the switch did not go on");
        chrome.compose(&world, &series, &log, (1280, 720), 1.0);
        assert_eq!(
            chrome.occupies(),
            None,
            "screensaver mode left a panel on the frame at {panel:?}"
        );

        // And back again, because a mode that could only be entered would be a mode nobody
        // would ever use.
        chrome.toggle();
        chrome.compose(&world, &series, &log, (1280, 720), 1.0);
        assert!(
            chrome.occupies().is_some(),
            "the chrome did not come back when the switch went off"
        );
    }

    /// ⭐⭐ **`C2`.** The charts show the population, the biomass and the ledger over time.
    ///
    /// Two halves, and they are different claims.
    ///
    /// The first is about the **numbers**: `charts` is a free function over a [`Series`], so what
    /// each chart is drawn from can be checked against the records it came from with no graphics
    /// card anywhere near it. The population chart has to follow the populations, and the ledger
    /// chart has to be SPEC section 5's accounts as shares of a whole that adds up.
    ///
    /// The second is about the **frame**, and it is the one that catches a chart that is a picture
    /// of a chart: the same panel is drawn over the same world twice, once with the run's own
    /// series behind it and once with an empty one, and the two frames have to differ. A `charts`
    /// that returned perfect data into a `draw_chart` that painted nothing would pass the first
    /// half and fail this.
    #[test]
    fn the_charts_show_population_biomass_and_the_ledger_over_time() {
        let (world, series, log) = ticked(400);
        let drawn = charts(&series);

        assert_eq!(
            drawn.iter().map(|chart| chart.name).collect::<Vec<_>>(),
            ["alive", "biomass", "energy"],
            "the charts are not the three C2 asks for"
        );
        assert!(
            series.samples().len() >= 4,
            "the series behind these charts holds {} records, which is not a shape",
            series.samples().len()
        );

        // ⭐ The population chart is the populations, scaled to the greatest of them - so the
        // reading that was the peak is at the top of the box and every other one is where it
        // stands against it. Checked against the records rather than against a number written
        // here, which is what makes it a claim about `charts` and not about this world.
        let alive = &drawn[0].bands[0];
        let peak = series
            .samples()
            .iter()
            .map(|sample| sample.population)
            .max()
            .expect("there are records");
        for (height, sample) in alive.top.iter().zip(series.samples()) {
            let wanted = f64::from(sample.population) / f64::from(peak);
            assert!(
                (f64::from(*height) - wanted).abs() < 1e-6,
                "the population chart puts tick {}'s reading of {} at {height} of the box, and \
                 against a peak of {peak} it belongs at {wanted}",
                sample.tick,
                sample.population
            );
        }
        assert!(
            alive.top.iter().any(|height| *height > 0.0),
            "every point on the population chart is on the floor of its box, so there is no shape \
             on it at all"
        );

        // ⭐ The ledger chart is SPEC section 5's accounts as shares of a whole. Four bands, each
        // at or above the one below it, and the top one filling the box exactly - which is the
        // conservation law drawn: what the world holds is all of what the world holds.
        let energy = &drawn[2];
        assert_eq!(
            energy.bands.len(),
            4,
            "the ledger chart is not four accounts"
        );
        for reading in 0..series.samples().len() {
            let mut below = 0.0_f32;
            for band in &energy.bands {
                assert!(
                    band.top[reading] >= below - 1e-6,
                    "the ledger chart's bands are not stacked: {} sits below {below}",
                    band.top[reading]
                );
                below = band.top[reading];
            }

            assert!(
                (below - 1.0).abs() < 1e-4,
                "the four accounts fill {below} of the ledger chart at reading {reading}, and \
                 SPEC section 5 says they are everything the world is holding"
            );
        }

        // And the other half: a chart with a run behind it draws something a chart with no run
        // behind it does not.
        let Some(gpu) = shared() else {
            return;
        };

        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu, shipped());

        chrome.compose(&world, &Series::new(), &log, FRAME, 1.0);
        renderer.forget();
        let empty = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        chrome.compose(&world, &series, &log, FRAME, 1.0);
        renderer.forget();
        let charted = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);

        let differing = charted
            .pixels()
            .iter()
            .zip(empty.pixels())
            .filter(|(with, without)| with != without)
            .count();

        assert!(
            differing > 200,
            "a frame drawn with 400 ticks of history behind it differs from one drawn with none \
             in {differing} bytes, so the charts are boxes with nothing in them"
        );
    }

    /// ⭐⭐ **`Q29`, stated as a measurement.** The chrome is a small part of *whatever it is
    /// drawn into*.
    ///
    /// Group B shipped a panel sized in egui's **points**, and a point is whatever the display
    /// says it is - 1.5 pixels on this machine. So the chrome that is 4.9% of a 1920 by 1080
    /// dumped frame is about **22% of the 1280 by 720 window this program actually opens**, and
    /// the person running it is looking at a fifth of their picture being interface. SPEC section
    /// 12: *"The simulation is the subject; the chrome should nearly disappear."* A fifth of the
    /// frame is not nearly disappearing, and no amount of quietness per widget fixes a size.
    ///
    /// The claim is deliberately stated over **both** shapes and both display scales at once,
    /// because the fault is exactly that the two came apart: a bound that held on the frame this
    /// project judges itself by and not on the window it opens is how `Q29` happened in the first
    /// place.
    #[test]
    fn the_chrome_is_a_small_part_of_whatever_it_is_drawn_into() {
        let Some(gpu) = shared() else {
            return;
        };

        let (world, series, log) = ticked(400);
        let mut chrome = Chrome::new(gpu, shipped());

        for (frame, display) in [
            // The frame every measurement in this project is taken on.
            ((1920_u32, 1080_u32), 1.0_f32),
            // The window this program opens, on this machine's display. `Q29`.
            ((1280, 720), 1.5),
            // And the same window on a display that is not scaled at all.
            ((1280, 720), 1.0),
            // Somebody who dragged it larger, on either.
            ((2560, 1440), 1.5),
        ] {
            chrome.compose(&world, &series, &log, frame, display);
            let panel = chrome
                .occupies()
                .expect("the chrome is not hidden, so there is a panel somewhere");

            let taken = f64::from(panel[2]) * f64::from(panel[3]);
            let whole = f64::from(frame.0) * f64::from(frame.1);
            let share = taken / whole;

            println!(
                "{} by {} at a display scale of {display}: the chrome is {panel:?}, {:.1}% of the \
                 frame, at a point of {} pixels",
                frame.0,
                frame.1,
                share * 100.0,
                super::chrome_scale(frame, display)
            );
            assert!(
                share <= SHARE,
                "the chrome takes {:.1}% of a {} by {} frame at a display scale of {display}, \
                 which is {panel:?} of {whole} pixels. SPEC section 12 asks for chrome that \
                 nearly disappears, and the bound this project holds itself to is {:.1}%",
                share * 100.0,
                frame.0,
                frame.1,
                SHARE * 100.0
            );
        }
    }

    /// The most of any frame the chrome is allowed to be.
    ///
    /// A tenth. Group A's readings panel alone was 2% of a dumped frame and Group B's pair was
    /// 4.9%, so this is not a bound that shapes the design - it is the line past which the chrome
    /// has stopped being chrome, and it has to hold on the *window* as well as on the dump.
    const SHARE: f64 = 0.10;

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

        let (world, series, log) = ticked(400);
        let mut chrome = Chrome::new(gpu, shipped());
        chrome.compose(&world, &series, &log, FRAME, 1.0);

        let first = chrome
            .occupies()
            .expect("the very first composition produced no panel");

        // Where it was asked to be: `INSET` points in from the corner, at `WIDTH` plus its own
        // margins and border. A panel at the origin is egui's unsettled first pass showing
        // through.
        //
        // ⚠️ In *pixels*, which since `Q29` is not the same number: a point on a frame this size
        // is `chrome_scale`'s answer rather than one. Within a couple of pixels rather than
        // exactly, because `pixels_of` rounds outward by [`FEATHER`] and egui puts an area's own
        // corner on a whole pixel of the frame - neither of which this claim is about. What it is
        // about is that the panel is *inset* and not at the origin, which is where egui's
        // unsettled first pass reports it.
        let inset = super::down(super::INSET * super::chrome_scale(FRAME, 1.0));
        assert!(
            first[0].abs_diff(inset) <= 2 && first[1].abs_diff(inset) <= 2,
            "the panel came out at [{}, {}] rather than inset to [{inset}, {inset}], and the \
             corner of the frame is what egui's unsettled first pass reports",
            first[0],
            first[1]
        );

        // And it does not move afterwards. A panel that settled into a different shape on its
        // second frame would be a thing changing size on the screen for no reason in the world.
        for again in 1..4 {
            chrome.compose(&world, &series, &log, FRAME, 1.0);
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

        let (world, series, log) = ticked(400);
        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu, shipped());

        // ⚠️ `forget` before each, because a motion trail carried from one frame into the next
        // would make two renders of one scene two different pictures - which is `frame.rs`'s
        // own rule and would otherwise be measured here as a fault in the chrome.
        renderer.forget();
        let bare = renderer.render_through(gpu, &scene, &showing_all());

        chrome.compose(&world, &series, &log, FRAME, 1.0);
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

        let (world, series, log) = ticked(400);
        let scene = lit();
        let mut renderer = Renderer::new(gpu, FRAME.0, FRAME.1);
        let mut chrome = Chrome::new(gpu, shipped());

        renderer.forget();
        let no_chrome_at_all = renderer.render_through(gpu, &scene, &showing_all());

        chrome.compose(&world, &series, &log, FRAME, 1.0);
        renderer.forget();
        let shown = renderer.render_through_under(gpu, &scene, &showing_all(), &mut chrome);
        assert_ne!(
            shown.pixels(),
            no_chrome_at_all.pixels(),
            "the panel is not on the frame at all, so hiding it proves nothing"
        );

        // The key. Everything after this is what a person watching would see.
        chrome.toggle();
        chrome.compose(&world, &series, &log, FRAME, 1.0);
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

    /// ⭐⭐ **`Q27`, and `B1` end to end. The whole point of this test is that it exists.**
    ///
    /// A slider is driven from one end of its range to the other by pushing three
    /// `egui::Event`s - a move, a press, a move - and composing between them, which is exactly
    /// what a window does with a hand on the mouse. **There is no window anywhere in it.**
    ///
    /// That is the property Group A's decision table asked Group B to preserve, stated as a test
    /// rather than as a paragraph: *"an input path that only existed in the window would mean the
    /// panel on a dumped frame and the panel on the screen were composed by two different routes,
    /// and the frame would stop being evidence about the window."* If the events went into egui
    /// anywhere but [`Chrome::compose`]'s own `RawInput`, this test could not be written - and
    /// the day somebody moves them there, it stops compiling rather than stopping being true.
    ///
    /// ⭐ **And the setting that comes out the far end went through the gate.** The assertion is
    /// on `dials().config()`, which is a `coacervate_sim::config::Config` - a type with no
    /// constructor but `RawConfig::validate`. A slider that had assigned into the world directly
    /// would have had to build one, and it cannot.
    #[test]
    fn a_slider_answers_a_pointer_with_no_window_anywhere_near_it() {
        let Some(gpu) = shared() else {
            return;
        };

        let (world, series, log) = ticked(400);
        let mut chrome = Chrome::new(gpu, shipped());

        // `[light]` is the fold that opens by itself, so `influx` is on the panel from the first
        // frame - see `lay_out_controls` for why that one and not another.
        let dial: &Dial = DIALS
            .iter()
            .find(|dial| dial.field() == "light.influx")
            .expect("light.influx is a live setting");

        // Two compositions with no input at all, to settle the fonts and to give the widgets
        // somewhere to be. A slider cannot be found before it has been laid out once, which is
        // as true of a hand on a mouse as it is of this.
        chrome.compose(&world, &series, &log, (1280, 720), 1.0);
        chrome.compose(&world, &series, &log, (1280, 720), 1.0);

        let before = chrome.dials().value(dial);
        let accepted = chrome.dials().accepted();
        assert_eq!(accepted, 0, "nothing has been touched yet");

        // ⚠️ Where the slider actually is. The rail is the right-hand `RAIL` points of the
        // controls panel, and the row is found by counting: the pause row, the ticks-per-second
        // row, the separator, the `light` fold's own header, and then `influx`. Rather than
        // count, the pointer is walked down the panel until something moves - which is what a
        // hand does, and which does not have to be edited every time a row is added.
        let panel = chrome
            .occupies()
            .expect("the chrome is not hidden, so there are panels");
        let right = f32::from(u16::try_from(panel[0] + panel[2]).expect("a panel is on a frame"));

        let mut moved = None;
        for row in 0_u8..80 {
            let down = f32::from(u16::try_from(panel[1]).expect("a panel is on a frame"))
                + f32::from(row) * 3.0;
            // The far right of the rail: the top of the dial's range.
            let at = egui::pos2(right - 20.0, down);

            chrome.feels(egui::Event::PointerMoved(at));
            chrome.feels(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            chrome.compose(&world, &series, &log, (1280, 720), 1.0);

            chrome.feels(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            chrome.compose(&world, &series, &log, (1280, 720), 1.0);

            if chrome.dials().accepted() > accepted {
                moved = Some(at);
                break;
            }
        }

        let at = moved.unwrap_or_else(|| {
            panic!(
                "eighty rows down the controls panel {panel:?}, nothing answered a pointer at \
                 all - so the events pushed onto the chrome are not reaching the RawInput that \
                 Chrome::compose builds, and every slider on this panel is a picture of a slider"
            )
        });

        // Something moved, and it went through the gate: `Dials::config` is a validated
        // `Config`, and there is no other way to have one.
        assert!(
            chrome.dials().accepted() > 0,
            "a change was made and not counted"
        );
        assert!(
            chrome.dials().refused().is_none(),
            "dragging a slider produced a value the configuration gate refuses, which means a \
             dial's range and the gate's have come apart: {:?}",
            chrome.dials().refused()
        );

        // ⚠️ And the pointer is the chrome's while it is over it, which is the other half of
        // `Q27`. `controls.rs` is what does something with this; here it only has to be true.
        assert!(
            chrome.wants_pointer(),
            "the pointer is sitting on a widget at {at:?} and the chrome says it does not want \
             it, so the camera would be panning under the hand that is dragging this slider"
        );

        // Every live setting is one the world could be given, whatever was dragged.
        let now = chrome.dials().value(dial);
        assert!(
            chrome.dials().config().light.influx >= 0.0,
            "the settings the world would be handed are not a world"
        );
        println!("light.influx went from {before} to {now}");
    }

    /// ⭐ **`B4`, at the panel's end.** The panel's own buttons ask for the same things the keys
    /// do, and ask once.
    ///
    /// The claim is about the *list*: `controls.rs`'s `Ask` is what a key produces and what a
    /// button produces, so `Space` and the pause button cannot become two implementations of
    /// pausing that disagree about whether the run is stopped.
    ///
    /// ⚠️ **And it asks once.** egui's unsettled first pass builds every widget - it simply has
    /// no glyphs to lay them out with - so a composition that ran twice and kept both passes'
    /// actions would press every button on the panel twice on the first frame of a run. `compose`
    /// clears the list at the top of each settle pass, and this is what holds it there.
    #[test]
    fn the_panel_asks_for_a_pause_once_per_press() {
        let Some(gpu) = shared() else {
            return;
        };

        let (world, series, log) = ticked(400);
        let mut chrome = Chrome::new(gpu, shipped());

        chrome.compose(&world, &series, &log, (1280, 720), 1.0);
        assert!(
            chrome.asked().is_empty(),
            "a panel nobody has touched asked for something"
        );

        let panel = chrome
            .occupies()
            .expect("the chrome is not hidden, so there are panels");
        let left = f32::from(u16::try_from(panel[0]).expect("a panel is on a frame"));
        let top = f32::from(u16::try_from(panel[1]).expect("a panel is on a frame"));

        // The pause button is the first widget on the controls panel, which sits below the
        // readings - so it is found the same way the slider above is, by walking down.
        let mut pressed = Vec::new();
        for row in 0_u8..80 {
            let at = egui::pos2(left + 24.0, top + f32::from(row) * 3.0);

            chrome.feels(egui::Event::PointerMoved(at));
            chrome.feels(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            chrome.compose(&world, &series, &log, (1280, 720), 1.0);
            pressed.extend(chrome.asked());

            chrome.feels(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            chrome.compose(&world, &series, &log, (1280, 720), 1.0);
            pressed.extend(chrome.asked());

            if pressed.contains(&Ask::Pause) {
                break;
            }
        }

        assert!(
            pressed.contains(&Ask::Pause),
            "nothing on the controls panel asked for a pause, so `B4`'s button is a picture of a \
             button: {pressed:?}"
        );
        assert_eq!(
            pressed.iter().filter(|ask| **ask == Ask::Pause).count(),
            1,
            "one press of the pause button asked for {} pauses, which toggles the run back to \
             where it was",
            pressed.iter().filter(|ask| **ask == Ask::Pause).count()
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
