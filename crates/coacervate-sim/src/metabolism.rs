//! What being alive costs, and what is left when it cannot be paid.
//!
//! See `SPEC.md` section 10 for the life cycle this completes.
//!
//! `behaviour.rs` is the income side of the ledger: cells harvest the light, eat one another
//! and work their muscles. This is the other side. Every cell in the world pays for itself
//! every tick whether or not it does anything, an organism that cannot pay dies, an organism
//! that has been alive long enough dies anyway, and what is left of either sinks through the
//! water and rots back into it.
//!
//! Between them the two files close the loop the whole project turns on. Before this one
//! there was no way to lose, so there was nothing for selection to select against.
//!
//! # Death is where energy conservation is easiest to get wrong
//!
//! Four of the traps are worth naming before the code, because none of them announces itself.
//!
//! **An organism can end a tick holding less than nothing.** `Organism::lose` deliberately
//! does not refuse it and SPEC section 5 says why: the invariant claims energy is conserved,
//! not that any account is solvent. So a dying body's energy is *clamped* before any of it is
//! moved into `detritus` - a negative movement is refused outright by `ledger.rs`, and would
//! stop the run.
//!
//! **The clamping leaves a difference, and the difference has to go somewhere.** An organism
//! that died holding minus a hundredth spent a hundredth it never had, and `dissipated` was
//! credited with it. Clamp and walk away and that hundredth sits in `biomass` for the rest of
//! the run; do it on every death of a long run and the living-biomass figure ends up large
//! and negative while every organism in the world is holding something positive. It is undone
//! instead, at the moment of death, through [`Ledger::write_off`].
//!
//! **A corpse cannot carry more than the organism was holding.** SPEC section 10 says a body
//! becomes "detritus particles at each cell's position, carrying that cell's construction
//! energy", and construction energy is a property of a *cell kind* rather than of what the
//! organism had in its pocket. Those two numbers are not the same and only one of them
//! conserves energy, so construction energy decides **how the corpse is shared out between
//! its cells**, and what there is to share is what the organism actually held. See
//! [`Metabolism::scatter`].
//!
//! **And a grain never quite finishes rotting.** A tile is 32 bits wide, so once a grain is
//! down to its last ten-millionth the water rounds the whole offer away and the grain keeps
//! it, for ever, in an arena that cannot grow. Measured before it was fixed: eight corpses
//! left eight grains that were still in the water 118,000 ticks later. See
//! [`Metabolism::rot`].
//!
//! # Everything here walks slots, never the raw arena
//!
//! `organism.rs` gives every organism a fixed stretch of the world's cell array, and the
//! stretches nobody lives in still hold cells - left over from whoever was there before, or
//! never written to at all. A pass that walked the array would charge upkeep for them.
//!
//! # And it sweeps those slots in index order, which is load-bearing
//!
//! See [`Metabolism::reap`]. The short version: the order deaths are reaped in decides the
//! order slots return to the free list, which decides which slot the next birth takes, which
//! decides where in the crowd that body's cells sit, which decides the order the physics sums
//! its forces in - and floating-point addition is not associative, so it decides the last bit
//! of every position in the world. Nothing announces a change of that order. The run simply
//! stops matching its own recording.

use crate::behaviour::Detritus;
use crate::cell::Cell;
use crate::config::{Config, LimitsConfig};
use crate::grid::Grid;
use crate::ledger::Ledger;
use crate::organism::{Organism, cell_slot};
use crate::physics::DT;

/// How much a cell spends on merely existing before it has finished being alive.
///
/// ⭐ **SPEC section 10 gives death two causes - "energy reaches zero, or age exceeds a
/// genome-derived maximum" - and says nothing whatever about how the maximum is derived.
/// This is the derivation.**
///
/// An organism's allowance is `LIFETIME_UPKEEP` per cell, and it dies when it has been alive
/// long enough to have spent it: `max_age = LIFETIME_UPKEEP × cells ÷ what it costs per
/// tick`. A body of cheap tissue therefore lives a long time and a body of expensive tissue
/// burns through its allowance quickly.
///
/// # Why this rather than a number of ticks
///
/// A flat lifespan would be the obvious thing and it is not genome-derived at all - every
/// lineage would get the same one, and SPEC asks for a maximum the *genome* decides.
///
/// The tempting genome-derived answer is worse than obvious: a lifespan proportional to how
/// many genes a genome has. That makes a longer genome a longer life, which is a selection
/// pressure **towards** the gene cap - pulling directly against the per-gene cost this same
/// group adds for the opposite reason. Two of this file's numbers would spend the run arguing
/// with each other.
///
/// What a genome unambiguously decides is the **body**: SPEC section 7 makes development a
/// pure function of the genome, so what a body is made of is as genome-derived as anything in
/// the simulation gets. Keying the lifespan to what that body costs to run gives a real
/// trade-off rather than a free parameter - a lineage can buy itself a longer life by
/// building cheaper tissue, and cheap tissue is exactly the tissue that earns nothing. It is
/// also a claim biology has made about real animals for a century, under the name the
/// rate-of-living hypothesis, so it is at least the familiar shape of wrong.
///
/// # The number
///
/// Eight, which is what a photocyte at SPEC section 6's `0.004` a tick spends over **two
/// thousand ticks**. That is the shipped world's default lifespan for the plainest body there
/// is: a myocyte body, at 0.014, gets 571; a sclerocyte body, at 0.002, gets four thousand.
///
/// Two thousand ticks is a little over half a simulated minute, and - the figure that
/// actually decided it - somewhere between ten and twenty times how long a photosynthetic
/// body takes to accumulate what Group C will ask of it before it reproduces. A lineage
/// therefore gets a good many chances to breed before age takes it, while a world sitting at
/// its population cap still turns over completely within a few thousand ticks. Both halves
/// matter: with no old age at all, a full world is a world where nothing can be born, and
/// evolution stops the moment the last slot fills.
///
/// # Why it is not a configuration key
///
/// Because `upkeep_scale` already is one. Lifespan here is `LIFETIME_UPKEEP × cells ÷ (
/// upkeep_scale × ...)`, so the "temperature" slider SPEC section 3 already ships **is** the
/// lifespan slider, in the direction that reads correctly: a hotter world is one where
/// everything costs more and everything dies younger. A second key would be a second way to
/// say the same thing, and `config.rs`'s transcribed table would grow a number SPEC does not
/// have.
const LIFETIME_UPKEEP: f64 = 8.0;

/// How fast a grain of dead biomass falls, in world units per simulated second.
///
/// SPEC section 10 says detritus "sinks slowly" and gives no number, and SPEC section 12 wants
/// the same grains drawn as marine snow, so the figure has to be slow enough to read as
/// drifting and fast enough to be going somewhere.
///
/// Twelve units a second is a fifth of a unit a tick - a fifteenth of a cell's width - so
/// nothing is ever seen to fall. What it adds up to is the part that matters: a grain rots
/// away over something like a thousand ticks and covers a couple of hundred units on the way
/// down, which is a sixth of the world's depth. That is enough to be a **pump**. Energy that
/// dies near the surface is returned to the water some way below where it was taken from, and
/// the tiles down there have lower ceilings, so a good deal of what sinks arrives somewhere
/// that cannot hold it and leaves the world entirely through SPEC section 4's spill. Detritus
/// is therefore not a free second helping of the same energy; it is a slow leak downwards, and
/// that is exactly what the biological pump does in a real ocean.
const SINK_SPEED: f32 = 12.0;

/// What fraction of itself a grain rots into the water beneath it each tick.
///
/// Anchored against the one number it competes with. `behaviour.rs` lets a devorocyte take
/// `DEVOUR_RATE` - 0.05 - out of a grain it is touching, per tick; if rot were faster than a
/// mouth, everything worth eating would be gone before anything could reach it and half of
/// SPEC section 6's devorocyte would be a function that never pays. At 0.4% a tick a grain has
/// to be holding more than twelve units before it rots faster than one devorocyte drains it,
/// and an ordinary corpse holds a small fraction of that. **Scavenging is worth doing**, which
/// is the whole point of writing the rate down against that one rather than picking a round
/// number.
///
/// The consequence is a half-life of about 170 ticks and a grain that is all but gone within a
/// thousand - half a photocyte's life, so a corpse is a pulse in the water rather than a
/// permanent feature of it.
const DECAY_RATE: f64 = 0.004;

