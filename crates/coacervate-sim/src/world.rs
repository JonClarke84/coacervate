//! The world, and the tick that moves it.
//!
//! Everything the simulation is made of so far - the field of energy, the books it is
//! counted in, the cells and the forces that push them about, the run's randomness - is
//! owned here, in one place, and a run is nothing more than [`World::tick`] called over and
//! over. There is no other state anywhere: hand this module a configuration and a number of
//! ticks and the result is fixed.
//!
//! # What one tick does, and why in that order
//!
//! The light falls; the energy spreads sideways; tiles that cannot hold what has arrived
//! shed it; then the cells move.
//!
//! The first three are one call, because `grid.rs` keeps them together and should: a tile
//! pushed past its ceiling by diffusion has to be cut back in the same breath, rather than
//! left standing over its ceiling for whoever ticks next to remember about. SPEC section 4
//! gives them in that order and gives the reason - a ceiling enforced before the energy has
//! finished moving is not a ceiling.
//!
//! The physics goes last, and in Phase 2 the order genuinely does not matter: nothing that
//! moves a cell reads a tile, and nothing that moves energy reads a position. It is written
//! this way for where Phase 4's work has to go. Harvesting sits between the two halves, and
//! it wants a field the light has already fallen on and cells that have not yet swum away
//! from the tile they were sitting on.
//!
//! # The books are checked here rather than by the things that move the energy
//!
//! `grid.rs` tells the ledger what the light put in and what the tiles could not hold, and
//! `ledger.rs` refuses any movement that is not a movement. Neither of them ever asks
//! whether the world still adds up, and neither could sensibly: the question is about the
//! grid and the accounts *together*, so it can only be asked by something holding both.
//! That is this module, and it asks on SPEC section 5's cadence - every tick while
//! debugging, every thousandth in a release build, and always on tick zero.
//!
//! Tick zero is not ceremony. A world is checked before it has done anything so that Phase
//! 3 cannot seed an organism out of thin air and have it show up eight hours later as a
//! drift nobody can account for. SPEC section 5 is explicit that seeding must take its
//! energy out of the field, and this is where getting that wrong stops the run immediately.
//!
//! # Everything is allocated when the world is built, and never again
//!
//! CLAUDE.md: *a simulation that cannot allocate cannot leak.* The cell arena and the spring
//! list are built at the largest size the configuration could ever need - every organism the
//! world allows, each with every cell it allows - and nothing here has any way to grow them.
//! At SPEC section 3's defaults that is four thousand organisms of sixty-four cells apiece:
//! **256,000 cells, about 7 MB, and the same number of springs at about 6 MB**. The physics
//! builds its own working arrays at the same capacity and costs about 15 MB more; the
//! resource field is a rounding error beside them at under half a megabyte. A default world
//! is therefore around **29 MB**, against CLAUDE.md's resident target of 2 GB.
//!
//! The two arenas are built with room reserved and nothing in them, which costs the
//! reservation and no work at all - a world starts with no organisms in it, and Phase 3 is
//! what fills them.
//!
//! # What is deliberately not here
//!
//! **Organisms.** There is a cell arena and there is nothing that puts a cell in it. Bodies
//! are grown from a genome in Phase 3 and fed, reproduced and killed in Phase 4; a tick that
//! did any of that now would be a tick written before there is anything for it to be right
//! about.
//!
//! **A runner.** Nothing here decides when a run ends. SPEC section 3's `max_wall_clock_hours`
//! is a wall-clock bound, and a wall clock is exactly what a deterministic simulation must
//! not read - `clippy.toml` refuses `Instant` and `SystemTime` in this crate outright, so
//! **the world cannot time itself**, by design. Whatever ends a run does it from outside, by
//! counting the ticks this module has taken.
//!
//! **Anything that draws a random number.** The run's randomness is owned here because it
//! belongs to the world rather than to any one part of it, and in Phase 2 nothing asks it
//! for anything. Development and mutation are its first callers.

