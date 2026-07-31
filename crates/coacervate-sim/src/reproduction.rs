//! Having offspring, which is the only way anything new ever enters the world.
//!
//! See `SPEC.md` section 10 for the paragraph this implements, and section 7 for the six
//! operators that make a copy imperfect.
//!
//! `behaviour.rs` is what a body earns and `metabolism.rs` is what it costs and how it ends.
//! This is the third side, and it is the one that closes the circle: until it existed a
//! population could only ever fall, so nothing that a mutation happened to make better could
//! ever become common. **Selection is not a thing this file does. It is what happens once this
//! file exists**, because a lineage that pays for itself leaves more copies of itself behind
//! than one that does not, and there is now a way to leave one.
//!
//! # What a birth actually is
//!
//! SPEC section 10 gives it in one sentence: *"When an organism's stored energy exceeds
//! `reproduction_threshold × body_construction_cost` and it has at least one gonocyte, it
//! reproduces: copy the genome, mutate, develop the new body, place the seed cell adjacent to a
//! gonocyte with a small random offset, transfer `offspring_share` of parent energy. Asexual
//! only."* Every clause of that is a line below, in that order.
//!
//! The two conditions are doing different jobs and it is worth saying which. The **threshold**
//! is what makes reproduction cost something: a body has to have earned twice over what it
//! would take to build another one before it may. The **gonocyte** is what makes it cost
//! something *structurally* - SPEC section 6 says in as many words that "an organism with no
//! gonocyte cannot reproduce", so a lineage has to spend part of its body, and part of its
//! upkeep every tick of its life, on tissue that earns nothing and does nothing but let it
//! breed. Without that, every body in the world would be a pure feeding machine that also
//! happened to reproduce, and SPEC section 6's whole table of trade-offs would have one fewer
//! row in it.
//!
//! # The offspring's energy comes out of the parent, and nowhere else
//!
//! ⚠️ **This is the exact failure SPEC section 5 says the ledger cannot see**, and it is worth
//! being blunt about because it is the most dangerous line in the file. An organism's energy
//! and the ledger's `biomass` account are two records of one quantity. Handing a newborn a
//! share of energy that its parent never gave up leaves all five accounts exactly as they were,
//! so the books balance perfectly, the invariant is silent, and a body stands in the world
//! holding energy nobody counted. It stays silent until that body dies and its energy is moved
//! out of an account that never received it, which is hours into a run with no cause to find.
//!
//! What guards it is not the invariant. It is
//! `a_birth_transfers_offspring_share_of_the_parents_energy`, which asserts that the parent
//! went **down** by exactly what the child went **up** by. The general rule, which this project
//! has now learned in three separate phases: a conservation check cannot see energy that was
//! never declared, only energy that was declared wrongly.
//!
//! The movement itself is [`Ledger::inherit`] - `biomass → biomass`, so no total in the world
//! changes. It is a named operation anyway, for the reason `Ledger::predate` gives about the
//! one other movement of that shape: the alternative to naming it is somebody adding to one
//! organism and subtracting from another by hand, which is the same arithmetic with nothing
//! watching it.
//!
//! # Every random number a birth needs comes out of the parent's own stream
//!
//! `rng.rs` gives each organism a private sequence fixed by its serial number, and this is the
//! first thing in the project to draw from one. Both draws a birth makes - the mutations, and
//! which way the newborn is put down - come from the **parent's** stream, and the consequence
//! is the one SPEC section 2 asks for: a lineage mutates the same way whatever else is alive in
//! the world, whatever order the population happened to be walked in, and however many threads
//! a later phase spreads it across. `a_lineage_is_still_deterministic_across_generations` is
//! that claim, and it makes it by breeding the same founder in two worlds with different
//! neighbours in them.
//!
//! # Births happen in slot order, for the same reason deaths do
//!
//! `metabolism.rs` explains it at length and it applies here unchanged, one arena along. The
//! order births happen in decides which slot each newborn takes; a slot decides where in the
//! cell arena a body sits; that decides the order `World::gather` packs the crowd; and that
//! decides the order `physics.rs` sums the forces on each cell. Floating-point addition is not
//! associative, so it decides the last bit of every position in the world. Nothing announces a
//! change of that order - the run simply stops matching its own recording.
//!
//! # A body does not breed on the tick it was born
//!
//! The pass walks the slots from the front, and a newborn can land in a slot the walk has not
//! reached yet. Without something to stop it, a rich body would give birth to a child holding
//! enough to give birth to a child, and the whole population would appear inside a single tick
//! - a world that goes from one organism to its cap between two frames.
//!
//! What stops it is that an organism has to have been alive for a tick. `world.rs` ages
//! everything before this pass runs, so anything that was here at the start of the tick is at
//! least one tick old and anything born during it is nought. It reads as a rule about
//! organisms rather than as a guard against a loop, which is what it is: a body that has not
//! lived a tick has not had a tick in which to accumulate anything.
//!
//! # Where the tests are
//!
//! In `world.rs`, and deliberately. Every claim here is about a whole world - that a slot was
//! taken from the free list, that a body was grown from a mutated genome and laid down in the
//! arena, that no arena grew when there was nowhere to put one - and those are things only a
//! `World` has. `metabolism.rs` builds its scenes by hand because its claims are about
//! *amounts*, and an amount can be measured without a world around it.