/// The least a grain gives up in a tick, and therefore the thing that makes it finite.
///
/// A strictly proportional decay never reaches nothing: a grain would go on halving for ever,
/// holding quantities too small to matter and occupying a place in an arena that cannot grow.
/// With a floor under it, a grain that has fallen below `DECAY_FLOOR ÷ DECAY_RATE` - a quarter
/// of a unit - stops halving and empties in equal steps instead, so **every grain has a last
/// tick**.
///
/// That is what bounds the drift. A grain holding as much as the richest tile in the world is
/// finished inside a couple of thousand ticks, whatever it started with, which is what makes
/// it possible to say how many grains can be in the water at once and to allocate for exactly
/// that. See `world.rs`.
const DECAY_FLOOR: f64 = 0.001;

/// What a body of these cells was worth to build, added up.
///
/// One place, called from two: this file shares a corpse out between its cells in this
/// proportion, and Group C's reproduction threshold is `reproduction_threshold ×` this. SPEC
/// section 10 uses the same phrase for both and they must not be allowed to become two
/// different sums.
///
/// The cells are the body's own - `World::cells_of` hands out exactly that slice - and never
/// the whole arena, which is mostly slots nobody lives in.
#[must_use]
pub fn construction_energy(cells: &[Cell]) -> f64 {
    cells
        .iter()
        .map(|cell| f64::from(cell.kind.construction()))
        .sum()
}

/// Everything the expense pass is allowed to touch.
///
/// Grouped into one argument rather than passed as four, for the same reason `behaviour.rs`
/// groups its own: four references threaded through a call is a place to get two of them the
/// wrong way round.
pub(crate) struct Mortal<'a> {
    /// The whole cell arena, read-only, walked by slot and never end to end.
    pub cells: &'a [Cell],

    /// Who is in each slot, and nothing at all in the slots that are free.
    pub organisms: &'a mut [Option<Organism>],

    /// The slots nobody is in. A death pushes onto this; `world.rs` pops from it at a birth.
    pub free: &'a mut Vec<usize>,

    /// The dead biomass on its way down.
    pub drift: &'a mut Vec<Detritus>,
}

/// The numbers a configuration decides about what living costs, and nothing else.
///
/// There is no working room here and no arena: the drift lives in `world.rs` beside the cells
/// and the springs, because the behaviour pass needs it in the same tick and an arena owned by
/// one pass and borrowed by another is two mutable borrows of one structure. What this type is
/// is a handful of settings copied out of the configuration once, so that the tick does not
/// reach into a `Config` in its innermost loop.
pub struct Metabolism {
    /// SPEC section 3's `metabolism.upkeep_scale`: the "temperature" every cost here is
    /// multiplied by.
    upkeep_scale: f64,

    /// SPEC section 3's `metabolism.gene_cost`: what one gene costs its organism per tick.
    /// See [`crate::config::MetabolismConfig::gene_cost`] for why it exists, which is not the
    /// reason it looks like.
    gene_cost: f64,

    /// The arena sizes, which is how a slot number is turned into a stretch of the cell
    /// array. Kept rather than reduced to a single width so that the arithmetic stays
    /// `organism.rs`'s, in one place, rather than being written out a second time here.
    limits: LimitsConfig,

    /// How deep the world is. SPEC section 8 closes it top and bottom, so this is where a
    /// sinking grain stops.
    floor: f32,

    /// How many grains the drift can hold, which is how many it will ever hold. `world.rs`
    /// allocates it; this is the copy that decides whether a corpse can be laid down.
    room: usize,
}

