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
    /// From the organism's genome fingerprint rather than from its founder, which is Group A's
    /// decision and SPEC section 12's *"hue drifts as the lineage drifts genetically"*: two
    /// lineages that converge on the same genome look alike, and one that splits comes apart
    /// on screen.
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
    }
}

/// The hue of a lineage running this genome.
///
/// The top sixteen bits of Group A's FNV-1a fingerprint, spread over the wheel. Sixteen bits
/// is far more colour than an eye can tell apart, and taking the *top* of the hash rather than
/// the bottom matters because FNV-1a's avalanche is weakest in its low bits - two genomes
/// differing by one gene would come out adjacent on the wheel, which is exactly the difference
/// speciation has to be visible through.
#[must_use]
pub fn hue_of(genome_hash: u64) -> f32 {
    let top = u16::try_from(genome_hash >> 48).expect("the top sixteen bits of a u64 are a u16");

    f32::from(top) / (f32::from(u16::MAX) + 1.0)
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

/// Everything to be drawn this frame, and how big the world it is in is.
///
/// The world's size is carried beside the cells because the camera needs it and because the
/// seam is at its width - see `cells.wgsl`, where a cell near one edge is drawn at the other
/// as well.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// One per living cell, in whatever order the simulation packed them.
    pub cells: Vec<Instance>,

    /// How wide the world is, in world units. The seam is here.
    pub width: f32,

    /// How deep the world is, in world units.
    pub height: f32,
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
                    .map_or(LOST_HUE, |organism| hue_of(organism.genome_hash())),
                energy_flow: cell.energy_flow,
                kind: kind_number(cell.kind),
            })
            .collect();

        Self {
            cells,
            width: world.config().world.width,
            height: world.config().world.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Instance, Scene, hue_of, kind_number};
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
            [0, 1, 2, 3, 4, 5],
            "the six kinds are not numbered in SPEC section 6's order"
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

        for (drawn, cell) in scene.cells.iter().zip(world.living_cells()) {
            assert_eq!(drawn.position, [cell.pos.x, cell.pos.y]);
            assert_eq!(drawn.radius, cell.radius);
            assert_eq!(drawn.energy_flow, cell.energy_flow);
            assert_eq!(drawn.kind, kind_number(cell.kind));
            assert_eq!(
                drawn.hue,
                hue_of(hash),
                "a cell's colour does not come from its lineage's genome"
            );
        }
    }

    /// A hue is somewhere on the wheel, and two different genomes are somewhere different.
    ///
    /// The second claim is the one worth making. A fingerprint mapped through the *bottom* of
    /// the hash would put two genomes differing by a single gene next to one another, and
    /// SPEC section 12 wants speciation to be visible as it happens - which means a lineage
    /// splitting has to come apart on screen rather than shading into itself.
    #[test]
    fn a_hue_is_on_the_wheel_and_a_different_genome_is_somewhere_else() {
        for hash in [0, 1, u64::MAX, 0x8000_0000_0000_0000, 12_345_678_901] {
            let hue = hue_of(hash);
            assert!(
                (0.0..1.0).contains(&hue),
                "a hash of {hash} produced a hue of {hue}, which is not on the wheel"
            );
        }

        let apart = hue_of(0x1234_0000_0000_0000) - hue_of(0xfedc_0000_0000_0000);
        assert!(
            apart.abs() > 0.5,
            "two very different fingerprints came out {apart} apart on the wheel"
        );
    }
}
