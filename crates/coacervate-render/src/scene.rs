//! What the card is given: one record per cell, and nothing else.
//!
//! SPEC section 12 names the five things an instance carries - *"position, radius, hue, energy
//! flow, kind"* - and [`Instance`] is those five and no more. It is the whole of what the
//! renderer knows about a cell. There is no organism here, no genome, no spring: a body is a
//! group of cells that happen to be near one another, and the fragment shader is what makes
//! that group read as one animal.
//!
//! # The dependency goes one way
//!
//! This module reads `coacervate-sim` and `coacervate-sim` has never heard of it. CLAUDE.md:
//! *"The simulation crate must not know that rendering exists."* Group A's accessors -
//! [`World::living_cells`], [`World::living_cell_owners`] - are readings, not hooks, and this
//! is the only place in the project that turns them into something a graphics card wants.
//!
//! [`World::living_cells`]: coacervate_sim::world::World::living_cells
//! [`World::living_cell_owners`]: coacervate_sim::world::World::living_cell_owners

use bytemuck::{Pod, Zeroable};
use coacervate_sim::cell::CellKind;
use coacervate_sim::world::World;

/// One cell, as the graphics card sees it.
///
/// `repr(C)` because the field order *is* the vertex layout - see [`Instance::LAYOUT`], whose
/// offsets are written against these fields and checked against them by the tests. `Pod` is
/// what lets the whole array become bytes without a pointer cast, which CLAUDE.md's
/// `#![forbid(unsafe_code)]` would not allow this crate to write.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Instance {
    /// Where the cell is, in world units. `x` is always inside the world's width - the
    /// physics keeps it there - and the seam is the camera's problem rather than this one's.
    pub position: [f32; 2],

    /// How wide the cell is, in world units. Not how far its light reaches: see
    /// [`crate::camera::GLOW`].
    pub radius: f32,

    /// Its lineage's colour, as a fraction of the way round the wheel.
    ///
    /// ⭐ **This is the organism's lineage marker, read as a place on the colour wheel, and this
    /// line is the only place in the project where that reading happens.** `coacervate-sim` has
    /// never heard of colour: what it carries is a number a lineage inherits from its parent and
    /// shifts a little when the genome changes - see `organism.rs`'s `marker`.
    ///
    /// It was built the other way first. Group B took the hue from the genome fingerprint, which
    /// SPEC section 12's *"hue drifts as the lineage drifts genetically"* seemed to ask for, and
    /// the frame showed what a hash actually gives: **a hash does not drift, it jumps.** Any
    /// mutation at all rerolls every bit of it, so a child was an unrelated colour from its
    /// mother and a colony came out as confetti - cyan, magenta and orange side by side with no
    /// pattern in it. SPEC section 11 now carries the correction and says why the two clauses
    /// could not both hold. `docs/frames/phase5-groupb.png` is the frame.
    pub hue: f32,

    /// What the cell gained or lost on the last tick it took.
    pub energy_flow: f32,

    /// Which of SPEC section 6's six kinds it is, numbered in that section's order.
    ///
    /// A number rather than the enumeration because a graphics card has no enumerations.
    /// [`kind_number`] is the one place the two are tied together.
    pub kind: u32,
}

impl Instance {
    /// Where the five things are, for the card to find them.
    ///
    /// The offsets are worked out by `wgpu`'s own macro from the formats, in order, which is
    /// exactly what makes `cells_are_drawn_in_one_instanced_call` worth writing: it compares
    /// these against Rust's own field offsets, so a field inserted in the middle of the record
    /// without a matching entry here is caught rather than quietly shifting every attribute
    /// after it.
    const ATTRIBUTES: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32,
        2 => Float32,
        3 => Float32,
        4 => Uint32,
    ];

    /// How the card walks the instance buffer.
    ///
    /// One step per *instance* rather than per vertex, which is the whole of SPEC section 12's
    /// "one instanced draw call": every vertex of the twelve a cell is drawn with reads the
    /// same record, and the twelve are drawn again for the next cell without the host being
    /// asked anything.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: u64::try_from(size_of::<Self>())
                .expect("an instance record is a few dozen bytes"),
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// Which number each of SPEC section 6's kinds is drawn as.
///
/// Written out rather than derived, so that a kind added to the middle of `CellKind::ALL` -
/// which Phase 3 forbids for its own reasons - could not silently repaint every existing cell.
#[must_use]
pub const fn kind_number(kind: CellKind) -> u32 {
    match kind {
        CellKind::Photocyte => 0,
        CellKind::Devorocyte => 1,
        CellKind::Myocyte => 2,
        CellKind::Sclerocyte => 3,
        CellKind::Sensocyte => 4,
        CellKind::Gonocyte => 5,
        CellKind::Flagellocyte => 6,
    }
}

