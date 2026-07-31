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
//! shed it; then the bodies move, and everything alive gets a tick older.
//!
//! The first three are one call, because `grid.rs` keeps them together and should: a tile
//! pushed past its ceiling by diffusion has to be cut back in the same breath, rather than
//! left standing over its ceiling for whoever ticks next to remember about. SPEC section 4
//! gives them in that order and gives the reason - a ceiling enforced before the energy has
//! finished moving is not a ceiling.
//!
//! The physics goes last, and the order still genuinely does not matter: nothing that moves a
//! cell reads a tile, and nothing that moves energy reads a position. It is written this way
//! for where Phase 4's work has to go. Harvesting sits between the two halves, and it wants a
//! field the light has already fallen on and cells that have not yet swum away from the tile
//! they were sitting on.
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
//! ⚠️ **The invariant does not catch a seeding that conjures its energy, and Phase 2 expected
//! it to.** Written before there was anything to try it on, the note here said that an
//! organism given energy out of nothing would be stopped immediately by the tick-zero check.
//! It is not, and the reason is worth knowing: an organism whose energy was never *told* to
//! the ledger leaves all five accounts exactly as they were, so the books balance perfectly
//! while a body stands in the world holding energy nobody counted. Nothing announces itself
//! until Phase 4, when that organism dies and its energy is moved out of a `biomass` account
//! that never received it - and the run then stops hours in, with no obvious cause, which is
//! precisely the failure the check was supposed to prevent.
//!
//! What actually stands between this project and that is
//! `seeding_an_organism_takes_its_energy_out_of_the_field`, which asserts that the *field*
//! went down rather than that the books balanced. The tick-zero check is still worth having -
//! it catches the other half, an account credited without the field being debited - but it is
//! half of the guard rather than the whole of it.
//!
//! # Everything is allocated when the world is built, and never again
//!
//! CLAUDE.md: *a simulation that cannot allocate cannot leak.* Every arena is built at the
//! largest size the configuration could ever need - every organism the world allows, each
//! with every cell it allows - and nothing here has any way to grow one. At SPEC section 3's
//! defaults that is four thousand organisms of sixty-four cells apiece: **256,000 cells,
//! about 7 MB, and 252,000 springs at about 6 MB**. The dense copies the tick hands to the
//! physics are the same two arenas again, plus one index per cell to put the crowd back where
//! it came from, so another 15 MB; the physics builds its own working arrays at 15 MB more;
//! the four thousand organisms are a third of a megabyte and the resource field is under half
//! of one. A default world is therefore around **44 MB**, against CLAUDE.md's resident target
//! of 2 GB.
//!
//! The cell and spring arenas are built at their full *length* rather than merely reserved,
//! because an organism is written into a slot and a slot has to be there to be written into.
//! That costs a memory of zeroes at startup and nothing whatever afterwards.
//!
//! # Where an organism lives, and why nothing is ever moved
//!
//! An organism owns a fixed stretch of the two arenas: **organism `n` owns cells
//! `[n × max_cells_per_organism, …)` and nothing else ever does**, with its springs slotted
//! the same way one place narrower. `organism.rs` argues the decision in full; the part that
//! matters here is that a slot is either occupied or free, that death frees one and birth
//! takes one, and that no body ever moves. There is therefore no index anywhere that has to
//! be fixed up when a population changes, which is the awkward problem Phase 2 left behind
//! and this arrangement makes not exist.
//!
//! The price is that the arena has gaps in it - a one-celled organism in a slot of sixty-four
//! leaves sixty-three cells standing empty - and the physics must not be handed those gaps. A
//! cell parked in an unused slot would be an invisible obstacle that living bodies bump into,
//! and paying for every empty slot on every tick would make a nearly-empty world cost what a
//! full one does.
//!
//! So the tick gathers the living cells into a **crowd**: a dense list, in slot order,
//! holding only what is alive, with the springs' endpoints shifted from their own body's
//! numbering into the crowd's. The physics is handed that, knows nothing about organisms or
//! slots, and hands back cells that have moved; the tick writes them home again. The two
//! walks cost one copy per living cell and nothing per empty slot, and the arrangement keeps
//! the whole of `physics.rs` free of any idea that an organism exists.
//!
//! # Seeding an organism takes its energy out of the water
//!
//! SPEC section 5 is explicit, and Phase 2 flagged it as the easy mistake of this phase: a
//! seeded organism *feels* as though it comes from outside the world, so giving it a starting
//! energy that came from nowhere is a leak on tick zero. It is taken out of the field - out
//! of the tiles the body is actually standing on - through exactly the door a photocyte will
//! use to eat, and if those tiles cannot pay, the seeding fails rather than inventing the
//! difference. See [`World::seed`].
//!
//! One consequence is worth knowing before Phase 4 tunes anything: **a world starts dark**.
//! The field fills under the light over several hundred ticks, so an organism seeded on tick
//! zero can be given nothing at all, because there is nothing there yet. Whoever seeds the
//! first population has to decide whether to let the world fill first.
//!
//! # What is deliberately not here
//!
//! **Living.** Nothing eats, spends, reproduces or dies. An organism exists, occupies a slot,
//! sits where it was put and gets older; harvesting, upkeep, reproduction, death and detritus
//! are all Phase 4. The tick moves the water and the bodies and nothing else.
//!
//! **A runner.** Nothing here decides when a run ends. SPEC section 3's `max_wall_clock_hours`
//! is a wall-clock bound, and a wall clock is exactly what a deterministic simulation must
//! not read - `clippy.toml` refuses `Instant` and `SystemTime` in this crate outright, so
//! **the world cannot time itself**, by design. Whatever ends a run does it from outside, by
//! counting the ticks this module has taken.

use crate::cell::{Cell, CellKind, Vec2};
use crate::config::Config;
use crate::development::develop;
use crate::genome::Genome;
use crate::grid::Grid;
use crate::ledger::Ledger;
use crate::organism::{Organism, cell_slot, cells_per_slot, spring_slot, springs_per_slot};
use crate::physics::{Physics, Spring, cell_capacity, wrapped};
use crate::rng::WorldRng;

/// Why an organism was not born.
///
/// Neither of these is a fault, and that is the point of naming them rather than returning
/// some general failure. Both are things a healthy world does: it fills up, and its water
/// runs thin. CLAUDE.md and SPEC section 10 both call the first one deliberate - *"a full
/// world should mean nowhere to reproduce into"* - and it is the thing that makes the memory
/// guarantee hold, because a birth that cannot find a slot is a birth that does not allocate
/// one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Refused {
    /// Every slot has somebody in it.
    WorldIsFull,

    /// The tiles the body would have stood on do not hold what it was to start life with.
    ///
    /// Both numbers are carried because the difference between them is the interesting part:
    /// an organism refused for want of a hundredth of a unit is a different situation from
    /// one asking for a hundred times what the water holds, and a caller deciding where to
    /// put its next attempt would want to know which it was looking at.
    FieldTooPoor {
        /// What the seeding asked for.
        wanted: f64,
        /// What the tiles underneath were holding between them.
        available: f64,
    },
}