use crate::cell::Cell;
use crate::config::Config;
use crate::grid::Grid;
use crate::ledger::Ledger;
use crate::physics::{Physics, Spring, cell_capacity};
use crate::rng::WorldRng;

/// A whole simulated world: everything in it, and how far through its run it is.
///
/// Built once from a configuration and then only ticked. Every field is private and there
/// is no way to reach past the accessors below, which is what keeps the two guarantees this
/// type exists for: that the arenas cannot grow, and that energy cannot enter or leave the
/// world except through the doors `ledger.rs` has written down.
pub struct World {
    /// The energy in the water, and the light that puts it there.
    grid: Grid,

    /// Where every unit of that energy is, and the assertion that none has been invented.
    ledger: Ledger,

    /// What pushes the cells around, and the arrays it needs to do it.
    physics: Physics,

    /// Every cell in the world. Empty until Phase 3, and never larger than the capacity it
    /// was built with.
    cells: Vec<Cell>,

    /// Every adhesion between those cells. Phase 3 makes them; nothing here does.
    springs: Vec<Spring>,

    /// The run's randomness. Nothing draws from it yet - see the module documentation.
    rng: WorldRng,

    /// How many ticks this world has taken.
    ///
    /// The only clock the simulation has. SPEC section 2 keeps wall-clock time out of the
    /// physics entirely, so this is what "when" means here: an overnight run is tens of
    /// millions of these, and SPEC section 2's deep-time display is this number multiplied
    /// by `years_per_tick` for the benefit of a person reading it.
    ticks: u64,
}

impl World {
    /// Build the world a configuration describes.
    ///
    /// Deterministic and complete: what comes back has its field already shaped by the
    /// seed's blotchiness, its books open, and every arena it will ever use allocated at the
    /// size the configuration implies. Nothing is built lazily on the first tick, because a
    /// thing built on the first tick is a thing that can fail on the first tick of an
    /// overnight run rather than at the moment somebody was watching.
    ///
    /// The ledger's opening balance is *measured* off the grid rather than assumed to be
    /// nothing. It is nothing today - a world starts dark and fills under the light - and
    /// writing it as a measurement means Phase 3 can seed a world that starts with something
    /// in it without this line having to be found and changed.
    ///
    /// # Panics
    ///
    /// If the books do not balance the moment they are opened, which today cannot happen and
    /// from Phase 3 is the first thing that will. See the module documentation.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let capacity = cell_capacity(config);
        let grid = Grid::new(config);
        let ledger = Ledger::new(grid.total_energy());

        let world = Self {
            grid,
            ledger,
            physics: Physics::new(config),
            cells: Vec::with_capacity(capacity),
            springs: Vec::with_capacity(capacity),
            rng: WorldRng::from_seed(config.world.seed),
            ticks: 0,
        };

        // SPEC section 5's tick-zero check, which `Ledger::should_check` answers yes to in
        // both profiles. Written as a plain call rather than behind that question, because
        // a question with one possible answer reads as though it might have another.
        world.ledger.check(world.grid.total_energy());