use crate::cell::{Cell, CellKind, Vec2};
use crate::config::{Config, LimitsConfig, MutationConfig, WorldConfig};
use crate::development::develop;
use crate::ledger::Ledger;
use crate::metabolism::construction_energy;
use crate::mutation::mutate;
use crate::organism::{Organism, cell_slot, lay_out};
use crate::physics::Spring;
use crate::rng::WorldRng;
use rand::RngExt;
use rand::rngs::ChaCha8Rng;
use std::f32::consts::PI;

/// Everything a birth is allowed to touch.
///
/// Grouped into one argument rather than passed as five, exactly as `behaviour.rs` and
/// `metabolism.rs` group their own and for the same reason: references threaded through a call
/// one at a time are a place to get two of them the wrong way round.
pub(crate) struct Fertile<'a> {
    /// The whole cell arena. Read by slot for a parent's body, written by slot for a newborn's,
    /// and never walked end to end - the slots nobody lives in still hold cells.
    pub cells: &'a mut [Cell],

    /// The whole spring arena, written the same way. A newborn's adhesions count from its own
    /// first cell; see `organism.rs`.
    pub springs: &'a mut [Spring],

    /// Who is in each slot, and nothing at all in the slots that are free.
    pub organisms: &'a mut [Option<Organism>],

    /// The slots nobody is in. A birth pops from this; `metabolism.rs` pushes onto it at a
    /// death. **An empty one is what makes a birth fail** - see [`Reproduction::run`].
    pub free: &'a mut Vec<usize>,

    /// The next serial number to hand out, which is never handed out twice.
    pub next_serial: &'a mut u64,
}

/// The numbers a configuration decides about breeding, and nothing else.
///
/// No working room and no arena, for the same reason `metabolism.rs` has none: everything a
/// birth writes into belongs to `world.rs`. What this type is is a handful of settings copied
/// out of the configuration once, so that the tick does not reach into a `Config` while it is
/// walking the population.
pub struct Reproduction {
    /// SPEC section 3's `metabolism.reproduction_threshold`: how many times over a body must be
    /// holding what it would cost to build before it may build another.
    threshold: f64,

    /// SPEC section 3's `metabolism.offspring_share`: what fraction of the parent goes with the
    /// child.
    share: f64,

    /// SPEC section 3's `[mutation]` table, which [`mutate`] reads.
    mutation: MutationConfig,

    /// The arena sizes, which is how a slot number becomes a stretch of the cell array, and
    /// what `development.rs` and `mutation.rs` are held inside.
    limits: LimitsConfig,

    /// How big the world is, because a newborn is put down inside it. See `organism.rs`.
    world: WorldConfig,
}

