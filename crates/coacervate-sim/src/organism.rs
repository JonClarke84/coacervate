//! An organism, and the stretch of the world's arenas it lives in.
//!
//! See `SPEC.md` section 10 for the life cycle this is the beginning of.
//!
//! Everything before this file describes a *part* of a living thing: `genome.rs` holds the
//! growth program, `development.rs` runs it into a shape, `cell.rs` is what that shape is
//! made of, `mutation.rs` copies the program imperfectly. This is the thing that has all of
//! them at once and is therefore the first object in the project that is alive: a genome, the
//! energy it is running on, how long it has been running, and a name that is its own for the
//! length of the run.
//!
//! # Where a body lives, and why the answer is a fixed slot
//!
//! A body is a *range* of the world's flat cell array. That is the shape a graphics card
//! wants to read and the shape the neighbour search in `physics.rs` is built around, and it
//! has one awkward consequence: if bodies are packed in end to end, removing a dead one from
//! the middle shifts every body above it, and every spring index pointing into them is
//! suddenly pointing at the wrong cell. The tempting answers - swap the last body into the
//! hole and fix up the indices, hand out generational handles, keep a free list over ranges
//! of different sizes - are all fragmentation problems wearing different hats, and every one
//! of them is a bug waiting to be written.
//!
//! So the arrangement is decided in advance and never changes: **organism `n` owns cells
//! `[n × max_cells_per_organism, …)` and nothing else ever does.** A slot is either occupied
//! or free. Death marks a slot free, birth takes a free one, and no range ever moves - so
//! there is no index to fix, and the awkward problem simply does not exist.
//!
//! It costs nothing that was not already being paid. CLAUDE.md requires every arena to be
//! allocated at startup at `max_organisms × max_cells_per_organism`, which is exactly the
//! room the slots need. A body of one cell in a slot of sixty-four leaves sixty-three cells
//! standing empty, and those cells were allocated either way.
//!
//! Springs are slotted the same way, at one fewer per organism than the cells. Development
//! makes a spring only when a gene divides a cell into a daughter that *adheres*, and a
//! daughter is made once, so a body of `n` cells can carry at most `n - 1` springs.
//!
//! # A spring's ends are numbered from its own body
//!
//! [`Organism`]'s springs live in the world's spring arena but their two endpoints count from
//! the organism's own first cell rather than from the arena's. That is a real guarantee
//! rather than a convention: SPEC section 8 warns that springs are not found by the
//! neighbour search and have no length limit, so a spring joining two cells on opposite sides
//! of the world will quietly haul them together through the seam. An endpoint that cannot
//! *name* a cell outside its own body cannot express that spring at all.
//!
//! # The generator is rebuilt from two numbers, not stored
//!
//! `rng.rs` gives every organism its own private sequence of random numbers, fixed at birth
//! and unaffected by what any other organism does or when - which is what lets later phases
//! run the population across every core of the machine without changing the result. The
//! obvious way to hold on to that is to keep the generator itself on the organism.
//!
//! It is kept as **`(serial, word_position)`** instead, and the generator is rebuilt from the
//! world's seed whenever it is wanted. That is two numbers rather than a cipher's 136 bytes of
//! internal state, and nothing that can drift out of step with the seed it was derived from.
//! The reason it was decided this way is narrower than either: it keeps `coacervate-sim`'s
//! direct dependencies at exactly `rand` and `serde`, which CLAUDE.md requires and which is
//! also what makes `thread_rng` a compile error rather than a rule somebody has to remember.
//!
//! The contract is in [`Organism::stream`] and [`Organism::remember`], and it is the same
//! contract `rng.rs` describes: draw from the stream, then put back where you got to. An
//! organism that forgets to put back replays the same numbers for the rest of its life,
//! which is a lineage that mutates the same way every generation.

use crate::config::LimitsConfig;
use crate::genome::Genome;
use crate::rng::WorldRng;
use rand::rngs::ChaCha8Rng;
use std::ops::Range;

/// One living thing: SPEC section 10's organism.
///
/// The fields are private, and the reason is `energy`. Every other number here can be read
/// and written by whoever owns the organism without anything going wrong, but energy is
/// counted twice - once here and once in the ledger's `biomass` account - and the two are
/// only allowed to move together. Keeping them in step is the whole of `ledger.rs`'s job, and
/// a public field would be a way round it.
#[derive(Debug, Clone, PartialEq)]
pub struct Organism {
    /// The growth program this body was grown from, and the thing its offspring will inherit
    /// an imperfect copy of.
    ///
    /// Kept rather than thrown away once the body exists, because reproduction copies the
    /// *program* and not the body - which is the whole reason the genome is a program.
    genome: Genome,