impl Metabolism {
    /// Build the expense side of a configuration.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            upkeep_scale: f64::from(config.metabolism.upkeep_scale),
            gene_cost: f64::from(config.metabolism.gene_cost),
            limits: config.limits.clone(),
            floor: config.world.height,
            room: crate::physics::cell_capacity(config),
        }
    }

    /// Charge everything alive for the tick it has just had, and take away whatever did not
    /// survive it.
    ///
    /// The drift is worked through *before* the reaping, which is not tidiness. Rotting is
    /// what takes grains out of the drift, so doing it first is what makes room for the
    /// corpses this tick produces - and it means a body that has only just died waits a tick
    /// before it begins to rot, which is the reading of SPEC section 10 that does not have a
    /// corpse decaying during the tick it was still alive for.
    pub(crate) fn run(&self, mortal: Mortal<'_>, grid: &mut Grid, ledger: &mut Ledger) {
        let Mortal {
            cells,
            organisms,
            free,
            drift,
        } = mortal;

        self.rot(drift, grid, ledger);
        self.reap(cells, organisms, free, drift, ledger);
    }

    /// Let every grain in the drift fall a little and give a little of itself back to the
    /// water.
    ///
    /// SPEC section 10's whole sentence about detritus, and SPEC section 5's `detritus →
    /// field`. See [`SINK_SPEED`], [`DECAY_RATE`] and [`DECAY_FLOOR`] for the three numbers.
    ///
    /// # "The tile beneath it" is the tile it is in
    ///
    /// A grain is *in* the water rather than resting on top of a column of it, so the tile
    /// beneath a grain is the tile it occupies - the same question a photocyte asks of its own
    /// position. It rots where it is and then falls, rather than the other way round, so a
    /// grain's last tick is spent in the tile it was in rather than in the one it was about to
    /// reach.
    ///
    /// # Grains keep their order, and empty grains are removed rather than blanked
    ///
    /// The drift is compacted in place with the order preserved: the surviving grains come out
    /// in the order they went in, and new corpses join the end. The alternative - swapping the
    /// last grain into the hole - is one instruction cheaper and permutes the drift, and the
    /// order grains are visited in decides which of two devorocytes reaches a grain first and
    /// the order a sensocyte's pulls are summed in. It is the same hazard as the reaping order
    /// below, one arena along.
    fn rot(&self, drift: &mut Vec<Detritus>, grid: &mut Grid, ledger: &mut Ledger) {
        let (floor, fall) = (self.floor, SINK_SPEED * DT);

        for grain in drift.iter_mut() {
            let tile = grid.tile_at(grain.pos);

            // A proportion of what it holds, but never less than the floor, and never more
            // than there is. The floor is what stops a grain halving for ever; see
            // `DECAY_FLOOR`.
            let wanted = (grain.energy * DECAY_RATE)
                .max(DECAY_FLOOR)
                .min(grain.energy);

            let given = grid.deposit(ledger, tile, wanted);
            grain.energy -= given;

            // ⚠️ **The last crumb of a grain is too small for the water to notice, and
            // without this it stays in the drift for ever.**
            //
            // A tile is a 32-bit number holding something of order one, so the smallest
            // change it can express is around a ten-millionth. A grain works its way down to
            // a residue of about that size - the leftovers of a thousand deposits, each
            // rounded to what the tile could take - and then offers it, and the tile rounds
            // the whole offer away. `Grid::deposit` correctly reports that nothing moved, so
            // the grain correctly keeps what it still has, and the two of them do that on
            // every tick until the run ends. Measured: eight corpses left eight grains that
            // were still in the water 118,000 ticks later.
            //
            // What happens to the crumb is what SPEC section 4 already says happens to energy
            // that arrives somewhere it cannot be held: it leaves the world. Written as the
            // two movements it actually is - the grain rots into the tile, and the tile
            // cannot hold it - so there is no new kind of event here and no new hole for
            // energy to go out through.
            //
            // The size condition is what keeps this to crumbs. A configuration is free to set
            // a ceiling of a million, where a tile cannot express a sixteenth of a unit and
            // no grain would ever rot at all; such a world fills its drift and refuses new
            // corpses, which is a bounded failure. It does not get to lose whole bodies here.
            if given <= 0.0 && grain.energy > 0.0 && grain.energy < DECAY_FLOOR {
                ledger.decay(grain.energy);
                ledger.overflow(grain.energy);
                grain.energy = 0.0;
            }

            grain.pos.y = (grain.pos.y + fall).min(floor);
        }

        // `retain` keeps what is left in the order it was in, and does not touch the
        // allocation - so the drift can shrink and be refilled for the life of a run without
        // ever asking the machine for memory.
        drift.retain(|grain| grain.energy > 0.0);
    }

    /// What one organism pays this tick simply for being in the world.
    ///
    /// SPEC section 10: *"Each tick, every cell pays `upkeep × upkeep_scale`."* The upkeep of
    /// a cell is SPEC section 6's table, which lives on [`crate::cell::CellKind`] beside the
    /// radius and the toughness, because a kind is a trade-off and a trade-off split across
    /// three files is one nobody can see.
    ///
    /// # And every gene, once for the organism rather than once per cell
    ///
    /// SPEC section 3's `gene_cost`. Why the cost exists at all is argued on
    /// [`crate::config::MetabolismConfig::gene_cost`] and it is not the reason it looks like;
    /// the decision *here* is narrower, and it is that the genome is charged for **once per
    /// organism**.
    ///
    /// A real cell carries its own copy of the genome, so charging per cell has the better
    /// biological story - and it would make the cost scale with body size, which is the wrong
    /// pressure entirely. A sixty-four-celled body would pay sixty-four times for the same
    /// program, so the cost would fall hardest on large bodies rather than on long genomes,
    /// and the thing it is meant to be selecting on would be buried under the thing it is not.
    /// A genome is a fixed overhead. It is charged as one.
    ///
    /// The consequence is worth knowing, because it is not neutral: a genome at the cap is
    /// three photocytes' worth of upkeep whether the body is one cell or sixty-four, so an
    /// elaborate program is proportionally hardest on the *smallest* bodies. That is the right
    /// way round - a single cell carrying a hundred and twenty-eight rules is carrying a
    /// program it has no body to run.
    fn due(&self, cells: &[Cell], slot: usize, organism: &Organism) -> f64 {
        let first = cell_slot(slot, &self.limits).start;
        let body = &cells[first..first + organism.cells()];

        let tissue: f64 = body.iter().map(|cell| f64::from(cell.kind.upkeep())).sum();

        let genes = f64::from(
            u32::try_from(organism.genome().genes().len()).expect("a genome cap fits in a word"),
        );

        self.upkeep_scale * self.gene_cost.mul_add(genes, tissue)
    }

    /// How many ticks an organism of this shape is allowed, given what it costs to run.
    ///
    /// See [`LIFETIME_UPKEEP`] for why the answer is derived from the body rather than
    /// written down.
    fn lifespan(&self, due: f64, cells: usize) -> u64 {
        let body = f64::from(u32::try_from(cells).expect("a body is not four billion cells"));

        // Rounded rather than truncated, and the difference shows up immediately. SPEC
        // section 6's upkeeps are written as decimals and stored at 32 bits, so a
        // sclerocyte's `0.002` is really 0.0020000000949, and eight units of it is
        // 3,999.9998 ticks rather than four thousand. Truncated, the tidiest number in the
        // table comes out one short of itself.
        let ticks = (LIFETIME_UPKEEP * body / due).round();

        // A run of a million million million ticks is five hundred million years at SPEC
        // section 2's sixty ticks a second, so the clamp is a formality that makes the
        // conversion total rather than a limit anybody will meet. It is here because
        // `due` can be made arbitrarily small by turning the temperature down, and a
        // lifespan that overflowed would be a lifespan of nothing at all.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is clamped between nought and a number far inside the range \
                      of the type it is becoming, so the conversion neither truncates nor \
                      loses a sign"
        )]
        let ticks = ticks.clamp(0.0, 1e18) as u64;

        ticks
    }

    /// Charge every organism for the tick, and take away the ones that did not come through
    /// it.
    ///
    /// **The slots are swept in index order and that is not a detail.** See this module's
    /// documentation.
    fn reap(
        &self,
        cells: &[Cell],
        organisms: &mut [Option<Organism>],
        free: &mut Vec<usize>,
        drift: &mut Vec<Detritus>,
        ledger: &mut Ledger,
    ) {
        // `enumerate` over the slots in order, which is the whole of the guarantee this
        // method's documentation is about: there is no version of this loop that visits the
        // slots in any other order, because there is no order in it to change.
        for (slot, entry) in organisms.iter_mut().enumerate() {
            let Some(organism) = entry.as_mut() else {
                continue;
            };

            let due = self.due(cells, slot, organism);
            ledger.spend(due);
            organism.lose(due);

            let starved = organism.energy() <= 0.0;
            let old = organism.age() >= self.lifespan(due, organism.cells());
            if !starved && !old {
                continue;
            }

            let held = organism.energy();
            let body = organism.cells();
            if held > 0.0 {
                self.scatter(cells, slot, body, held, drift, ledger);
            } else {
                // It died owing. See this module's documentation: the shortfall was spent
                // and never existed, so the spending is undone rather than left sitting in
                // `biomass` for the rest of the run.
                ledger.write_off(-held);
            }

            *entry = None;
            free.push(slot);
        }
    }

    /// Lay a corpse down: one grain at each of a dead body's cells, holding that cell's share
    /// of what the body was carrying.
    ///
    /// SPEC section 10 asks for "detritus particles at each cell's position, carrying that
    /// cell's construction energy", and the two halves of that sentence cannot both be taken
    /// literally - see this module's documentation. Construction energy decides the **shares**;
    /// what there is to share is `held`, which is everything the organism had left.
    ///
    /// The last grain is given the remainder rather than its own computed share, so the
    /// grains add up to exactly what left the `biomass` account rather than to within a
    /// rounding of it. A fraction adrift per death is a fraction adrift on every death of a
    /// run that lasts all night.
    ///
    /// # Each share is moved out of `biomass` as it is laid down
    ///
    /// Rather than the corpse being moved in one go and then divided up. It is one line
    /// longer and it is what makes the paragraph below possible: a share that has nowhere to
    /// go can be sent to a different account instead of having to be clawed back out of
    /// `detritus`, and there is no moment at which the books are holding a corpse that does
    /// not exist.
    ///
    /// # If the drift is full, the share is dissipated instead
    ///
    /// The same answer every other cap in this project gives: CLAUDE.md's *"a simulation that
    /// cannot allocate cannot leak"*, and births failing at the population cap rather than
    /// growing an arena. A world so thick with the dead that there is nowhere to put another
    /// grain is one where a corpse is lost rather than eaten, which is a defensible thing for
    /// such a world to do - and it is bounded rather than arbitrary, because the drift is
    /// built to hold **a grain for every cell the world can contain at once**.
    fn scatter(
        &self,
        cells: &[Cell],
        slot: usize,
        body: usize,
        held: f64,
        drift: &mut Vec<Detritus>,
        ledger: &mut Ledger,
    ) {
        let first = cell_slot(slot, &self.limits).start;
        let corpse = &cells[first..first + body];
        let worth = construction_energy(corpse);

        let mut left = held;
        for (index, cell) in corpse.iter().enumerate() {
            let last = index + 1 == corpse.len();
            let share = if last || worth <= 0.0 {
                left
            } else {
                held * f64::from(cell.kind.construction()) / worth
            };

            left -= share;
            if share <= 0.0 {
                continue;
            }

            if drift.len() < self.room {
                ledger.die(share);
                drift.push(Detritus {
                    pos: cell.pos,
                    energy: share,
                });
            } else {
                ledger.spend(share);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{CellKind, Vec2};
    use crate::config::{Config, RawConfig, spec_defaults};
    use crate::genome::{Action, Gene, Genome, SensorTarget, State};
    use crate::physics::cell_capacity;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// A small world to try one death in: some bodies in their slots, the water they stand
    /// in, and the books that watch them.
    ///
    /// Built by hand rather than through [`crate::world::World`], exactly as `behaviour.rs`
    /// builds its own scenes and for the same reason. Every test here is about an *amount* -
    /// what a body costs, how long it lasts, what is left of it - and growing a genome that
    /// produces a body of a known shape is a second puzzle standing between the test and what
    /// it is trying to say. `world.rs` is where the wiring is proved; this is where the
    /// arithmetic is.
    struct Scene {
        config: Config,
        grid: Grid,
        ledger: Ledger,
        cells: Vec<Cell>,
        organisms: Vec<Option<Organism>>,
        free: Vec<usize>,
        drift: Vec<Detritus>,
        metabolism: Metabolism,
    }

    impl Scene {
        /// An empty world: water, slots, nobody in them.
        fn new(config: &Config) -> Self {
            let grid = Grid::new(config);
            let ledger = Ledger::new(grid.total_energy());
            let slots = usize::try_from(config.limits.max_organisms.get())
                .expect("a population cap fits in a machine word");

            Self {
                config: config.clone(),
                grid,
                ledger,
                cells: vec![Cell::new(CellKind::Photocyte, Vec2::ZERO); cell_capacity(config)],
                organisms: vec![None; slots],
                free: (0..slots).rev().collect(),
                drift: Vec::with_capacity(cell_capacity(config)),
                metabolism: Metabolism::new(config),
            }
        }

        /// Let the light fall until the tiles are full, as a real run must before anything can
        /// be seeded into it. A world starts dark.
        fn fill_with_light(&mut self) {
            for _ in 0..1_000 {
                self.grid.tick(&mut self.ledger);
            }
        }

        /// Put a body in the next free slot, with its energy taken out of the field the way
        /// `World::seed` takes it, and hand back the slot.
        fn add(&mut self, energy: f64, body: &[(CellKind, Vec2)]) -> usize {
            self.add_carrying(energy, 0, body)
        }

        /// The same, for a body whose genome carries `genes` rules.
        ///
        /// What is in them does not matter here - nothing in this file reads a gene, it only
        /// counts them - so they are all the same blank rule. That is exactly the case the
        /// per-gene cost is about: SPEC section 7 says non-firing genes accumulate as neutral
        /// material and calls that a feature, and the question this file answers is what
        /// carrying them costs.
        fn add_carrying(&mut self, energy: f64, genes: usize, body: &[(CellKind, Vec2)]) -> usize {
            let slot = self.add_body(energy, body);
            let blank = Gene {
                trigger_state: State::ZERO,
                min_step: 0,
                max_step: 0,
                action: Action::Terminate,
                angle: 0.0,
                adhere: false,
                child_state: State::ZERO,
                child_kind: CellKind::Photocyte,
                rest_length: 0.0,
                stiffness: 0.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            };

            let organism = self.organisms[slot]
                .as_ref()
                .expect("the slot the body just went into holds it");
            let (energy, cells) = (organism.energy(), organism.cells());
            self.organisms[slot] = Some(Organism::new(
                Genome::new(vec![blank; genes], &self.config.limits),
                energy,
                u64::try_from(slot).expect("a slot number fits in a serial"),
                cells,
                0,
            ));

            slot
        }

        /// Put a body in the next free slot, with an empty genome.
        fn add_body(&mut self, energy: f64, body: &[(CellKind, Vec2)]) -> usize {
            let slot = self.free.pop().expect("this scene has a slot free");
            let first = cell_slot(slot, &self.config.limits).start;

            for (local, &(kind, at)) in body.iter().enumerate() {
                self.cells[first + local] = Cell::new(kind, at);
            }

            // Out of the water, through the same door a photocyte eats through, so that a
            // scene never holds energy the books were not told about. Starting under the body
            // and working along the field from there, because these tests hand out far more
            // than one tile can hold and a body standing on a hole in the water would be a
            // light gradient this test never meant to put there.
            let tiles = self.grid.cols() * self.grid.rows();
            let start = self.grid.tile_at(body[0].1);
            let mut taken = 0.0;
            for step in 0..tiles {
                if taken >= energy {
                    break;
                }
                taken +=
                    self.grid
                        .harvest(&mut self.ledger, (start + step) % tiles, energy - taken);
            }
            assert!(
                (taken - energy).abs() < 1e-6,
                "this scene's whole field holds {taken} and the body was asked for {energy}"
            );

            self.organisms[slot] = Some(Organism::new(
                Genome::new(Vec::new(), &self.config.limits),
                taken,
                u64::try_from(slot).expect("a slot number fits in a serial"),
                body.len(),
                0,
            ));

            slot
        }

        /// One tick of expense, and nothing else.
        fn run(&mut self) {
            self.metabolism.run(
                Mortal {
                    cells: &self.cells,
                    organisms: &mut self.organisms,
                    free: &mut self.free,
                    drift: &mut self.drift,
                },
                &mut self.grid,
                &mut self.ledger,
            );
        }

        /// Everybody gets a tick older, as `world.rs` ages them.
        fn age_everybody(&mut self) {
            for organism in self.organisms.iter_mut().flatten() {
                organism.grow_older();
            }
        }

        /// What the organism in this slot is holding, or nothing at all if it has died.
        fn energy(&self, slot: usize) -> Option<f64> {
            self.organisms[slot].as_ref().map(Organism::energy)
        }

        /// Put a grain of dead biomass in the water, holding energy that came out of the
        /// field the long way round - through a body and out the other side.
        ///
        /// The energy is taken from a far corner of the world rather than from under the
        /// grain, which is both what really happens - a corpse is made of what something ate
        /// somewhere else - and quietly necessary, since a test that dug a hole under every
        /// grain it dropped would be measuring the hole.
        fn drop_detritus(&mut self, pos: Vec2, energy: f64) -> f64 {
            let tile = self.grid.tile_at(Vec2::ZERO);
            let taken = self.grid.harvest(&mut self.ledger, tile, energy);
            assert!(
                (taken - energy).abs() < 1e-6,
                "this scene's corner tile holds {taken} and a grain asked for {energy}"
            );

            self.ledger.die(taken);
            self.drift.push(Detritus { pos, energy: taken });

            taken
        }

        /// Age everybody and charge them, over and over, until the organism in this slot is
        /// gone - and hand back what it was holding when its last tick began.
        fn live_out(&mut self, slot: usize, most: u64) -> (u64, f64) {
            for tick in 1..=most {
                let holding = self.energy(slot).expect("it has not died yet");
                self.age_everybody();
                self.run();

                if self.energy(slot).is_none() {
                    return (tick, holding);
                }
            }

            panic!("the organism in slot {slot} outlived the {most} ticks this test allows it");
        }

        /// Spend some of an organism's energy on nothing in particular, through the books, so
        /// a test can put a body wherever on the solvency line it needs it.
        fn drain(&mut self, slot: usize, amount: f64) {
            self.ledger.spend(amount);
            self.organisms[slot]
                .as_mut()
                .expect("this scene put an organism in that slot")
                .lose(amount);
        }
    }

    /// A world small enough to reason about, with four slots of four cells.
    fn a_small_world() -> Config {
        config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 4;
        })
    }

    /// ⭐ **B1.** Every cell in the world pays for itself every tick, whatever it is and
    /// whatever it did.
    ///
    /// SPEC section 10: *"Each tick, every cell pays `upkeep × upkeep_scale`."* This is the
    /// first outgoing payment in the project that is not conditional on doing something -
    /// `behaviour.rs` charges a myocyte for the work it did, and a muscle that does not move
    /// pays nothing. Upkeep is what a body pays for being a body.
    ///
    /// # The assertions are about the accounts, not about the books balancing
    ///
    /// SPEC section 5's lesson, which Phase 2 learned twice: a conservation check cannot see
    /// energy that was never declared. An implementation that took the energy off the organism
    /// and told nobody would balance its books perfectly for ever - all five accounts exactly
    /// as they were, and a body standing in the world holding less than the ledger thinks.
    /// So what is measured is that `dissipated` went **up** by what the organisms went
    /// **down** by, to the last part in a million million.
    ///
    /// # Why two different bodies
    ///
    /// Because a flat charge per organism, or per cell regardless of kind, would satisfy a
    /// test with one body in it - and it would quietly delete the whole of SPEC section 6's
    /// design intent, which is that specialising is a *trade*. A sclerocyte is the widest cell
    /// in the world and the cheapest to run; a myocyte is the most expensive. The two bodies
    /// here are the two ends of that table, and the charge has to tell them apart.
    ///
    /// # And why the temperature is turned up
    ///
    /// `upkeep_scale` is SPEC section 3's live environmental pressure and the one lever a
    /// person watching a run can pull to make it harder. A version of this that only ever ran
    /// at the shipped `1.0` would pass against an implementation that ignored the setting
    /// entirely, and the failure would look like a slider that does nothing.
    #[test]
    fn every_cell_pays_upkeep_every_tick() {
        let mut scene = Scene::new(&config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 4;
            raw.metabolism.upkeep_scale = 2.5;
        }));
        scene.fill_with_light();

        // SPEC section 6's cheapest tissue and its most expensive, side by side.
        let armoured = scene.add(
            20.0,
            &[
                (CellKind::Sclerocyte, Vec2::new(40.0, 40.0)),
                (CellKind::Sclerocyte, Vec2::new(48.0, 40.0)),
            ],
        );
        let muscular = scene.add(
            20.0,
            &[
                (CellKind::Myocyte, Vec2::new(160.0, 40.0)),
                (CellKind::Myocyte, Vec2::new(168.0, 40.0)),
            ],
        );

        let dissipated_before = scene.ledger.dissipated();
        let biomass_before = scene.ledger.biomass();

        scene.run();

        // SPEC section 6's table, multiplied out here from the specification rather than
        // asked of the code: two sclerocytes at 0.002 and two myocytes at 0.014, both at a
        // temperature of 2.5. The literals are widened from 32 bits first, because that is
        // where a cell's upkeep is stored and `0.014` is not a number either size can hold
        // exactly - so this is the specification's figure at the width the simulation keeps
        // it, rather than a fifteen-digit number the world could never produce.
        let hard = 2.0 * f64::from(0.002f32) * 2.5;
        let soft = 2.0 * f64::from(0.014f32) * 2.5;

        let armoured_paid = 20.0 - scene.energy(armoured).expect("nothing has died here");
        let muscular_paid = 20.0 - scene.energy(muscular).expect("nothing has died here");

        assert!(
            (armoured_paid - hard).abs() < 1e-9,
            "a body of two sclerocytes paid {armoured_paid} for a tick, against the {hard} \
             SPEC section 6's table asks for at a temperature of 2.5"
        );
        assert!(
            (muscular_paid - soft).abs() < 1e-9,
            "a body of two myocytes paid {muscular_paid} for a tick, against the {soft} SPEC \
             section 6's table asks for at a temperature of 2.5"
        );
        assert!(
            muscular_paid > armoured_paid * 6.0,
            "muscle costs {muscular_paid} a tick and armour costs {armoured_paid}, so \
             specialising is not the trade SPEC section 6's table describes"
        );

        // The books were told. Both halves: what left the organisms arrived in the one
        // account that records energy leaving the world, and nothing else moved.
        let spent = armoured_paid + muscular_paid;
        assert!(
            (scene.ledger.dissipated() - dissipated_before - spent).abs() < 1e-12,
            "the organisms paid {spent} between them and the dissipated account moved by {}",
            scene.ledger.dissipated() - dissipated_before
        );
        assert!(
            (biomass_before - scene.ledger.biomass() - spent).abs() < 1e-12,
            "the organisms paid {spent} between them and the biomass account moved by {}",
            biomass_before - scene.ledger.biomass()
        );
        assert!(
            scene.ledger.detritus() < f64::EPSILON,
            "paying upkeep left something dead in the world"
        );

        // And it is every tick, not once. A charge that only happened on the first tick of a
        // body's life would be indistinguishable from this after one call.
        for _ in 0..9 {
            scene.run();
        }
        let after_ten = 20.0 - scene.energy(muscular).expect("nothing has died here");
        assert!(
            (after_ten - soft * 10.0).abs() < 1e-9,
            "ten ticks cost a body of two myocytes {after_ten}, against the {} ten ticks of \
             its own upkeep comes to",
            soft * 10.0
        );
    }

    /// ⭐ **B2.** An organism whose energy runs out dies, and what it was holding becomes
    /// dead biomass rather than disappearing.
    ///
    /// SPEC section 10: *"Organisms whose energy reaches zero die."* SPEC section 5 is
    /// explicit that nothing else in the project will notice - *"the invariant says energy is
    /// conserved, not that any account is solvent"* - so this is the only place insolvency
    /// means anything at all.
    ///
    /// # The clamp, which is the trap Group A left behind
    ///
    /// `Organism::lose` deliberately does not refuse to take a body below nothing, so an
    /// organism can genuinely end a tick holding minus something. `Ledger::die` refuses a
    /// negative movement outright and would stop the run - so the second body here is
    /// deliberately driven past the line before it is reaped, and the test that it does not
    /// panic is half of what this is for.
    ///
    /// # What the accounts have to do
    ///
    /// `biomass` down by what the corpse was holding and `detritus` up by the same. A version
    /// of this that only checked the organism had vanished would pass against an
    /// implementation that deleted its energy along with it, which is a leak of exactly the
    /// size of every body that ever dies.
    #[test]
    fn an_organism_that_runs_out_of_energy_dies() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        let solvent = scene.add(4.0, &[(CellKind::Photocyte, Vec2::new(40.0, 40.0))]);
        let doomed = scene.add(4.0, &[(CellKind::Photocyte, Vec2::new(160.0, 40.0))]);

        // Everything but a hair of the doomed body's energy goes on nothing in particular,
        // so that one tick of upkeep is more than it can afford.
        scene.drain(doomed, 3.999);

        let biomass_before = scene.ledger.biomass();
        let detritus_before = scene.ledger.detritus();
        let free_before = scene.free.len();
        let left = scene
            .energy(doomed)
            .expect("it is alive until it is reaped");

        scene.run();

        assert!(
            scene.energy(doomed).is_none(),
            "an organism holding {left} paid a tick of upkeep it could not afford and is \
             still alive"
        );
        assert!(
            scene.energy(solvent).is_some(),
            "the body that could afford its upkeep died as well"
        );
        assert_eq!(
            scene.free.len(),
            free_before + 1,
            "a death did not give its slot back"
        );

        // Whatever it had left is dead biomass now. It is a hair either way: one tick of
        // upkeep is 0.004 and the body was holding 0.001, so what reaches `detritus` is
        // nothing at all - which is the honest answer. A body that starved to death has
        // nothing left to leave.
        let died_with = (left - 0.004).max(0.0);
        assert!(
            (scene.ledger.detritus() - detritus_before - died_with).abs() < 1e-12,
            "a body holding {left} died and {} reached the detritus account, against the \
             {died_with} it had left after its last tick of upkeep",
            scene.ledger.detritus() - detritus_before
        );

        // And the living biomass account still agrees with the living. This is the assertion
        // that catches a corpse's debts being left behind: the doomed body could not pay its
        // last tick, and if that shortfall were simply clamped away it would sit in `biomass`
        // for the rest of the run.
        let alive: f64 = scene.organisms.iter().flatten().map(Organism::energy).sum();
        assert!(
            (scene.ledger.biomass() - alive).abs() < 1e-12,
            "the books say {} is alive in this world and the only organism left is holding \
             {alive}",
            scene.ledger.biomass()
        );
        assert!(
            scene.ledger.biomass() < biomass_before,
            "a death did not take anything out of the living population"
        );
    }

    /// ⭐ **B2, the half that has no ledger movement in it.** An organism that ends a tick
    /// holding *less than nothing* is reaped without the world losing count of anything.
    ///
    /// This is the case Group A handed forward in as many words. `Organism::lose` does not
    /// refuse an overdraft, so a vigorous swimmer can spend a great deal more on one tick's
    /// work than it was holding, and arrive at the reaping owing energy it has already been
    /// recorded as having spent.
    ///
    /// Two things then have to be true, and they pull in different directions.
    ///
    /// **Nothing negative may be moved into `detritus`.** `ledger.rs` refuses a negative
    /// movement outright, because a transfer that runs backwards is a way of spending energy
    /// into existence, so an unclamped death would stop the run with a message about an
    /// energy movement rather than about a bankruptcy.
    ///
    /// **And the shortfall may not simply be forgotten.** The energy was spent, `dissipated`
    /// was credited with it, and it never existed. Clamped and walked away from, it stays in
    /// `biomass` for the rest of the run - one death at a time, until an overnight run's
    /// living-biomass figure is large and negative while every organism in the world is
    /// holding something positive. So the closing assertion is not about the death at all: it
    /// is that the books' idea of the living population still equals what the living
    /// population is actually holding.
    #[test]
    fn a_death_does_not_leave_its_debts_in_the_books() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        let survivor = scene.add(30.0, &[(CellKind::Photocyte, Vec2::new(40.0, 40.0))]);
        let bankrupt = scene.add(4.0, &[(CellKind::Myocyte, Vec2::new(160.0, 40.0))]);

        // A tick of very energetic swimming: it spends five units it did not have.
        scene.drain(bankrupt, 9.0);
        let owing = scene
            .energy(bankrupt)
            .expect("it is alive until it is reaped");
        assert!(
            owing < -4.0,
            "this test needs a body holding less than nothing and it is holding {owing}"
        );

        let detritus_before = scene.ledger.detritus();

        scene.run();

        assert!(
            scene.energy(bankrupt).is_none(),
            "a body holding {owing} was left alive"
        );
        assert!(
            (scene.ledger.detritus() - detritus_before).abs() < f64::EPSILON,
            "a body that died owing {owing} left {} of dead biomass behind it, so something \
             negative was moved into the detritus account or something was invented to fill \
             the hole",
            scene.ledger.detritus() - detritus_before
        );

        let alive: f64 = scene.organisms.iter().flatten().map(Organism::energy).sum();
        assert!(
            (scene.ledger.biomass() - alive).abs() < 1e-12,
            "a body died owing {owing} and the books still carry the debt: they say {} is \
             alive in this world and the survivor is holding {alive}",
            scene.ledger.biomass()
        );
        assert!(
            scene.energy(survivor).is_some(),
            "the solvent body died alongside the bankrupt one"
        );
    }

    /// ⭐ **B3.** An organism with plenty of energy dies anyway, once it has been alive long
    /// enough - and how long that is comes from what its body is made of.
    ///
    /// SPEC section 10 gives death a second cause and describes it in five words: *"age
    /// exceeds a genome-derived maximum"*. It does not say how it is derived. [`LIFETIME_UPKEEP`]
    /// is where that decision is argued; this is where it is pinned.
    ///
    /// # Why old age has to exist at all
    ///
    /// Because without it a world at its population cap is a world where nothing can ever be
    /// born again. Every slot is full of something that will never leave it, births fail
    /// silently as SPEC section 10 says they must, and evolution stops - not with an
    /// extinction, which would at least be visible, but with a world that looks perfectly
    /// healthy and has quietly stopped changing.
    ///
    /// # What the two bodies here are for
    ///
    /// The claim is not "organisms die at two thousand ticks", which any hard-coded number
    /// would satisfy. It is that the maximum is **derived from the body**, so the two bodies
    /// are SPEC section 6's cheapest tissue and its most expensive and the sclerocyte has to
    /// outlive the myocyte by exactly the ratio of their upkeeps - seven to one.
    ///
    /// Both are seeded holding far more than they can spend in their lifetimes, so that what
    /// kills them is unambiguously age. A body that ran out of energy on the way would pass a
    /// weaker version of this test for the wrong reason.
    #[test]
    fn an_organism_dies_of_old_age() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        let armoured = scene.add(60.0, &[(CellKind::Sclerocyte, Vec2::new(40.0, 40.0))]);
        let muscular = scene.add(60.0, &[(CellKind::Myocyte, Vec2::new(160.0, 40.0))]);

        let mut armoured_lived = 0u64;
        let mut muscular_lived = 0u64;

        for tick in 1..=5_000u64 {
            scene.age_everybody();
            scene.run();

            if armoured_lived == 0 && scene.energy(armoured).is_none() {
                armoured_lived = tick;
            }
            if muscular_lived == 0 && scene.energy(muscular).is_none() {
                muscular_lived = tick;
            }
        }

        // `LIFETIME_UPKEEP` of 8 against SPEC section 6's 0.002 and 0.014, at the shipped
        // temperature of one. Written out from the specification's numbers rather than asked
        // of the code, so the two can disagree.
        assert_eq!(
            muscular_lived, 571,
            "a body of one myocyte lived {muscular_lived} ticks, against the 8 ÷ 0.014 its \
             upkeep allows it"
        );
        assert_eq!(
            armoured_lived, 4_000,
            "a body of one sclerocyte lived {armoured_lived} ticks, against the 8 ÷ 0.002 \
             its upkeep allows it"
        );

        // Neither of them starved on the way, which is what makes this a test about age.
        assert!(
            60.0 - f64::from(0.014f32) * 571.0 > 50.0,
            "the myocyte body was seeded with too little to outlive its own upkeep, so this \
             test cannot tell old age from starvation"
        );
    }

    /// ⭐ **B4.** A body that dies leaves a grain of dead biomass at each of its cells, and
    /// what each grain carries is that cell's share of what the body was worth to build.
    ///
    /// SPEC section 10: *"The body becomes detritus particles at each cell's position,
    /// carrying that cell's construction energy."*
    ///
    /// # The sentence cannot be read literally, and the reason is worth understanding
    ///
    /// Construction energy is a property of a *cell kind* - see [`crate::cell::CellKind::construction`] -
    /// and what an organism is holding when it dies is a property of the life it had. The two
    /// are different numbers, and only one of them can be moved into `detritus` without
    /// energy being invented or destroyed. A corpse that carried its construction energy
    /// regardless of what the organism held would be a machine for printing energy: let a body
    /// starve, kill it, collect a full corpse.
    ///
    /// So construction energy is read as **the rule for sharing the corpse out**, and what
    /// there is to share is what the organism actually had. The photocyte and the sclerocyte
    /// here cost 0.004 and 0.002 a tick to keep, so the photocyte was twice the animal to
    /// build, so its grain gets two thirds of whatever is left. And what is left is what the
    /// `biomass` account gives up, to the last part in a million million.
    ///
    /// # Why a starved body leaves nothing at all
    ///
    /// Because it has nothing to leave. In this world energy is the only substance there is -
    /// there is no separate account of what a body is *made* of - so a lineage that spends
    /// itself down to nothing genuinely leaves no trace. Marine snow comes from the well-fed
    /// dead, which is also what it is made of in the sea.
    #[test]
    fn a_dead_body_becomes_detritus_carrying_its_construction_energy() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        // SPEC section 6's 0.004 against its 0.002: one cell was twice the other to build.
        let photocyte = Vec2::new(40.0, 40.0);
        let sclerocyte = Vec2::new(48.0, 64.0);
        let rich = scene.add(
            40.0,
            &[
                (CellKind::Photocyte, photocyte),
                (CellKind::Sclerocyte, sclerocyte),
            ],
        );

        let biomass_before = scene.ledger.biomass();
        let detritus_before = scene.ledger.detritus();
        let (age, last_held) = scene.live_out(rich, 4_000);

        assert!(
            age > 2_000,
            "the body died at {age} ticks, so it starved rather than growing old and this \
             test is not looking at a well-fed corpse"
        );

        let corpse: f64 = scene.drift.iter().map(|grain| grain.energy).sum();
        assert_eq!(
            scene.drift.len(),
            2,
            "a two-celled body left {} grains behind it, and SPEC section 10 asks for one at \
             each cell's position",
            scene.drift.len()
        );
        assert!(
            corpse > 20.0,
            "the corpse is holding {corpse}, and the body was holding {last_held} when its \
             last tick began"
        );

        // The books moved, and they moved together. This is the assertion that a corpse
        // carrying its construction energy regardless of what the organism held would fail:
        // the two cells are worth 6 units to have built and the body died holding far more.
        assert!(
            (scene.ledger.detritus() - detritus_before - corpse).abs() < 1e-12,
            "the drift is holding {corpse} and the detritus account moved by {}",
            scene.ledger.detritus() - detritus_before
        );
        // What the corpse holds is what the body had left after its last tick of upkeep, and
        // the living-biomass account is now empty because that body was the only thing alive.
        // The second half is the one that catches a corpse being conjured: the account has to
        // have given up exactly what the drift picked up, and it started the run holding the
        // forty units the body was seeded with.
        let last_upkeep = f64::from(0.004f32) + f64::from(0.002f32);
        assert!(
            (corpse - (last_held - last_upkeep)).abs() < 1e-9,
            "the corpse is holding {corpse} and the body was holding {last_held} when its \
             last tick began, less the {last_upkeep} that tick cost it"
        );
        assert!(
            scene.ledger.biomass().abs() < 1e-9,
            "the only organism in the world has died and the books say {} is still alive in \
             it",
            scene.ledger.biomass()
        );
        assert!(
            biomass_before > corpse,
            "the body was seeded holding {biomass_before} and left a corpse of {corpse}, so \
             it did not pay for the {age} ticks it was alive"
        );
        assert!(
            (corpse
                - construction_energy(&[
                    Cell::new(CellKind::Photocyte, photocyte),
                    Cell::new(CellKind::Sclerocyte, sclerocyte),
                ]))
            .abs()
                > 1.0,
            "the corpse happens to be holding exactly what the body cost to build, so this \
             test cannot tell the two readings of SPEC section 10 apart"
        );

        // A grain apiece, where the cells were, split by what each cost to build.
        let grains = scene.drift.clone();
        assert_eq!(
            grains[0].pos, photocyte,
            "the first grain is not where its cell was"
        );
        assert_eq!(
            grains[1].pos, sclerocyte,
            "the second grain is not where its cell was"
        );
        assert!(
            (grains[0].energy - 2.0 * grains[1].energy).abs() < 1e-9,
            "the photocyte's grain holds {} and the sclerocyte's holds {}, and SPEC section \
             6 makes one twice the other to build",
            grains[0].energy,
            grains[1].energy
        );

        // Construction energy is the same sum Group C will ask for, and it is the upkeep
        // table multiplied by a thousand ticks.
        let one_photocyte = construction_energy(&[Cell::new(CellKind::Photocyte, photocyte)]);
        assert!(
            (one_photocyte - f64::from(0.004f32 * 1_000.0f32)).abs() < 1e-9,
            "a photocyte is reckoned to have cost {one_photocyte} to build, against the \
             thousand ticks of its own upkeep this project defines it as"
        );

        // And a body that spent itself to nothing leaves nothing whatever behind it.
        let pauper = scene.add(0.5, &[(CellKind::Photocyte, Vec2::new(200.0, 40.0))]);
        scene.drain(pauper, 0.5);
        let grains_before = scene.drift.len();
        scene.run();

        assert!(scene.energy(pauper).is_none(), "the pauper did not starve");
        assert_eq!(
            scene.drift.len(),
            grains_before,
            "a body that died holding nothing left {} grains of nothing lying in the water",
            scene.drift.len() - grains_before
        );
    }

    /// ⭐ **B5.** A grain of dead biomass falls through the water and rots into it as it goes.
    ///
    /// SPEC section 10: *"Detritus sinks slowly and decays into the field tile beneath it."*
    /// SPEC section 5 lists the movement - `detritus → field` - and SPEC section 12 asks for
    /// the same grains to be drawn as marine snow, which is the one piece of this simulation
    /// that is atmosphere and mechanism at the same time.
    ///
    /// # What the assertions are about
    ///
    /// The `field` account is the resource grid itself, so "decays into the tile beneath it"
    /// is a claim about a **particular tile** rather than about a total. A version of this
    /// that only checked the two accounts had moved would pass against an implementation that
    /// rotted every grain into tile zero, and the light gradient - which is the whole reason
    /// depth means anything - would be quietly fed from the wrong end.
    ///
    /// # Why it must reach nothing
    ///
    /// A proportional decay never does. A grain that went on halving for ever would hold a
    /// millionth of a unit and occupy a place in an arena that cannot grow, and a world that
    /// had been running overnight would be full of them. [`DECAY_FLOOR`] is what gives every
    /// grain a last tick, and the closing assertions are that the drift really does empty and
    /// that everything it was holding arrived in the water.
    #[test]
    fn detritus_sinks_and_decays_into_the_tile_beneath_it() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        let at = Vec2::new(100.0, 60.0);
        let dropped = scene.drop_detritus(at, 4.0);
        let tile = scene.grid.tile_at(at);

        let field_before = scene.grid.total_energy();
        let detritus_before = scene.ledger.detritus();
        let tile_before = f64::from(scene.grid.tiles()[tile]);
        let tiles_before: Vec<f32> = scene.grid.tiles().to_vec();

        scene.run();

        // It fell, straight down, by exactly one tick's worth.
        let grain = scene.drift[0];
        assert!(
            (grain.pos.y - (at.y + SINK_SPEED / 60.0)).abs() < 1e-6,
            "the grain is at {} after one tick, against the {} a fall of {SINK_SPEED} units a \
             second puts it at",
            grain.pos.y,
            at.y + SINK_SPEED / 60.0
        );
        assert!(
            (grain.pos.x - at.x).abs() < f32::EPSILON,
            "the grain is at {} across, having been dropped at {} - it drifted sideways, and \
             nothing in SPEC section 10 pushes it",
            grain.pos.x,
            at.x
        );

        // And it rotted, into the tile it was in and no other.
        // A hair's tolerance rather than none, and the hair is the point. What the grain gave
        // up is what the *tile* took, which is not quite what was offered: a tile is a 32-bit
        // number holding something near eight, so a sixty-fourth of a unit added to it lands
        // on the nearest value it can express. `Grid::deposit` hands back the realised figure
        // precisely so that the grain keeps what the water would not take.
        let rotted = dropped - grain.energy;
        let wanted = (dropped * DECAY_RATE).max(DECAY_FLOOR);
        assert!(
            (rotted - wanted).abs() < 1e-5,
            "a grain holding {dropped} gave up {rotted} in a tick, against the {wanted} \
             {DECAY_RATE} of itself comes to"
        );
        assert!(
            (f64::from(scene.grid.tiles()[tile]) - tile_before - rotted).abs() < 1e-9,
            "the grain gave up {rotted} and the tile it was lying in gained {}",
            f64::from(scene.grid.tiles()[tile]) - tile_before
        );
        for (other, (before, after)) in tiles_before.iter().zip(scene.grid.tiles()).enumerate() {
            assert!(
                other == tile || (f64::from(*after) - f64::from(*before)).abs() < f64::EPSILON,
                "tile {other} is nowhere near the grain and gained {}",
                f64::from(*after) - f64::from(*before)
            );
        }
        assert!(
            (scene.grid.total_energy() - field_before - rotted).abs() < 1e-9,
            "the field gained {} while the grain gave up {rotted}",
            scene.grid.total_energy() - field_before
        );
        assert!(
            (detritus_before - scene.ledger.detritus() - rotted).abs() < 1e-12,
            "the detritus account gave up {} while the grain gave up {rotted}",
            detritus_before - scene.ledger.detritus()
        );

        // Left alone, it finishes: the drift empties, and every unit it was carrying has
        // either reached the water or left the world. Two thousand ticks is generous - the
        // arithmetic says a grain of four units is done inside a thousand - and the
        // generosity is the point, because a grain that never quite reached nothing would be
        // caught by the assertion rather than by the loop running out.
        let dissipated_before = scene.ledger.dissipated();
        for _ in 0..2_000 {
            scene.run();
        }

        assert!(
            scene.drift.is_empty(),
            "{} grains are still in the water after two thousand ticks of rotting",
            scene.drift.len()
        );
        assert!(
            scene.ledger.detritus() < 1e-12,
            "the drift is empty and the detritus account still says {} is lying dead",
            scene.ledger.detritus()
        );

        // Nearly all of it went into the water; what is left is the last crumb, which no tile
        // could express and which therefore left the world. Both are counted, because the
        // point of the claim is that none of it simply vanished.
        let into_the_water = scene.grid.total_energy() - field_before;
        let out_of_the_world = scene.ledger.dissipated() - dissipated_before;
        assert!(
            (into_the_water + out_of_the_world - dropped).abs() < 1e-9,
            "a grain holding {dropped} rotted away, {into_the_water} of it into the water and \
             {out_of_the_world} out of the world, and the two do not add up to what it held"
        );
        assert!(
            out_of_the_world < 1e-5,
            "{out_of_the_world} of a grain holding {dropped} left the world rather than \
             reaching the water, and the only part that is meant to is the last crumb - a \
             residue smaller than the smallest change a tile can express"
        );
    }

    /// ⭐⭐ **B7.** A genome costs its organism something to carry, and that is what keeps gene
    /// duplication available to a lineage that has become elaborate.
    ///
    /// **The reasoning runs the opposite way to the sentence in CLAUDE.md that asks for this,
    /// and the doc comment on [`crate::config::MetabolismConfig::gene_cost`] is where it is
    /// set out.** In short: the cost is not a brake on bloat. With SPEC section 7's shipped
    /// rates genomes drift upward, a lineage left alone ends up pressed against `max_genes`,
    /// and *at* the cap a lengthening mutation fails - so the operator the whole genome design
    /// exists for switches itself off exactly when a lineage is at its most complex. The cost
    /// is what stops a lineage ever getting there.
    ///
    /// # What the number has to be, and the arithmetic that fixes it
    ///
    /// Two conditions pulling against each other.
    ///
    /// **Small enough not to dominate an ordinary body.** At 0.0001 a gene, a ten-gene genome
    /// (already a fairly elaborate growth program) costs 0.001 a tick, against the 0.016 a
    /// four-celled photosynthetic body pays for its cells. Six per cent, which is a pressure
    /// rather than a tax.
    ///
    /// **Large enough that a saturated genome is genuinely felt.** At the cap of 128 genes it
    /// is 0.0128 a tick: **three and a fifth times a photocyte's entire upkeep**, and 91% of a
    /// myocyte's, which is the most expensive cell SPEC section 6 has. A lineage at the cap is
    /// carrying the running cost of three extra photocytes that earn it nothing.
    ///
    /// The useful way to read the size of it is that **twenty genes cost what one sclerocyte
    /// costs** - SPEC section 6's cheapest cell, at 0.002. A genome is priced in units of
    /// structural tissue, which is the only per-kind economic number the specification gives.
    ///
    /// # What this test measures, and why it is a race rather than a subtraction
    ///
    /// The arithmetic is checked first, because it has to be. But an equality on a charge
    /// would be satisfied by a cost that no lineage could ever feel, so the second half is the
    /// consequence: two identical bodies with identical purses, one carrying a saturated
    /// genome and one carrying eight genes, and the saturated one **runs out first**. That is
    /// selection, written as a number - and it is what "pushed back from the cap long before
    /// it arrives" has to mean if it means anything.
    #[test]
    fn a_per_gene_metabolic_cost_keeps_duplication_alive() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        // Two bodies alike in every way except what they are carrying.
        let saturated =
            scene.add_carrying(1.0, 128, &[(CellKind::Photocyte, Vec2::new(40.0, 40.0))]);
        let lean = scene.add_carrying(1.0, 8, &[(CellKind::Photocyte, Vec2::new(160.0, 40.0))]);

        scene.run();

        // SPEC section 3's `gene_cost` and section 6's photocyte, multiplied out here from
        // the specification rather than asked of the code.
        let tissue = f64::from(0.004f32);
        let per_gene = f64::from(0.0001f32);
        let saturated_paid = 1.0 - scene.energy(saturated).expect("nothing has died yet");
        let lean_paid = 1.0 - scene.energy(lean).expect("nothing has died yet");

        assert!(
            (saturated_paid - (tissue + 128.0 * per_gene)).abs() < 1e-9,
            "a one-celled body carrying 128 genes paid {saturated_paid} for a tick, against \
             the {} its cell and its genome come to",
            tissue + 128.0 * per_gene
        );
        assert!(
            (lean_paid - (tissue + 8.0 * per_gene)).abs() < 1e-9,
            "a one-celled body carrying 8 genes paid {lean_paid} for a tick, against the {} \
             its cell and its genome come to",
            tissue + 8.0 * per_gene
        );

        // The two conditions the size of the number has to satisfy, asserted rather than
        // asserted about. A saturated genome costs more than three photocytes; ten genes cost
        // under a tenth of a four-celled body.
        assert!(
            128.0 * per_gene > 3.0 * tissue,
            "a genome at the cap costs {} a tick against a photocyte's {tissue}, so a lineage \
             pressed against `max_genes` is not being pushed back by anything it can feel",
            128.0 * per_gene
        );
        assert!(
            10.0 * per_gene < 0.1 * 4.0 * tissue,
            "ten genes cost {} a tick against the {} a four-celled body pays for its cells, \
             so carrying an ordinary growth program dominates the cost of having a body",
            10.0 * per_gene,
            4.0 * tissue
        );

        // ⭐ And the consequence. Same body, same purse, and the one carrying more of a
        // genome starves first.
        let mut saturated_lived = 0u64;
        let mut lean_lived = 0u64;
        for tick in 2..=1_000u64 {
            scene.age_everybody();
            scene.run();

            if saturated_lived == 0 && scene.energy(saturated).is_none() {
                saturated_lived = tick;
            }
            if lean_lived == 0 && scene.energy(lean).is_none() {
                lean_lived = tick;
            }
        }

        assert!(
            saturated_lived > 0 && lean_lived > saturated_lived * 3,
            "the body carrying 128 genes lasted {saturated_lived} ticks and the one carrying \
             8 lasted {lean_lived}, so a genome pressed against the cap costs a lineage \
             nothing it would ever be selected on"
        );
    }

    /// A grain that has reached the floor stays on it, and goes on rotting there.
    ///
    /// The world is closed vertically - SPEC section 8 - so there is nowhere below the floor
    /// for a grain to be, and a grain that fell out of the world would take the energy it was
    /// carrying with it. This is the same clamp `physics.rs` applies to a cell, applied to the
    /// one other thing in the simulation that has a position.
    #[test]
    fn a_grain_cannot_sink_out_of_the_world() {
        let mut scene = Scene::new(&a_small_world());
        scene.fill_with_light();

        let floor = scene.config.world.height;
        let dropped = scene.drop_detritus(Vec2::new(100.0, floor - 1.0), 2.0);

        for _ in 0..200 {
            scene.run();
        }

        let grain = scene.drift[0];
        assert!(
            (grain.pos.y - floor).abs() < f32::EPSILON,
            "a grain two hundred ticks past the floor is at {} in a world {floor} deep",
            grain.pos.y
        );
        assert!(
            grain.energy < dropped,
            "a grain resting on the floor stopped rotting"
        );
    }
}