/// One grain of marine snow, as the graphics card sees it.
///
/// ⭐ **SPEC section 12: marine snow *"which is the actual detritus, not decoration"*.** Every
/// one of these is a real grain out of [`World::drift`], at the position the body that made it
/// died at, holding what the ledger's `detritus` account says it holds. Nothing here is
/// scattered by a random number and nothing is made up to fill the water: a world where nothing
/// has died yet has no snow in it at all, and a world that has just lost a colony is thick with
/// it right where the colony was.
///
/// [`World::drift`]: coacervate_sim::world::World::drift
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Grain {
    /// Where it is, in world units.
    pub position: [f32; 2],

    /// How much energy it still holds, which is what decides how big and how bright it is
    /// drawn. A grain rots away over its fall, so the oldest snow is the faintest.
    pub energy: f32,
}

impl Grain {
    /// Where the two things are, for the card to find them.
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32,
    ];

    /// How the card walks the grain buffer: one step per grain.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: u64::try_from(size_of::<Self>()).expect("a grain is twelve bytes"),
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

/// The hue of a cell whose organism is no longer in the slot it names.
///
/// ⚠️ Group A's `Q19`: [`World::living_cells`] is the population of the tick just *taken*, so a
/// body that died during that tick is still in the list and the slot it names may now be empty
/// or hold a stranger. There is nothing to look its lineage up in, and refusing to draw it
/// would make a corpse blink out a tick before it should.
///
/// [`World::living_cells`]: coacervate_sim::world::World::living_cells
const LOST_HUE: f32 = 0.0;

/// How many ticks pass between two steps of the light shafts' drift.
///
/// ⚠️ **The shafts are driven by the world's own clock and not by a wall clock**, and that is
/// CLAUDE.md's *"visually calm"* worked out rather than a convenience. A background animated on
/// real time keeps moving when the simulation is paused, moves at a different speed on a
/// different machine, and puts something on the screen that is not about the world. Driven by
/// ticks, the shafts stop when the world stops, and a frame dumped from a given tick is the same
/// picture every time it is dumped.
///
/// Thirty-two ticks a step, over 65,536 steps, is a full turn every **2,097,152 ticks** - about
/// two billion years of SPEC section 3's deep time, and about fifty-four minutes of watching. At
/// the six hundred and fifty ticks a second a watched run manages, the shafts cross under half a
/// pixel a second, which `frame.rs`'s
/// `the_background_is_a_depth_gradient_with_light_shafts` turns into the strongest form of
/// "barely perceptible" a picture can be held to: **a second of watching changes no pixel of the
/// frame by more than one byte.**
const TICKS_PER_SHAFT_STEP: u64 = 32;

/// How many steps there are in a full turn of the shafts.
///
/// A `u16`'s worth, so the conversion to a number the shader can use is exact rather than a cast.
const SHAFT_STEPS: u64 = 65_536;

/// Where the light shafts have drifted to, as a fraction of one full turn.
#[must_use]
pub fn shaft_phase(ticks: u64) -> f32 {
    let step = u16::try_from((ticks / TICKS_PER_SHAFT_STEP) % SHAFT_STEPS)
        .expect("a remainder modulo 65,536 is a u16");

    f32::from(step) / 65_536.0
}

/// Everything to be drawn this frame, and how big the world it is in is.
///
/// The world's size is carried beside the cells because the camera needs it and because the
/// seam is at its width - see `cells.wgsl`, where a cell near one edge is drawn at the other
/// as well.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// One per living cell, in whatever order the simulation packed them.
    pub cells: Vec<Instance>,

    /// One per grain of the drift. See [`Grain`].
    pub snow: Vec<Grain>,

    /// How wide the world is, in world units. The seam is here.
    pub width: f32,

    /// How deep the world is, in world units.
    pub height: f32,

    /// Where the light shafts have drifted to. See [`shaft_phase`].
    pub phase: f32,
}