    /// What the organism is holding, in the same units as everything else in the world.
    ///
    /// # This is 64 bits, and everything geometric in the simulation is 32
    ///
    /// SPEC section 2 requires simulation state to be `f32` so it can live on a graphics
    /// card, and SPEC section 5 then carves out an exception for the ledger's five accounts,
    /// because a running total behaves differently from a position. This is the same
    /// exception for the same reason, and it is not really a second one: **an organism's
    /// energy is a share of the ledger's `biomass` account**, and the two have to agree
    /// exactly rather than nearly.
    ///
    /// Nearly is not good enough because of what Phase 4 does with the number. Death moves
    /// what an organism held out of `biomass` and into `detritus`; if the organism's own
    /// figure is a rounded copy, the amount moved is not the amount that was there, and the
    /// difference is energy invented or destroyed on every single death. Held at the same
    /// width as the account, every movement is exact and there is no difference to lose.
    energy: f64,

    /// How many ticks this organism has been alive.
    ///
    /// SPEC section 10 gives death two causes, and this is half of the second one: energy
    /// reaching zero, or age passing a limit the genome decides. Nothing kills anything yet -
    /// that is Phase 4 - but the count has to start at the moment of birth, and the moment of
    /// birth is here.
    age: u64,

    /// Which organism this is, for as long as the run lasts.
    ///
    /// Not the slot. A slot is a place in the arena and is handed on to whoever is born there
    /// next; a serial is minted once, never reused, and is what names this organism's private
    /// sequence of random numbers. Two organisms that lived in the same slot at different
    /// times are different organisms and must not draw the same numbers.
    serial: u64,

    /// How far into its own sequence of random numbers this organism has got.
    ///
    /// The other half of what [`Organism::stream`] needs. See this module's documentation for
    /// why the position is stored rather than the generator.
    word_position: u128,

    /// How many of its slot's cells this organism's body actually uses.
    ///
    /// Between one and `max_cells_per_organism`: development always grows at least the seed
    /// cell and is stopped by the cap.
    cells: usize,

    /// How many of its slot's springs the body uses, which is at most one fewer than its
    /// cells.
    springs: usize,
}

impl Organism {
    /// A newborn organism, at the start of its own sequence of random numbers.
    ///
    /// The word position is nought rather than something passed in, and that is the claim
    /// `rng.rs`'s `a_fresh_organism_stream_starts_at_word_position_zero` makes from the other
    /// end: an organism begins at the beginning of its own numbers, whatever its serial and
    /// whatever else has happened in the run.
    #[must_use]
    pub(crate) fn new(
        genome: Genome,
        energy: f64,
        serial: u64,
        cells: usize,
        springs: usize,
    ) -> Self {
        Self {
            genome,
            energy,
            age: 0,
            serial,
            word_position: 0,
            cells,
            springs,
        }
    }

    /// The growth program this organism was grown from.
    #[must_use]
    pub fn genome(&self) -> &Genome {
        &self.genome
    }

    /// What it is holding.
    #[must_use]
    pub fn energy(&self) -> f64 {
        self.energy
    }

    /// How many ticks it has been alive.
    #[must_use]
    pub fn age(&self) -> u64 {
        self.age
    }

    /// Which organism it is.
    #[must_use]
    pub fn serial(&self) -> u64 {
        self.serial
    }

    /// How many cells of its slot its body uses.
    #[must_use]
    pub fn cells(&self) -> usize {
        self.cells
    }

    /// How many springs of its slot its body uses.
    #[must_use]
    pub fn springs(&self) -> usize {
        self.springs
    }

    /// One tick older.
    pub(crate) fn grow_older(&mut self) {
        self.age += 1;
    }

    /// `amount` has come in.
    ///
    /// ⚠️ **Never call this without moving the same amount in the ledger.** An organism's
    /// energy and the ledger's `biomass` account are two records of one quantity, and SPEC
    /// section 5 spells out what happens when they part company: the books balance perfectly
    /// while a body stands in the world holding energy nobody counted, and nothing announces
    /// it until that energy is moved out of an account that never received it - hours into a
    /// run, with no cause to find.
    ///
    /// This is why the field is private and why there is no setter. The only callers are
    /// `world.rs`, at a birth, and `behaviour.rs`, which does every one of its movements in
    /// one place for exactly this reason.
    pub(crate) fn gain(&mut self, amount: f64) {
        self.energy += amount;
    }

    /// `amount` has gone out.
    ///
    /// The same warning as [`Organism::gain`], and one more. **This does not refuse to take
    /// an organism below nothing**, and that is deliberate rather than missing. SPEC section
    /// 5 is explicit that the invariant says energy is conserved and *not* that any account
    /// is solvent - "spending more than an organism holds drives `biomass` negative while the
    /// books still balance perfectly". Insolvency is not a bookkeeping error to be prevented
    /// here; it is what Group B turns into death.
    pub(crate) fn lose(&mut self, amount: f64) {
        self.energy -= amount;
    }