        world
    }

    /// Move the whole world on by one tick.
    ///
    /// The four things a tick is, in the order the module documentation argues for, and then
    /// the books. Nothing else: no growing, no dying, no eating, and nothing that reads a
    /// clock.
    ///
    /// # Panics
    ///
    /// If the energy in the world stops matching the energy the books say is in it. SPEC
    /// section 5 asks for exactly this and gives the reason: eight hours of quietly wrong
    /// output is worse than a crash, and a ledger that is out at all is a ledger whose
    /// numbers have stopped describing anything.
    pub fn tick(&mut self) {
        self.grid.tick(&mut self.ledger);
        self.physics.step(&mut self.cells, &self.springs);

        self.ticks += 1;

        if Ledger::should_check(self.ticks) {
            self.ledger.check(self.grid.total_energy());
        }
    }

    /// How many ticks this world has taken.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// The field of energy, to read.
    #[must_use]
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The books, to read. Nothing outside this module can move energy between accounts,
    /// because nothing outside it can obtain the ledger any other way.
    #[must_use]
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Every cell in the world, to read.
    #[must_use]
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// The run's randomness.
    ///
    /// Phase 3 is the first caller: development and mutation are the only things in the
    /// design that draw a random number, and an organism is handed its own private sequence
    /// from here when it is born, so that running organisms across every core of the machine
    /// does not change what happens. See `rng.rs`.
    pub fn rng(&mut self) -> &mut WorldRng {
        &mut self.rng
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{CellKind, Vec2};
    use crate::config::{RawConfig, spec_defaults};
    use proptest::prelude::*;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    ///
    /// Every test here starts from the shipped defaults and alters only what it is about, so
    /// a test that talks about determinism is visibly not also quietly turning the light
    /// down.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// How far the two sides of SPEC section 5's invariant have drifted apart, as a fraction
    /// of everything the world has ever contained.
    ///
    /// Written out here from the specification rather than asked of [`Ledger::check`], and
    /// that is the whole point of it. `check` answers "is this still inside the tolerance",
    /// which is a yes or a no; this answers "by how much", which is what makes it possible to
    /// say that a hundred thousand ticks left nearly seven orders of magnitude of headroom
    /// rather than merely that they did not fail. A test that called `check` would also be a
    /// test that agreed with whatever `check` happened to do.
    fn relative_error(world: &World) -> f64 {
        let ledger = world.ledger();
        let held = world.grid().total_energy()
            + ledger.biomass()
            + ledger.detritus()
            + ledger.dissipated();
        let expected = ledger.initial_total() + ledger.influx_total();

        (held - expected).abs() / expected.abs().max(1.0)
    }

    /// Every number in the world, laid out end to end exactly as the arithmetic holds it.
    ///
    /// Used by `a_run_is_still_reproducible` to compare two runs. Bit patterns rather than
    /// the numbers themselves: two runs of the same seed are supposed to agree exactly, and
    /// a comparison with any tolerance in it would wave through a difference in the last
    /// place - which is precisely how a determinism failure starts before it grows into a
    /// recording that no longer replays.
    fn every_number_in(world: &World) -> Vec<u32> {
        let mut written = Vec::new();

        written.extend(world.grid().tiles().iter().map(|tile| tile.to_bits()));

        for cell in world.cells() {
            written.extend([
                cell.pos.x.to_bits(),
                cell.pos.y.to_bits(),
                cell.vel.x.to_bits(),
                cell.vel.y.to_bits(),
                cell.radius.to_bits(),
                cell.energy_flow.to_bits(),
                u32::from(cell.state),
            ]);
        }

        written
    }

    /// Put a few cells in the world by hand, so that a tick is a whole tick.
    ///
    /// There is nothing in Phase 2 that creates a cell - bodies are grown from a genome in
    /// Phase 3 - so a world left to itself exercises the field and none of the physics. These
    /// are placed close enough together to overlap, with a spring across one pair, so that
    /// both of the forces `physics.rs` knows about are actually being computed while the
    /// energy is being counted.
    fn place_a_few_cells(world: &mut World, width: f32, height: f32) {
        let middle = height / 2.0;

        for (index, along) in [0.25, 0.26, 0.27, 0.60].into_iter().enumerate() {
            let sag = if index % 2 == 0 { 0.0 } else { 1.5 };
            world.cells.push(Cell::new(
                CellKind::Photocyte,
                Vec2::new(width * along, middle + sag),
            ));
        }

        world.springs.push(Spring {
            a: 0,
            b: 3,
            rest_length: 8.0,
            stiffness: 20.0,
        });
    }

    /// The arenas are the size the configuration asks for, and they are that size for the
    /// whole run.
    ///
    /// CLAUDE.md: *a simulation that cannot allocate cannot leak.* The defence against a
    /// runaway simulation eating the machine is not that the code is careful, it is that
    /// there is nowhere for the memory to come from - so the interesting claim is not that
    /// the arenas are large enough but that they never change.
    ///
    /// Three things, then. That the capacity comes from the *configuration* rather than from
    /// a number somebody wrote down while testing against the defaults, which is why the
    /// second world here is nine organisms of five cells. That a world's default arenas cost
    /// what the module documentation says they cost, so the figure in the prose is checked
    /// rather than remembered. And that a thousand ticks leave both arenas at the same
    /// address and the same size, which is what would stop being true the moment anything in
    /// here pushed onto a full vector.
    #[test]
    fn the_arenas_are_allocated_once_at_the_size_the_config_asks_for() {
        let default_world = World::new(&config(|_| {}));

        assert_eq!(
            default_world.cells.capacity(),
            256_000,
            "SPEC section 3's four thousand organisms of sixty-four cells"
        );
        assert_eq!(
            default_world.springs.capacity(),
            256_000,
            "a spring is made once per cell that stays attached to its parent, so there \
             can be no more springs than cells"
        );
        assert!(
            default_world.cells.is_empty() && default_world.springs.is_empty(),
            "a world starts with no organisms in it, and there is nothing in Phase 2 that \
             could have made one"
        );

        // What the two arenas actually cost, so the figure in the module documentation is
        // checked rather than remembered. 28 bytes a cell and 24 a spring, which is
        // 13,312,000 bytes - about a hundred and fiftieth of CLAUDE.md's 2 GB target, and
        // the reason the arenas can be built at their worst case without anybody having to
        // think about it.
        let cost = default_world.cells.capacity() * size_of::<Cell>()
            + default_world.springs.capacity() * size_of::<Spring>();
        assert!(
            (12_000_000..15_000_000).contains(&cost),
            "the two arenas of a default world cost {cost} bytes, against the 13,312,000 \
             recorded here"
        );

        let odd_shape = World::new(&config(|raw| {
            raw.limits.max_organisms = 9;
            raw.limits.max_cells_per_organism = 5;
        }));
        assert_eq!(
            odd_shape.cells.capacity(),
            45,
            "the arena ignored the configured limits"
        );

        // And it is that size once. A world that reallocated as it ran would copy every cell
        // in it to a new address and hand the old space back, for ever, on a machine with
        // other things to be getting on with.
        let mut running = World::new(&config(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
        }));
        place_a_few_cells(&mut running, 2048.0, 1152.0);

        let addresses = [
            running.cells.as_ptr().cast::<u8>(),
            running.springs.as_ptr().cast::<u8>(),
        ];
        let sizes = [running.cells.capacity(), running.springs.capacity()];

        for _ in 0..1_000 {
            running.tick();
        }

        assert_eq!(
            addresses,
            [
                running.cells.as_ptr().cast::<u8>(),
                running.springs.as_ptr().cast::<u8>()
            ],
            "a thousand ticks moved the cells somewhere else in memory, so something in \
             here is allocating as it goes"
        );
        assert_eq!(
            sizes,
            [running.cells.capacity(), running.springs.capacity()],
            "a thousand ticks changed how much room the world takes up"
        );
        assert_eq!(
            running.ticks(),
            1_000,
            "the world does not know how far through its run it is"
        );
    }

    /// ⭐ **The phase's done-criterion.** A hundred thousand ticks of the world SPEC section
    /// 3 actually ships, and the books still balance.
    ///
    /// CLAUDE.md's phase table gives this as the whole of what Phase 2 is for, and the
    /// reason it is a hundred thousand ticks rather than a hundred is that the failure this
    /// guards against does not happen, it *accumulates*. Every one of Group A's and Group
    /// B's tests would pass against a world losing a millionth of a unit per tick. This is
    /// the only one that would not.
    ///
    /// # What it measures, and why that is more than "it did not panic"
    ///
    /// [`World::tick`] already stops the run if the invariant breaks, so a test that merely
    /// ran a hundred thousand ticks would be asserting nothing that the world does not
    /// assert about itself. What is added here is the *size* of the discrepancy, measured
    /// independently from SPEC section 5's formula, at every thousandth tick and at the end.
    ///
    /// The number that comes out is the phase's headline: **a relative error of 1.74e-10
    /// after a hundred thousand ticks**, against a tolerance of 1e-3. That is nearly seven
    /// orders of magnitude of headroom, and it is a statement about the whole world rather
    /// than the grid alone - Group B measured 1.74e-10 for the field by itself, so owning
    /// the field inside a `World` and ticking the physics beside it has cost nothing at all.
    ///
    /// # And why it insists the world was doing something
    ///
    /// Because energy is trivially conserved in a world where nothing happens. A [`World`]
    /// whose `tick` had been emptied out would sail through every conservation claim here,
    /// which is exactly how this test failed before the tick was written. So the closing
    /// assertions are about the world having *run*: light has fallen, the field is standing
    /// with a real quantity in it, and energy has been leaving through the tiles that cannot
    /// hold what reaches them.
    ///
    /// # How much light a full world actually takes, which is not what it is offered
    ///
    /// The default world is offered `0.012 x 36,864`, about 442 units a tick. It absorbs
    /// **793,408 over a hundred thousand ticks**, which is under eight a tick - not two per
    /// cent of what is on offer. That is not a fault, and it is worth writing down because
    /// the arithmetic looks alarming until it is explained.
    ///
    /// A tile takes light only up to its ceiling, and this world reaches its ceilings within
    /// about seven hundred ticks and stays there. What keeps a full tile taking anything at
    /// all is diffusion draining it downhill, and at SPEC's defaults that drain is small: the
    /// ceilings of two vertically neighbouring tiles differ by about 0.042, of which
    /// diffusion moves 4% - roughly 0.0017 a tick, against the 0.009 of light the tile is
    /// offered. So nearly every tile sits pinned at its ceiling, taking in and shedding the
    /// trickle that passes through it.
    ///
    /// The consequence for Phase 4 is the interesting part: **the standing field is nearly
    /// the whole of the world's energy budget, and the flow through it is not.** A world
    /// holding 184,030 units is turning over eight a tick, so an ecology that lives off the
    /// flow rather than off the standing stock has a great deal less to eat than the total
    /// suggests. Organisms harvesting a tile will pull it below its ceiling, at which point
    /// that tile starts taking its full share of light again - so the throughput is not fixed
    /// at eight a tick either. It rises towards the 442 on offer as the population grows.
    ///
    /// # The drift does not grow
    ///
    /// The last claim is the one that matters beyond this test. An overnight run is tens of
    /// millions of ticks, so an error that is comfortably inside the tolerance at a hundred
    /// thousand and *growing* is a run that stops in the small hours with no bug to find.
    /// Group B met exactly that: rounding thrown away by diffusion cost 2.8e-4 by tick
    /// 100,000, inside the tolerance and on course to cross it at around a million.
    ///
    /// So the worst error over the whole run is compared against the worst over its first
    /// tenth. An error accumulating in one direction would be ten times larger by the end;
    /// what is actually there is a bounded wobble that the first ten thousand ticks have
    /// already shown the whole of.
    ///
    /// Recorded 31 July 2026, Windows 11 x86-64.
    ///
    /// # Why this one is marked `ignore` and still runs on every check
    ///
    /// A hundred thousand ticks of a 36,864-tile field is 289 seconds of arithmetic in a
    /// debug build and 27 in a release one — the same work either way, but debug leaves
    /// every bounds check and overflow check in place. Run in both, it made the check suite
    /// take five and a half minutes, which is long enough that somebody stops running it.
    ///
    /// So it is skipped by default and `scripts/check.ps1` passes `--include-ignored` to the
    /// **release** pass only. The phase's headline claim is still proved on every single
    /// check; it just gets proved once instead of twice, in the profile where it is 10×
    /// cheaper. Both profiles were verified to produce identical figures before this was
    /// done, so nothing is lost by only measuring in one of them.
    ///
    /// To run it in debug anyway:
    /// `cargo test -p coacervate-sim -- --ignored energy_is_conserved_over_100k_ticks`
    #[test]
    #[ignore = "289s in debug; check.ps1 runs it via --include-ignored in the release pass"]
    fn energy_is_conserved_over_100k_ticks() {
        let mut world = World::new(&config(|_| {}));

        let mut worst = 0.0f64;
        let mut worst_early = 0.0f64;

        for tick in 1..=100_000u64 {
            world.tick();

            if tick % 1_000 == 0 {
                let error = relative_error(&world);
                worst = worst.max(error);
                if tick <= 10_000 {
                    worst_early = worst_early.max(error);
                }
            }
        }

        let final_error = relative_error(&world);
        let held = world.grid().total_energy();
        let ledger = world.ledger();

        assert_eq!(
            world.ticks(),
            100_000,
            "the world lost count of its own ticks"
        );

        assert!(
            final_error < 1e-6,
            "after a hundred thousand ticks the two sides of the invariant are {final_error} \
             apart in relative terms, and SPEC section 5's tolerance of 1e-3 is meant to be \
             covering the rounding in `f32` diffusion rather than an actual leak"
        );
        assert!(
            worst < 1e-6,
            "the books were out by {worst} at their worst moment during the run, even \
             though they came back inside by the end"
        );

        // The world was running, rather than sitting still and conserving nothing perfectly.
        assert!(
            ledger.influx_total() > 700_000.0,
            "only {} units of light fell over a hundred thousand ticks, and the default \
             world is 36,864 tiles offered 0.012 each",
            ledger.influx_total()
        );
        assert!(
            held > 100_000.0,
            "the field is holding {held} after a hundred thousand ticks of light, so \
             either the light is not reaching the tiles or the tiles are not keeping it"
        );
        assert!(
            ledger.dissipated() > 500_000.0,
            "only {} units have left the world through tiles that could not hold what \
             reached them, and a world with a depth gradient sheds nearly everything the \
             light gives it once it is full",
            ledger.dissipated()
        );
        assert!(
            ledger.biomass() < f64::EPSILON && ledger.detritus() < f64::EPSILON,
            "something is alive or dead in a phase that has neither"
        );

        // The drift is bounded rather than accumulating. See the note above.
        assert!(
            worst < worst_early * 4.0,
            "the worst discrepancy over the whole run was {worst}, against {worst_early} \
             over its first tenth - so the error is growing with time rather than wobbling \
             about a fixed size, and a run a hundred times longer would not survive it"
        );

        // The four numbers this world actually produces, written down. Every claim above is
        // a statement about the *kind* of world it is - balanced, running, not drifting - and
        // all of them would stay green through a change that moved all four. These are what
        // make a change of behaviour something that has to be argued for rather than absorbed
        // by an inequality that still holds.
        //
        // ⚠️ If these fail, investigate before touching them. Either the arithmetic of the
        // world has changed, in which case every archived run is now something else, or this
        // machine computes differently from the one they were recorded on - which is a larger
        // finding than a stale test. Pasting in the new numbers destroys the evidence.
        assert!(
            (1.0e-10..3.0e-10).contains(&final_error),
            "the run finished {final_error} out in relative terms, against the 1.739e-10 \
             recorded here"
        );
        assert!(
            (held - 184_030.35).abs() < 0.01,
            "the field came to rest holding {held} rather than the 184,030.35 recorded here"
        );
        assert!(
            (ledger.influx_total() - 793_407.70).abs() < 0.01,
            "the world took in {} units of light rather than the 793,407.70 recorded here",
            ledger.influx_total()
        );
        assert!(
            (ledger.dissipated() - 609_377.35).abs() < 0.01,
            "the world shed {} units through tiles that could not hold them, rather than \
             the 609,377.35 recorded here",
            ledger.dissipated()
        );
    }

    /// Two runs of the same seed are the same run, down to the last bit, over a world that
    /// now does something.
    ///
    /// SPEC section 2 calls determinism load-bearing and gives the reason: when something
    /// interesting happens you will want to see it again. Phase 1 made the claim about the
    /// random number generator, which is the easy half. This makes it about a world with a
    /// field of energy being lit, spread and shed, and cells being pushed around inside it.
    ///
    /// # Why the whole field is compared rather than a total
    ///
    /// Because a total is a summary, and summaries agree by accident. Two fields holding the
    /// same energy in different places have the same total, the same ledger and the same
    /// tick count, and are not the same world - and "same total" is exactly what a broken
    /// determinism would still give you, since the arithmetic that went wrong is
    /// conservative. So what is compared is every tile and every number on every cell, as
    /// bit patterns, which is the strongest form of the claim there is.
    ///
    /// # And why a different seed has to come out different
    ///
    /// Because the first half of this test passes perfectly in a world where the seed
    /// reaches nothing at all. A `World` that ignored its configuration's seed entirely
    /// would be magnificently reproducible. The second half is what makes the first half a
    /// statement about the seed.
    ///
    /// It asks the question of the field alone, and that is worth being plain about: in
    /// Phase 2 nothing in the physics reads a random number, so two worlds differing only in
    /// their seed push their cells around identically. What the seed reaches is the
    /// blotchiness of the light - the ceilings the tiles fill to - and it reaches all of it:
    /// measured, **every one of the 1,536 tiles differs** between seed 42 and seed 43, and
    /// the only 28 numbers that match are the ones describing the cells. Phase 3 puts genomes
    /// and
    /// development on the other end of the same seed, and this test should grow to cover
    /// them then.
    #[test]
    fn a_run_is_still_reproducible() {
        let run = |seed: u64| -> World {
            let mut world = World::new(&config(|raw| {
                raw.world.seed = seed;
                raw.world.grid_cols = 48;
                raw.world.grid_rows = 32;
                raw.limits.max_organisms = 4;
            }));
            place_a_few_cells(&mut world, 2048.0, 1152.0);

            for _ in 0..2_000 {
                world.tick();
            }

            world
        };

        let once = run(42);
        let again = run(42);
        let elsewhere = run(43);

        let first = every_number_in(&once);
        let second = every_number_in(&again);
        let other = every_number_in(&elsewhere);

        assert!(
            first.len() > 1_500,
            "only {} numbers were compared, so this test is not looking at the world it \
             thinks it is",
            first.len()
        );

        let differs = first
            .iter()
            .zip(&second)
            .position(|(here, there)| here != there);
        assert_eq!(
            differs, None,
            "two runs of seed 42 disagree, first at number {differs:?} of the world"
        );

        assert!(
            (once.grid().total_energy() - again.grid().total_energy()).abs() < f64::EPSILON,
            "the two runs hold different amounts of energy"
        );
        assert_eq!(
            once.ticks(),
            again.ticks(),
            "the two runs did not take the same number of ticks"
        );

        // A different seed is a different world. Counted rather than merely found, so that a
        // seed reaching one corner of the noise and nothing else could not satisfy it.
        let apart = first
            .iter()
            .zip(&other)
            .filter(|(here, there)| here != there)
            .count();
        assert!(
            apart * 10 > first.len() * 9,
            "only {apart} of {} numbers in the world differ between seed 42 and seed 43, so \
             the seed is barely reaching the world it is supposed to be shaping",
            first.len()
        );
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The test above runs the one configuration SPEC section 3 ships. SPEC section 15 asks
    // for the claim over *any* configuration, which is a different question: the default is
    // a world somebody chose because it behaves well, and the shapes that break arithmetic
    // are the ones nobody would choose - a world one tile wide, a world with no light in it
    // at all, a ceiling a thousand times the influx, energy spreading as fast as the
    // arithmetic will allow.
    // ---------------------------------------------------------------------------------

    proptest! {
        // Fewer cases than the property tests in `grid.rs` and `ledger.rs`, which run 256.
        // Every case here builds a whole world and ticks it five hundred times, so this is
        // the most expensive property in the crate by a wide margin; the value in it is the
        // breadth of the configurations rather than the number of times each is tried.
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// However the world is configured, the books balance.
        ///
        /// SPEC section 15 asks for this in as many words. What is generated is every
        /// setting that decides how energy behaves - the shape of the field, how much light
        /// falls on it, how far the light reaches down, how blotchy it is, and how fast
        /// energy spreads - and the world is ticked with cells in it, so each case is a
        /// whole tick rather than the two thirds of one the grid accounts for.
        ///
        /// # The one bound that is not arbitrary
        ///
        /// `diffusion` is drawn up to **a quarter and no further**, and that is not this
        /// test steering around the cases it would fail. An explicit five-point stencil
        /// overshoots above a quarter and the overshoot compounds until the field is
        /// nonsense - and it conserves energy perfectly the whole way down, because
        /// overshoot moves energy rather than inventing it. So the ledger would go on
        /// reporting a healthy world, this test would go on passing, and the numbers in the
        /// field would mean nothing. `config.rs` refuses such a configuration outright,
        /// which is why generating one here would be generating a world the program will
        /// not run.
        ///
        /// The light is drawn strictly positive for a different reason: a dark world is a
        /// legitimate thing to configure and a useless thing to test conservation on, since
        /// a field that never gains anything cannot lose it either.
        ///
        /// The limits are left small and fixed. They decide how large the arenas are and
        /// nothing whatever about energy, so a property test that varied them would spend
        /// its time in the allocator instead of in the arithmetic this is about.
        #[test]
        fn energy_is_conserved_for_any_config(
            seed: u64,
            cols in 1u32..32,
            rows in 1u32..32,
            width in 64.0f64..2048.0,
            height in 64.0f64..1152.0,
            influx in 0.001f64..0.5,
            cap in 0.1f64..64.0,
            gradient in 0.0f64..=1.0,
            patchiness in 0.0f64..=1.0,
            diffusion in 0.0f64..=0.25,
        ) {
            let mut world = World::new(&config(|raw| {
                raw.world.seed = seed;
                raw.world.width = width;
                raw.world.height = height;
                raw.world.grid_cols = cols;
                raw.world.grid_rows = rows;
                raw.light.influx = influx;
                raw.light.cap = cap;
                raw.light.gradient = gradient;
                raw.light.patchiness = patchiness;
                raw.light.diffusion = diffusion;
                raw.limits.max_organisms = 1;
                raw.limits.max_cells_per_organism = 4;
            }));

            #[expect(
                clippy::cast_possible_truncation,
                reason = "the world's size came out of a configuration that has already \
                          narrowed it to 32 bits and refused anything that would not fit, \
                          so this is reading back a number that is already this size"
            )]
            let (across, down) = (width as f32, height as f32);
            place_a_few_cells(&mut world, across, down);

            for tick in 1..=500u32 {
                world.tick();

                // Measured here as well as inside the tick, because a release build only
                // asks the world itself every thousandth tick - and five hundred ticks of a
                // release build would otherwise be five hundred ticks nobody looked at.
                if tick % 50 == 0 {
                    let error = relative_error(&world);
                    prop_assert!(
                        error < 1e-6,
                        "after {tick} ticks of a {cols} x {rows} world lit at {influx} to a \
                         ceiling of {cap}, spreading at {diffusion}, the two sides of the \
                         invariant are {error} apart in relative terms"
                    );
                }
            }

            prop_assert!(
                world.ledger().influx_total() > 0.0,
                "no light fell at all on a world lit at {influx}, so this case has \
                 established nothing about conservation"
            );
        }
    }
}