impl Scene {
    /// Read a world.
    ///
    /// One instance per living cell and nothing for the empty slots, which is the whole reason
    /// Group A added [`World::living_cells`]: at SPEC section 3's defaults the cell arena is
    /// 256,000 records of which a few thousand belong to anybody, and anything that walked the
    /// arena would draw a quarter of a million dots piled at the origin.
    ///
    /// [`World::living_cells`]: coacervate_sim::world::World::living_cells
    #[must_use]
    pub fn of(world: &World) -> Self {
        let organisms = world.organisms();
        let owners = world.living_cell_owners();

        let cells = world
            .living_cells()
            .iter()
            .zip(owners)
            .map(|(cell, &slot)| Instance {
                position: [cell.pos.x, cell.pos.y],
                radius: cell.radius,
                hue: organisms[slot]
                    .as_ref()
                    .map_or(LOST_HUE, coacervate_sim::organism::Organism::marker),
                energy_flow: cell.energy_flow,
                kind: kind_number(cell.kind),
            })
            .collect();

        let snow = world
            .drift()
            .iter()
            .map(|grain| Grain {
                position: [grain.pos.x, grain.pos.y],
                energy: narrowed(grain.energy),
            })
            .collect();

        Self {
            cells,
            snow,
            width: world.config().world.width,
            height: world.config().world.height,
            phase: shaft_phase(world.ticks()),
        }
    }
}

/// A quantity the simulation counts at full precision, as a number a graphics card wants.
///
/// The same conversion, written out for the same reason, as `organism.rs`'s: the standard
/// library offers no `TryFrom` between the two widths, so a project that forbids lossy casts
/// says out loud what it is giving up. A grain's energy is a display quantity here - it decides
/// how big and how bright a speck is drawn - and the digits it loses are far below anything a
/// frame could show.
fn narrowed(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a grain's energy is a handful of units and is being turned into the size of a \
                  speck a pixel or two across; the lost digits cannot reach the frame"
    )]
    let narrow = value as f32;
    narrow
}

#[cfg(test)]
mod tests {
    use super::{Instance, SHAFT_STEPS, Scene, TICKS_PER_SHAFT_STEP, kind_number, shaft_phase};
    use coacervate_sim::cell::{CellKind, Vec2};
    use coacervate_sim::config::spec_defaults;
    use coacervate_sim::genome::{Action, Gene, Genome, SensorTarget, State};
    use coacervate_sim::world::World;

    /// How long the light is left to fall before a body is put in the water.
    ///
    /// Enough for the tiles under one body to hold what it is seeded with, at the brightened
    /// influx below. `founding.rs` does this properly, watching the field until it stops
    /// filling; this test only needs water that is not empty.
    const DAWN: u32 = 200;