impl Reproduction {
    /// Build the renewal side of a configuration.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            threshold: f64::from(config.metabolism.reproduction_threshold),
            share: f64::from(config.metabolism.offspring_share),
            mutation: config.mutation.clone(),
            limits: config.limits.clone(),
            world: config.world.clone(),
        }
    }

    /// Give every organism that can afford one, and has somewhere to put one, a child.
    ///
    /// SPEC section 10's sentence, clause by clause, and nothing else: there is no courtship,
    /// no mate, no cost beyond the share that goes with the child, and no second chance in a
    /// tick. CLAUDE.md's decision log keeps sexual reproduction out of scope until the asexual
    /// case is stable, so a birth here is one parent and one copy.
    ///
    /// **The slots are walked in index order and that is not a detail.** See this module's
    /// documentation.
    ///
    /// # A birth that has nowhere to go simply does not happen
    ///
    /// CLAUDE.md: *"When the population cap is reached, births fail rather than allocating.
    /// (This is also biologically reasonable: a full world should mean nowhere to reproduce
    /// into.) A simulation that cannot allocate cannot leak."* SPEC section 10 says the same
    /// from the other end and adds the word that matters - **silently**. There is no error to
    /// return and nothing is logged, because a full world is not a fault; it is the pressure the
    /// whole ecology is supposed to grow out of.
    ///
    /// The failure is checked *before* anything is spent, which is the half that would be easy
    /// to get wrong. A parent that had already handed over its share and then found there was
    /// nowhere to put the child would be paying for children that do not exist - and paying in
    /// energy that has left the `biomass` account for nothing, on every tick of a world that
    /// spends most of a long run at its cap.
    ///
    /// # Panics
    ///
    /// If the run has minted every serial number there is, which would take eighteen
    /// quintillion births. `rng.rs` keeps the last one for the world's own randomness, and an
    /// organism holding it would be drawing the world's numbers.
    pub(crate) fn run(&self, fertile: Fertile<'_>, rng: &WorldRng, ledger: &mut Ledger) {
        let Fertile {
            cells,
            springs,
            organisms,
            free,
            next_serial,
        } = fertile;

        // `0..len` rather than an iterator over the slots, because the body of the loop writes
        // into a *different* slot of the same array. The order is the same either way and it is
        // the order the whole of this module's determinism argument is about.
        for slot in 0..organisms.len() {
            let Some(parent) = organisms[slot].as_ref() else {
                continue;
            };

            // A body that has not lived a tick has not had a tick in which to accumulate
            // anything. See this module's documentation: it is also what stops a single rich
            // organism filling the world between one tick and the next.
            if parent.age() == 0 {
                continue;
            }

            let first = cell_slot(slot, &self.limits).start;
            let body = &cells[first..first + parent.cells()];

            // SPEC section 10's two conditions. `construction_energy` is `metabolism.rs`'s, and
            // is the same sum a corpse is shared out in proportion to - SPEC uses one phrase for
            // both and they must not become two different numbers.
            if parent.energy() <= self.threshold * construction_energy(body) {
                continue;
            }
            let Some(gonocyte) = nursery(body) else {
                continue;
            };

            // Nowhere to put it. Checked before a single unit has moved; see above.
            let Some(&nest) = free.last() else {
                continue;
            };

            // Both draws come out of the parent's own sequence, which is what makes a lineage
            // replay identically whatever else is happening in the world.
            let mut stream = parent.stream(rng);
            let genome = mutate(parent.genome(), &self.mutation, &self.limits, &mut stream);
            let grown = develop(&genome, &self.limits);
            let at = beside(gonocyte, grown.cells[0].kind, &mut stream);
            let inheritance = self.share * parent.energy();

            lay_out(nest, &grown, at, &self.world, &self.limits, cells, springs);

            free.pop();
            organisms[nest] = Some(Organism::new(
                genome,
                inheritance,
                *next_serial,
                grown.cells.len(),
                grown.springs.len(),
            ));
            *next_serial = next_serial
                .checked_add(1)
                .expect("a run has minted every serial number there is");

            // ⚠️ The parent pays, and the books are told. Neither line may be dropped without
            // the other; see this module's documentation for what a birth from nowhere looks
            // like, which is: nothing at all, for several hours.
            let parent = organisms[slot]
                .as_mut()
                .expect("the slot being read from a moment ago still holds its organism");
            parent.remember(&stream);
            parent.lose(inheritance);
            ledger.inherit(inheritance);
        }
    }
}

/// Where in this body the next generation is put together, if anywhere.
///
/// SPEC section 6 gives the gonocyte one function - *"accumulates energy toward reproduction.
/// An organism with no gonocyte cannot reproduce"* - and SPEC section 10 says a newborn is
/// placed next to one. This answers both questions at once: nothing means no birth, and a
/// position means put the child there.
///
/// **The first one in the body**, when there are several, which is the order development made
/// them in and therefore an order the genome decides rather than an accident of the arena. The
/// alternatives were a random one, which spends a draw from the parent's stream to no visible
/// end, and the richest or the nearest, neither of which means anything: a gonocyte has no
/// energy of its own, and there is nothing for a nursery to be near.
///
/// A lineage that wants its offspring budded from somewhere else has a way to arrange it, and
/// it is the way everything else in this simulation is arranged - grow the gonocyte it wants
/// used first. Development's order is the genome's order.
fn nursery(body: &[Cell]) -> Option<Vec2> {
    body.iter()
        .find(|cell| cell.kind == CellKind::Gonocyte)
        .map(|cell| cell.pos)
}

/// Where a newborn's seed cell goes: touching the parent's gonocyte, on a side drawn at random.
///
/// SPEC section 10 asks for the seed cell to be placed *"adjacent to a gonocyte with a small
/// random offset"* and leaves both halves of that open.
///
/// **Adjacent** is read as exactly touching: the two radii added together, so the newborn's
/// first cell is resting against the gonocyte that made it rather than overlapping it. That is
/// the one distance SPEC section 8's collision does not immediately do something about, and it
/// is the same reading `development.rs` already takes when it buds a daughter that does not
/// adhere. Placing a newborn *on top of* its parent would work - the physics would shove them
/// apart over the following ticks - but it would mean every birth in the world started with a
/// spike of collision force, and a spike is the one thing an explicit integrator does not want.
///
/// **The small random offset** is which way. A fixed direction would put every child of every
/// body in the world on the same side of its parent, and - much worse - would stack a body's
/// successive children in exactly the same spot, since a gonocyte does not move between one
/// birth and the next. Cells at exactly the same point are the case `physics.rs` has to break a
/// tie for, so a whole brood would be pushed apart along one line. A direction drawn from the
/// parent's own stream costs one number and scatters them.
///
/// The rest of the body follows from the seed cell, as it does for a seeding: `organism.rs`
/// lays the shape down around wherever the seed went, and brings the whole of it inside the
/// world.
fn beside(gonocyte: Vec2, newborn: CellKind, stream: &mut ChaCha8Rng) -> Vec2 {
    let angle: f32 = stream.random_range(-PI..PI);
    let reach = CellKind::Gonocyte.radius() + newborn.radius();

    gonocyte + Vec2::new(angle.cos() * reach, angle.sin() * reach)
}