/// A whole simulated world: everything in it, and how far through its run it is.
///
/// Built once from a configuration and then only ticked. Every field is private and there
/// is no way to reach past the accessors below, which is what keeps the two guarantees this
/// type exists for: that the arenas cannot grow, and that energy cannot enter or leave the
/// world except through the doors `ledger.rs` has written down.
pub struct World {
    /// The settings this world was built from, kept because the world outlives the building
    /// of it.
    ///
    /// Phase 2 did not keep them and did not need to: everything the configuration decided
    /// had already been turned into an array of the right size or a number copied into the
    /// physics. From here on that stops being true. Seeding an organism has to grow a body,
    /// which needs the development caps; the slot arithmetic needs the size of a slot; and
    /// Phase 4's metabolism and mutation each need a whole table of their own. Working any of
    /// them out again from the arenas would be reading a decision back out of its
    /// consequences.
    config: Config,

    /// The energy in the water, and the light that puts it there.
    grid: Grid,

    /// Where every unit of that energy is, and the assertion that none has been invented.
    ledger: Ledger,

    /// What pushes the cells around, and the arrays it needs to do it.
    physics: Physics,

    /// Every cell the world could ever hold, arranged in slots of `max_cells_per_organism`.
    ///
    /// Full length from the moment the world is built rather than filled as organisms
    /// arrive, because a slot has to be *there* to be written into. The cells of a slot
    /// nothing lives in are at the origin and are read by nothing.
    cells: Vec<Cell>,

    /// Every adhesion those cells could ever have, in slots of one fewer.
    ///
    /// Endpoints count from their own organism's first cell rather than from the start of
    /// this array. See `organism.rs`.
    springs: Vec<Spring>,

    /// Who is in each slot, and nothing at all in the slots that are free.
    organisms: Vec<Option<Organism>>,

    /// The slots nobody is in, most recently freed at the end.
    ///
    /// A stack rather than a search: taking the last is one instruction where scanning the
    /// arena for a gap is the whole population. It starts holding every slot in reverse, so
    /// the first organism born into an empty world takes slot 0 and the run fills the arena
    /// from the front.
    free: Vec<usize>,

    /// The next serial number to hand out, which is never handed out twice.
    ///
    /// Not the same as a slot. A slot is a place and is reused; a serial names an organism
    /// and names its private sequence of random numbers, so reusing one would give two
    /// different organisms the same numbers.
    next_serial: u64,

    /// The living cells, packed together, as the physics wants them.
    ///
    /// Rebuilt at the start of every tick and written home at the end of it. See the module
    /// documentation for why the physics is not simply handed the arena.
    crowd: Vec<Cell>,

    /// The living springs, with their endpoints shifted into the crowd's numbering.
    bonds: Vec<Spring>,

    /// Which cell of the arena each cell of the crowd came from, so it can be written back.
    live: Vec<usize>,

    /// Which tiles the body being seeded is standing on, each named once.
    ///
    /// Working room for [`World::seed`], kept here rather than made on the spot so that a
    /// birth allocates nothing.
    beneath: Vec<usize>,

    /// The run's randomness. See `rng.rs`: an organism's numbers come from its serial rather
    /// than from here, so this is the world's own supply.
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
        let slots = usize::try_from(config.limits.max_organisms.get())
            .expect("a population cap fits in a machine word");
        let adhesions = slots * springs_per_slot(&config.limits);
        let grid = Grid::new(config);
        let ledger = Ledger::new(grid.total_energy());