    /// The five fields SPEC section 12 names are the five fields there are, they are laid out
    /// exactly as the vertex layout claims, and they come out of the world holding what the
    /// world holds.
    ///
    /// The layout half of this is the part that fails silently. A vertex attribute whose offset
    /// is a few bytes out does not crash and does not warn: the shader reads a radius out of
    /// the middle of a position and every cell in the world is drawn the wrong size, which
    /// looks like a bug in the maths.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "these are values copied out of the simulation unchanged, so anything but an \
                  exact match means a field was transformed on its way through"
    )]
    fn an_instance_carries_what_spec_section_12_lists() {
        assert_eq!(
            size_of::<Instance>(),
            24,
            "the instance record has changed size, so cells.wgsl's vertex layout is describing \
             a struct that no longer exists"
        );

        // SPEC section 6's order, which is also `CellKind::ALL`'s and is what Phase 3's
        // mutation draws over. A kind renumbered here repaints every cell in the world.
        assert_eq!(
            CellKind::ALL.map(kind_number),
            [0, 1, 2, 3, 4, 5, 6],
            "the kinds are not numbered in SPEC section 6's order"
        );

        // A world with one body in it, read.
        //
        // The light is turned right up, and only so that this test does not have to sit
        // through `founding.rs`'s nine-thousand-tick dawn to put a body in water that has
        // anything in it. Nothing here depends on the number: what is being checked is that
        // five values come out of the simulation unchanged.
        let mut raw = spec_defaults();
        raw.light.influx = 0.5;
        let config = raw
            .validate()
            .expect("SPEC section 3's defaults with a brighter light are still a world");
        let mut world = World::new(&config);
        for _ in 0..DAWN {
            world.tick();
        }
        let genome = Genome::new(
            vec![Gene {
                trigger_state: State::ZERO,
                min_step: 0,
                max_step: 0,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(1),
                child_kind: CellKind::Gonocyte,
                rest_length: 8.0,
                stiffness: 10.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            }],
            &config.limits,
        );
        let hash = genome.hash();
        let slot = world
            .seed(genome, Vec2::new(100.0, 200.0), 2.0)
            .expect("a lit world has room and water for one body");

        assert!(
            Scene::of(&world).cells.is_empty(),
            "a world that has not taken a tick since the body was put in it has no gathered \
             crowd to draw, and something invented one"
        );

        world.tick();
        let scene = Scene::of(&world);

        assert_eq!(scene.width, config.world.width);
        assert_eq!(scene.height, config.world.height);
        assert_eq!(
            scene.cells.len(),
            world.cells_of(slot).len(),
            "the scene does not hold one instance per living cell"
        );

        let marker = world.organisms()[slot]
            .as_ref()
            .expect("the organism just seeded is in its slot")
            .marker();
        assert_ne!(
            marker,
            hue_of_the_fingerprint(hash),
            "this body's lineage marker happens to be exactly where its genome fingerprint would \
             have put it, so the assertion below cannot tell the two apart"
        );

        for (drawn, cell) in scene.cells.iter().zip(world.living_cells()) {
            assert_eq!(drawn.position, [cell.pos.x, cell.pos.y]);
            assert_eq!(drawn.radius, cell.radius);
            assert_eq!(drawn.energy_flow, cell.energy_flow);
            assert_eq!(drawn.kind, kind_number(cell.kind));
            assert_eq!(
                drawn.hue, marker,
                "a cell's colour does not come from its lineage's inherited marker"
            );
        }
    }

    /// Group B's hue: the top sixteen bits of the genome fingerprint, spread over the wheel.
    ///
    /// Kept only so that the tests here can say what they are *rejecting*. Nothing outside this
    /// module computes a colour this way any more, and SPEC section 11 records why.
    fn hue_of_the_fingerprint(genome_hash: u64) -> f32 {
        let top =
            u16::try_from(genome_hash >> 48).expect("the top sixteen bits of a u64 are a u16");

        f32::from(top) / (f32::from(u16::MAX) + 1.0)
    }

    /// ⭐⭐ **D5, end to end.** Colour tells you which colony a body belongs to.
    ///
    /// `coacervate-sim`'s `a_lineage_marker_is_inherited_and_drifts_with_the_genome` is the
    /// measurement of the number itself - siblings close, strangers far. This is the claim on
    /// the other side of the wall, and it is the one a person looking at a frame would make:
    /// **every body in the world is drawn near the colour of the founder it descends from.**
    ///
    /// Two founders are seeded half the wheel apart and the world is left to breed. Under Group
    /// B's fingerprint the hues would be spread evenly over the whole wheel - a genome
    /// fingerprint knows nothing about descent, so a body is as likely to be drawn opposite its
    /// founder as beside it, and roughly two bodies in five would land outside the band below.
    /// Under an inherited marker every one of them is inside it.
    ///
    /// The second half is the one that stops this from being a test that colour is *constant*: a
    /// population that had not drifted at all would sit exactly on the two founders' hues, and
    /// there would be nothing to see a lineage splitting by. So the spread within a colony has to
    /// be real as well - measurably more than nothing.
    #[test]
    fn hue_comes_from_lineage_and_drifts_with_it() {
        use coacervate_sim::organism::founding_marker;

        let mut raw = spec_defaults();
        raw.world.width = 256.0;
        raw.world.height = 144.0;
        raw.world.grid_cols = 32;
        raw.world.grid_rows = 18;
        raw.light.cap = 80.0;
        raw.light.influx = 0.4;
        raw.limits.max_organisms = 128;
        raw.limits.max_cells_per_organism = 4;
        // Turned up for the same reason `world.rs`'s own measurement turns them up: at the
        // shipped rates a few thousand ticks of this small a world produce near-identical
        // clones, and a population that never diverged cannot show whether colour follows
        // descent or merely follows the genome.
        raw.mutation.point_rate = 0.30;
        raw.mutation.duplication_rate = 0.05;
        raw.mutation.deletion_rate = 0.05;
        let config = raw
            .validate()
            .expect("a small bright world with lively mutation is still a world");

        let mut world = World::new(&config);
        for _ in 0..300 {
            world.tick();
        }

        let breeder = Genome::new(
            vec![Gene {
                trigger_state: State::ZERO,
                min_step: 0,
                max_step: 0,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(1),
                child_kind: CellKind::Gonocyte,
                rest_length: 8.0,
                stiffness: 10.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            }],
            &config.limits,
        );

        let mut founders = Vec::new();
        for at in [Vec2::new(64.0, 72.0), Vec2::new(192.0, 72.0)] {
            let slot = world
                .seed(breeder.clone(), at, 3.0)
                .expect("a lit world has room and water for two founders");
            founders.push(
                world.organisms()[slot]
                    .as_ref()
                    .expect("the organism just seeded is in its slot")
                    .marker(),
            );
        }
        assert_eq!(
            founders,
            vec![founding_marker(0), founding_marker(1)],
            "the two founders were not given the spaced markers a founder gets"
        );

        for _ in 0..3_000 {
            world.tick();
        }

        // Every drawn cell, against the colony it came from.
        let scene = Scene::of(&world);
        assert!(
            scene.cells.len() > 40,
            "only {} cells are alive, so this world has nothing to be coloured",
            scene.cells.len()
        );

        let apart = |one: f32, other: f32| {
            let gap = (one - other).abs();
            gap.min(1.0 - gap)
        };

        let mut furthest = 0.0_f32;
        let mut spread = 0.0_f32;
        for drawn in &scene.cells {
            let home = apart(drawn.hue, founders[0]).min(apart(drawn.hue, founders[1]));
            furthest = furthest.max(home);
            spread += home;
        }

        // ⚠️ **Phase 7 moved this band from 0.15 to 0.2, and the reason is worth writing down
        // rather than the number being quietly changed.** `genome.rs`'s `MAX_SENSOR_GAIN` went
        // from 1 to 8, and at the tenfold point rate this test runs at, a gain used to spend
        // most of its life pressed against its own clamp - where a point mutation changes
        // nothing whatever, so `divergence_from` sees an identical gene and the marker does not
        // move at all. Off the clamp the same mutations are real ones, and lineages drift
        // faster. Measured: the furthest a body was drawn from its founder went from inside
        // 0.15 to **0.1512**.
        //
        // The band is still doing its job, and the assertion after it is what says so rather
        // than a number chosen to be comfortable: the counterfactual is *measured*, on this
        // very population, by colouring it Group B's way instead.
        assert!(
            furthest < 0.2,
            "a body is drawn {furthest} round the wheel from the founder it descends from. \
             Colour is supposed to say which colony something belongs to, and at that distance \
             it no longer does - which is exactly what a hue taken from the genome fingerprint \
             gives, because a fingerprint knows nothing about descent"
        );

        // ⭐ The same bodies, coloured from their genome fingerprints. A fingerprint knows
        // nothing about descent, so this is what the band above would be measuring if the
        // marker were not inherited.
        let mut by_fingerprint = 0.0_f32;
        for organism in world.organisms().iter().flatten() {
            let hue = hue_of_the_fingerprint(organism.genome_hash());
            by_fingerprint =
                by_fingerprint.max(apart(hue, founders[0]).min(apart(hue, founders[1])));
        }

        assert!(
            by_fingerprint > furthest * 1.5,
            "the same population coloured from its genome fingerprint puts a body \
             {by_fingerprint} round the wheel from its founder, against the inherited marker's \
             {furthest}. Those are close enough that this test cannot tell descent from a \
             hash, and the band above is measuring nothing"
        );
        assert!(
            spread > 0.0,
            "not one body in this world has moved from its founder's colour, so a lineage that \
             split would still be drawn as one thing and speciation would be invisible"
        );
    }

    /// ⭐ **D4.** The marine snow is the drift, grain for grain.
    ///
    /// SPEC section 12 asks for *"marine snow — which is the actual detritus, not decoration"*,
    /// and the difference between the two is entirely testable: decoration would be there in a
    /// world where nothing has died, and would not move when the drift does. So a world with no
    /// deaths in it has no snow, and once something has died every grain in the scene is a grain
    /// of `World::drift` at its own position holding its own energy.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a grain's position is copied out of the simulation unchanged, so anything but \
                  an exact match means the snow is being put somewhere other than where the body \
                  died - which is the whole difference between detritus and decoration"
    )]
    fn marine_snow_is_the_actual_detritus() {
        let mut raw = spec_defaults();
        raw.world.width = 256.0;
        raw.world.height = 144.0;
        raw.world.grid_cols = 32;
        raw.world.grid_rows = 18;
        raw.light.influx = 0.5;
        raw.limits.max_organisms = 32;
        raw.limits.max_cells_per_organism = 4;
        let config = raw
            .validate()
            .expect("SPEC section 3's defaults over a small bright world are still a world");

        let mut world = World::new(&config);
        for _ in 0..DAWN {
            world.tick();
        }

        assert!(
            Scene::of(&world).snow.is_empty(),
            "a world in which nothing has ever died is full of marine snow, so the snow is \
             decoration rather than the drift"
        );

        // A single photocyte with no gonocyte: it cannot breed and it earns more than it spends,
        // so what ends it is age - and `world.rs`'s `the_drift_can_be_read` records why that is
        // the death to wait for rather than a starvation. **A body that starves dies holding
        // nothing and leaves nothing behind**; a body that dies of old age dies rich, and its
        // richness is what the grains are made of.
        world
            .seed(
                Genome::new(Vec::new(), &config.limits),
                Vec2::new(128.0, 72.0),
                0.02,
            )
            .expect("a lit world has room for one body");

        let mut ticks = 0;
        while world.drift().is_empty() && ticks < 8_000 {
            world.tick();
            ticks += 1;
        }

        let scene = Scene::of(&world);
        assert_eq!(
            scene.snow.len(),
            world.drift().len(),
            "the scene does not hold one grain per grain of the drift"
        );
        assert!(
            !scene.snow.is_empty(),
            "a body has died in this world and left no drift behind it, so this test measured \
             nothing"
        );

        for (drawn, grain) in scene.snow.iter().zip(world.drift()) {
            assert_eq!(drawn.position, [grain.pos.x, grain.pos.y]);
            // Within a millionth of itself rather than exactly: the simulation counts energy at
            // twice the width a graphics card wants - `ledger.rs` argues why - so the conversion
            // gives up digits, and how many it gives up depends on how large the number is.
            assert!(
                (f64::from(drawn.energy) - grain.energy).abs() < grain.energy.abs() * 1e-6 + 1e-9,
                "a grain of snow holding {} is drawn for a grain of drift holding {}",
                drawn.energy,
                grain.energy
            );
        }
    }

    /// The light shafts drift, they drift by an amount nobody could see happen, and they are
    /// driven by the world's clock rather than by a wall clock.
    ///
    /// CLAUDE.md's *"visually calm"* is the whole subject here. SPEC section 12 asks for
    /// *"slowly drifting light shafts"* and CLAUDE.md forbids *"anything that pulls the eye"*, so
    /// the question is only how slow. One step is a 65,536th of a turn and a step takes
    /// thirty-two ticks: at the six hundred and fifty ticks a second a watched run manages, the
    /// shafts move under half a pixel a second.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the phase is a whole number of steps out of 65,536, so every value here is \
                  exact in binary - and a tolerance would hide shafts that had failed to come \
                  back round after a full turn, which is the thing that would show as a jump"
    )]
    fn the_light_shafts_drift_at_the_speed_of_the_world() {
        assert_eq!(shaft_phase(0), 0.0);
        assert_eq!(
            shaft_phase(TICKS_PER_SHAFT_STEP),
            1.0 / 65_536.0,
            "one step of the shafts is not one step of the shafts"
        );
        assert_eq!(
            shaft_phase(TICKS_PER_SHAFT_STEP * SHAFT_STEPS),
            0.0,
            "a full turn of the shafts does not come back to where it started"
        );

        // Nothing perceptible happens in a second of watching. Six hundred and fifty ticks is
        // about what a watched run takes in one second - see `docs/PHASE5.md`'s Group C.
        let in_a_second = shaft_phase(650) - shaft_phase(0);
        assert!(
            in_a_second > 0.0 && in_a_second < 0.001,
            "the shafts move {in_a_second} of a turn in a second of watching, which is either \
             nothing at all or enough to see"
        );

        // And they are the same at the same tick, whenever that tick is drawn.
        assert_eq!(shaft_phase(30_000), shaft_phase(30_000));
    }
}
