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
//! shed it; then the bodies feed and move; then everything alive gets a tick older, pays for
//! the tick it has had, and is taken away if it did not survive it.
//!
//! The first three are one call, because `grid.rs` keeps them together and should: a tile
//! pushed past its ceiling by diffusion has to be cut back in the same breath, rather than
//! left standing over its ceiling for whoever ticks next to remember about. SPEC section 4
//! gives them in that order and gives the reason - a ceiling enforced before the energy has
//! finished moving is not a ceiling.
//!
//! Then, last of all, the survivors have children. See the note beside the call: a slot freed
//! by this tick's deaths can be born into on this tick, a body that did not survive the tick
//! does not breed on its way out, and a newborn is put down beside a parent the physics has
//! already finished moving.
//!
//! The behaviour goes between the water and the bodies, which is where Phase 2 said it would
//! have to: harvesting wants a field the light has already fallen on and cells that have not
//! yet swum away from the tile they were sitting on. It also has to run before the physics for
//! a second reason that only arrived with it - a myocyte's whole function is to change the
//! rest length of a spring, and a spring changed after the forces have been worked out is a
//! spring that does nothing until next tick.
//!
//! The expense pass goes after the physics rather than before it, and for one reason: a body
//! that dies leaves a grain of detritus at each of its cells, and the cells it should leave
//! them at are where the physics has just put them rather than where they were a tick ago.
//! Ageing goes just before it, so that an organism is judged on the age it has this moment
//! reached rather than the one it started the tick with.
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
//! about 9 MB, and 252,000 springs at about 6 MB**. The dense copies the tick hands to the
//! physics are the same two arenas again, plus two indices per cell - one to put the crowd
//! back where it came from and one saying whose cell it is - so another 19 MB; the physics
//! builds its own working arrays at 15 MB more and the behaviour pass at 16 MB more than that;
//! the drift of dead biomass is a grain per cell at 4 MB; the four thousand organisms are a
//! third of a megabyte and the resource field is under half of one. A default world is
//! therefore around **70 MB**, against CLAUDE.md's resident target of 2 GB.
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
//! # Two ways an organism arrives, and only one of them is somebody's decision
//!
//! [`World::seed`] is the door from outside: a genome, a place, and an amount of energy taken
//! out of the water the body is standing in. It is how a run begins and it is the only thing in
//! the project that can put a genome into a world that did not come from a parent.
//!
//! Everything after that comes through `reproduction.rs`, which nothing outside this module can
//! call. A birth needs no place chosen for it and no energy found for it - it goes beside its
//! parent's gonocyte and it is paid for out of its parent - so there is nothing for a caller to
//! decide and no way for one to interfere. Between them the two doors are the whole of how the
//! `organisms` array ever gains an entry.
//!
//! # What is deliberately not here
//!
//! **A runner.** Nothing here decides when a run ends. SPEC section 3's `max_wall_clock_hours`
//! is a wall-clock bound, and a wall clock is exactly what a deterministic simulation must
//! not read - `clippy.toml` refuses `Instant` and `SystemTime` in this crate outright, so
//! **the world cannot time itself**, by design. Whatever ends a run does it from outside, by
//! counting the ticks this module has taken.
//!
use crate::behaviour::{Behaviour, Detritus, Living};
use crate::cell::{Cell, CellKind, Vec2};
use crate::config::Config;
use crate::development::develop;
use crate::genome::Genome;
use crate::grid::Grid;
use crate::ledger::Ledger;
use crate::metabolism::{Metabolism, Mortal};
use crate::organism::{
    Organism, cell_slot, cells_per_slot, founding_marker, lay_out, spring_slot, springs_per_slot,
};
use crate::physics::{Physics, Spring, cell_capacity};
use crate::reproduction::{Fertile, Reproduction};
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

    /// What the cells do for themselves: eat, sense, and work their muscles.
    behaviour: Behaviour,

    /// What being alive costs them, and what becomes of them when they stop paying it.
    metabolism: Metabolism,

    /// How a body that has earned enough turns part of itself into another body.
    reproduction: Reproduction,

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

    /// The dead biomass on its way down: SPEC section 10's marine snow.
    ///
    /// Unlike the cells and the springs this one is a list rather than a set of slots, because
    /// a grain has no identity worth preserving - nothing points at one, so nothing breaks
    /// when they are shuffled up. What it shares with them is the thing that matters: it is
    /// built once, at the largest size the configuration could ever need, and **nothing in
    /// here can grow it**. `metabolism.rs` refuses to lay a grain down rather than push onto
    /// a full vector, exactly as a birth is refused at the population cap.
    ///
    /// The size is one grain for every cell the world can hold at once - at SPEC section 3's
    /// defaults, 256,000 grains at sixteen bytes, so **4 MB**. That is the whole world dead
    /// simultaneously, every cell of it leaving a grain, which is the largest corpse the
    /// simulation can produce from a single tick. It can be exceeded, because grains outlive
    /// the tick that made them by around a thousand, so a world dying and being reborn faster
    /// than the drift rots would eventually meet the cap - and meet it by losing a corpse
    /// rather than by allocating.
    drift: Vec<Detritus>,

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

    /// Which organism each cell of the crowd belongs to.
    ///
    /// Built beside the crowd rather than worked out from `live` on demand, because the
    /// behaviour pass asks the question in its innermost loops - a devorocyte has to know
    /// whether the cell it is touching is its own, and a sensocyte has to know whether the
    /// cell it can smell is somebody else's - and a division per candidate pair is a division
    /// a few million times a tick.
    owner: Vec<usize>,

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
            behaviour: Behaviour::new(config),
            metabolism: Metabolism::new(config),
            reproduction: Reproduction::new(config),
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
            drift: Vec::with_capacity(capacity),
            // Reversed, so that popping the last hands out slot 0 first.
            free: (0..slots).rev().collect(),
            next_serial: 0,
            crowd: Vec::with_capacity(capacity),
            bonds: Vec::with_capacity(adhesions),
            live: Vec::with_capacity(capacity),
            owner: Vec::with_capacity(capacity),
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

        self.behaviour.run(
            Living {
                cells: &mut self.crowd,
                springs: &mut self.bonds,
                owner: &self.owner,
                organisms: &mut self.organisms,
                detritus: &mut self.drift,
            },
            &mut self.grid,
            &mut self.ledger,
            self.ticks,
        );

        self.physics.step(&mut self.crowd, &self.bonds);
        self.scatter();

        // Ageing comes before the expense pass rather than after it, so that an organism is
        // judged on the age it has just reached rather than on the one it had at the start of
        // the tick. It has to be after `scatter`, too: a corpse leaves a grain at each of its
        // cells, and the cells it leaves them at should be where the physics has just put
        // them rather than where they were a tick ago.
        for organism in self.organisms.iter_mut().flatten() {
            organism.grow_older();
        }

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

        // Births come last, after the reaping, and there are three reasons rather than one. A
        // slot that fell vacant this tick is available to be born into on the same tick, which
        // is what keeps a world at its cap turning over rather than waiting a tick between
        // every death and the birth that replaces it. An organism that did not survive the tick
        // does not get to breed on its way out. And a newborn is put down beside cells the
        // physics has already finished moving, so it is placed where its parent *is* rather
        // than where its parent was when the tick began.
        //
        // The consequence worth knowing is that a newborn has one free tick: it is aged, fed
        // and charged for the first time on the tick after the one it appeared on. It is also
        // what stops a birth from cascading - see `reproduction.rs`.
        self.reproduction.run(
            Fertile {
                cells: &mut self.cells,
                springs: &mut self.springs,
                organisms: &mut self.organisms,
                free: &mut self.free,
                next_serial: &mut self.next_serial,
            },
            &self.rng,
            &mut self.ledger,
        );

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

        // The body is written into its slot before the seeding is settled, which is safe
        // precisely because the slot is free: nothing in the world reads a slot nobody is in,
        // so a refusal below leaves cells lying in it that no tick will ever look at. The
        // alternative is a second copy of the body somewhere to hold it while the water is
        // counted, which is a copy made on every birth for the sake of the births that fail.
        //
        // `lay_out` is `organism.rs`'s, and is the same call `reproduction.rs` makes when a
        // parent has a child. Where a body's cells go, given where its seed cell went, is one
        // question and has one answer.
        lay_out(
            slot,
            &body,
            at,
            &self.config.world,
            &self.config.limits,
            &mut self.cells,
            &mut self.springs,
        );

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
            // No parent. This is the door from outside the world, so there is nothing in the
            // world a body coming through it could be the child of. See `organism.rs` for why
            // that is nothing at all rather than a zero.
            None,
            // And nothing to inherit a lineage marker from either, so it is spaced round the
            // circle by serial - `founding.rs` seeds every founder of a run with the same
            // genome, and a marker taken from that would give all of them the same one.
            founding_marker(self.next_serial),
            body.cells.len(),
            body.springs.len(),
        ));
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .expect("a run has minted every serial number there is");

        // ⭐⭐ **Phase 7's Group L: the seasons start here, and here is the only place they can
        // start.** SPEC section 4's season is a fact about a world with something living in it.
        // `founding.rs` fills the field first and stops when it stops filling - a
        // **light-dependent** test - so a season running through the dawn would change how long
        // the dawn takes, and a seasoned run and a flat run would begin at different ticks
        // against different fields. Idempotent: this is the moment the first body arrives, and
        // every body after the first arrives at a world whose clock is already going.
        self.grid.begin_season();

        Ok(slot)
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
        self.owner.clear();

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
                self.owner.push(slot);
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

    /// ⭐ **Change the conditions a running world is living under.** SPEC section 3's live half.
    ///
    /// > `[world]`, `[limits]` and `seed` lock at run start; the rest can be changed live,
    /// > **which is how environmental events work.**
    ///
    /// That last clause is the whole reason this exists, and it is worth reading it the way it
    /// is meant: raising `metabolism.upkeep_scale` here is not a settings change, it is the
    /// weather turning. SPEC section 3 has the measurements - at 3 and above a founder dies of
    /// old age before it has earned the reproduction threshold - and section 11's event log
    /// lists *"environmental changes made by the user"* among the things worth recording.
    ///
    /// # ⚠️ There is one copy of each number and this is why it is a method here
    ///
    /// The tick does not read `Config`. `grid.rs` precomputes each tile's ceiling and each
    /// row's regrowth, `physics.rs` and `metabolism.rs` hold their numbers widened, and
    /// `reproduction.rs` keeps the whole `[mutation]` table - because reading a configuration
    /// on the inside of a loop over a quarter of a million cells is not what a configuration is
    /// for. So a change that only replaced [`World::config`] would move the number **the panel
    /// reads back** and leave every number the simulation actually charges exactly where it
    /// was: a world reporting weather it is not having. This walks all five.
    ///
    /// # ⚠️ Nothing here allocates, and that is a guarantee rather than an observation
    ///
    /// CLAUDE.md: *"Every arena is allocated at startup at fixed capacity derived from the
    /// config, and never resized… A simulation that cannot allocate cannot leak."* Every arena
    /// in this world is sized from `[world]` or `[limits]`, and those are exactly the tables
    /// this refuses to change - so the guarantee is not weakened by a run being retunable. The
    /// refusal is a panic and not a returned error, in CLAUDE.md's terms: *"invariants are
    /// asserted at runtime, not just in tests"*. `panel.rs` never offers a locked setting, so
    /// reaching this is a program that has gone wrong rather than a person who typed something.
    ///
    /// # Panics
    ///
    /// If `[world]`, `[limits]` or the seed differ from the ones this world was built with.
    pub fn retune(&mut self, config: &Config) {
        assert!(
            config.world == self.config.world,
            "world.{{width, height, grid_cols, grid_rows, years_per_tick, seed}} lock at run \
             start - SPEC section 3 - and every arena in this world was sized from them. Asked \
             for {:?} on a world built as {:?}",
            config.world,
            self.config.world
        );
        assert!(
            config.limits == self.config.limits,
            "limits.max_organisms, limits.max_cells_per_organism, limits.max_genes and \
             limits.max_dev_steps lock at run start - SPEC section 3 - and every arena in this \
             world was sized from them. Asked for {:?} on a world built as {:?}",
            config.limits,
            self.config.limits
        );

        self.grid.relight(config);
        self.physics.retune(config);
        self.behaviour.retune(config);
        self.metabolism.retune(config);
        self.reproduction.retune(config);
        self.config = config.clone();
    }

    /// Who is in each slot, and nothing in the slots that are free.
    #[must_use]
    pub fn organisms(&self) -> &[Option<Organism>] {
        &self.organisms
    }

    /// How many organisms have ever existed in this world, counting the ones that are gone.
    ///
    /// A serial number is minted for every organism and is never handed out twice - that is
    /// what `rng.rs` needs it for - so the next one to be minted is also a count of every
    /// organism there has ever been. Subtracting the living population from it gives the
    /// number that have died, and differencing it across a stretch of ticks gives the births
    /// and the deaths over that stretch.
    ///
    /// That is what makes turnover measurable from outside. A world sitting at a steady
    /// population is either **still**, with nothing being born and nothing dying, or it is
    /// **turning over**, with a birth for every death - and those two are the same number of
    /// organisms and completely different worlds. Nothing else the world exposes can tell
    /// them apart.
    #[must_use]
    pub fn born(&self) -> u64 {
        self.next_serial
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

    /// Every cell that belongs to something alive, packed together with no gaps.
    ///
    /// This is the crowd the tick already builds for the physics - see [`World::tick`] and this
    /// module's documentation - handed out to be read rather than rebuilt for the asking. What
    /// makes it worth having over [`World::cells`] is what is *not* in it: at SPEC section 3's
    /// defaults the arena is 256,000 cells of which a few thousand belong to anybody, and the
    /// rest are sitting at the origin where the arena was built. Anything that walked the arena
    /// and drew what it found would draw all of them.
    ///
    /// [`World::living_cell_owners`] says whose each of these is, index for index.
    ///
    /// # ⚠️ It is the population of the tick just taken, which is not quite the population now
    ///
    /// The crowd is gathered at the *start* of a tick, and a tick ends by reaping the dead and
    /// then letting the survivors breed. So immediately after a tick this list still holds the
    /// cells of anything that died during it, and does not yet hold the cells of anything born
    /// during it. Both are one tick out of date - a sixtieth of a simulated second - and
    /// `the_living_cells_can_be_read_without_the_empty_slots` pins that contract rather than
    /// leaving it to be discovered.
    ///
    /// The alternative was to gather a second time at the end of every tick so that the list
    /// were always current. That is one copy per living cell per tick, paid by every headless
    /// run, for the benefit of a reader that may not exist - and a run that is not being
    /// watched is what this project is mostly for.
    ///
    /// Before the world's first tick it is empty, whatever has been seeded into the world.
    #[must_use]
    pub fn living_cells(&self) -> &[Cell] {
        &self.crowd
    }

    /// Which slot each cell of [`World::living_cells`] belongs to, in the same order.
    ///
    /// Always exactly as long as that list. A caller that wants to know anything about the
    /// organism a cell belongs to - its genome, its age, its lineage - looks the slot up in
    /// [`World::organisms`].
    ///
    /// It is a *slot* rather than a serial number because a slot is what indexes the arrays: a
    /// serial would have to be searched for. The caveat is the one `organism.rs` states, and it
    /// bites here for exactly the reason the note above gives: a slot is a place and is handed
    /// on to whoever is born there next, so a cell whose owner died during the last tick may
    /// name a slot that is now empty or holds somebody else entirely. Read the serial off the
    /// organism if two moments have to be compared.
    #[must_use]
    pub fn living_cell_owners(&self) -> &[usize] {
        &self.owner
    }

    /// The dead biomass on its way down: SPEC section 10's marine snow, where it is and what it
    /// is still holding.
    ///
    /// SPEC section 12 asks for the snow in the background to be **the actual detritus** rather
    /// than a decoration drawn over the top of it, and this is what makes that possible. The
    /// grains are real: `metabolism.rs` lays one at each cell of every body that dies, they
    /// fall, and they give what they hold back to the water as they go. What they hold between
    /// them is the ledger's `detritus` account exactly.
    ///
    /// Unlike the cells this is current the moment a tick ends, because the drift is not
    /// gathered or copied anywhere - it is the list itself.
    #[must_use]
    pub fn drift(&self) -> &[Detritus] {
        &self.drift
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
    use crate::metabolism::construction_energy;
    use crate::mutation::mutate;
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

    /// The same, with a light of the tests' own rather than the shipped one.
    ///
    /// ⭐ Most of the tests in this file need only *a world that has filled up*: something in
    /// the water for a body to be seeded out of, and something for a photocyte to eat. How long
    /// a field takes to fill is `light.cap / light.influx`, and Group D moved `light.influx` by
    /// a factor of twelve when it tuned the ecology - so every one of those tests would
    /// otherwise have quietly become a test about a world a twelfth full, and several of them
    /// simply stopped being able to seed a body at all.
    ///
    /// The light is therefore pinned here, at the value the tests were written against, and
    /// **the tests that are genuinely about the shipped configuration do not use this**:
    /// `energy_is_conserved_over_100k_ticks` and
    /// `energy_is_still_conserved_with_organisms_present` both take `config` directly, because
    /// what they are measuring is the world this project actually ships.
    fn a_lit_world(change: impl FnOnce(&mut RawConfig)) -> Config {
        config(|raw| {
            raw.light.influx = 0.012;
            change(raw);
        })
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
                    // A cell with no gene is written as a number no gene can be, rather than
                    // as nought, which is gene zero and a perfectly ordinary answer.
                    cell.gene.map_or(u32::MAX, u32::from),
                    // ⭐ Phase 7's Group L. A myocyte's remembered contraction decides what its
                    // organism is charged on the *next* tick, so two runs that agreed about
                    // everything else and disagreed about this would be two runs that were
                    // about to part company. An absence is written as not-a-number, which a
                    // contraction can never be, for the same reason a missing gene is written
                    // as a number no gene can be.
                    cell.contraction.map_or(u32::MAX, f32::to_bits),
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
    /// # They have to be given something to live on, which is new in Group B
    ///
    /// Before upkeep existed these two were seeded asking for **no energy**, because a world
    /// starts dark and there was nothing for a body to do with any. There is now: every cell
    /// pays for itself every tick, so a body seeded holding nothing is a body that dies at the
    /// end of its first tick and a test that ticks a thousand times is a test ticking an empty
    /// world. Each caller therefore lets the light fall first and says what the two are to
    /// hold.
    fn place_two_bodies(world: &mut World, energy: f64) {
        let limits = world.config().limits.clone();
        let (width, height) = (world.config().world.width, world.config().world.height);
        let at = Vec2::new(width * 0.25, height * 0.5);

        for offset in [Vec2::ZERO, Vec2::new(3.0, 1.5)] {
            world
                .seed(a_chain(3, &limits), at + offset, energy)
                .expect("these tests configure a world with room for two organisms");
        }
    }

    /// What a quarter of the tile at `at` is holding, which is an amount a body standing there
    /// can certainly be seeded with.
    ///
    /// A body spans one tile or two, and `World::seed` draws on all of the tiles beneath it, so
    /// a quarter of the first one is affordable twice over - which is what the two bodies
    /// [`place_two_bodies`] puts down need between them.
    fn a_quarter_of_the_tile(world: &World, at: Vec2) -> f64 {
        f64::from(world.grid().tiles()[world.grid().tile_at(at)]) * 0.25
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

    /// A genome that grows a single cell of one kind, and nothing else.
    ///
    /// Development always begins with one photocyte - SPEC section 7's seed cell - so the only
    /// way to a body made of anything else is a gene that differentiates it in place. One
    /// gene, firing on step zero only, which leaves a body of exactly one cell of the kind
    /// asked for.
    ///
    /// It is what lets a test choose how long a body lives: `metabolism.rs` derives a maximum
    /// age from what a body costs to run, so a myocyte lives 571 ticks and a sclerocyte four
    /// thousand.
    fn a_single_cell(kind: CellKind, limits: &LimitsConfig) -> Genome {
        Genome::new(
            vec![Gene {
                trigger_state: State::ZERO,
                min_step: 0,
                max_step: 0,
                action: Action::Differentiate,
                angle: 0.0,
                adhere: false,
                child_state: State::ZERO,
                child_kind: CellKind::Photocyte,
                rest_length: 0.0,
                stiffness: 0.0,
                new_kind: kind,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            }],
            limits,
        )
    }

    /// ⭐⭐ **B6.** A slot a death frees is used again, and the order slots come back in is
    /// fixed - because it decides the last bit of the physics and nothing announces it if it
    /// changes.
    ///
    /// `docs/PHASE3.md` flagged this as the nasty one of Group B, and the reason is worth
    /// setting out in full because it is not obvious and it is not detectable.
    ///
    /// The free list is a stack. A death pushes a slot onto it and a birth pops one off, so
    /// **the order deaths are reaped in decides which slot the next organism is born into**.
    /// A slot decides which stretch of the cell arena a body occupies; the arena's order
    /// decides the order `World::gather` packs the crowd in; the crowd's order decides the
    /// order `physics.rs` visits pairs of touching cells and therefore the order it *adds up*
    /// the forces on each of them. Floating-point addition is not associative, so a different
    /// order is a different answer in the last bit. It is a very small difference and it
    /// compounds: two runs of one seed drift apart, a recording stops replaying, and there is
    /// nothing in any log to say why.
    ///
    /// Reaping in slot order costs nothing and closes the whole of that off. Reaping in
    /// whatever order a parallel pass finished in - which is the natural thing to write the
    /// day the population is spread across the machine's cores - opens it.
    ///
    /// # This test is built so that a permuted reap fails it, rather than so that it passes
    ///
    /// A test that killed one organism and watched its slot come back would pass against any
    /// order whatever, because one thing has only one order. So **three** organisms die on the
    /// same tick, out of four, and what is asserted is where the next three births land.
    ///
    /// Slots 0, 1 and 3 die together. Swept in index order they are pushed onto the free list
    /// as `[0, 1, 3]`, and a stack hands back what went on last - so the next three bodies
    /// take slots **3, then 1, then 0**. Swept backwards they would go on as `[3, 1, 0]` and
    /// the same three bodies would take 0, then 1, then 3. Any of the six orders the three
    /// deaths could be reaped in gives a different answer, and the three bodies are
    /// deliberately different sizes so that the arena says plainly which is which.
    ///
    /// The last assertions are what make it a claim about the *physics* rather than about
    /// bookkeeping: the arena indices each body actually occupies, which is the thing
    /// `World::gather` reads and the thing whose order the argument above is about.
    ///
    /// # It was checked by breaking it
    ///
    /// The sweep in `metabolism.rs` was reversed and the whole suite run. **One test failed:
    /// this one.** That is the finding rather than a footnote - a hundred and fifteen other
    /// tests, including every conservation claim, every determinism claim and a hundred and
    /// twenty thousand ticks of a world with bodies living and dying in it, all went green
    /// against a reaping order that would silently stop a run matching its own recording.
    #[test]
    fn a_freed_slot_is_reusable_and_reaping_is_deterministic() {
        let mut world = World::new(&config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            // A deep enough larder that a single tile can pay for a body outright, and light
            // fast enough to fill it in a few hundred ticks rather than a few thousand.
            raw.light.cap = 40.0;
            raw.light.influx = 0.2;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 4;
        }));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        // Three bodies of muscle, which `metabolism.rs` allows 1,600 ticks, and one of armour,
        // which it allows four thousand. All four are seeded on the same tick and holding far
        // more than they can spend, so what takes the three is age and they go together.
        for (along, kind) in [
            (30.0f32, CellKind::Myocyte),
            (90.0, CellKind::Myocyte),
            (150.0, CellKind::Sclerocyte),
            (210.0, CellKind::Myocyte),
        ] {
            world
                .seed(a_single_cell(kind, &limits), Vec2::new(along, 40.0), 12.0)
                .expect("a lit world holds twelve units under a single cell");
        }

        assert!(
            world.free.is_empty(),
            "the world is not full, so the slots the deaths free cannot be told apart from \
             slots that were never taken"
        );
        assert_eq!(
            world.seed(
                a_single_cell(CellKind::Myocyte, &limits),
                Vec2::new(30.0, 90.0),
                1.0
            ),
            Err(Refused::WorldIsFull),
            "a full world accepted a fifth organism"
        );

        // What the world costs to hold, before anything dies in it. Nothing here may grow.
        let capacities = [
            world.cells.capacity(),
            world.springs.capacity(),
            world.organisms.capacity(),
            world.free.capacity(),
            world.drift.capacity(),
        ];
        let biomass_before = world.ledger().biomass();
        let dissipated_before = world.ledger().dissipated();

        // Long enough for the muscle to reach its limit and not for the armour to reach its.
        //
        // ⚠️ **Re-recorded from 600 ticks when a myocyte's upkeep moved from 0.014 to 0.005**
        // - see `cell.rs`. The allowance is `LIFETIME_UPKEEP ÷ upkeep`, so a body of muscle
        // went from 571 ticks to 1,600 and six hundred ticks stopped reaching it. Nothing
        // about the claim changed: this is still the first moment after the muscle's limit
        // and well before the armour's four thousand, and the twelve units each body was
        // seeded with still outlast the eight its own upkeep spends getting there.
        for _ in 0..1_700 {
            world.tick();
        }

        assert_eq!(
            world.organisms().iter().flatten().count(),
            1,
            "the three bodies of muscle were allowed 1,600 ticks apiece and the world still \
             holds {} organisms",
            world.organisms().iter().flatten().count()
        );
        assert!(
            world.organisms()[2].is_some(),
            "the body of armour, which is allowed four thousand ticks, died with the muscle"
        );

        // The books moved, which is the half a conservation check cannot see. Upkeep left the
        // living for good, and what the dead were still holding is lying in the water.
        assert!(
            world.ledger().dissipated() > dissipated_before,
            "two thousand ticks of four bodies paying upkeep dissipated nothing at all, so \
             the tick is not charging anybody"
        );
        assert!(
            world.ledger().biomass() < biomass_before,
            "the living population is holding more than it was after three of it died"
        );
        assert!(
            world.ledger().detritus() > 0.0 && !world.drift.is_empty(),
            "three well-fed bodies died and left {} grains holding {} between them",
            world.drift.len(),
            world.ledger().detritus()
        );

        // ⭐ The claim. Three slots came back in index order, so a stack hands them out
        // backwards, so the next three bodies take 3, then 1, then 0.
        let mut took = Vec::new();
        for segments in [2u8, 3, 4] {
            took.push(
                world
                    .seed(a_chain(segments, &limits), Vec2::new(120.0, 100.0), 1.0)
                    .expect("three deaths left three slots free"),
            );
        }

        assert_eq!(
            took,
            vec![3, 1, 0],
            "slots 0, 1 and 3 fell vacant on the same tick and the next three births took \
             {took:?}. Swept backwards they would have taken [0, 1, 3]; any other order the \
             three deaths could have been reaped in gives another answer again - and the \
             answer decides where in the cell arena each body sits, which decides the order \
             the physics adds up its forces"
        );
        assert_eq!(
            world.seed(a_chain(2, &limits), Vec2::new(120.0, 100.0), 1.0),
            Err(Refused::WorldIsFull),
            "the world is full again and accepted a fifth organism"
        );

        // Where those three bodies actually landed in the arena, which is what `gather` reads.
        assert_eq!(
            world.cells_of(3).len(),
            2,
            "the two-celled body is not in slot 3"
        );
        assert_eq!(
            world.cells_of(1).len(),
            3,
            "the three-celled body is not in slot 1"
        );
        assert_eq!(
            world.cells_of(0).len(),
            4,
            "the four-celled body is not in slot 0"
        );
        assert_eq!(
            world.cells_of(2).len(),
            1,
            "the body that survived has been written over"
        );

        // And none of it allocated. A death that grew an arena would be a run whose memory is
        // decided by how many things happen to die in it.
        assert_eq!(
            capacities,
            [
                world.cells.capacity(),
                world.springs.capacity(),
                world.organisms.capacity(),
                world.free.capacity(),
                world.drift.capacity(),
            ],
            "three deaths and three births grew one of the world's arenas"
        );
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
        let mut world = World::new(&a_lit_world(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        // The light falls first, and the bodies are given something to live on. From Group B
        // a body holding nothing dies at the end of its first tick, and the hundred ticks
        // below are meant to be a hundred ticks of two organisms sitting in their slots.
        for _ in 0..700 {
            world.tick();
        }

        let first = world
            .seed(a_chain(3, &limits), Vec2::new(100.0, 200.0), 3.0)
            .expect("an empty world has room for an organism");
        let second = world
            .seed(a_chain(5, &limits), Vec2::new(400.0, 200.0), 3.0)
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

    /// ⭐ **A2.** The living cells can be read as one dense list, with none of the arena's
    /// empty slots in it.
    ///
    /// [`World::cells`] is the whole arena, and most of a running world's arena is empty slots
    /// holding cells that were never written to - which is to say, cells sitting at the origin.
    /// Anything that walked that array and drew what it found would put a pile of ghosts in the
    /// top-left corner of the world, one for every cell every dead or unborn organism could
    /// have had. At SPEC section 3's defaults that is 256,000 cells of which a few thousand are
    /// real.
    ///
    /// The tick already builds the list that is wanted - see `gather` and this module's
    /// documentation - because the physics needs exactly the same thing for exactly the same
    /// reason. This is that list, and the map from a place in it back to the organism whose
    /// cell it is, which is what a caller needs to colour a body by whose it is rather than by
    /// what it is made of.
    ///
    /// # ⚠️ The contract is "the population of the tick just taken", and that is not the same
    /// as "the population right now"
    ///
    /// The crowd is gathered at the *start* of a tick, and a tick ends by reaping the dead and
    /// then letting the survivors breed. So when a tick returns, the crowd still holds the
    /// cells of anything that died during it and does not yet hold the cells of anything born
    /// during it. Both are one tick out of date, which is a sixtieth of a simulated second.
    ///
    /// That is asserted here rather than merely written down, because it is the sort of thing
    /// that gets discovered later by somebody wondering why a corpse flickered. **The
    /// alternative was to gather a second time at the end of every tick**, which is one copy
    /// per living cell per tick paid by every headless run for the benefit of a reader that
    /// may not exist.
    ///
    /// # Why a death is what proves the claim
    ///
    /// A world where nothing has ever died has a crowd that is the same length as its
    /// population however the list was built - so the sizes below are chosen to be
    /// individually recognisable, and one of the two bodies is deliberately starved. A
    /// three-celled body sits in a slot of eight, so the arena holds five cells belonging to
    /// nobody between the two bodies; a list built by walking the arena would be eight cells
    /// long and would include them.
    #[test]
    fn the_living_cells_can_be_read_without_the_empty_slots() {
        let mut world = World::new(&a_lit_world(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        for _ in 0..700 {
            world.tick();
        }

        // A chain of photocytes, which earns its keep, and one myocyte holding nothing at all.
        // A myocyte harvests nothing and costs 0.005 a tick, so it cannot survive the tick it
        // is seeded on; a photocyte in this light earns several times its upkeep.
        world
            .seed(a_chain(3, &limits), Vec2::new(100.0, 100.0), 3.0)
            .expect("a lit world has room and water for a three-celled body");
        world
            .seed(
                a_single_cell(CellKind::Myocyte, &limits),
                Vec2::new(300.0, 100.0),
                0.0,
            )
            .expect("a body asking for nothing can always be afforded");

        world.tick();

        // The arena, for contrast: four slots of eight cells, of which four are somebody's.
        assert_eq!(world.cells().len(), 32, "the arena is four slots of eight");
        assert_eq!(
            world
                .cells()
                .iter()
                .filter(|cell| cell.pos == Vec2::ZERO)
                .count(),
            28,
            "the whole arena should be twenty-eight cells at the origin and four real ones, \
             so this is the pile of ghosts a caller walking it would draw"
        );

        // ⭐ And the dense list is only the real ones.
        assert_eq!(
            world.living_cells().len(),
            4,
            "the dense list is {} cells long and there are four in the world",
            world.living_cells().len()
        );
        assert!(
            world
                .living_cells()
                .iter()
                .all(|cell| cell.pos != Vec2::ZERO),
            "a cell in the dense list is sitting at the origin, which is where the arena was \
             built and where nothing living has been put"
        );
        assert_eq!(
            world.living_cell_owners(),
            &[0, 0, 0, 1],
            "the dense list does not say whose each cell is, so nothing reading it can \
             tell one body from another"
        );
        assert_eq!(
            world.living_cells().len(),
            world.living_cell_owners().len(),
            "the two lists are different lengths, so an index into one does not mean the \
             same cell in the other"
        );

        // Every cell of the dense list is the cell its owner's slot says it is, which is what
        // makes the owner map worth having. Slots that are now empty are skipped, and that
        // exclusion is itself the contract below being demonstrated: the body that died during
        // this tick is still in the list and its slot no longer answers for it.
        let mut checked = 0;
        for (index, cell) in world.living_cells().iter().enumerate() {
            let slot = world.living_cell_owners()[index];
            if world.organisms()[slot].is_none() {
                continue;
            }

            assert!(
                world.cells_of(slot).iter().any(|own| own.pos == cell.pos),
                "cell {index} of the dense list is said to belong to slot {slot} and is not \
                 one of that slot's cells"
            );
            checked += 1;
        }
        assert_eq!(
            checked, 3,
            "only {checked} cells were checked against the slot they claim, so this loop is \
             skipping the ones it was meant to be about"
        );

        // ⚠️ The myocyte is already dead and is still in the list, because the list is the
        // population of the tick just taken. This is the contract, written as an assertion so
        // it cannot be discovered by accident later.
        assert!(
            world.organisms()[1].is_none(),
            "the starved body survived the tick, so this case is not about a death"
        );
        assert_eq!(
            world.living_cell_owners().last(),
            Some(&1),
            "a body that died during the tick has already gone from the list, so the list is \
             not the population of the tick that was taken and the documented contract is \
             wrong"
        );

        // And on the next tick it is gone, because that tick gathered afresh.
        world.tick();

        assert_eq!(
            world.living_cells().len(),
            3,
            "the dead body is still in the list a whole tick after it died"
        );
        assert_eq!(world.living_cell_owners(), &[0, 0, 0]);
    }

    /// ⭐ **A3.** The drift can be read: the grains of dead biomass, where they are and what
    /// they are still holding.
    ///
    /// SPEC section 12 asks for marine snow that **is the actual detritus** rather than a
    /// decoration drawn over the top of it, and this is the only reason the request is
    /// answerable: the grains already exist, they already fall, and they already rot back into
    /// the water. Nothing here is new - `metabolism.rs` has made and moved them since Phase 4 -
    /// and the world simply had no way of being asked.
    ///
    /// # Three claims, and the third is what makes the first two mean anything
    ///
    /// That an untouched world's drift is empty, so the list is not merely always full of
    /// something. That a death puts a grain at each of the dead body's cells, which is what
    /// makes the snow the *actual* detritus rather than particles scattered where a renderer
    /// felt like it. And that what the grains hold between them is the ledger's `detritus`
    /// account **exactly** - because a reader that could see grains the books did not know
    /// about would be a reader watching energy that is not in the world.
    #[test]
    fn the_drift_can_be_read() {
        let mut world = World::new(&a_lit_world(|raw| {
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        for _ in 0..700 {
            world.tick();
        }

        assert!(
            world.drift().is_empty(),
            "nothing has ever died in this world and there are {} grains in the water",
            world.drift().len()
        );

        // A chain of three photocytes, springs already at their rest length so that nothing
        // pushes or pulls and the body stands exactly where it was put. It has no gonocyte, so
        // it cannot breed; it earns more than it spends, so it does not starve. What ends it is
        // age - `metabolism.rs` allows a three-celled photocyte body about two thousand ticks -
        // and a body that dies of old age dies rich, which is what puts something in the
        // grains worth reading.
        world
            .seed(a_chain(3, &limits), Vec2::new(100.0, 100.0), 0.02)
            .expect("a lit world has room and water for a three-celled body");
        let mut body: Vec<Vec2> = world.cells_of(0).iter().map(|cell| cell.pos).collect();
        assert_eq!(body.len(), 3);

        // ⚠️ **Where the body was on the tick it died, not where it was seeded.** SPEC section
        // 8's buoyancy makes a body's depth a function of what it is made of, and three
        // photocytes float - about a unit and a half over the two thousand ticks this one
        // lives. The claim below is that a corpse leaves a grain *at each of its cells*, which
        // is a claim about the body's last moment; comparing against the seeding position
        // would be quietly asserting that nothing in this world can move.
        let mut ticks = 0;
        while world.drift().is_empty() && ticks < 6_000 {
            let alive = world.cells_of(0);
            if !alive.is_empty() {
                body = alive.iter().map(|cell| cell.pos).collect();
            }
            world.tick();
            ticks += 1;
        }

        assert_eq!(
            world.drift().len(),
            3,
            "a three-celled body died on tick {ticks} and left {} grains, so the snow is not \
             one grain per cell",
            world.drift().len()
        );
        for grain in world.drift() {
            assert!(
                body.iter().any(|cell| (grain.pos - *cell).length() < 0.5),
                "a grain is at {:?} and the body that made it had cells at {body:?}, so the \
                 snow is being scattered rather than left where the body was",
                grain.pos
            );
            assert!(
                grain.energy > 0.0,
                "a grain holding nothing is still in the drift"
            );
        }

        // ⭐ And what the grains hold is exactly what the books say is lying in the water.
        let held: f64 = world.drift().iter().map(|grain| grain.energy).sum();
        assert!(
            (held - world.ledger().detritus()).abs() < 1e-12,
            "the grains hold {held} between them and the detritus account says {}",
            world.ledger().detritus()
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
        let mut world = World::new(&a_lit_world(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 3;
            raw.limits.max_cells_per_organism = 4;
        }));
        let limits = world.config().limits.clone();

        // The light falls first, so that the three bodies can be given something to live on:
        // from Group B a body holding nothing dies at the end of the tick at the bottom of
        // this test, and there would be nothing left in the world to have refused a birth.
        for _ in 0..700 {
            world.tick();
        }

        for (slot, along) in [40.0f32, 100.0, 160.0].into_iter().enumerate() {
            let born = world.seed(a_chain(2, &limits), Vec2::new(along, 72.0), 2.0);
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
            world.drift.capacity(),
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
                world.drift.capacity(),
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
        let mut world = World::new(&a_lit_world(|raw| {
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

    /// A genome that grows one photocyte with one myocyte sprung to it, and gives that myocyte
    /// a rhythm.
    ///
    /// ⚠️ **Rewritten in Phase 7, and the rewrite is the change it is testing.** It used to be
    /// two genes - one that budded the myocyte and a second, silent `Terminate` gene answering
    /// to the daughter's *state*, which is where `behaviour.rs` used to look a cell's rhythm
    /// up. A cell now takes its behaviour from the gene that **built** it, so the rhythm
    /// belongs on the gene that says `child_kind: Myocyte`, and the second gene has nothing
    /// left to do. See `development.rs`'s [`crate::development::develop`] for why, and for the
    /// measurement that says the old arrangement almost never happened by accident: **0.05% of
    /// grown cells** in the shipped world were in a state their own genome named.
    ///
    /// The silent gene is kept anyway, unchanged, because it costs nothing and it is a second
    /// claim: a gene that answers to the myocyte's state and carries no frequency does **not**
    /// take that myocyte's behaviour over. Delete the `osc_freq` below and this genome stops
    /// swimming, which is what `a_tick_feeds_the_bodies_in_the_world` would then say.
    fn a_swimmer(limits: &LimitsConfig) -> Genome {
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

        Genome::new(
            vec![
                Gene {
                    action: Action::Divide,
                    adhere: true,
                    child_state: State::new(1),
                    child_kind: CellKind::Myocyte,
                    rest_length: 8.0,
                    stiffness: 10.0,
                    osc_freq: 3.0,
                    ..blank
                },
                Gene {
                    trigger_state: State::new(1),
                    max_step: u8::MAX,
                    ..blank
                },
            ],
            limits,
        )
    }

    /// ⭐ A tick actually feeds the bodies in the world, and the books know about it.
    ///
    /// Everything `behaviour.rs` does is proved in `behaviour.rs`, against scenes built by
    /// hand so that a claim about a shadow is a claim about a shadow. This is the other half:
    /// that a [`World`] hands its bodies to that pass at all, in the right order, and puts the
    /// results back where they came from.
    ///
    /// # It asserts the accounts moved, not that they balance
    ///
    /// SPEC section 5's lesson, which Phase 2 learned twice: a conservation check cannot see
    /// energy that was never declared. A world whose behaviour pass had been quietly left out
    /// of the tick would balance its books perfectly for ever. So what is measured is that the
    /// **field went down**, that **biomass went up** by the same amount, and that the organisms
    /// are holding it.
    ///
    /// # And that a myocyte does not eat its own genome
    ///
    /// The rest length a gene asked for lives in the world's spring arena; what a myocyte
    /// works is the *copy* the tick hands the physics, which is rebuilt from the arena every
    /// tick. Get that the wrong way round and each tick's contraction is applied on top of the
    /// last one's, and a body winds itself up until it tears apart - slowly enough that the
    /// first thousand ticks look fine. So after five hundred ticks the arena is checked
    /// against the number in the genome, exactly, while the body is checked for having
    /// actually been worked.
    #[test]
    fn a_tick_feeds_the_bodies_in_the_world() {
        let mut world = World::new(&a_lit_world(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 8;
            raw.limits.max_cells_per_organism = 8;
        }));
        let limits = world.config().limits.clone();

        // The field fills under the light first. A world starts dark.
        //
        // ⚠️ **Three thousand rather than one, since Phase 7's Group G**, and the reason is
        // the claim below rather than the length of the dawn. What is asserted is that the
        // field paid for what the organism gained, which is only a statement about the
        // organism if the light is not *also* putting energy into the same tiles during the
        // tick being measured - and a tile at its ceiling takes nothing. `light.patchiness`
        // moved from 0.15 to 0.5, so the world's ceilings are further apart and it takes
        // about a third longer to reach them; at a thousand ticks this world was still
        // filling, the light added 0.0078 to the tiles under the body during the measured
        // tick, and the subtraction came out on the wrong side by exactly that.
        for _ in 0..3_000 {
            world.tick();
        }

        let slot = world
            .seed(a_swimmer(&limits), Vec2::new(100.0, 60.0), 0.0)
            .expect("an empty world has room for an organism");

        let field_before = world.grid().total_energy();
        let biomass_before = world.ledger().biomass();

        world.tick();

        let holding = world.organisms()[slot]
            .as_ref()
            .expect("nothing dies in this group")
            .energy();
        let field_after = world.grid().total_energy();

        assert!(
            holding > 0.0,
            "an organism with a photocyte in it earned nothing over a tick of a lit world, \
             so the behaviour pass is not being run at all"
        );
        assert!(
            (field_before - field_after - holding) > -1e-9,
            "the field went down by {} and the organism is holding {holding}, so it has been \
             given energy the water did not pay for",
            field_before - field_after
        );
        assert!(
            (world.ledger().biomass() - biomass_before - holding).abs() < 1e-9,
            "the biomass account moved by {} while the only organism in the world gained \
             {holding}",
            world.ledger().biomass() - biomass_before
        );

        // The renderer's number is written, and written where the renderer will look for it -
        // in the arena, which means the crowd was copied home again.
        assert!(
            world.cells_of(slot)[0].energy_flow > 0.0,
            "the photocyte gained {holding} and its `energy_flow` says {}",
            world.cells_of(slot)[0].energy_flow
        );

        // Five hundred ticks of a working muscle.
        for _ in 0..500 {
            world.tick();
        }

        assert!(
            (world.springs_of(slot)[0].rest_length - 8.0).abs() < f32::EPSILON,
            "the spring in the world's arena is asking for {} rather than the eight units \
             its gene asked for, so each tick's contraction is being applied on top of the \
             last one's and this body is winding itself up",
            world.springs_of(slot)[0].rest_length
        );

        let body = world.cells_of(slot);
        let apart = (body[1].pos - body[0].pos).length();
        assert!(
            (apart - 8.0).abs() > 0.01,
            "the two cells are {apart} apart after five hundred ticks of a myocyte working \
             the spring between them, so nothing has been worked"
        );
        assert!(
            world.ledger().biomass() > 0.0,
            "the world's only organism has spent everything it earned"
        );
    }

    // ---------------------------------------------------------------------------------
    // Group C - renewal
    //
    // SPEC section 10's reproduction paragraph, clause by clause. The code is in
    // `reproduction.rs`, which argues every decision it had to take; the tests are here
    // because every claim below is about a whole world - a slot taken off the free list, a
    // body grown from a mutated genome and written into the arena, an arena that did not
    // grow when there was nowhere to put one - and those are things only a `World` has.
    // ---------------------------------------------------------------------------------

    /// A world small enough to reason about and bright enough to breed in.
    ///
    /// The light is turned up from SPEC's shipped 0.012 to 0.4 against a ceiling of 80, which
    /// is the same trick `a_freed_slot_is_reusable_and_reaping_is_deterministic` uses and for
    /// the same reason: a tile deep enough to pay for a body outright, filling in a few hundred
    /// ticks rather than a few thousand. It changes how *fast* these tests run and nothing
    /// whatever about the rules they are asserting - every threshold below is written out from
    /// SPEC section 3's `reproduction_threshold` and SPEC section 6's table, neither of which
    /// this touches.
    ///
    /// The seed is an argument because two of these tests are about what the seed reaches: one
    /// runs the same world twice and one runs it under two different seeds.
    fn a_bright_world(seed: u64, slots: u32) -> Config {
        config(|raw| {
            raw.world.seed = seed;
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.light.cap = 80.0;
            raw.light.influx = 0.4;
            raw.limits.max_organisms = slots;
            raw.limits.max_cells_per_organism = 4;
        })
    }

    /// SPEC section 10's bar for a body of these cells: `reproduction_threshold ×` what it cost
    /// to build.
    ///
    /// Written out from SPEC section 3's 2.2 and SPEC section 6's upkeep table rather than
    /// asked of the code, at the 32-bit width the simulation actually stores those numbers at -
    /// so this and `reproduction.rs` can disagree. The one thing it does borrow is
    /// `CONSTRUCTION_TICKS`, because how many ticks of upkeep a cell is worth is a decision
    /// `cell.rs` took and argues, not a number SPEC gives.
    fn the_bar_for(upkeeps: &[f32]) -> f64 {
        let worth: f64 = upkeeps
            .iter()
            .map(|&upkeep| f64::from(upkeep * CellKind::CONSTRUCTION_TICKS))
            .sum();

        f64::from(2.2f32) * worth
    }

    /// A genome that grows one photocyte with `gonocytes` gonocytes strung off it in a line.
    ///
    /// The plainest body in the world that can breed: something to earn with, and SPEC section
    /// 6's gonocyte, without which no amount of energy is enough. One gene per join, each
    /// firing on one step and handing its daughter a fresh state, so the body is a chain and
    /// the order the gonocytes were made in is the order the genome put them in.
    ///
    /// More than one of them is what `an_offspring_is_a_mutated_copy_placed_next_to_a_gonocyte`
    /// needs: with two, "next to a gonocyte" has two possible answers and the test can say
    /// which one is taken. The arm between them is 13 units - nearly the longest a gene may ask
    /// for - so that a newborn resting against one of them cannot be resting against the other
    /// as well, whichever way round it was put down. At the eight units the other genomes here
    /// use, the two answers overlap and the test could not tell them apart.
    fn a_breeder(gonocytes: u8, limits: &LimitsConfig) -> Genome {
        let genes = (0..gonocytes)
            .map(|generation| Gene {
                trigger_state: State::new(generation),
                min_step: generation,
                max_step: generation,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(generation + 1),
                child_kind: CellKind::Gonocyte,
                rest_length: 13.0,
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

    /// Which slots have somebody in them.
    fn occupied(world: &World) -> Vec<usize> {
        (0..world.organisms().len())
            .filter(|&slot| world.organisms()[slot].is_some())
            .collect()
    }

    /// What the organism in this slot is holding.
    fn holding(world: &World, slot: usize) -> f64 {
        world.organisms()[slot]
            .as_ref()
            .expect("this test does not expect that slot to be empty")
            .energy()
    }

    /// ⭐ **A5.** An organism knows which organism it came from, and carries a fingerprint of
    /// the genome it is running.
    ///
    /// SPEC section 12 asks for hue to come from lineage and to *drift as the lineage drifts
    /// genetically*, which is how speciation becomes something visible while it is happening
    /// rather than a fact reported afterwards. Until now an organism has had a serial number
    /// and a genome and no way at all to say where it came from: two bodies could be identical
    /// twins or entirely unrelated and nothing in the world could tell them apart.
    ///
    /// # A founder has no parent, and that is a `None` rather than a zero
    ///
    /// Serial numbers start at nought, so "no parent" written as a zero is the same value as
    /// "the child of the very first organism in the run" - and every founder in the world would
    /// read as a child of the first founder. Nothing about that fails; a lineage tree simply
    /// comes out with every root joined to one arbitrary body.
    ///
    /// # What is asserted about the fingerprint here, and what is asserted in `genome.rs`
    ///
    /// The fingerprint itself - that it is stable, that it is order-sensitive, that the length
    /// goes in before the genes - belongs to `genome.rs` and is proved there. What this test
    /// adds is the two claims that can only be made about a *living* organism: that the number
    /// an organism carries is the number its own genome hashes to, and that a child whose
    /// genome came out different from its parent's carries a different one.
    ///
    /// That second claim is the whole point of the exercise and it is the one this test has to
    /// work for. Mutation is stochastic, so a single birth very often produces a genome
    /// identical to its parent's - which is not a failure, it is what an unmutated copy looks
    /// like. So the world is run until enough births have happened for at least one of them to
    /// have changed something, and what is asserted is that **the ones that changed genetically
    /// are exactly the ones whose fingerprint changed**. A fingerprint that ignored the genome
    /// and returned, say, the serial would fail the first half; one that was recomputed from
    /// something else entirely would fail the second.
    #[test]
    fn an_organism_knows_its_parent() {
        let mut world = World::new(&a_bright_world(42, 64));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        // A founder, put in by hand through the only door there is from outside.
        let founder = world
            .seed(
                a_breeder(1, &limits),
                Vec2::new(128.0, 72.0),
                the_bar_for(&[CellKind::Photocyte.upkeep(), CellKind::Gonocyte.upkeep()]) * 0.9,
            )
            .expect("a bright world has room and water for one founder");
        let (founder_serial, founder_hash, founder_genome) = {
            let first = world.organisms()[founder]
                .as_ref()
                .expect("the organism just seeded is in its slot");

            assert_eq!(
                first.parent(),
                None,
                "an organism seeded from outside has a parent, and there is nothing in the \
                 world it could be the child of"
            );
            assert_eq!(
                first.genome_hash(),
                first.genome().hash(),
                "an organism's fingerprint is not the fingerprint of the genome it is running"
            );

            (first.serial(), first.genome_hash(), first.genome().clone())
        };

        // Long enough for several generations. Every organism in the world after this is
        // descended from the one founder above.
        for _ in 0..2_000 {
            world.tick();
        }

        let mut children = 0;
        let mut mutated = 0;
        for slot in 0..world.organisms().len() {
            let Some(organism) = world.organisms()[slot].as_ref() else {
                continue;
            };

            assert_eq!(
                organism.genome_hash(),
                organism.genome().hash(),
                "the organism in slot {slot} carries a fingerprint that is not its genome's, \
                 so the number a renderer would colour it by has come loose from the genome \
                 it is supposed to be following"
            );

            if organism.serial() == founder_serial {
                continue;
            }

            let parent = organism
                .parent()
                .expect("everything in this world but the founder was born to a parent");
            assert!(
                parent < organism.serial(),
                "the organism in slot {slot} is serial {} and says its parent is serial \
                 {parent}, which had not been born yet",
                organism.serial()
            );
            children += 1;

            if *organism.genome() == founder_genome {
                assert_eq!(
                    organism.genome_hash(),
                    founder_hash,
                    "the organism in slot {slot} is running the founder's genome exactly and \
                     hashes to a different number"
                );
            } else {
                mutated += 1;
                assert_ne!(
                    organism.genome_hash(),
                    founder_hash,
                    "the organism in slot {slot} is running a genome that differs from the \
                     founder's and hashes to the same number, so its lineage drifted \
                     genetically without its colour drifting with it"
                );
            }
        }

        assert!(
            children > 4,
            "only {children} organisms in this world were born to a parent, so barely \
             anything has reproduced and this test has established nothing"
        );
        assert!(
            mutated > 0,
            "not one of the {children} descendants differs from the founder genetically, so \
             the half of this test that is about drift never ran"
        );
    }

    /// How far apart two markers are, going whichever way round the circle is shorter.
    ///
    /// Never more than a half, which is what two markers on opposite sides of the circle are.
    fn apart_on_the_circle(one: f32, other: f32) -> f32 {
        let apart = (one - other).abs();

        apart.min(1.0 - apart)
    }

    /// The mean of a set of distances, or nothing at all if there were none.
    fn mean(distances: &[f32]) -> f32 {
        let count = f32::from(u16::try_from(distances.len()).expect("fewer than 65,536 pairs"));

        distances.iter().sum::<f32>() / count
    }

    /// ⭐⭐ **D5, and it is stated as a measurement because a measurement is what would have
    /// caught the thing that went wrong.** Two organisms of one lineage are closer on the marker
    /// circle than two organisms of different lineages.
    ///
    /// # What this test is a reaction to
    ///
    /// Group B built colour from the genome fingerprint, dumped a frame, and looked at it. SPEC
    /// section 11 now carries the correction in as many words: *"A hash does not drift; it jumps.
    /// Any mutation at all reseeds it, so every child is a completely unrelated colour from its
    /// parent. It was built that way and looked at, and the result is confetti: adjacent bodies
    /// within one colony come out cyan, magenta and orange at random, and colour makes speciation
    /// **less** visible than no colour would."* `docs/frames/phase5-groupb.png` is that frame.
    ///
    /// Nothing in the suite noticed, because every test written about the fingerprint was true of
    /// it: it was stable, it was order-sensitive, and a genome that changed changed it. What was
    /// never asserted is the property the whole thing exists for, which is that **relatives look
    /// alike** - a claim about two organisms rather than about one, and that is why no test of a
    /// single hue could have made it.
    ///
    /// # The three claims
    ///
    /// **One - siblings are close and strangers are not.** Two founders are seeded at opposite
    /// ends of a world, both lineages are followed for three thousand ticks, and two averages are
    /// taken: over pairs of living organisms sharing a parent, and over pairs descended from
    /// different founders. The first has to be several times smaller than the second. **Measured
    /// on Group B's arrangement, they come out at 0.102 and 0.146** - barely a difference, and
    /// the only reason it is not none at all is that a hash does keep exact clones together.
    ///
    /// **Two - a copy that changed nothing stands exactly where its parent did.** A child whose
    /// genome came out identical to its parent's carries its parent's marker to the last bit. At
    /// SPEC section 3's rates the great majority of births copy the genome unchanged, so this is
    /// what makes a whole colony sit in one small stretch of the circle.
    ///
    /// **Three - and one that changed did move.** Otherwise a marker that was simply inherited
    /// unaltered for ever would pass the first two claims and carry no information at all - every
    /// descendant of a founder would sit exactly where the founder did, and a lineage that split
    /// into two would still be one point.
    ///
    /// The ancestry has to be recorded as the run happens rather than walked at the end, because
    /// `Organism::parent` is a serial and the organism it names is long dead: a slot is handed on
    /// and a serial is not, which is exactly why `organism.rs` keeps a serial there.
    #[test]
    fn a_lineage_marker_is_inherited_and_drifts_with_the_genome() {
        // A `BTreeMap` rather than a `HashMap`, which `clippy.toml` disallows in this crate for
        // SPEC section 2's reason: nothing whose iteration order is unspecified may touch a
        // simulation. Nothing here iterates one - but a rule that has exceptions is a rule
        // nobody can check.
        use std::collections::BTreeMap;

        // `a_bright_world`'s water, with the mutation turned up. At SPEC section 3's shipped
        // rates only about a sixth of births change anything at all, so in a world this small a
        // few thousand ticks produce a population of near-identical clones - and a fingerprint
        // would then look *almost* inherited, for the wrong reason. Turning the rates up makes
        // every lineage here genuinely diverge, which is the case the two arrangements differ
        // in. Nothing about the marker depends on the rates; what depends on them is whether
        // this test has anything to measure.
        let mut world = World::new(&config(|raw| {
            raw.world.seed = 42;
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.light.cap = 80.0;
            raw.light.influx = 0.4;
            raw.limits.max_organisms = 128;
            raw.limits.max_cells_per_organism = 4;
            raw.mutation.point_rate = 0.30;
            raw.mutation.duplication_rate = 0.05;
            raw.mutation.deletion_rate = 0.05;
        }));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        // Two founders, at opposite ends of the world so that neither lineage is fed out of the
        // other's water and both have room to grow into.
        let mut founders = Vec::new();
        for at in [Vec2::new(64.0, 72.0), Vec2::new(192.0, 72.0)] {
            let slot = world
                .seed(
                    a_breeder(1, &limits),
                    at,
                    the_bar_for(&[CellKind::Photocyte.upkeep(), CellKind::Gonocyte.upkeep()]) * 0.9,
                )
                .expect("a bright world has room and water for two founders");
            founders.push(
                world.organisms()[slot]
                    .as_ref()
                    .expect("the organism just seeded is in its slot")
                    .serial(),
            );
        }

        let spacing =
            apart_on_the_circle(founding_marker(founders[0]), founding_marker(founders[1]));
        assert!(
            spacing > 0.3,
            "the two founders of this run were seeded {spacing} apart on the circle, so there is \
             hardly anything for the two lineages below to be told apart by"
        );

        // Which founder each organism ever born descends from, recorded as the run happens.
        let mut lineage: BTreeMap<u64, u64> = founders.iter().map(|&at| (at, at)).collect();
        for _ in 0..3_000 {
            world.tick();

            for organism in world.organisms().iter().flatten() {
                if lineage.contains_key(&organism.serial()) {
                    continue;
                }

                let root = organism
                    .parent()
                    .and_then(|parent| lineage.get(&parent).copied());
                if let Some(root) = root {
                    lineage.insert(organism.serial(), root);
                }
            }
        }

        let living: Vec<&Organism> = world.organisms().iter().flatten().collect();
        assert!(
            living.len() > 20,
            "only {} organisms are alive, so there is not enough of a population here to say \
             anything about how the marker is spread over it",
            living.len()
        );

        // --- one: siblings against strangers ---
        let mut siblings = Vec::new();
        let mut strangers = Vec::new();
        for (at, one) in living.iter().enumerate() {
            for other in &living[at + 1..] {
                let apart = apart_on_the_circle(one.marker(), other.marker());

                match (one.parent(), other.parent()) {
                    (Some(mother), Some(father)) if mother == father => siblings.push(apart),
                    _ => {}
                }

                if let (Some(&here), Some(&there)) =
                    (lineage.get(&one.serial()), lineage.get(&other.serial()))
                    && here != there
                {
                    strangers.push(apart);
                }
            }
        }

        assert!(
            siblings.len() > 20 && strangers.len() > 200,
            "this world produced {} pairs of siblings and {} pairs of strangers, which is not \
             enough of either to average",
            siblings.len(),
            strangers.len()
        );

        let (close, far) = (mean(&siblings), mean(&strangers));
        assert!(
            close * 4.0 < far,
            "two organisms with the same mother are {close} apart on the circle on average and \
             two from different founders are {far} apart, which is barely a difference: the \
             marker is carrying almost nothing about descent, and anything reading it would show \
             two lineages as one. Measured on Group B's arrangement - a marker taken from the \
             genome fingerprint - those two come out at 0.102 and 0.146, because a hash jumps \
             the moment anything mutates and the only relatives it keeps together are the ones \
             that are still exact clones. docs/frames/phase5-groupb.png is what that looked like"
        );

        // --- two and three: a copy that changed nothing, and one that did ---
        let by_serial: BTreeMap<u64, &Organism> =
            living.iter().map(|&who| (who.serial(), who)).collect();
        let (mut unchanged, mut moved) = (0, 0);
        for child in &living {
            let Some(mother) = child.parent().and_then(|parent| by_serial.get(&parent)) else {
                continue;
            };

            if child.genome() == mother.genome() {
                unchanged += 1;
                assert!(
                    child.marker().to_bits() == mother.marker().to_bits(),
                    "serial {} copied its mother's genome exactly and came out at {} on the \
                     circle against her {}, so a lineage that is not changing is drifting anyway",
                    child.serial(),
                    child.marker(),
                    mother.marker()
                );
            } else if child.marker().to_bits() != mother.marker().to_bits() {
                moved += 1;
            }
        }

        assert!(
            unchanged > 0,
            "not one living organism here is running its own mother's genome unaltered, so the \
             claim that an unchanged copy stands exactly where its mother did was never tested"
        );
        assert!(
            moved > 0,
            "{unchanged} children carry their mother's genome and not one child whose genome \
             differs from its mother's has moved on the circle, so the marker is inherited and \
             never drifts - every descendant of a founder would sit exactly where the founder \
             does for ever, and a lineage splitting would still be one point"
        );
    }

    /// ⭐ **C1.** An organism reproduces once it is holding more than
    /// `reproduction_threshold ×` what its body cost to build, and not before.
    ///
    /// SPEC section 10's first clause. The bar is the interesting part of it: reproduction is
    /// not free and it is not cheap, and what it is priced against is the organism's **own
    /// body**, so an elaborate lineage has to earn more before it may copy itself than a plain
    /// one does. That is the only thing in the simulation pushing back against bodies simply
    /// getting bigger.
    ///
    /// # `construction_energy` is `metabolism.rs`'s, and that matters
    ///
    /// SPEC uses the phrase "construction energy" twice - here, and for what a corpse is shared
    /// out in proportion to - and never defines it. Two sums would be two definitions of one
    /// phrase, free to drift apart, and nothing would report it. So the last assertion here is
    /// that the bar this test wrote out from SPEC's own numbers is `reproduction_threshold ×`
    /// the same sum `metabolism.rs` shares a corpse by.
    ///
    /// # Why it watches every tick rather than looking at the end
    ///
    /// Because "reproduces above the threshold" is two claims and only one of them is about a
    /// birth happening. A body seeded a unit under the bar earns its way across it over a few
    /// ticks, and what is asserted is that **every tick before the birth ended with the parent
    /// at or under the bar** - so the birth did not happen early - and that the tick it did
    /// happen on, the parent was holding more than the bar. Without the first half, an
    /// implementation that reproduced whenever it felt like it would pass.
    ///
    /// What the parent was holding at the moment of the birth is not directly visible, because
    /// by the time the tick has finished it has already given part of it away. It is
    /// reconstructed as `parent + child`, which is the same claim from the other side: those
    /// two together are exactly what the one of them was holding a moment earlier, and
    /// `a_birth_transfers_offspring_share_of_the_parents_energy` is what makes that true.
    #[test]
    fn an_organism_reproduces_above_the_threshold() {
        let mut world = World::new(&a_bright_world(42, 8));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        // SPEC section 6: a photocyte at 0.004 a tick and a gonocyte at 0.005.
        let bar = the_bar_for(&[0.004, 0.005]);
        let parent = world
            .seed(a_breeder(1, &limits), Vec2::new(120.0, 72.0), bar - 1.0)
            .expect("a lit world holds nineteen units under a two-celled body");

        assert!(
            (bar - f64::from(world.config().metabolism.reproduction_threshold)
                * construction_energy(world.cells_of(parent)))
            .abs()
                < 1e-9,
            "this test's bar of {bar} is not `reproduction_threshold` times the same \
             construction energy `metabolism.rs` shares a corpse out by, so SPEC section 10's \
             one phrase has become two different sums"
        );

        let mut born_on = 0u64;
        let mut at_the_birth = 0.0f64;

        for tick in 1..=400u64 {
            world.tick();

            let alive = occupied(&world);
            if alive.len() == 1 {
                let short = holding(&world, parent);
                assert!(
                    short <= bar,
                    "the parent finished tick {tick} holding {short}, which is over SPEC \
                     section 10's bar of {bar}, and it did not reproduce"
                );
                continue;
            }

            born_on = tick;
            at_the_birth = alive.iter().map(|&slot| holding(&world, slot)).sum();
            break;
        }

        assert!(
            born_on > 0,
            "four hundred ticks of a lit world and a body that started a unit under the bar \
             never reproduced at all"
        );
        assert!(
            at_the_birth > bar,
            "a birth happened on tick {born_on} with the parent holding {at_the_birth}, and \
             SPEC section 10's bar is {bar}"
        );
        assert_eq!(
            occupied(&world).len(),
            2,
            "one organism crossing the bar produced more than one child in a tick"
        );
    }

    /// ⭐ **C2.** However rich it gets, an organism with no gonocyte never reproduces.
    ///
    /// SPEC section 6, in as many words: *"an organism with no gonocyte cannot reproduce"*, and
    /// section 6's design intent says why - *"requiring a gonocyte means reproduction has a
    /// real structural cost"*. A lineage has to spend part of the body it could have spent on
    /// feeding, and part of its upkeep on every tick of its life, on tissue that earns nothing
    /// whatever. That is the whole of what makes breeding a trade rather than a reward.
    ///
    /// # The two bodies are in the same world on purpose
    ///
    /// Two separate worlds would make this a comparison of two runs, and a run is a great many
    /// things at once. Here the same light falls on both, both are seeded on the same tick
    /// holding the same energy, and both are far above the bar their own bodies set - so the
    /// only thing that differs is the one thing SPEC names.
    ///
    /// # And what says the barren one *could* have bred
    ///
    /// That it grows, and keeps growing. A body that reproduces gives away nearly half of
    /// itself every time it does, so it saws back and forth across its own bar for its whole
    /// life; a body that cannot reproduce simply accumulates. So the closing assertions are
    /// that the barren body ended well past twice its own bar, and that it never once fell by
    /// so much as a unit in a tick - which is what handing over `offspring_share` of anything
    /// above that bar would look like.
    #[test]
    fn an_organism_with_no_gonocyte_cannot_reproduce() {
        let mut world = World::new(&a_bright_world(42, 6));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        let fertile = world
            .seed(a_breeder(1, &limits), Vec2::new(64.0, 72.0), 30.0)
            .expect("a lit world holds thirty units under a two-celled body");
        let barren = world
            .seed(a_chain(2, &limits), Vec2::new(192.0, 72.0), 30.0)
            .expect("a lit world holds thirty units under a two-celled body");

        // Two photocytes at SPEC section 6's 0.004, which is a *lower* bar than the fertile
        // body's - so the barren one is not being kept childless by an accountant.
        let barren_bar = the_bar_for(&[0.004, 0.004]);
        assert!(
            barren_bar < the_bar_for(&[0.004, 0.005]),
            "the barren body is being asked for more than the fertile one, so this test \
             cannot tell a missing gonocyte from a bar it never reached"
        );

        let mut worst_fall = 0.0f64;
        let mut previously = holding(&world, barren);

        for _ in 0..400 {
            world.tick();

            let now = holding(&world, barren);
            worst_fall = worst_fall.max(previously - now);
            previously = now;
        }

        assert!(
            occupied(&world).len() > 2,
            "four hundred ticks and the body with a gonocyte in it never reproduced either, \
             so this test proves nothing about gonocytes"
        );
        assert!(
            previously > barren_bar * 2.0,
            "the barren body finished holding {previously}, against its own bar of \
             {barren_bar} - so it never got rich enough for the missing gonocyte to be what \
             stopped it"
        );
        assert!(
            worst_fall < 1.0,
            "the barren body lost {worst_fall} in a single tick, and handing over \
             `offspring_share` of anything above {barren_bar} would look like exactly that"
        );
        assert_eq!(
            world.cells_of(barren).len(),
            2,
            "the barren body is not the body this test seeded"
        );
        assert!(
            !world
                .cells_of(barren)
                .iter()
                .any(|cell| cell.kind == CellKind::Gonocyte),
            "the barren body has grown a gonocyte, which nothing in the simulation can do"
        );
        assert!(
            world
                .cells_of(fertile)
                .iter()
                .any(|cell| cell.kind == CellKind::Gonocyte),
            "the body that did reproduce has no gonocyte either, so the two bodies do not \
             differ in the one thing this test is about"
        );
    }

    /// ⭐ **C3.** An offspring is a **mutated copy** of its parent's genome, and its body is put
    /// down touching one of its parent's gonocytes.
    ///
    /// SPEC section 10's middle three clauses: *"copy the genome, mutate, develop the new body,
    /// place the seed cell adjacent to a gonocyte with a small random offset"*.
    ///
    /// # What "a mutated copy" is asserted against
    ///
    /// The child's genome is recomputed here, from the parent's genome and a fresh stream of
    /// the parent's own random numbers, and compared gene for gene. That is a stronger claim
    /// than "it differs sometimes": it says the child is *this* copy rather than any copy, that
    /// the copy was made with the operators `mutation.rs` implements, and - the part Phase 4
    /// actually needs - that the numbers came out of the **parent's** sequence rather than the
    /// world's. `a_lineage_is_still_deterministic_across_generations` is the same claim made
    /// without looking inside.
    ///
    /// Most copies are perfect, because most reproductions mutate nothing: SPEC section 3's
    /// rates are per gene and per genome and they are all small. So the last claim here is over
    /// a run long enough for the operators to bite, and it is that somewhere in the world there
    /// is now a genome that is not the founder's.
    ///
    /// # "Adjacent" and "a small random offset" are both decisions, and both are pinned
    ///
    /// SPEC gives neither a distance nor a direction. `reproduction.rs` reads adjacent as
    /// **exactly touching** - the two radii added together, from SPEC section 6's table - and
    /// the offset as **which way**, drawn from the parent's stream. The two bodies here have
    /// two gonocytes apiece so that "a gonocyte" has more than one answer, and what is asserted
    /// is that the child went next to the **first** one, which is the one the genome grew
    /// first.
    ///
    /// The direction is pinned by running the same world under a second seed. Everything about
    /// the two runs is identical up to this point - the physics reads no random numbers and a
    /// seeded body is placed where it is asked for - so a child landing on a different side is
    /// a direction that was *drawn*. A fixed one would stack every child of every body in the
    /// world on the same spot, which is the case `physics.rs` has to break a tie for.
    ///
    /// # And that a newborn beside its parent does not blow the physics up
    ///
    /// A body put down touching another body is a body the collision force will push away, and
    /// an explicit integrator pushed hard enough diverges - a lineage flung across the world
    /// rather than an error message. So the world is then left to breed freely for four hundred
    /// ticks, and every cell in it is checked for being somewhere real: inside the world, at a
    /// finite position, and not moving faster than anything in a viscous soup has any business
    /// moving.
    #[test]
    fn an_offspring_is_a_mutated_copy_placed_next_to_a_gonocyte() {
        // The same world twice, differing only in its seed. Everything up to the first birth is
        // identical in the two.
        let first_child = |seed: u64| -> (World, usize, usize) {
            let mut world = World::new(&a_bright_world(seed, 32));
            let limits = world.config().limits.clone();

            for _ in 0..300 {
                world.tick();
            }

            // Three cells - a photocyte and two gonocytes - so "adjacent to a gonocyte" has two
            // possible answers. Seeded well over the bar, so the birth is on the first tick.
            let parent = world
                .seed(a_breeder(2, &limits), Vec2::new(96.0, 72.0), 34.0)
                .expect("a lit world holds thirty-four units under a three-celled body");
            assert!(
                34.0 > the_bar_for(&[0.004, 0.005, 0.005]),
                "the parent is not seeded above its own bar, so no birth is expected at all"
            );

            world.tick();

            let child = *occupied(&world)
                .iter()
                .find(|&&slot| slot != parent)
                .expect("a body seeded over the bar reproduces on its first tick");

            (world, parent, child)
        };

        let (mut world, parent, child) = first_child(42);
        let (elsewhere, _, other_child) = first_child(43);

        // ⭐ The child's genome, recomputed from the parent's own numbers.
        let founder = a_breeder(2, &world.config().limits);
        let mut stream = WorldRng::from_seed(world.config().world.seed).new_organism_stream(
            world.organisms()[parent]
                .as_ref()
                .expect("the parent is alive")
                .serial(),
        );
        let expected = mutate(
            &founder,
            &world.config().mutation,
            &world.config().limits,
            &mut stream,
        );

        assert_eq!(
            world.organisms()[child]
                .as_ref()
                .expect("the child is alive")
                .genome(),
            &expected,
            "the child's genome is not what `mutation.rs` makes of its parent's genome from \
             the parent's own stream of random numbers"
        );
        assert_eq!(
            world.organisms()[child]
                .as_ref()
                .expect("the child is alive")
                .age(),
            0,
            "a newborn is not newborn"
        );

        // ⭐ Where it went: touching the **first** of its parent's two gonocytes.
        let body = world.cells_of(parent);
        let (near, far) = (body[1].pos, body[2].pos);
        assert!(
            body[1].kind == CellKind::Gonocyte && body[2].kind == CellKind::Gonocyte,
            "this body is meant to have two gonocytes on it and has {:?}",
            body.iter().map(|cell| cell.kind).collect::<Vec<_>>()
        );

        let seed_cell = world.cells_of(child)[0];
        let touching = CellKind::Gonocyte.radius() + seed_cell.kind.radius();
        let reach = (seed_cell.pos - near).length();

        assert!(
            (reach - touching).abs() < 1e-4,
            "the newborn's seed cell is {reach} from its parent's first gonocyte, and SPEC \
             section 6's radii put the two exactly touching at {touching}"
        );
        assert!(
            (seed_cell.pos - far).length() > touching + 0.5,
            "the newborn is resting against the *second* of its parent's gonocytes at {} away, \
             and the first one is the one the genome grew first",
            (seed_cell.pos - far).length()
        );

        // ⭐ Which side it went on was drawn rather than fixed.
        let here = seed_cell.pos - near;
        let there = elsewhere.cells_of(other_child)[0].pos - elsewhere.cells_of(parent)[1].pos;
        assert!(
            (here - there).length() > 1.0,
            "two runs differing only in their seed put the first newborn on the same side of \
             its parent, at {here:?} and {there:?} - so the offset is fixed, and every child a \
             body ever has would be laid down in the same spot"
        );

        // The copy is imperfect, given enough births for SPEC section 3's rates to bite. Most
        // of them change nothing: the rates are per gene and per genome and they are all small,
        // so a two-gene genome comes through about five reproductions in six untouched.
        for _ in 0..1_200 {
            world.tick();
        }

        assert!(
            occupied(&world).len() > 8,
            "twelve hundred ticks of a breeding world and it holds only {} organisms, so \
             there have not been enough births for a mutation to be expected in any of them",
            occupied(&world).len()
        );
        assert!(
            occupied(&world)
                .iter()
                .filter_map(|&slot| world.organisms()[slot].as_ref())
                .any(|organism| organism.genome() != &founder),
            "twelve hundred ticks of a breeding world and every genome in it is still exactly \
             the founder's, so the copy is perfect and nothing can ever change"
        );

        // ⭐ And nothing was flung anywhere. A newborn is laid down touching its parent, which
        // is a collision force from the first tick of its life.
        let (width, height) = (world.config().world.width, world.config().world.height);
        for slot in occupied(&world) {
            for cell in world.cells_of(slot) {
                assert!(
                    cell.pos.x >= 0.0 && cell.pos.x < width,
                    "a cell of the body in slot {slot} is at x = {} in a world {width} wide",
                    cell.pos.x
                );
                assert!(
                    cell.pos.y >= 0.0 && cell.pos.y <= height,
                    "a cell of the body in slot {slot} is at y = {} in a world {height} deep",
                    cell.pos.y
                );
                assert!(
                    cell.vel.length() < 1_000.0,
                    "a cell of the body in slot {slot} is moving at {} units a second, and \
                     SPEC section 8 says a cell shoved at sixty travels under two body-widths \
                     before it stops",
                    cell.vel.length()
                );
            }
        }
    }

    /// ⭐⭐ **C4.** A birth moves `offspring_share` of the parent's energy to the child - **out
    /// of the parent**, and not from anywhere else.
    ///
    /// SPEC section 10's last clause, and the most dangerous line in Group C.
    ///
    /// # Why the assertion is that the parent went down
    ///
    /// SPEC section 5 spells out the failure this is guarding against, and it is the one the
    /// energy invariant provably **cannot** see. An organism's energy and the ledger's
    /// `biomass` account are two records of one quantity. A newborn handed a share its parent
    /// never gave up leaves all five accounts exactly where they were - the books balance
    /// perfectly, the check at the end of the tick says nothing, and a body stands in the world
    /// holding energy nobody counted. It stays silent until that body dies and its energy is
    /// moved out of an account that never received it, hours into a run, with no cause to find.
    ///
    /// Phase 2 learned this about seeded organisms and wrote it into SPEC. This is the same
    /// trap one phase along, and the same answer: assert the parent went **down** by what the
    /// child went **up** by, rather than that the books balance.
    ///
    /// # The parent is a body that cannot earn, which is what makes the arithmetic exact
    ///
    /// A single gonocyte: SPEC section 6 gives it no way to take anything out of the water, so
    /// over one tick the only thing that happens to its energy besides the birth is one tick of
    /// upkeep. Every figure below is therefore written out in full from SPEC's own numbers -
    /// what it was seeded with, less its upkeep, split 45:55 - rather than measured and
    /// compared with itself. A photosynthetic parent would have earned an unknown amount in the
    /// same tick and the test would have had to ask the code what that was.
    ///
    /// # And the tick after, which pins a decision SPEC does not make
    ///
    /// The child is born holding more than its own bar, and it does **not** reproduce on the
    /// tick it was born. `reproduction.rs` argues why: the pass walks the slots from the front,
    /// so without that rule one rich body would fill the entire world between two ticks. It
    /// breeds on the very next tick instead, which is what the closing assertion checks - the
    /// rule delays a birth by a tick rather than forbidding one.
    #[test]
    fn a_birth_transfers_offspring_share_of_the_parents_energy() {
        let mut world = World::new(&a_bright_world(42, 8));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        // One gonocyte, one gene. SPEC section 6 gives it an upkeep of 0.005 and no income at
        // all, and SPEC section 3's `gene_cost` is 0.0001 for the one gene it carries.
        let parent = world
            .seed(
                a_single_cell(CellKind::Gonocyte, &limits),
                Vec2::new(120.0, 72.0),
                30.0,
            )
            .expect("a lit world holds thirty units under a single cell");

        let seeded = holding(&world, parent);
        let biomass_before = world.ledger().biomass();
        let upkeep = f64::from(0.005f32) + f64::from(0.0001f32);
        let bar = the_bar_for(&[0.005]);
        assert!(
            seeded > bar,
            "the parent is holding {seeded} against a bar of {bar}, so no birth is expected"
        );

        world.tick();

        let alive = occupied(&world);
        assert_eq!(alive.len(), 2, "a body well over the bar had no child");
        let child = *alive
            .iter()
            .find(|&&slot| slot != parent)
            .expect("two organisms and one of them is the parent");

        // What SPEC's arithmetic says the two of them hold, worked out from the outside.
        let at_the_birth = seeded - upkeep;
        let dowry = f64::from(0.45f32) * at_the_birth;

        assert!(
            (holding(&world, child) - dowry).abs() < 1e-12,
            "the child is holding {}, against the {dowry} that `offspring_share` of the {} its \
             parent had comes to",
            holding(&world, child),
            at_the_birth
        );
        // ⭐ The claim. What the parent has left is what it had less exactly what the child is
        // holding - so the child's energy came out of the parent and out of nothing else.
        assert!(
            (holding(&world, parent) - (at_the_birth - dowry)).abs() < 1e-12,
            "the parent was holding {at_the_birth} and gave away {dowry}, and it is left with \
             {} rather than {}",
            holding(&world, parent),
            at_the_birth - dowry
        );
        assert!(
            holding(&world, parent) < seeded,
            "the parent finished the tick it reproduced on holding more than it started with"
        );

        // And the books never had to move, because both ends of a birth are living tissue. That
        // is precisely why this failure is invisible to them, and why the assertion that
        // catches it is the one above rather than this one.
        let alive_now: f64 = alive.iter().map(|&slot| holding(&world, slot)).sum();
        assert!(
            (world.ledger().biomass() - alive_now).abs() < 1e-12,
            "the books say {} is alive in this world and the two organisms in it are holding \
             {alive_now} between them - so a birth has either invented energy or lost it",
            world.ledger().biomass()
        );
        assert!(
            (biomass_before - world.ledger().biomass() - upkeep).abs() < 1e-12,
            "a tick with a birth in it moved the living-biomass account by {}, and the only \
             thing that should have moved it is one tick of upkeep at {upkeep}",
            biomass_before - world.ledger().biomass()
        );

        // The child is already over its own bar and waited a tick anyway. See above.
        assert!(
            holding(&world, child) > bar,
            "the child is holding {} against a bar of {bar}, so this test cannot tell a body \
             that waited from a body that could not afford to breed",
            holding(&world, child)
        );

        world.tick();

        assert_eq!(
            occupied(&world).len(),
            4,
            "the parent and the child were both over the bar on the tick after the birth and \
             the world holds {} organisms",
            occupied(&world).len()
        );
    }

    /// ⭐⭐ **C5.** When there is nowhere to put a child, the birth **does not happen** - and
    /// nothing is spent on it, and no arena grows.
    ///
    /// CLAUDE.md: *"When the population cap is reached, births fail rather than allocating.
    /// (This is also biologically reasonable: a full world should mean nowhere to reproduce
    /// into.) A simulation that cannot allocate cannot leak."* SPEC section 10 adds the word
    /// that matters - **silently**. There is nothing to report, because a full world is not a
    /// fault; it is the pressure the whole ecology is meant to grow out of.
    ///
    /// `a_birth_fails_at_the_cap_rather_than_allocating` already makes this claim about
    /// [`World::seed`], which is the door from outside. This is the same claim about the door
    /// organisms come in through by themselves, which is the one a run of tens of millions of
    /// ticks actually uses.
    ///
    /// # ⚠️ Half of this test passes against a world where nothing is ever born
    ///
    /// That is the trap in it, and it is worth stating plainly because it is how this test
    /// first ran: with reproduction not yet written, a full world declining to produce children
    /// is exactly what a world with no reproduction in it looks like. Every assertion about the
    /// cap went green immediately and meant nothing whatever.
    ///
    /// So the test is in two halves and the second one is not optional. The same three bodies,
    /// in the same world with **one slot free**, and one of them has a child. Without that, this
    /// is a test that a feature which does not exist has not been used.
    ///
    /// # What is measured is capacity, not length
    ///
    /// For the reason `a_birth_fails_at_the_cap_rather_than_allocating` gives at length: the
    /// failure being guarded against is not a wrong answer, it is a `Vec` quietly doubling
    /// itself in a process that is meant to have a fixed footprint all night. A vector that has
    /// grown has moved, so the addresses of the two largest are compared as well.
    ///
    /// # And nothing was spent on the birth that did not happen
    ///
    /// The three bodies are lone gonocytes, which earn nothing, so a tick costs each of them
    /// exactly one upkeep and there is nothing else for their energy to do. A parent that had
    /// handed over its share and *then* found there was nowhere to put the child would show up
    /// here as a body 45% lighter - and in a run that spends most of a long night at its cap,
    /// that is every organism in the world paying for children that do not exist, every tick.
    #[test]
    fn births_fail_silently_at_the_population_cap() {
        let mut world = World::new(&a_bright_world(42, 3));
        let limits = world.config().limits.clone();

        for _ in 0..300 {
            world.tick();
        }

        for along in [40.0f32, 120.0, 200.0] {
            world
                .seed(
                    a_single_cell(CellKind::Gonocyte, &limits),
                    Vec2::new(along, 72.0),
                    30.0,
                )
                .expect("a lit world holds thirty units under a single cell");
        }

        assert!(
            world.free.is_empty(),
            "this test needs a full world and there are {} slots free",
            world.free.len()
        );

        let capacities = [
            world.cells.capacity(),
            world.springs.capacity(),
            world.organisms.capacity(),
            world.free.capacity(),
            world.crowd.capacity(),
            world.bonds.capacity(),
            world.live.capacity(),
            world.owner.capacity(),
            world.drift.capacity(),
        ];
        let addresses = [
            world.cells.as_ptr().cast::<u8>(),
            world.springs.as_ptr().cast::<u8>(),
        ];
        let before: Vec<f64> = (0..3).map(|slot| holding(&world, slot)).collect();
        let upkeep = f64::from(0.005f32) + f64::from(0.0001f32);
        let bar = the_bar_for(&[0.005]);
        assert!(
            before.iter().all(|&held| held > bar),
            "all three bodies have to be over the bar of {bar} for this to be a test about \
             the cap, and they are holding {before:?}"
        );

        world.tick();

        assert_eq!(
            occupied(&world).len(),
            3,
            "a full world produced a child anyway"
        );
        for (slot, &held) in before.iter().enumerate() {
            let spent = held - holding(&world, slot);
            assert!(
                (spent - upkeep).abs() < 1e-12,
                "the body in slot {slot} paid {spent} over a tick in which it could not \
                 reproduce, against the {upkeep} a tick of its upkeep costs - so it handed \
                 over a share for a child that was never born"
            );
        }
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
                world.owner.capacity(),
                world.drift.capacity(),
            ],
            "a birth that could not happen grew one of the world's arenas, so the memory this \
             run uses is decided by how many organisms try to be born rather than by the \
             configuration"
        );
        assert_eq!(
            addresses,
            [
                world.cells.as_ptr().cast::<u8>(),
                world.springs.as_ptr().cast::<u8>()
            ],
            "a birth that could not happen moved an arena somewhere else in memory"
        );

        // And the world is undamaged: it still ticks, and it still holds three organisms.
        world.tick();
        assert_eq!(occupied(&world).len(), 3);

        // ⭐ The positive control. Everything above is also true of a world in which nothing is
        // ever born, so the same three bodies are given one slot to breed into.
        let mut roomy = World::new(&a_bright_world(42, 4));
        for _ in 0..300 {
            roomy.tick();
        }
        for along in [40.0f32, 120.0, 200.0] {
            roomy
                .seed(
                    a_single_cell(CellKind::Gonocyte, &limits),
                    Vec2::new(along, 72.0),
                    30.0,
                )
                .expect("a lit world holds thirty units under a single cell");
        }
        let roomy_before: Vec<f64> = (0..3).map(|slot| holding(&roomy, slot)).collect();

        roomy.tick();

        assert_eq!(
            occupied(&roomy),
            vec![0, 1, 2, 3],
            "three bodies over the bar with one slot between them did not fill it"
        );
        assert!(
            roomy_before[0] - holding(&roomy, 0) > 1.0,
            "the body in slot 0 paid {} for the one child there was room for, and a share of \
             thirty units is a great deal more than that",
            roomy_before[0] - holding(&roomy, 0)
        );
        for slot in [1, 2] {
            let spent = roomy_before[slot] - holding(&roomy, slot);
            assert!(
                (spent - upkeep).abs() < 1e-12,
                "the body in slot {slot} paid {spent} over the tick, and the one free slot had \
                 already gone to the body in slot 0 - so the slots are not being walked from \
                 the front, and which body gets the last place in a full world is not decided \
                 by anything written down"
            );
        }
    }

    /// ⭐⭐ **C6.** A lineage mutates the same way whatever else is alive around it, and a whole
    /// world of them replays exactly.
    ///
    /// SPEC section 2 calls determinism load-bearing and gives the reason: when something
    /// interesting happens you will want to see it again. `a_run_is_still_reproducible` makes
    /// that claim about a world of water, light and bodies being pushed around, and says in its
    /// own documentation that *"the seed will reach the organisms in Phase 4, when reproduction
    /// mutates a genome out of an organism's own stream, and this test should grow to cover
    /// that when it does"*. This is that test.
    ///
    /// # The two halves are different claims and the second is the load-bearing one
    ///
    /// The first is the ordinary one: two runs of one seed, with births and deaths in them,
    /// agree down to the last bit. It would pass against an implementation that drew every
    /// mutation from a single world-wide generator, because a single generator is perfectly
    /// reproducible as long as nothing changes the order it is drawn from in.
    ///
    /// The second is what SPEC section 2 actually asks for: *"each organism carries its own RNG
    /// stream, seeded deterministically from `(world_seed, organism_serial)`. This keeps
    /// organism-level randomness independent of evaluation order, which is what allows `rayon`
    /// parallelism without breaking reproducibility."* So the same founder is bred in two
    /// worlds - one where it is alone, and one where three other bodies are sitting on top of
    /// it, shading it and shoving it about - and its first child has to be the same child. It
    /// is born on a **different tick** in the two, because its parent earns differently under a
    /// crowd, and the assertion that the two ticks differ is what makes the genome assertion
    /// mean something.
    ///
    /// Under a world-wide generator the crowded run would have drawn a different number of
    /// times before that birth, and the child would be somebody else.
    #[test]
    fn a_lineage_is_still_deterministic_across_generations() {
        // Half one: the same seed, twice, over a world where things are born.
        let run = |seed: u64| -> World {
            let mut world = World::new(&a_bright_world(seed, 8));
            let limits = world.config().limits.clone();

            for _ in 0..300 {
                world.tick();
            }
            world
                .seed(a_breeder(1, &limits), Vec2::new(120.0, 72.0), 30.0)
                .expect("a lit world holds thirty units under a two-celled body");
            for _ in 0..600 {
                world.tick();
            }

            world
        };

        let once = run(42);
        let again = run(42);

        assert!(
            occupied(&once).len() > 2,
            "six hundred ticks and the founder had at most one child, so this test is not \
             looking at generations"
        );
        assert_eq!(
            every_number_in(&once),
            every_number_in(&again),
            "two runs of seed 42, with organisms breeding in them, no longer agree"
        );
        assert_eq!(
            occupied(&once)
                .iter()
                .map(|&slot| once.organisms()[slot]
                    .as_ref()
                    .expect("this slot is occupied")
                    .genome()
                    .clone())
                .collect::<Vec<_>>(),
            occupied(&again)
                .iter()
                .map(|&slot| again.organisms()[slot]
                    .as_ref()
                    .expect("this slot is occupied")
                    .genome()
                    .clone())
                .collect::<Vec<_>>(),
            "two runs of seed 42 grew different genomes, so the mutations are being drawn \
             from something that is not the seed"
        );

        // ⭐ Half two: the same founder, with and without neighbours.
        let lineage = |crowded: bool| -> (u64, Genome) {
            let mut world = World::new(&a_bright_world(42, 8));
            let limits = world.config().limits.clone();

            for _ in 0..300 {
                world.tick();
            }

            // The founder is seeded first in both worlds, so it holds serial 0 either way.
            world
                .seed(a_breeder(1, &limits), Vec2::new(120.0, 72.0), 12.0)
                .expect("a lit world holds twelve units under a two-celled body");
            let seeded = if crowded {
                for over in [1.0f32, 2.0, 3.0] {
                    world
                        .seed(
                            a_chain(3, &limits),
                            Vec2::new(120.0 + over, 72.0 - over * 2.0),
                            2.0,
                        )
                        .expect("a lit world holds two units under a three-celled body");
                }
                4
            } else {
                1
            };

            for tick in 1..=2_000u64 {
                world.tick();
                if occupied(&world).len() > seeded {
                    let child = *occupied(&world)
                        .iter()
                        .find(|&&slot| slot >= seeded)
                        .expect("the population grew, so somebody is in a new slot");

                    return (
                        tick,
                        world.organisms()[child]
                            .as_ref()
                            .expect("the newborn is alive")
                            .genome()
                            .clone(),
                    );
                }
            }

            panic!("two thousand ticks and the founder never reproduced");
        };

        let (alone_on, alone) = lineage(false);
        let (crowded_on, crowded) = lineage(true);

        assert_ne!(
            alone_on, crowded_on,
            "the founder had its first child on tick {alone_on} in both worlds, so the three \
             bodies sitting on top of it changed nothing and this test has not established \
             that its numbers are its own"
        );
        assert_eq!(
            alone, crowded,
            "the same founder, breeding on tick {alone_on} alone and tick {crowded_on} under a \
             crowd, produced two different children - so an organism's mutations depend on what \
             else was in the world and in what order it was walked, and SPEC section 2's \
             per-organism streams are not doing the one thing they exist for"
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
        // checked rather than remembered. 36 bytes a cell and 24 a spring: 15,264,000 bytes
        // for the two that hold the world, and the same again less a little for the two the
        // tick packs the living into, plus one index per cell to put them back.
        //
        // ⚠️ **Re-recorded in Phase 7's Group L, and the earlier figure is kept: 28 bytes a
        // cell and 13,216,000 for the pair.** [`Cell::contraction`] is what a myocyte's
        // controller last multiplied its springs by, and it is eight bytes because it is
        // written as an **absence** rather than as a number standing for one - the distinction
        // is what stops a muscle being charged, once, at birth, for a journey it never made.
        // Two megabytes against CLAUDE.md's two-gigabyte resident target.
        let arenas = default_world.cells.len() * size_of::<Cell>()
            + default_world.springs.len() * size_of::<Spring>();
        let mirrors = default_world.crowd.capacity() * size_of::<Cell>()
            + default_world.bonds.capacity() * size_of::<Spring>()
            + default_world.live.capacity() * size_of::<usize>()
            + default_world.owner.capacity() * size_of::<usize>();
        assert!(
            (14_000_000..17_000_000).contains(&arenas),
            "the two arenas of a default world cost {arenas} bytes, against the 15,264,000 \
             recorded here"
        );
        assert!(
            (18_000_000..21_000_000).contains(&mirrors),
            "the dense copies the physics is handed cost {mirrors} bytes, against the \
             19,360,000 recorded here"
        );

        // And the drift, which is Group B's addition: a grain for every cell the world can
        // hold, at sixteen bytes apiece.
        let drift = default_world.drift.capacity() * size_of::<Detritus>();
        assert_eq!(
            default_world.drift.capacity(),
            256_000,
            "the drift is meant to have room for a grain for every cell in the world, so that \
             the whole world dying at once has somewhere to go"
        );
        assert!(
            default_world.drift.is_empty(),
            "a world starts with nothing dead in it"
        );
        assert!(
            (3_500_000..4_500_000).contains(&drift),
            "the drift of a default world costs {drift} bytes, against the 4,096,000 recorded \
             here"
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
        for _ in 0..700 {
            running.tick();
        }
        let purse = a_quarter_of_the_tile(
            &running,
            Vec2::new(
                running.config().world.width * 0.25,
                running.config().world.height * 0.5,
            ),
        );
        place_two_bodies(&mut running, purse);

        let addresses = [
            running.cells.as_ptr().cast::<u8>(),
            running.springs.as_ptr().cast::<u8>(),
            running.crowd.as_ptr().cast::<u8>(),
            running.bonds.as_ptr().cast::<u8>(),
            running.drift.as_ptr().cast::<u8>(),
        ];
        let sizes = [
            running.cells.capacity(),
            running.springs.capacity(),
            running.crowd.capacity(),
            running.bonds.capacity(),
            running.drift.capacity(),
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
                running.drift.as_ptr().cast::<u8>(),
            ],
            "a thousand ticks moved the cells somewhere else in memory, so something in \
             here is allocating as it goes - and the three that are emptied and refilled on \
             every single tick are the ones to suspect: the two mirrors, and the drift"
        );
        assert_eq!(
            sizes,
            [
                running.cells.capacity(),
                running.springs.capacity(),
                running.crowd.capacity(),
                running.bonds.capacity(),
                running.drift.capacity(),
            ],
            "a thousand ticks changed how much room the world takes up"
        );
        assert_eq!(
            running.ticks(),
            1_700,
            "the world does not know how far through its run it is: seven hundred ticks of \
             light to fill the field, and a thousand with two bodies standing in it"
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
    /// The number that comes out is the phase's headline: **a relative error of 1.43e-9
    /// after a hundred thousand ticks**, against a tolerance of 1e-3. That is nearly six
    /// orders of magnitude of headroom, and it is a statement about the whole world rather
    /// than the grid alone.
    ///
    /// It was 1.74e-10 until Group D lowered `light.influx` from 0.012 to 0.001, and the
    /// eightfold difference is a fact about the *filling* rather than about the resting
    /// world. A dimmer world takes twelve thousand ticks to reach its ceilings instead of
    /// seven hundred, and it is while the field is filling that diffusion is moving the most
    /// energy and rounding the most of it - so the worst moment of the run is longer and
    /// worse. Both readings below say so: `worst` and `worst_early` are now the same number,
    /// which means the worst moment of a hundred thousand ticks is inside the first ten
    /// thousand.
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
    /// The default world is offered `0.001 x 36,864`, about 37 units a tick. It absorbs
    /// **748,044 over a hundred thousand ticks**, which is seven and a half a tick - a fifth
    /// of what is on offer. That is not a fault, and it is worth writing down because the
    /// arithmetic looks alarming until it is explained.
    ///
    /// A tile takes light only up to its ceiling, and this world reaches its ceilings after
    /// about twelve thousand ticks and stays there. What keeps a full tile taking anything at
    /// all is diffusion draining it downhill, and at SPEC's defaults that drain is small: the
    /// ceilings of two vertically neighbouring tiles differ by about 0.042, of which
    /// diffusion moves 4% - roughly 0.0017 a tick. So nearly every tile sits pinned at its
    /// ceiling, taking in and shedding the trickle that passes through it.
    ///
    /// ⭐ **That trickle is set by diffusion and not by the light, which is why Group D
    /// lowering `light.influx` twelvefold barely moved this figure at all** - 748,044 against
    /// the 793,408 the old light gave. A resting world is not consuming its income; it is
    /// passing along a gradient it has already filled, and it could do that on a great deal
    /// less light than it is offered.
    ///
    /// The consequence for the ecology is the interesting part: **the standing field is
    /// nearly the whole of the world's energy budget, and the flow through it is not.** A
    /// world holding 183,837 units is turning over seven and a half a tick, so an ecology that
    /// lives off the flow rather than off the standing stock has a great deal less to eat than
    /// the total suggests. Organisms harvesting a tile pull it below its ceiling, at which
    /// point that tile starts taking its full share of light again - so the throughput is not
    /// fixed at seven and a half a tick either. It rises towards the 37 on offer as the
    /// population grows, and at the shipped defaults an equilibrium population of about 2,200
    /// draws roughly 20 a tick out of that 37.
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
             world is 36,864 tiles offered 0.001 each",
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
        //
        // They have been re-measured twice, and each time only because the thing they describe
        // was changed on purpose. This test is deliberately one of the two that read the
        // shipped configuration, so every superseded set is kept here and the change is
        // visible rather than silent.
        //
        //   Phase 7, Group G   `light.patchiness` 0.15 -> 0.5 and `light.patch_drift` 0 -> 0.0006
        //                      superseding 1.433e-9, 183,837.38, 748,044.02 and 564,206.64
        //   Phase 4, Group D   `light.influx` 0.012 -> 0.001
        //                      superseding 1.739e-10, 184,030.35, 793,407.70 and 609,377.35
        //
        // ⭐ **Group G moved three of the four in the same direction and for one reason**, and
        // it is worth reading rather than absorbing. Deeper blotches mean a wider spread of
        // ceilings at every depth: the tiles above the average hold more, and the tiles below
        // it fill sooner and then shed everything that reaches them for the rest of the run.
        // So the world takes in **twice** the light (1,582,148 against 748,044) and sheds two
        // and a half times as much (1,406,403 against 564,207), while the standing field ends
        // up **lower** than before (175,746 against 183,837) rather than higher. That is the
        // biological pump running harder, which is what a blotchier ocean is.
        //
        // The error came *down* by a factor of fifteen, to 9.466e-11, and that is the same
        // fact once more: the drift keeps recomputing the ceilings, and a tile cut back to a
        // freshly written ceiling is a tile whose rounding has just been reset.
        assert!(
            (5.0e-11..2.0e-10).contains(&final_error),
            "the run finished {final_error} out in relative terms, against the 9.466e-11 \
             recorded here"
        );
        assert!(
            (held - 175_745.80).abs() < 0.01,
            "the field came to rest holding {held} rather than the 175,745.80 recorded here"
        );
        assert!(
            (ledger.influx_total() - 1_582_148.47).abs() < 0.01,
            "the world took in {} units of light rather than the 1,582,148.47 recorded here",
            ledger.influx_total()
        );
        assert!(
            (ledger.dissipated() - 1_406_402.67).abs() < 0.01,
            "the world shed {} units through tiles that could not hold them, rather than \
             the 1,406,402.67 recorded here",
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
    /// # ⭐⭐ Group B turned this into a whole life, and the numbers are the phase's headline
    ///
    /// It was written when a body sat in the water and did nothing; Group A gave it income;
    /// Group B gave it costs and an ending. So this is now the only test in the project that
    /// runs the whole of SPEC section 10 from one end to the other - a body is seeded, earns,
    /// pays, ages, dies, falls apart into grains, and those grains sink and rot back into the
    /// water - and asks whether the books survive it.
    ///
    /// Measured 31 July 2026, Windows 11 x86-64, eight four-celled photocyte bodies seeded
    /// holding two units apiece, at the light Group D settled on:
    ///
    /// | | |
    /// | --- | --- |
    /// | All eight died at tick | **1,963** - old age, together, as their bodies are identical |
    /// | Held between them at their richest | **1,112** - and every unit of it became detritus |
    /// | Drift empty at tick | **3,545**, so a corpse takes about 1,580 ticks to rot away |
    /// | Relative error after 120,000 ticks | **7.75e-9**, against a tolerance of 1e-3 |
    ///
    /// # ⭐⭐ The bloom prediction, and what Group D did about it
    ///
    /// Group A measured a photocyte earning about seven times its own upkeep *while shaded*
    /// and predicted that the risk ahead was a world that fills and stagnates rather than one
    /// that starves. Group B re-measured it with the costs switched on and it stood: **6.75
    /// times upkeep** at the light SPEC first shipped.
    ///
    /// It was right, and `docs/PHASE4.md`'s Q15 records what it turned out to mean. At
    /// `light.influx = 0.012` a single seeded body reaches `limits.max_organisms` in under
    /// twenty thousand ticks and sits there, with the field **1.6%** below what the light alone
    /// leaves it holding. The world fills, births start failing for want of a slot, and nothing
    /// is scarce - which is a population under no selection at all.
    ///
    /// **Group D's answer was `light.influx`, and it is now 0.001.** SPEC section 3 carries the
    /// measurement and the sweep; the short version is that carrying capacity is very nearly
    /// proportional to influx, so twelve times less light is a world of about 2,200 bodies
    /// instead of one pressed against an arena built for 4,000. `upkeep_scale` was tried first,
    /// because a static energy-budget calculation asks for it, and it is the wrong lever: it is
    /// also the lifespan slider, so raising it shortens a life while lengthening the time
    /// needed to earn a child, and at 3 it kills every world it is applied to before a single
    /// birth.
    ///
    /// The margin below is the same measurement at the new light: **0.0224 a tick earned
    /// against 0.00408 to keep, a margin of 5.49**. It is lower than the 6.75 because the tiles
    /// these eight bodies are standing on refill twelve times more slowly, and it is still far
    /// from the break-even a photosynthetic body would need to be in trouble. **A body is not
    /// short of light in this world. The world is short of bodies' worth of light**, which is a
    /// different claim and the one carrying capacity is about.
    ///
    /// # The drift settles, and what settles is not what a reader would expect
    ///
    /// The same argument as Phase 2's: an error that is inside the tolerance at ten thousand
    /// ticks and *growing* is a run that stops in the small hours with no bug to find. So the
    /// worst discrepancy over the whole run is compared against the worst over its first
    /// tenth, and an error accumulating in one direction would be ten times larger by the end.
    /// **Measured: 7.85e-9 over the whole run against 3.14e-9 over its first tenth** - so the
    /// error roughly doubles over the remaining nine tenths and then stops, which is a wobble
    /// settling rather than a leak accumulating. Run against ten times the ticks it would be
    /// the same number.
    ///
    /// That claim is about the *relative* error, and the distinction is worth spelling out
    /// because the absolute figure behaves differently and would alarm anybody who looked at
    /// it. The books run **short**, by a few billionths of a unit per tick, and that shortfall
    /// does **not** level off - it grows in a straight line for as long as a run goes on. It is
    /// a real slow loss in the field's arithmetic rather than energy in transit; see this
    /// phase's Q12, which is where it is written up, because it belongs to `grid.rs` rather
    /// than to anything Groups B, C or D added.
    ///
    /// The reason it is nevertheless harmless is that the other side of the invariant grows in
    /// a straight line too. **The ratio therefore converges**, and it converges far below the
    /// tolerance. SPEC section 5 states the invariant as a *relative* one, and this is the case
    /// that makes that wording load-bearing rather than cosmetic - an overnight run of tens of
    /// millions of ticks sits at a few parts in a billion, a hundred thousand times inside the
    /// 1e-3 allowed.
    ///
    /// Recorded 31 July 2026, Windows 11 x86-64: relative error **7.75e-9** after 120,000
    /// ticks, against SPEC section 5's tolerance of 1e-3. (At the light SPEC first shipped this
    /// run finished at 5.77e-9. The difference is where it always is: a dimmer world spends
    /// twelve times longer filling, and a field that is filling is a field diffusion is moving
    /// - and rounding - the most energy in.)
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
        //
        // Twelve thousand ticks rather than the thousand this took before Group D, and the
        // reason is the whole of the tuning: `light.cap / light.influx` is how long a field
        // takes to fill, and Group D lowered `light.influx` twelvefold. This test reads the
        // shipped configuration on purpose, so it waits the shipped dawn.
        for _ in 0..12_000 {
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
        let mut richest = 0.0f64;
        let mut most_dead = 0.0f64;
        let mut first_death = 0u64;
        let mut last_death = 0u64;
        let mut drift_emptied = 0u64;
        let mut moved = 0usize;
        let mut at_a_thousand = 0.0f64;

        for tick in 1..=120_000u64 {
            world.tick();

            let living = world.organisms().iter().flatten().count();
            richest = richest.max(world.ledger().biomass());
            most_dead = most_dead.max(world.ledger().detritus());

            if tick == 1_000 {
                at_a_thousand = world.ledger().biomass();
                // While everything is still alive, and before anything has moved far, count
                // how many bodies the physics has actually pushed about.
                moved = (0..population)
                    .filter(|&slot| {
                        world
                            .organisms()
                            .get(slot)
                            .and_then(Option::as_ref)
                            .is_some()
                            && (world.cells_of(slot)[0].pos - seeded_at[slot]).length() > 1.0
                    })
                    .count();
            }
            if first_death == 0 && living < population {
                first_death = tick;
            }
            if last_death == 0 && living == 0 {
                last_death = tick;
            }
            if drift_emptied == 0 && last_death > 0 && world.drift.is_empty() {
                drift_emptied = tick;
            }

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
            "with eight bodies living and dying in it, the two sides of the invariant \
             finished {final_error} apart in relative terms, and SPEC section 5's tolerance \
             of 1e-3 is meant to be covering the rounding in `f32` diffusion rather than an \
             actual leak"
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
            (alive - 16.0).abs() < 1e-6,
            "eight organisms seeded with two units each left {alive} in the biomass account"
        );
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
            moved >= 6,
            "only {moved} of the {population} bodies had moved a whole world unit from where \
             they were seeded after a thousand ticks, and they were deliberately seeded on \
             top of one another - so either the physics is not being handed the organisms or \
             it is not being run at all"
        );

        // ⭐ The whole life cycle happened, in order, and the world came back to where it
        // started. Everything the eight bodies ever earned is now either in the water or gone
        // as heat, and there is not a fraction of it unaccounted for anywhere.
        assert!(
            (1_900..2_100).contains(&first_death),
            "the first body died at tick {first_death}, against the 1,963 ticks a four-celled \
             photocyte body with a three-gene genome is allowed - so either the lifespan has \
             moved or something starved"
        );
        assert!(
            last_death > 0 && last_death < 3_000,
            "the last of the eight was still alive at tick {last_death}, and none of them can \
             outlive its allowance by much"
        );
        assert!(
            drift_emptied > last_death && drift_emptied < 20_000,
            "the last body died at tick {last_death} and the drift was still holding grains \
             at {drift_emptied}"
        );
        assert!(
            world.drift.is_empty() && ledger.detritus() < 1e-9,
            "{} grains are still in the water holding {} between them",
            world.drift.len(),
            ledger.detritus()
        );
        assert!(
            ledger.biomass().abs() < 1e-9,
            "nothing is alive in this world and the books say {} is",
            ledger.biomass()
        );
        assert!(
            most_dead > 100.0,
            "the eight corpses never held more than {most_dead} between them, so either they \
             starved rather than growing old or a corpse is not carrying what its body held"
        );

        // ⭐⭐ **The bloom question, re-measured with upkeep switched on.** Group A predicted
        // the world would fill and stagnate on the strength of a photocyte earning about seven
        // times its own upkeep. This is the same measurement with the cost side of the ledger
        // actually running, and the answer is in the doc comment above.
        //
        // Net accumulation over a thousand ticks, per photocyte, plus what that photocyte paid
        // over the same thousand ticks, is what it earned.
        let cells = f64::from(population_cells());
        let net = (at_a_thousand - alive) / (1_000.0 * cells);
        let upkeep = f64::from(0.004f32) + 3.0 * f64::from(0.0001f32) / 4.0;
        let margin = (net + upkeep) / upkeep;

        assert!(
            (4.0..8.0).contains(&margin),
            "a photocyte in a shaded body earned {} a tick against the {upkeep} it costs to \
             keep - a margin of {margin} times upkeep, against the 5.49 recorded here",
            net + upkeep
        );
        assert!(
            richest > 1_000.0,
            "the eight bodies never held more than {richest} between them, and eight bodies \
             earning five times their upkeep for two thousand ticks should reach well past a \
             thousand"
        );
    }

    /// How many photocytes the eight bodies of
    /// [`energy_is_still_conserved_with_organisms_present`] have between them.
    ///
    /// Written as a function rather than a literal so the sum it feeds is visibly eight bodies
    /// of four cells rather than a number somebody wrote down.
    fn population_cells() -> u32 {
        8 * 4
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
    /// place. **The seed reaches the organisms from Group C**, where reproduction mutates a
    /// genome out of an organism's own stream - and it is
    /// `a_lineage_is_still_deterministic_across_generations` that covers it, rather than this
    /// test, because the interesting half of that claim is not about a seed at all. It is that
    /// a lineage draws its numbers independently of what else is alive in the world, which is
    /// what SPEC section 2 wants per-organism streams for and what no amount of running the
    /// same seed twice can establish. The two bodies here have no gonocyte between them, so
    /// nothing in this test is ever born.
    ///
    /// What the seed reaches today is the blotchiness of the light - the ceilings the tiles
    /// fill to - and it reaches all of it: measured, **every one of the 1,536 tiles differs**
    /// between seed 42 and seed 43. Since Group A it reaches a little further than that, by a
    /// route worth noticing: the two bodies are standing on tiles whose ceilings differ, so
    /// they harvest different amounts and the `energy_flow` written on their cells differs
    /// too. Their positions still do not, because nothing in the physics or in development
    /// reads a random number.
    #[test]
    fn a_run_is_still_reproducible() {
        let run = |seed: u64| -> World {
            let mut world = World::new(&a_lit_world(|raw| {
                raw.world.seed = seed;
                raw.world.grid_cols = 48;
                raw.world.grid_rows = 32;
                raw.limits.max_organisms = 4;
            }));

            // The light falls, then two bodies are put in it holding enough to outlast the
            // run. A body seeded holding nothing dies on its first tick from Group B, and a
            // determinism test over an empty world is a determinism test about the water.
            for _ in 0..700 {
                world.tick();
            }
            let purse = a_quarter_of_the_tile(
                &world,
                Vec2::new(
                    world.config().world.width * 0.25,
                    world.config().world.height * 0.5,
                ),
            );
            place_two_bodies(&mut world, purse);

            for _ in 0..1_500 {
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

    /// ⭐ **Phase 6, B1.** The live settings really do change what a running world does.
    ///
    /// SPEC section 3: *"`[world]`, `[limits]` and `seed` lock at run start; the rest can be
    /// changed live, **which is how environmental events work**."* That last clause is the whole
    /// of why [`World::retune`] exists. Raising `metabolism.upkeep_scale` mid-run is not a
    /// settings change - it is the weather turning, and SPEC section 3's own measurements say
    /// what it does: *"`3` and `4` both go extinct with the founder's death, before a single
    /// birth"*, because upkeep shortens a life in proportion while lengthening the time it takes
    /// to earn the reproduction threshold.
    ///
    /// So the claim is stated the way somebody watching would state it: **the same world, from
    /// the same tick, spends more when the temperature goes up.** Two copies of one world are
    /// ticked side by side and only one of them is retuned, so the difference is the setting and
    /// nothing else.
    ///
    /// ⚠️ The second half is the one that would go wrong quietly. A retune that only replaced
    /// `World::config` - the field the panel reads back - would make the *panel* say the
    /// temperature had risen while `metabolism.rs`, which holds its own copy of the number, went
    /// on charging the old one. That is a world that reports weather it is not having, and it is
    /// exactly the failure `census.rs`'s opening paragraph is about: two copies of one quantity,
    /// out of step.
    #[test]
    fn the_live_settings_can_be_changed_while_a_run_is_going() {
        let settled = a_lit_world(|_| {});
        let mut unchanged = World::new(&settled);
        for _ in 0..300 {
            unchanged.tick();
        }

        // The same world, to the last bit: same seed, same configuration, same number of ticks.
        let mut warmed = World::new(&settled);
        for _ in 0..300 {
            warmed.tick();
        }
        // Bit for bit, and deliberately: SPEC section 2's determinism is what makes the
        // comparison below a measurement of the *setting* rather than of two similar worlds.
        #[expect(
            clippy::float_cmp,
            reason = "two worlds built from one seed and one configuration and ticked the same \
                      number of times are the same world to the last bit - SPEC section 2 - and \
                      an approximate match here would let a difference that is not the retune \
                      through into the comparison this test is actually about"
        )]
        {
            assert_eq!(
                warmed.ledger().dissipated(),
                unchanged.ledger().dissipated(),
                "the two worlds were not identical before one of them was retuned"
            );
        }

        // Something alive in both, or "the warmer world spends more" is a claim about nothing.
        let limits = settled.limits.clone();
        let mut seeded = None;
        for world in [&mut unchanged, &mut warmed] {
            let middle = Vec2::new(
                world.config().world.width * 0.5,
                world.config().world.height * 0.5,
            );
            let purse = a_quarter_of_the_tile(world, middle);
            seeded = Some(
                world
                    .seed(a_chain(3, &limits), middle, purse)
                    .expect("a lit world can afford one body"),
            );
        }
        let slot = seeded.expect("both worlds were seeded");

        // The weather turns.
        let warmer = config(|raw| {
            raw.light.influx = 0.012;
            raw.metabolism.upkeep_scale = 4.0;
        });
        warmed.retune(&warmer);

        let spent_before = (
            unchanged.ledger().dissipated(),
            warmed.ledger().dissipated(),
        );
        for _ in 0..200 {
            unchanged.tick();
            warmed.tick();
        }
        let cold = unchanged.ledger().dissipated() - spent_before.0;
        let hot = warmed.ledger().dissipated() - spent_before.1;

        // ⭐ **The difference between the two, against what the retune bought, rather than the
        // ratio of their totals.** This used to read `hot > cold * 2`, and Phase 7's Group G
        // is what showed that to be the wrong instrument rather than merely a loose one.
        //
        // `dissipated` is everything the world spends, and the largest part of it here is not
        // metabolism at all: it is SPEC section 4's ceiling, shedding what will not fit out of
        // tiles near the floor. When `light.patchiness` went from 0.15 to 0.5 the deepest
        // ceilings in this world dropped from about 1.7 to about 1.0, so tiles that used to
        // reach them after the measurement window began reaching them inside it - and three
        // hundred units of spill arrived on both sides of a comparison about two units of
        // upkeep. The old assertion had been passing on a world where no tile had yet filled.
        //
        // Everything except the cost of living cancels in the *difference*, and cancels
        // exactly, because SPEC section 2's determinism makes these two the same world: the
        // same light falls on the same tiles, the same tiles spill, and the same body earns
        // the same income, since what an organism harvests depends on the tile and the body
        // and not on what it happens to be holding. So `hot - cold` is precisely the three
        // extra multiples of upkeep the retune bought, and that is a figure this test can work
        // out for itself from SPEC section 6's table rather than compare against a threshold.
        let body: f64 = warmed
            .cells_of(slot)
            .iter()
            .map(|cell| f64::from(cell.kind.upkeep()))
            .sum();
        let genes = f64::from(
            u32::try_from(
                warmed.organisms()[slot]
                    .as_ref()
                    .expect("the body seeded above is still alive")
                    .genome()
                    .genes()
                    .len(),
            )
            .expect("a genome cap fits in a word"),
        );
        let bought = 3.0 * 200.0 * (body + genes * f64::from(warmer.metabolism.gene_cost));

        assert!(
            (hot - cold - bought).abs() < bought * 1e-3,
            "two hundred ticks cost the world at upkeep_scale 1.0 {cold} and the same world at \
             4.0 {hot}, a difference of {}. Going from 1.0 to 4.0 on a body of {body} a tick \
             plus {genes} genes should have cost exactly {bought} more, so `metabolism.rs` is \
             not charging the number the panel is showing",
            hot - cold
        );

        // And the panel's copy agrees with the one that is being charged.
        assert!(
            (warmed.config().metabolism.upkeep_scale - 4.0).abs() < f32::EPSILON,
            "the world reports an upkeep_scale of {} after being retuned to 4.0",
            warmed.config().metabolism.upkeep_scale
        );

        // ⭐ `[light]` as well, and this one has to reach `grid.rs`'s precomputed tables rather
        // than a single field. A tile's ceiling and the light offered to its row are both worked
        // out once, at construction, from `light.cap` and `light.gradient` - so a retune that
        // forgot them would leave the water filling to the old ceiling for ever.
        let mut dimmed = World::new(&settled);
        for _ in 0..800 {
            dimmed.tick();
        }
        let full = dimmed.grid().total_energy();
        dimmed.retune(&config(|raw| {
            raw.light.influx = 0.012;
            raw.light.cap = 1.0;
        }));
        for _ in 0..50 {
            dimmed.tick();
        }
        assert!(
            dimmed.grid().total_energy() < full * 0.5,
            "the ceiling was lowered from 8.0 to 1.0 and the field went from {full} to {}, so \
             `light.cap` did not reach the tile targets it decides",
            dimmed.grid().total_energy()
        );
    }

    /// ⭐ **Phase 6, B2, at the near end.** The locked settings cannot be changed, and saying so
    /// is a panic rather than a shrug.
    ///
    /// CLAUDE.md's memory guarantee is *"every arena is allocated at startup at fixed capacity
    /// derived from the config, and never resized"*, and SPEC section 3 locks `[world]`,
    /// `[limits]` and `seed` for exactly that reason. `panel.rs` never offers them, which is B2 -
    /// but *"the interface does not offer it"* is a promise about a screen, and the arenas are a
    /// promise about memory. This is the second one, asserted where the change would arrive:
    /// CLAUDE.md's *"invariants are asserted at runtime, not just in tests"*.
    #[test]
    #[should_panic(expected = "limits.max_organisms")]
    fn the_locked_settings_cannot_be_changed_while_a_run_is_going() {
        let mut world = World::new(&a_lit_world(|_| {}));

        world.retune(&a_lit_world(|raw| raw.limits.max_organisms = 8));
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
        ///
        /// # Group B widened what this is asking
        ///
        /// The two settings that decide what living *costs* are generated now as well, and
        /// the two bodies are given a purse out of the water before the run rather than being
        /// seeded holding nothing. Between them that means each case lands somewhere different
        /// on the one question Group B introduced: in a bright world the two bodies live out
        /// the whole five hundred ticks, and in a dim or expensive one they starve within a
        /// few, die, leave grains, and those grains sink and rot back into the field. **Both
        /// halves have to conserve energy**, and the second half is four movements the ledger
        /// had never been asked about across sixty-four differently-shaped worlds.
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
            upkeep_scale in 0.05f64..4.0,
            gene_cost in 0.0f64..0.01,
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
                raw.metabolism.upkeep_scale = upkeep_scale;
                raw.metabolism.gene_cost = gene_cost;
                raw.limits.max_organisms = 2;
                raw.limits.max_cells_per_organism = 4;
            }));

            // The light falls first, so that there is something in the water for the two
            // bodies to be seeded out of. A quarter of one tile apiece: affordable in any
            // world this generates, and enough to keep them alive in a bright one.
            for _ in 0..200 {
                world.tick();
            }
            let purse = a_quarter_of_the_tile(
                &world,
                Vec2::new(world.config().world.width * 0.25, world.config().world.height * 0.5),
            );
            place_two_bodies(&mut world, purse);

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