        let world = Self {
            config: config.clone(),
            grid,
            ledger,
            physics: Physics::new(config),
            cells: vec![Cell::new(CellKind::Photocyte, Vec2::ZERO); capacity],
            // A spring joining a cell to itself, which is what an unused slot holds and what
            // nothing ever looks at. It is never handed to the physics: only the springs an
            // organism says it has are gathered into the crowd.
            springs: vec![
                Spring {
                    a: 0,
                    b: 0,
                    rest_length: 0.0,
                    stiffness: 0.0,
                };
                adhesions
            ],
            organisms: vec![None; slots],
            // Reversed, so that popping the last hands out slot 0 first.
            free: (0..slots).rev().collect(),
            next_serial: 0,
            crowd: Vec::with_capacity(capacity),
            bonds: Vec::with_capacity(adhesions),
            live: Vec::with_capacity(capacity),
            beneath: Vec::with_capacity(cells_per_slot(&config.limits)),
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
    /// Everything a tick is, in the order the module documentation argues for: the water, the
    /// bodies in it, a year off everybody's life, and then the books. Nothing else - no
    /// growing, no dying, no eating, and nothing that reads a clock.
    ///
    /// The bodies are handed to the physics as a dense crowd rather than as the arena they
    /// live in, and put back afterwards. See the module documentation, and `gather` below.
    ///
    /// # Panics
    ///
    /// If the energy in the world stops matching the energy the books say is in it. SPEC
    /// section 5 asks for exactly this and gives the reason: eight hours of quietly wrong
    /// output is worse than a crash, and a ledger that is out at all is a ledger whose
    /// numbers have stopped describing anything.
    pub fn tick(&mut self) {
        self.grid.tick(&mut self.ledger);

        self.gather();
        self.physics.step(&mut self.crowd, &self.bonds);
        self.scatter();

        for organism in self.organisms.iter_mut().flatten() {
            organism.grow_older();
        }

        self.ticks += 1;

        if Ledger::should_check(self.ticks) {
            self.ledger.check(self.grid.total_energy());
        }
    }

    /// Put a new organism in the world, grown from this genome, with its seed cell here.
    ///
    /// The two things that can go wrong are both refusals rather than errors, and both are
    /// ordinary events in a running world rather than faults - see [`Refused`].
    ///
    /// # Where the body goes
    ///
    /// `development.rs` grows a body as a set of offsets from its seed cell, which is a shape
    /// and not a position; this is where the shape is put somewhere. Every cell lands at `at`
    /// plus its own offset, wrapped sideways and clamped at the surface and the floor,
    /// exactly as SPEC section 8 says a world behaves. A body straddling the join is
    /// therefore a perfectly ordinary body - the physics measures every pair the short way
    /// round - and a body pushed against the floor is squashed against it rather than
    /// hanging outside the world.
    ///
    /// # Where the energy comes from, which is the whole point of this being here
    ///
    /// **Out of the tiles the body is standing on**, through [`Grid::harvest`], which is the
    /// same door a photocyte will eat through in Phase 4. SPEC section 5 requires it in as
    /// many words and Phase 2 flagged it as the easy mistake of this phase: an organism
    /// handed energy from nowhere is a leak, and it is a leak on tick zero.
    ///
    /// Nothing is spread evenly or taken fairly. The tiles under the body are visited in the
    /// order the body's cells were grown, each named once however many cells stand on it, and
    /// each gives up as much as is still wanted or as much as it has, whichever is less. A
    /// body seeded on rich water pays for itself out of the first tile it is standing on.
    ///
    /// # If the water cannot pay, nothing happens at all
    ///
    /// The tiles are counted before any of them is touched, and a seeding that cannot be
    /// afforded is refused with the field exactly as it was. The alternative - take what
    /// there is and let the organism start short - would be a birth that quietly means
    /// something different from what was asked for, and it is the same choice every other cap
    /// in this project makes: a full world refuses a birth rather than making room.
    ///
    /// Worth knowing: **a world starts dark**. A seeding on tick zero can only ever be
    /// afforded if it asks for nothing, because the field is empty until the light has been
    /// falling for some hundreds of ticks.
    ///
    /// # Panics
    ///
    /// If `energy` is not a quantity of energy - negative, infinite, or not a number. That is
    /// `ledger.rs`'s rule about its own amounts applied at the door an organism comes in
    /// through, and for the same reason: a not-a-number reaching a tile is a tile whose
    /// contents stop meaning anything, and every comparison made about the world afterwards
    /// silently answers no.
    ///
    /// If the run has minted every serial number there is, which would take eighteen
    /// quintillion births. `rng.rs` keeps the last one for the world's own randomness, and an
    /// organism holding it would be drawing the world's numbers.
    pub fn seed(&mut self, genome: Genome, at: Vec2, energy: f64) -> Result<usize, Refused> {
        assert!(
            energy.is_finite() && energy >= 0.0,
            "an organism cannot be seeded holding {energy}"
        );

        let Some(&slot) = self.free.last() else {
            return Err(Refused::WorldIsFull);
        };

        let body = develop(&genome, &self.config.limits);
        let first_cell = cell_slot(slot, &self.config.limits).start;
        let first_spring = spring_slot(slot, &self.config.limits).start;

        // The body is written into its slot before the seeding is settled, which is safe
        // precisely because the slot is free: nothing in the world reads a slot nobody is in,
        // so a refusal below leaves cells lying in it that no tick will ever look at. The
        // alternative is a second copy of the body somewhere to hold it while the water is
        // counted, which is a copy made on every birth for the sake of the births that fail.
        for (local, grown) in body.cells.iter().enumerate() {
            let mut cell = Cell::new(grown.kind, self.placed(at, grown.offset));
            cell.state = grown.state.get();
            self.cells[first_cell + local] = cell;
        }
        for (local, spring) in body.springs.iter().enumerate() {
            self.springs[first_spring + local] = *spring;
        }

        // The tiles underneath, each named once. A body is a few cells across and a tile is
        // several cells wide, so most bodies stand on one or two tiles and several of their
        // cells share each - and a tile counted twice is a tile that appears to hold twice
        // what it does.
        self.beneath.clear();
        for local in 0..body.cells.len() {
            let tile = self.grid.tile_at(self.cells[first_cell + local].pos);
            if !self.beneath.contains(&tile) {
                self.beneath.push(tile);
            }
        }

        let available: f64 = self
            .beneath
            .iter()
            .map(|&tile| f64::from(self.grid.tiles()[tile]))
            .sum();
        if available < energy {
            return Err(Refused::FieldTooPoor {
                wanted: energy,
                available,
            });
        }

        // What the tiles actually gave up, rather than what was asked of them. The two differ
        // by a rounding, because a tile is a 32-bit number and this is not; the organism is
        // given the realised figure so that its energy and the ledger's `biomass` account are
        // the same quantity rather than nearly the same one.
        let mut taken = 0.0;
        for &tile in &self.beneath {
            if taken >= energy {
                break;
            }
            taken += self.grid.harvest(&mut self.ledger, tile, energy - taken);
        }

        self.free.pop();
        self.organisms[slot] = Some(Organism::new(
            genome,
            taken,
            self.next_serial,
            body.cells.len(),
            body.springs.len(),
        ));
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .expect("a run has minted every serial number there is");

        Ok(slot)
    }

    /// Where a body's cell goes, given where its seed cell was put.
    ///
    /// SPEC section 8's boundaries, applied once at birth rather than left for the physics to
    /// sort out on the first tick: sideways the world joins up, and top and bottom it stops.
    /// The clamp is against cell *centres*, which is the literal reading SPEC section 8 gives
    /// and says it is giving - a cell resting on the floor has half of itself below it.
    fn placed(&self, at: Vec2, offset: Vec2) -> Vec2 {
        let world = &self.config.world;

        Vec2::new(
            wrapped(at.x + offset.x, world.width),
            (at.y + offset.y).clamp(0.0, world.height),
        )
    }

    /// Collect the living cells into one dense crowd for the physics.
    ///
    /// Slot order, so the crowd is the same crowd in the same order every time it is built
    /// from the same world - which is what stops the population being processed in a
    /// different order between two runs of one seed and giving two different sets of
    /// roundings.
    ///
    /// Springs are shifted as they are copied: an organism's spring joins its own cells 0 and
    /// 1, and the crowd wants whatever those cells became once the bodies before it had been
    /// laid down. That shift is the only translation in the whole arrangement, and it is
    /// possible only because a spring never joins two organisms.
    fn gather(&mut self) {
        self.crowd.clear();
        self.bonds.clear();
        self.live.clear();

        for slot in 0..self.organisms.len() {
            let Some((cells, springs)) = self.organisms[slot]
                .as_ref()
                .map(|organism| (organism.cells(), organism.springs()))
            else {
                continue;
            };

            // Where this organism's body begins in the crowd, which is what its springs have
            // to be shifted by.
            let here = self.crowd.len();

            let first_cell = cell_slot(slot, &self.config.limits).start;
            for index in first_cell..first_cell + cells {
                self.live.push(index);
                self.crowd.push(self.cells[index]);
            }

            let first_spring = spring_slot(slot, &self.config.limits).start;
            for index in first_spring..first_spring + springs {
                let spring = self.springs[index];
                self.bonds.push(Spring {
                    a: here + spring.a,
                    b: here + spring.b,
                    ..spring
                });
            }
        }
    }

    /// Write the crowd back into the arena it came from.
    fn scatter(&mut self) {
        for (dense, &index) in self.live.iter().enumerate() {
            self.cells[index] = self.crowd[dense];
        }
    }

    /// How many ticks this world has taken.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// The settings this world was built from.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Who is in each slot, and nothing in the slots that are free.
    #[must_use]
    pub fn organisms(&self) -> &[Option<Organism>] {
        &self.organisms
    }

    /// The cells of the organism in this slot, and none of the slot's spare room.
    ///
    /// Empty for a slot nobody is in, which is the honest answer rather than a refusal: an
    /// empty slot has no cells, and there is nothing for a caller to do about it that it
    /// would not do about an organism of no cells.
    ///
    /// # Panics
    ///
    /// If there is no such slot in this world.
    #[must_use]
    pub fn cells_of(&self, slot: usize) -> &[Cell] {
        let first = cell_slot(slot, &self.config.limits).start;
        let count = self.organisms[slot].as_ref().map_or(0, Organism::cells);

        &self.cells[first..first + count]
    }

    /// The adhesions of the organism in this slot, with their endpoints numbered from that
    /// organism's own first cell.
    ///
    /// # Panics
    ///
    /// If there is no such slot in this world.
    #[must_use]
    pub fn springs_of(&self, slot: usize) -> &[Spring] {
        let first = spring_slot(slot, &self.config.limits).start;
        let count = self.organisms[slot].as_ref().map_or(0, Organism::springs);

        &self.springs[first..first + count]
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

    /// The whole cell arena, to read - the slots that are lived in and the slots that are
    /// not.
    ///
    /// Whatever wants only the living should walk the slots with [`World::cells_of`]. This is
    /// here because the arena is what the Phase 9 port hands to a graphics card whole, and
    /// because a test that wants to prove an organism is in its own slot has to be able to
    /// look at the cells either side of it.
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
    use crate::config::{LimitsConfig, RawConfig, spec_defaults};
    use crate::genome::{Action, Gene, Genome, SensorTarget, State};
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

        // The living cells, slot by slot, rather than the whole arena. The arena is mostly
        // empty slots holding cells that nothing has ever written to, and two runs would
        // agree about those however wrong everything else had gone.
        for slot in 0..world.organisms().len() {
            for cell in world.cells_of(slot) {
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
        }

        written
    }

    /// Put two bodies in the world, so that a tick is a whole tick.
    ///
    /// A world with nothing alive in it exercises the field and none of the physics, so these
    /// tests would be measuring two thirds of a tick. The two chains are seeded a few units
    /// apart, which is closer than their cells are wide, so both of the forces `physics.rs`
    /// knows about are being computed while the energy is being counted: the springs down
    /// each chain, and the collisions between the two bodies.
    ///
    /// They are seeded asking for **no energy**, and that is not a shortcut around the
    /// ledger. A world starts dark and fills over some hundreds of ticks, so an organism
    /// seeded on tick zero could not be given anything anyway - and these tests are about the
    /// field and the physics rather than about what an organism holds. `D3` and `D4` are
    /// where the energy is the point, and both of them let the light fall first.
    fn place_two_bodies(world: &mut World) {
        let limits = world.config().limits.clone();
        let (width, height) = (world.config().world.width, world.config().world.height);
        let at = Vec2::new(width * 0.25, height * 0.5);

        for offset in [Vec2::ZERO, Vec2::new(3.0, 1.5)] {
            world
                .seed(a_chain(3, &limits), at + offset, 0.0)
                .expect("these tests configure a world with room for two organisms");
        }
    }

    /// A genome that grows a straight chain of `segments` cells, each sprung to the one
    /// before it.
    ///
    /// One gene per join, each firing on one step only and handing its daughter a fresh
    /// state, so exactly one cell divides per step and the result is a chain rather than a
    /// cluster. Every angle is nought, so the chain runs along the seed cell's own axis -
    /// which `development.rs` fixes as `+x` - and the arm length is the gene's `rest_length`,
    /// where an adhered daughter is placed.
    ///
    /// The shape matters to these tests only in that it is *known*: a body of `segments`
    /// cells at `8 × n` world units along `x` from the seed, with `segments - 1` springs
    /// joining consecutive cells. Every claim below about where an organism's cells landed is
    /// checked against that.
    fn a_chain(segments: u8, limits: &LimitsConfig) -> Genome {
        let genes = (0..segments.saturating_sub(1))
            .map(|generation| Gene {
                trigger_state: State::new(generation),
                min_step: generation,
                max_step: generation,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(generation + 1),
                child_kind: CellKind::Photocyte,
                rest_length: 8.0,
                stiffness: 10.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            })
            .collect();

        Genome::new(genes, limits)
    }

    /// ⭐ An organism's cells live in one fixed stretch of the arena, decided by its slot and
    /// by nothing else.
    ///
    /// `docs/PHASE3.md`'s first architectural decision, and the whole reason it was taken
    /// before any of this was written. A body is a *range* of a flat array, and the obvious
    /// alternative - packing bodies in end to end - means that removing a dead one from the
    /// middle moves every body above it and invalidates every spring index pointing into
    /// them. Fixed slots make that problem not exist: **organism `n` owns cells
    /// `[n × max_cells_per_organism, …)` and nothing else ever does**, so nothing ever moves
    /// and no index ever needs fixing.
    ///
    /// The claim is therefore about the *gap*, and that is what this test is mostly made of.
    /// A three-celled organism in slot 0 uses three of its eight cells and leaves five
    /// untouched; the five-celled organism that follows it starts at cell 8 rather than at
    /// cell 3. A packed arena would put it at 3, and every assertion here except that one
    /// would still pass.
    ///
    /// # Springs are slotted the same way, and their ends are numbered from their own body
    ///
    /// A spring belongs to the organism whose cells it joins - development creates one only
    /// when a gene divides a cell into a daughter that stays attached - so there can never be
    /// more than `max_cells_per_organism - 1` of them per organism, and they get a slot of
    /// that size. Their endpoints are stored **local to the organism**: the first spring of
    /// the body in slot 1 joins that body's cells 0 and 1, not the arena's cells 8 and 9.
    ///
    /// That is not tidiness. A spring holding two cells of *different* organisms would haul
    /// two bodies together across the world - SPEC section 8 warns that springs are not
    /// found by the neighbour search and have no length limit - and an endpoint that cannot
    /// name a cell outside its own body cannot express one.
    #[test]
    fn an_organism_occupies_one_fixed_slot() {
        let mut world = World::new(&config(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        let first = world
            .seed(a_chain(3, &limits), Vec2::new(100.0, 200.0), 0.0)
            .expect("an empty world has room for an organism");
        let second = world
            .seed(a_chain(5, &limits), Vec2::new(400.0, 200.0), 0.0)
            .expect("a world with one organism in it has room for another");

        assert_eq!(first, 0, "the first organism did not take the first slot");
        assert_eq!(
            second, 1,
            "the second organism did not take the second slot"
        );

        // Where each body actually is in the arena. Written out as arena indices rather than
        // asked of an accessor, because the accessor is what would be wrong if the slot
        // arithmetic were.
        assert_eq!(world.cells()[0].pos, Vec2::new(100.0, 200.0));
        assert_eq!(world.cells()[1].pos, Vec2::new(108.0, 200.0));
        assert_eq!(world.cells()[2].pos, Vec2::new(116.0, 200.0));
        assert_eq!(
            world.cells()[8].pos,
            Vec2::new(400.0, 200.0),
            "the second organism's body did not begin at cell 8, so bodies are being packed \
             in end to end and a death would move every body above it"
        );
        assert_eq!(world.cells()[12].pos, Vec2::new(432.0, 200.0));

        // The five cells of slot 0 that its three-celled organism did not need. A free cell
        // is at the origin because that is where the arena was built, and nothing has written
        // to it since.
        for spare in 3..8 {
            assert_eq!(
                world.cells()[spare].pos,
                Vec2::ZERO,
                "cell {spare} belongs to slot 0 and has been written to by something else"
            );
        }

        assert_eq!(world.cells_of(first).len(), 3, "the first body is 3 cells");
        assert_eq!(
            world.cells_of(second).len(),
            5,
            "the second body is 5 cells"
        );

        // Springs are slotted at seven per organism - one fewer than the cells - and their
        // ends count from their own body's first cell. The arena indices are written out as
        // literals for the same reason the cells' were: slot 1's springs begin at spring 7,
        // and asking an accessor where they begin would only be asking the arithmetic under
        // test to confirm itself.
        assert_eq!(world.springs_of(first).len(), 2);
        assert_eq!(world.springs_of(second).len(), 4);
        assert_eq!(
            (world.springs[7].a, world.springs[7].b),
            (0, 1),
            "the first spring of the organism in slot 1 is either not at spring 7 of the \
             arena, or names cells of the arena rather than cells of its own body - and an \
             endpoint that can name the arena is one that can point at another organism"
        );
        assert_eq!(
            (world.springs[10].a, world.springs[10].b),
            (3, 4),
            "the last spring of a five-celled body in slot 1 is not where the slot puts it"
        );

        // And the slot is fixed for the organism's life rather than only at the moment it was
        // born. Nothing in a tick may move a body from one stretch of the arena to another.
        for _ in 0..100 {
            world.tick();
        }

        assert_eq!(world.cells_of(first).len(), 3);
        assert_eq!(world.cells_of(second).len(), 5);
        for spare in 3..8 {
            assert_eq!(
                world.cells()[spare].pos,
                Vec2::ZERO,
                "a hundred ticks wrote into cell {spare}, which no organism owns"
            );
        }
        assert!(
            (world.cells()[8].pos.x - 400.0).abs() < 1.0,
            "the second organism's body has left its slot, or has been thrown across the \
             world by a force nothing asked for"
        );
    }

    /// ⭐ When there is nowhere to put an organism, the birth **fails**. It does not make
    /// room.
    ///
    /// CLAUDE.md, on the population cap: *"When the population cap is reached, births fail
    /// rather than allocating. (This is also biologically reasonable: a full world should
    /// mean nowhere to reproduce into.) A simulation that cannot allocate cannot leak."*
    /// SPEC section 10 says the same thing from the other end - births fail silently at
    /// `max_organisms` - and between them they are the whole of the memory guarantee. Every
    /// arena in this project is built at startup at the size the configuration implies, and
    /// what makes that a *guarantee* rather than an intention is that the code has nowhere to
    /// put an organism it has no room for.
    ///
    /// # Why this measures capacity rather than length
    ///
    /// Because a test that only checked the refusal would pass against an implementation that
    /// grew the arena and then declined to use it, and - much more likely - against one that
    /// grew the arena and *did* use it while returning the error from somewhere else. The
    /// failure this is guarding against is not a wrong answer, it is a `Vec` quietly doubling
    /// itself in a process that is meant to have a fixed footprint for the length of an
    /// overnight run.
    ///
    /// So what is compared is the capacity of every arena and mirror in the world, before and
    /// after a refused birth, together with the addresses the two largest of them live at. A
    /// vector that has grown has moved, and a vector that has moved has copied everything it
    /// held to somewhere new and handed the old space back.
    #[test]
    fn a_birth_fails_at_the_cap_rather_than_allocating() {
        let mut world = World::new(&config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 3;
            raw.limits.max_cells_per_organism = 4;
        }));
        let limits = world.config().limits.clone();

        for (slot, along) in [40.0f32, 100.0, 160.0].into_iter().enumerate() {
            let born = world.seed(a_chain(2, &limits), Vec2::new(along, 72.0), 0.0);
            assert_eq!(born, Ok(slot), "the world had room for organism {slot}");
        }

        let capacities = [
            world.cells.capacity(),
            world.springs.capacity(),
            world.organisms.capacity(),
            world.free.capacity(),
            world.crowd.capacity(),
            world.bonds.capacity(),
            world.live.capacity(),
        ];
        let addresses = [
            world.cells.as_ptr().cast::<u8>(),
            world.springs.as_ptr().cast::<u8>(),
        ];
        let arena = world.cells.len();

        let refused = world.seed(a_chain(2, &limits), Vec2::new(200.0, 72.0), 0.0);

        assert_eq!(
            refused,
            Err(Refused::WorldIsFull),
            "a world of three organisms accepted a fourth"
        );
        assert_eq!(
            capacities,
            [
                world.cells.capacity(),
                world.springs.capacity(),
                world.organisms.capacity(),
                world.free.capacity(),
                world.crowd.capacity(),
                world.bonds.capacity(),
                world.live.capacity(),
            ],
            "a refused birth grew one of the world's arenas, so the memory this run uses is \
             decided by how many organisms try to be born rather than by the configuration"
        );
        assert_eq!(
            addresses,
            [
                world.cells.as_ptr().cast::<u8>(),
                world.springs.as_ptr().cast::<u8>()
            ],
            "a refused birth moved an arena somewhere else in memory"
        );
        assert_eq!(
            arena,
            world.cells.len(),
            "a refused birth lengthened the arena"
        );

        // And the world is exactly as it was: three organisms, no slots free, and it still
        // ticks. A refusal is an ordinary event rather than damage.
        assert_eq!(world.organisms().iter().flatten().count(), 3);
        assert!(world.free.is_empty());
        world.tick();
        assert_eq!(world.organisms().iter().flatten().count(), 3);
    }

    /// ⭐ A seeded organism's energy comes **out of the field**, from the tiles its body is
    /// standing on.
    ///
    /// SPEC section 5, in as many words, and `docs/PHASE2.md` flagged it as the trap of this
    /// phase. A seeded organism feels as though it comes from outside the world - it is not
    /// born of a parent, nothing paid for it, somebody simply asked for it - so giving it a
    /// starting energy out of nowhere is the natural thing to write. It is a leak, and it is a
    /// leak on tick zero, which then shows up hours later as an invariant failure with no
    /// obvious cause.
    ///
    /// # What the assertions have to be about
    ///
    /// The field, and not only the organism. A version of this that checked the organism had
    /// been given what it asked for, and that the books still balanced, would pass perfectly
    /// against an implementation that conjured the energy and told nobody - because the books
    /// balance beautifully when energy is invented *outside* them. That is the whole reason
    /// this failure is hard to find. So what is measured here is that the tiles the body
    /// stands on went **down**, by what the organism went **up** by, and that no other tile
    /// in the world moved.
    ///
    /// # A world starts dark
    ///
    /// The first claim below is the one that catches everybody. The field is empty when a
    /// world is built and fills over some hundreds of ticks, so an organism seeded on tick
    /// zero can be given nothing whatever: there is nothing there to give it. The light is
    /// therefore left to fall for seven hundred ticks before anything is seeded here, and
    /// whoever sets a real run going has the same decision to make.
    ///
    /// # And if the water cannot pay
    ///
    /// The seeding is refused and the world is left exactly as it was - not part-paid, and
    /// not with a body standing in it holding less than it was meant to. That is the same
    /// answer the population cap gives in the test above, for the same reason: a cap that
    /// bends is not a cap.
    #[test]
    fn seeding_an_organism_takes_its_energy_out_of_the_field() {
        let mut world = World::new(&config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();
        let at = Vec2::new(100.0, 72.0);

        // A dark world can afford nothing at all.
        assert_eq!(
            world.seed(a_chain(3, &limits), at, 1.0),
            Err(Refused::FieldTooPoor {
                wanted: 1.0,
                available: 0.0
            }),
            "a world that has not been lit yet paid for an organism out of an empty field"
        );

        for _ in 0..700 {
            world.tick();
        }

        // The tiles this body will stand on: three cells eight units apart, over tiles eight
        // units wide.
        let field_before = world.grid().total_energy();
        let tiles_before: Vec<f32> = world.grid().tiles().to_vec();

        let slot = world
            .seed(a_chain(3, &limits), at, 5.0)
            .expect("a lit world holds more than five units under a three-celled body");

        let organism = world.organisms()[slot]
            .as_ref()
            .expect("the slot the seeding returned holds the organism it made");
        let field_after = world.grid().total_energy();

        assert!(
            (organism.energy() - 5.0).abs() < 1e-6,
            "the organism was given {} rather than the five units it was seeded with",
            organism.energy()
        );
        assert!(
            (field_before - field_after - organism.energy()).abs() < 1e-9,
            "the field went down by {} while the organism went up by {} - so the two are \
             not the same energy, and the world has either invented or destroyed the \
             difference",
            field_before - field_after,
            organism.energy()
        );
        assert!(
            (world.ledger().biomass() - organism.energy()).abs() < 1e-12,
            "the books say {} is alive in this world and the only organism in it is holding \
             {}",
            world.ledger().biomass(),
            organism.energy()
        );

        // Which tiles paid. The body stands on three of them and they are the three that
        // moved; everything else in the field is untouched, which is what says the energy was
        // taken from *under the body* rather than levied on the world at large.
        let standing_on: Vec<usize> = world
            .cells_of(slot)
            .iter()
            .map(|cell| world.grid().tile_at(cell.pos))
            .collect();
        assert_eq!(
            standing_on.len(),
            3,
            "the three cells of this body are meant to be on three different tiles, so the \
             test is not looking at what it thinks it is"
        );

        let mut paid = 0.0;
        for (tile, (before, after)) in tiles_before.iter().zip(world.grid().tiles()).enumerate() {
            let fell = f64::from(*before) - f64::from(*after);
            if standing_on.contains(&tile) {
                paid += fell;
            } else {
                assert!(
                    fell.abs() < f64::EPSILON,
                    "tile {tile} is nowhere near the organism and gave up {fell}"
                );
            }
        }
        assert!(
            (paid - organism.energy()).abs() < 1e-12,
            "the tiles under the body gave up {paid} and the organism is holding {}",
            organism.energy()
        );

        // A seeding nobody can afford changes nothing whatever.
        let untouched = world.grid().tiles().to_vec();
        let biomass = world.ledger().biomass();
        let refused = world.seed(a_chain(3, &limits), at, 10_000.0);

        assert!(
            matches!(refused, Err(Refused::FieldTooPoor { wanted, .. }) if wanted > 9_999.0),
            "a body was seeded holding ten thousand units out of water that has never held \
             more than a few thousand"
        );
        assert_eq!(
            untouched,
            world.grid().tiles(),
            "a refused seeding took energy out of the field on its way to failing, which is \
             the worst of both answers: the organism does not exist and the water paid for \
             it anyway"
        );
        assert!(
            (world.ledger().biomass() - biomass).abs() < f64::EPSILON,
            "a refused seeding moved the biomass account"
        );
        assert_eq!(
            world.organisms().iter().flatten().count(),
            1,
            "a refused seeding left an organism in the world"
        );
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
    /// rather than remembered. And that a thousand ticks leave every arena at the same
    /// address and the same size, which is what would stop being true the moment anything in
    /// here pushed onto a full vector.
    ///
    /// # The cell arena is full from the start, which is new in Phase 3
    ///
    /// Phase 2 built it empty and would have filled it as organisms arrived. It is now
    /// built at its full length, because an organism is given a fixed *slot* in it - the
    /// thousandth organism writes into cells 64,000 onwards whether or not the first
    /// nine hundred and ninety-nine ever existed - and a slot has to be there to be written
    /// into. What that costs is a memory of zeroes at startup and nothing at all afterwards.
    #[test]
    fn the_arenas_are_allocated_once_at_the_size_the_config_asks_for() {
        let default_world = World::new(&config(|_| {}));

        assert_eq!(
            default_world.cells.len(),
            256_000,
            "SPEC section 3's four thousand organisms of sixty-four cells"
        );
        assert_eq!(
            default_world.springs.len(),
            252_000,
            "a spring is made once per cell that stays attached to its parent, so a body of \
             sixty-four cells can have at most sixty-three of them"
        );
        assert_eq!(default_world.organisms.len(), 4_000);
        assert!(
            default_world.organisms.iter().all(Option::is_none)
                && default_world.free.len() == 4_000,
            "a world starts with every slot free and nothing alive in it"
        );

        // What the arenas actually cost, so the figures in the module documentation are
        // checked rather than remembered. 28 bytes a cell and 24 a spring: 13,216,000 bytes
        // for the two that hold the world, and the same again less a little for the two the
        // tick packs the living into, plus one index per cell to put them back.
        let arenas = default_world.cells.len() * size_of::<Cell>()
            + default_world.springs.len() * size_of::<Spring>();
        let mirrors = default_world.crowd.capacity() * size_of::<Cell>()
            + default_world.bonds.capacity() * size_of::<Spring>()
            + default_world.live.capacity() * size_of::<usize>();
        assert!(
            (12_000_000..15_000_000).contains(&arenas),
            "the two arenas of a default world cost {arenas} bytes, against the 13,216,000 \
             recorded here"
        );
        assert!(
            (14_000_000..17_000_000).contains(&mirrors),
            "the dense copies the physics is handed cost {mirrors} bytes, against the \
             15,264,000 recorded here"
        );

        let odd_shape = World::new(&config(|raw| {
            raw.limits.max_organisms = 9;
            raw.limits.max_cells_per_organism = 5;
        }));
        assert_eq!(
            odd_shape.cells.len(),
            45,
            "the arena ignored the configured limits"
        );
        assert_eq!(
            odd_shape.springs.len(),
            36,
            "nine slots of four springs, which is one fewer than five cells"
        );

        // And it is that size once. A world that reallocated as it ran would copy every cell
        // in it to a new address and hand the old space back, for ever, on a machine with
        // other things to be getting on with.
        let mut running = World::new(&config(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
        }));
        place_two_bodies(&mut running);

        let addresses = [
            running.cells.as_ptr().cast::<u8>(),
            running.springs.as_ptr().cast::<u8>(),
            running.crowd.as_ptr().cast::<u8>(),
            running.bonds.as_ptr().cast::<u8>(),
        ];
        let sizes = [
            running.cells.capacity(),
            running.springs.capacity(),
            running.crowd.capacity(),
            running.bonds.capacity(),
        ];

        for _ in 0..1_000 {
            running.tick();
        }

        assert_eq!(
            addresses,
            [
                running.cells.as_ptr().cast::<u8>(),
                running.springs.as_ptr().cast::<u8>(),
                running.crowd.as_ptr().cast::<u8>(),
                running.bonds.as_ptr().cast::<u8>(),
            ],
            "a thousand ticks moved the cells somewhere else in memory, so something in \
             here is allocating as it goes - and the two mirrors are the ones to suspect, \
             because they are emptied and refilled on every single tick"
        );
        assert_eq!(
            sizes,
            [
                running.cells.capacity(),
                running.springs.capacity(),
                running.crowd.capacity(),
                running.bonds.capacity(),
            ],
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

    /// ⭐ Phase 2's headline claim, re-run over a world that now has bodies in it.
    ///
    /// `energy_is_conserved_over_100k_ticks` proved the books balance over a world of water
    /// and light. Everything since then has put things *in* that world: organisms holding
    /// energy of their own, taken out of the field at the moment they were seeded, with cells
    /// being pushed about by the physics on every tick afterwards. This asks the same question
    /// of that world.
    ///
    /// # It has to prove the world was doing something, and that is most of the test
    ///
    /// Phase 2's lesson, learned the hard way: **energy is conserved perfectly in a world
    /// where nothing happens.** A `tick` that had been emptied out would sail through every
    /// conservation claim there is, and so would a world in which the seeding quietly did
    /// nothing at all. So the closing assertions are about the world having *run*: light has
    /// fallen, the field is standing at a real quantity, energy has been leaving through tiles
    /// that cannot hold what reaches them, there is biomass in the books, the organisms have
    /// aged, and their cells have **moved** from where they were put.
    ///
    /// That last one is the one this test adds. The bodies are seeded overlapping one another
    /// on purpose, so the collision force in `physics.rs` has something to do; a world whose
    /// organisms were seeded politely apart would sit perfectly still, because a body is grown
    /// with its springs already at rest.
    ///
    /// # And that the books know about the organisms
    ///
    /// The ledger's `biomass` account and the sum of what the organisms are holding are two
    /// records of one quantity, and they are checked against each other here. They are allowed
    /// to differ by a rounding and no more - both are 64-bit numbers built out of the same
    /// realised harvests, so anything larger than that means one of them has been written to
    /// without the other, which is precisely the bookkeeping that goes wrong.
    ///
    /// # The drift settles, and what settles is not what a reader would expect
    ///
    /// The same argument as Phase 2's: an error that is inside the tolerance at ten thousand
    /// ticks and *growing* is a run that stops in the small hours with no bug to find. So the
    /// worst discrepancy over the whole run is compared against the worst over its first
    /// tenth, and an error accumulating in one direction would be ten times larger by the end.
    /// **Measured: 6.12e-9 over the whole run against 2.94e-9 over its first tenth** - a
    /// factor of two where a leak would give ten.
    ///
    /// That claim is about the *relative* error, and the distinction is worth spelling out
    /// because the absolute figure behaves differently and would alarm anybody who looked at
    /// it. The books run **short**, by about five billionths of a unit per tick, and that
    /// shortfall does **not** level off: 4.8e-4 units by a hundred thousand ticks, 2.3e-3 by
    /// half a million, 4.2e-3 by nine hundred thousand, in a straight line. It is a real slow
    /// loss in the field's arithmetic rather than energy in transit - see this phase's Q12,
    /// which is where it is written up, because it belongs to `grid.rs` rather than to
    /// anything Group D added.
    ///
    /// The reason it is nevertheless harmless is that the other side of the invariant grows in
    /// a straight line too: the light puts about 0.78 units a tick into this world against
    /// that five billionths. **The ratio therefore converges**, and it converges to a number
    /// far below the tolerance: 4.9e-9 at fifty thousand ticks, 6.6e-9 at half a million,
    /// 6.71e-9 at nine hundred thousand and still 6.71e-9 after that. SPEC section 5 states
    /// the invariant as a *relative* one, and this is the case that makes that wording load-
    /// bearing rather than cosmetic - an overnight run of tens of millions of ticks sits at
    /// seven parts in a billion, a hundred and fifty thousand times inside the 1e-3 allowed.
    ///
    /// Recorded 31 July 2026, Windows 11 x86-64: relative error **5.88e-9** after 120,000
    /// ticks, against SPEC section 5's tolerance of 1e-3.
    ///
    /// # Why this one is marked `ignore` and still runs on every check
    ///
    /// The same trade as `energy_is_conserved_over_100k_ticks`, for the same reason: 120,000
    /// ticks is two seconds of arithmetic in a release build and half a minute in a debug
    /// one, and a suite that takes half a minute longer than it needs to is a suite somebody
    /// stops running. `scripts/check.ps1` passes `--include-ignored` to the release pass, so
    /// this is proved on every check - once, in the profile where it is cheap.
    ///
    /// The debug pass is not left without organisms in it. `an_organism_occupies_one_fixed_slot`
    /// ticks a world with two bodies in it, `the_arenas_are_allocated_once_at_the_size_the_config_asks_for`
    /// ticks one a thousand times, and `energy_is_conserved_for_any_config` ticks sixty-four
    /// differently-shaped ones with bodies in them five hundred times each.
    ///
    /// To run it in debug anyway:
    /// `cargo test -p coacervate-sim -- --ignored energy_is_still_conserved_with_organisms_present`
    #[test]
    #[ignore = "30s in debug; check.ps1 runs it via --include-ignored in the release pass"]
    fn energy_is_still_conserved_with_organisms_present() {
        let mut world = World::new(&config(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 16;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        // The field fills under the light first. A world seeded on tick zero is a world where
        // nothing can be afforded - see `seeding_an_organism_takes_its_energy_out_of_the_field`.
        for _ in 0..1_000 {
            world.tick();
        }

        // Two clumps of four, each clump seeded on top of itself so the bodies overlap and the
        // physics has real work to do.
        for clump in [100.0f32, 300.0] {
            for member in 0..4u8 {
                let along = f32::from(member);
                world
                    .seed(
                        a_chain(4, &limits),
                        Vec2::new(clump + along * 2.0, 144.0 + along),
                        2.0,
                    )
                    .expect("a lit world holds two units under a four-celled body");
            }
        }

        let population = world.organisms().iter().flatten().count();
        let seeded_at: Vec<Vec2> = (0..population)
            .map(|slot| world.cells_of(slot)[0].pos)
            .collect();
        let alive = world.ledger().biomass();

        let mut worst = 0.0f64;
        let mut worst_early = 0.0f64;

        for tick in 1..=120_000u64 {
            world.tick();

            if tick % 100 == 0 {
                let error = relative_error(&world);
                worst = worst.max(error);
                if tick <= 12_000 {
                    worst_early = worst_early.max(error);
                }
            }
        }

        let final_error = relative_error(&world);
        let held = world.grid().total_energy();
        let ledger = world.ledger();

        assert!(
            final_error < 1e-8,
            "with eight bodies in it, the two sides of the invariant finished {final_error} \
             apart in relative terms, and SPEC section 5's tolerance of 1e-3 is meant to be \
             covering the rounding in `f32` diffusion rather than an actual leak"
        );
        assert!(
            worst < 1e-8,
            "the books were out by {worst} at their worst moment of the run"
        );
        assert!(
            worst < worst_early * 4.0,
            "the worst discrepancy over the whole run was {worst}, against {worst_early} \
             over its first tenth - so the error is growing with time rather than settling, \
             and a run a hundred times longer would not survive it"
        );

        // The world was running rather than sitting still and conserving nothing perfectly.
        assert_eq!(population, 8, "the eight seedings did not all take");
        assert!(
            ledger.influx_total() > 90_000.0,
            "only {} units of light fell over a hundred and twenty thousand ticks",
            ledger.influx_total()
        );
        assert!(
            held > 10_000.0,
            "the field is holding {held} and should be full"
        );
        assert!(
            ledger.dissipated() > 10_000.0,
            "nothing has left the world through tiles that could not hold it"
        );
        assert!(
            ledger.detritus() < f64::EPSILON,
            "something has died in a phase that has no death in it"
        );

        // The organisms are in the books, and the books agree with them.
        let holding: f64 = world
            .organisms()
            .iter()
            .flatten()
            .map(Organism::energy)
            .sum();
        assert!(
            (alive - 16.0).abs() < 1e-6,
            "eight organisms seeded with two units each left {alive} in the biomass account"
        );
        assert!(
            (ledger.biomass() - holding).abs() < 1e-9,
            "the books say {} is alive and the organisms are holding {holding} between them",
            ledger.biomass()
        );

        // And the bodies were being pushed about while all that was counted.
        for slot in 0..population {
            let organism = world.organisms()[slot]
                .as_ref()
                .expect("nothing dies in this phase");
            assert_eq!(
                organism.age(),
                120_000,
                "the organism in slot {slot} did not age with the world"
            );
        }
        let moved = (0..population)
            .filter(|&slot| (world.cells_of(slot)[0].pos - seeded_at[slot]).length() > 1.0)
            .count();
        assert!(
            moved >= 6,
            "only {moved} of the {population} bodies have moved a whole world unit from \
             where they were seeded, and they were deliberately seeded on top of one \
             another - so either the physics is not being handed the organisms or it is not \
             being run at all"
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
    /// It asks the question of the field alone, and that is worth being plain about: nothing
    /// in the physics reads a random number, so two worlds differing only in their seed push
    /// their bodies around identically. Nor does anything else here: a genome handed to
    /// [`World::seed`] is the genome that grows, so two seeds grow the same body in the same
    /// place. **The seed will reach the organisms in Phase 4**, when reproduction mutates a
    /// genome out of an organism's own stream, and this test should grow to cover that when
    /// it does.
    ///
    /// What the seed reaches today is the blotchiness of the light - the ceilings the tiles
    /// fill to - and it reaches all of it: measured, **every one of the 1,536 tiles differs**
    /// between seed 42 and seed 43, and the only 42 numbers that match are the six living
    /// cells'.
    #[test]
    fn a_run_is_still_reproducible() {
        let run = |seed: u64| -> World {
            let mut world = World::new(&config(|raw| {
                raw.world.seed = seed;
                raw.world.grid_cols = 48;
                raw.world.grid_rows = 32;
                raw.limits.max_organisms = 4;
            }));
            place_two_bodies(&mut world);

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
                raw.limits.max_organisms = 2;
                raw.limits.max_cells_per_organism = 4;
            }));

            place_two_bodies(&mut world);

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