    /// This organism's own generator, wound forward to wherever it had got to.
    ///
    /// Rebuilt from the world's seed and this organism's serial every time it is asked for,
    /// which costs setting up a cipher and is the price of not storing one. See this module's
    /// documentation.
    ///
    /// The world is taken by shared reference, exactly as `rng.rs` hands out a stream, so
    /// nothing about asking for an organism's numbers disturbs the world or any other
    /// organism. That is what makes it safe to work through the population in any order or on
    /// any number of threads.
    #[must_use]
    pub fn stream(&self, rng: &WorldRng) -> ChaCha8Rng {
        let mut stream = rng.new_organism_stream(self.serial);
        stream.set_word_pos(self.word_position);
        stream
    }

    /// Put back how far through its numbers the organism has got.
    ///
    /// The other half of [`Organism::stream`], and the half that is easy to leave out. A
    /// caller that draws from the stream and does not come back here has an organism that
    /// starts from the same number every time it is asked for anything - which looks like a
    /// lineage with a fixed personality rather than like a bug.
    pub fn remember(&mut self, stream: &ChaCha8Rng) {
        self.word_position = stream.get_word_pos();
    }
}

/// How many cells one organism's slot holds, whether or not its body uses them all.
pub(crate) fn cells_per_slot(limits: &LimitsConfig) -> usize {
    usize::try_from(limits.max_cells_per_organism.get()).expect("a body-size cap fits in a word")
}

/// How many springs one organism's slot holds.
///
/// One fewer than its cells, because a spring is made only when a gene divides a cell into a
/// daughter that adheres, and a daughter is made once. A world whose bodies are allowed a
/// single cell therefore has no springs at all, and a slot of nothing is the right answer for
/// it rather than a special case.
pub(crate) fn springs_per_slot(limits: &LimitsConfig) -> usize {
    cells_per_slot(limits) - 1
}

/// Which cells of the world's arena belong to the organism in this slot.
pub(crate) fn cell_slot(slot: usize, limits: &LimitsConfig) -> Range<usize> {
    let width = cells_per_slot(limits);

    slot * width..(slot + 1) * width
}

/// Which springs of the world's arena belong to the organism in this slot.
pub(crate) fn spring_slot(slot: usize, limits: &LimitsConfig) -> Range<usize> {
    let width = springs_per_slot(limits);

    slot * width..(slot + 1) * width
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::spec_defaults;
    use rand::Rng;

    /// An organism's numbers survive being put down and picked up again, and that is what
    /// makes storing two numbers instead of a generator work.
    ///
    /// `docs/PHASE3.md`'s second architectural decision. The generator this project uses
    /// cannot be copied or stored, so an organism keeps its *serial* and *how far through its
    /// numbers it has got*, and the generator is rebuilt from the world's seed on demand.
    /// This is the test that the rebuilt one is the same generator rather than a new one
    /// wearing the same name.
    ///
    /// The shape of it is the shape of a life: draw a few numbers, put the position back, and
    /// later - having been left alone in the meantime - carry on. What comes out afterwards
    /// has to be what would have come out if the generator had never been put down.
    ///
    /// # The second half is what makes the first half mean anything
    ///
    /// An organism that never puts its position back would pass every "same numbers from the
    /// same seed" check there is, because it would faithfully hand out *the same numbers from
    /// the beginning*, for ever. That is the failure this design invites and it does not look
    /// like a failure: a lineage whose every generation mutates identically looks like a
    /// lineage that has settled down. So the last claim here is that the numbers drawn after
    /// the pause are ones the organism had **not** already seen.
    #[test]
    fn an_organisms_generator_is_rebuilt_from_two_numbers_rather_than_stored() {
        let world = WorldRng::from_seed(42);
        let limits = spec_defaults()
            .validate()
            .expect("SPEC's own defaults must be a configuration the program accepts")
            .limits;
        let mut organism = Organism::new(Genome::new(Vec::new(), &limits), 0.0, 7, 1, 0);

        // A life, in three sittings, with the position put back after each.
        let mut lived = Vec::new();
        for _ in 0..3 {
            let mut stream = organism.stream(&world);
            lived.extend((0..4).map(|_| stream.next_u64()));
            organism.remember(&stream);
        }

        // The same life, drawn in one go from the stream the organism was given at birth.
        let mut uninterrupted = world.new_organism_stream(7);
        let in_one_sitting: Vec<u64> = (0..12).map(|_| uninterrupted.next_u64()).collect();

        assert_eq!(
            lived, in_one_sitting,
            "an organism that put its generator down and picked it up again did not carry \
             on from where it left off"
        );

        // And it genuinely moved on rather than replaying its first four numbers three times.
        assert_ne!(
            lived[..4],
            lived[4..8],
            "the organism drew the same numbers in its second sitting as in its first, so \
             its position is not being put back and every generation of its lineage would \
             mutate identically"
        );
    }
}
