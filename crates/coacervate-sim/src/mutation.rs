//! Copying a genome imperfectly.
//!
//! See `SPEC.md` section 7 for the six operators this implements.
//!
//! This is the module the rest of the project is waiting for. `genome.rs` explains why an
//! organism is described by a variable-length list of rules rather than a fixed list of
//! numbers, and the answer is entirely about what can be done to it here: **copy a rule, then
//! change the state it answers to, and the body grows a part it did not have.** Without that
//! operator a lineage can only ever become a better single cell, because every mutation is a
//! new value in a slot that already existed. With it, a lineage can gain slots.
//!
//! Everything else in this file is in service of that one move, or is a wall put up to stop
//! it running away.
//!
//! # What a point mutation is here, which SPEC leaves ambiguous
//!
//! SPEC section 7 says: *"each gene, with `point_rate`: perturb numeric fields by
//! `N(0, point_sigma)`; discrete fields re-draw uniformly"*. That sentence can be read two
//! ways, and the two give very different simulations.
//!
//! Read as *"every field of the gene at once"*, a point mutation re-draws all ten of a gene's
//! discrete fields together - its action, both of its cell kinds, all three of its states and
//! its step window. A gene that has been hit is then a **brand-new random gene** wearing
//! nudged copies of its old numbers, and point mutation has become gene replacement under
//! another name. That reading quietly destroys the thing this whole project rests on: a
//! duplicated gene which is hit before selection has had time to notice it does not *diverge*,
//! it is *overwritten*, and "duplicate then diverge" collapses into "insert a random gene",
//! which the genome could already do without any of this machinery.
//!
//! So it is read the other way: **a gene that is hit has exactly one of its sixteen fields
//! changed**, chosen uniformly. That is what "point mutation" means everywhere else in
//! biology - one locus, one change - and it is the reading the rest of the design assumes.
//! `genome.rs` argues at length that a gene carries the parameters of all three actions so
//! that changing a gene's *action* is a step rather than a leap, "because the parameters that
//! action needs were already there". That argument is only true if the action can change on
//! its own, which is exactly this reading. It also agrees with `config/default.toml`, which
//! annotates `point_rate` as "per gene, per reproduction" rather than per field.
//!
//! The practical consequence is the one that matters: a duplicated gene whose `trigger_state`
//! is re-drawn keeps its angle, its adhesion, its cell kind and its timing, and therefore
//! grows *the same structure somewhere new*. That is a new body part rather than a new
//! accident, and it is what `a_duplicated_gene_that_diverges_is_a_new_body_part` demonstrates.
//!
//! # ⭐⭐ Which *distribution* a discrete re-draw uses, which SPEC also leaves open
//!
//! SPEC section 7 says *"discrete fields re-draw uniformly"*. That is a sentence about a
//! **field** - it says a state is re-drawn rather than nudged, because state 5 and state 6 have
//! nothing to do with one another - and it leaves the distribution the re-draw comes from
//! entirely open. Two of the three state fields now come from a **mixture**, and the reason is
//! the largest measurement this project has taken.
//!
//! Measured over 6.46 million cell-observations of the shipped world: **a genome contains a gene
//! naming its own cell's state for 2.2% of the cells it grew**, excluding the seed cell, which
//! is connected by construction. Uniform across every kind. Development matches on
//! `trigger_state`, so it **stops at 97.8% of the cells it visits** - which is why mean body size
//! sat at 1.98 cells for the first 140,000 ticks of every run ever measured here, and why the
//! founder is exactly two cells: its gene hands the daughter `child_state = 1` and nothing names
//! 1.
//!
//! The rule is not the thing to change. SPEC section 7 rests the whole justification for the
//! genome design on it - *"because conditions key on `state`, duplicating a gene and changing its
//! `trigger_state` creates a new body part"* - and a rule has to say which cells a gene acts on.
//! **What was wrong was the distribution.** Drawing a state uniformly over sixty-four when a
//! genome mentions three is what makes duplicate-and-diverge land on nothing.
//!
//! ## ⚠️ The two state fields want opposite biases
//!
//! They are not the same kind of thing and treating them alike would break the mechanism:
//!
//! - **`trigger_state` says which cells a gene acts on.** A duplicated gene whose copy is pointed
//!   at a state no cell is in fires nowhere, and the copy is wasted. So it should land on a state
//!   that **already exists in bodies**. That is the draw that makes duplicate-and-diverge pay.
//! - **`child_state` and `new_state` are the identity a gene *hands out*.** If those could only
//!   ever name states the genome already answers to, the set of addressable identities could
//!   never grow: no new body part could be invented, and a lineage would collapse onto the closed
//!   set of three or four states its founder happened to have. That is a different failure and a
//!   worse one, because it is the one that makes the whole design pointless rather than merely
//!   slow.
//!
//! So both are **mixtures rather than replacements**, in opposite proportions - see
//! [`TRIGGER_ONTO_AN_OCCUPIED_STATE`] and [`CHILD_ONTO_AN_ANSWERED_STATE`], which carry the
//! arithmetic.
//!
//! ## Where "the states that already exist" comes from, and what it costs
//!
//! **From the genome, not from a body.** The alternative is to develop the parent's body and read
//! off the states its cells are actually in, which is exact - and costs a whole development pass
//! per reproduction, on top of the one reproduction already does for the child. It is also *less*
//! stable than it looks: the states a body occupies are a function of the step windows and of
//! first-match-wins as well as of the states, so the answer would move under mutations that have
//! nothing to do with addressing.
//!
//! [`Alphabet`] is read straight off the gene list instead, in one pass over at most 128 genes
//! with no allocation - two 64-bit masks, one bit per state. It is a **superset** of what a
//! development pass would report, and provably so: every state a cell can be in is either state 0,
//! which SPEC section 7's development puts the seed cell in, or some gene's `child_state` or
//! `new_state`, because those are the only two ways a cell's state is ever written. So a draw from
//! it can miss - by naming a state that is written down but never reached - and it can never
//! exclude a state a cell really is in. That is the right direction for the error to point.
//!
//! # Numeric fields are held inside the bounds a random gene is drawn from
//!
//! `genome.rs` declares the bounds [`Gene::random`] draws from and explicitly leaves it to
//! this module to decide whether a *drifting* gene is held inside them. It is, and two of
//! those bounds are the reason.
//!
//! `MAX_STIFFNESS` is not a matter of taste: SPEC section 8 integrates the physics explicitly
//! at a sixtieth of a second, and a spring stiffer than `4 / dt²` — 14,400 at that tick rate —
//! makes that integrator diverge. The bound is a hundredth of that, and the two orders of
//! magnitude are headroom against exactly this operator. An unclamped stiffness is a lineage
//! on a slow random walk towards a numerical explosion, and the explosion arrives as a body
//! flung across the world rather than as an error message.
//!
//! `MAX_REST_LENGTH` is the same kind of number: it is chosen so the widest body a
//! genome can grow stays inside half the world's width, which is the distance at which SPEC
//! section 8's spring-through-the-seam warning starts to bite. Neither of those is a
//! preference about what organisms should look like. They are the edges of the arithmetic.
//!
//! The other three bounded fields - `osc_freq`, `sensor_gain` and the lower end of
//! `rest_length` - are clamped for consistency rather than necessity. A model where some
//! numbers are bounded and others wander makes the phrase "every field in range" untestable,
//! and being able to say it about *any* genome, however many mutations old, is worth more
//! than the extra reach an unbounded gain would give a sensocyte. Phase 4 owns those numbers
//! and can raise them.
//!
//! Clamping does pile a little probability up exactly on each boundary, which is a real
//! artefact and the honest way to read it is that the wall is where "as stiff as this world
//! allows" lives. The alternative - rejecting a perturbation that would leave the range -
//! sounds tidier and is worse: a gene sitting on the wall would then be unable to mutate that
//! field *at all*, so the field would freeze rather than the value being bounded.
//!
//! # ...except the two that are angles, which wrap
//!
//! `angle` and `osc_phase` are directions on a circle, not quantities. Clamping them would
//! put a barrier across a thing that has no ends: a gene whose daughters bud at just under
//! half a turn could never mutate to just over, and every lineage that walked that way would
//! pile up against a wall that exists only because the number had to be written down starting
//! somewhere. They wrap instead, which is what a circle means.
//!
//! # The Gaussian is written out here rather than taken from a crate
//!
//! `rand_distr` has the normal distribution in it and is the obvious answer. It is not used,
//! because it pulls in `num-traits` and `libm`, and CLAUDE.md pins this crate's dependencies
//! at exactly `rand` and `serde` - a rule that exists so the simulation cannot quietly grow a
//! dependency that later disagrees with itself about arithmetic. Ten lines of Marsaglia polar
//! is a smaller thing to own than two transitive crates. See [`gaussian`].
//!
//! # The cap, and what happens at it
//!
//! CLAUDE.md marks `max_genes` as the one cap that must never be raised without a metabolic
//! cost per gene, because duplication compounds and no amount of memory outruns an
//! exponential. SPEC section 7 says what happens when a genome is against it: **a mutation
//! that would lengthen it fails. It does not truncate.**
//!
//! That distinction is the whole of it. Truncating from the end is a silent, *biased*
//! operator, and the end of the genome is exactly where the neutral, non-firing material
//! accumulates - the raw material this design says duplication feeds on. A saturated lineage
//! that truncated would begin eating its own raw material from the far end and would quietly
//! stop being open-ended, with nothing anywhere reporting that it had happened. Failing
//! instead is what the rest of the simulation already does when it runs out of room: births
//! fail at the population cap rather than allocating. Deletion still works, so a full genome
//! can shrink and grow again; it simply cannot grow past the wall.
//!
//! # Every draw comes from the organism's own generator
//!
//! [`mutate`] takes a generator rather than reaching for one, and the one it is given is the
//! organism's own - `WorldRng::new_organism_stream`. That is what makes an offspring's
//! mutations a function of its parent's genome and its own serial number, and of nothing
//! else: not of how many organisms were born first, not of what order the world happened to
//! process them in, and not of how many threads Phase 4 runs them on.

use crate::cell::CellKind;
use crate::config::{LimitsConfig, MutationConfig};
use crate::genome::{
    Action, FIELDS_IN_A_GENE, Gene, Genome, MAX_OSC_FREQ, MAX_REST_LENGTH, MAX_SENSOR_GAIN,
    MAX_STIFFNESS, SensorTarget, State,
};
// `RngExt` is what supplies `random`, `random_range` and `random_bool`; `rand`'s `Rng` is
// the narrower trait beneath it and brings none of them.
use rand::RngExt;
use rand::rngs::ChaCha8Rng;
use std::f32::consts::{PI, TAU};

/// Copy a genome, imperfectly. SPEC section 7's six operators, in SPEC section 7's order.
///
/// The order is SPEC's and it is not arbitrary: point mutation runs before the operators that
/// change the genome's length, so a gene is perturbed as itself rather than as a copy of
/// something, and whole-genome duplication runs last, so the copy it appends is a copy of the
/// genome as this reproduction left it.
///
/// # Panics
///
/// If the operators between them have produced a genome longer than `max_genes`. Each of the
/// three that can lengthen a genome declines to when there is no room, so this cannot happen —
/// and the assertion is here anyway because of what would happen if it did. `Genome::new`
/// truncates, so an operator that overran the cap would be *silently* corrected by having its
/// far end trimmed off, which is exactly the biased, invisible failure SPEC section 7 says to
/// avoid: the far end is where the neutral material lives. The genome would still be inside
/// its cap, every test that counts genes would stay green, and a lineage would quietly stop
/// being open-ended. CLAUDE.md asks for invariants to be asserted at runtime rather than only
/// in tests, and this is one that fails silently in exactly the direction that matters.
#[must_use]
pub fn mutate(
    parent: &Genome,
    mutation: &MutationConfig,
    limits: &LimitsConfig,
    rng: &mut ChaCha8Rng,
) -> Genome {
    let mut genes = parent.genes().to_vec();

    let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap fits in a word");

    // ⭐⭐ Read once, off the genome **as it arrived**, and handed to every point mutation. Not
    // recomputed as the loop goes, for the same reason `physics.rs` reads every body axis before
    // any cell moves: a distribution that shifted under a half-mutated genome would make the
    // mutations of the genes at the back of a genome depend on what happened to the genes at the
    // front, which is order dependence with nothing to justify it. See this module's
    // documentation.
    let alphabet = Alphabet::of(&genes);

    for gene in &mut genes {
        if rng.random_bool(f64::from(mutation.point_rate)) {
            point_mutate(gene, alphabet, mutation.point_sigma, limits, rng);
        }
    }

    if rng.random_bool(f64::from(mutation.duplication_rate)) {
        duplicate_a_gene(&mut genes, cap, rng);
    }

    if rng.random_bool(f64::from(mutation.deletion_rate)) {
        delete_a_gene(&mut genes, rng);
    }

    if rng.random_bool(f64::from(mutation.insertion_rate)) {
        insert_a_random_gene(&mut genes, cap, limits, rng);
    }

    if rng.random_bool(f64::from(mutation.reorder_rate)) {
        swap_two_adjacent_genes(&mut genes, rng);
    }

    if rng.random_bool(f64::from(mutation.genome_duplication_rate)) {
        duplicate_the_whole_genome(&mut genes, cap);
    }

    assert!(
        genes.len() <= cap,
        "mutation produced a genome of {} genes where {cap} are allowed. At the cap a \
         lengthening mutation is supposed to fail; something lengthened one anyway, and \
         Genome::new would have hidden it by trimming the far end",
        genes.len()
    );

    Genome::new(genes, limits)
}

/// Copy one gene of the genome and put the copy immediately after it.
///
/// **The operator the whole project rests on.** SPEC section 7: "copy a random gene, insert
/// adjacent". Everything about the genome being a variable-length list of rules rather than a
/// struct of parameters is so that this can be done to it.
///
/// The copy goes *after* its original, which is the difference between an operator that costs
/// nothing and one that costs something. First-match-wins means a gene sitting behind an
/// identical gene can never fire, so the organism this produces is the organism it came from,
/// cell for cell — and a copy that changes nothing is a copy selection cannot punish, which is
/// exactly the condition under which it is free to drift somewhere new. Put the copy in front
/// instead and the gene doing the organism's work would be the one that was made a moment ago
/// rather than the one that has been earning its keep.
///
/// At the cap it does nothing at all. See this module's documentation for why failing is the
/// only honest answer there and truncating is not.
fn duplicate_a_gene(genes: &mut Vec<Gene>, cap: usize, rng: &mut ChaCha8Rng) {
    if genes.is_empty() || genes.len() >= cap {
        return;
    }

    let which = rng.random_range(0..genes.len());
    genes.insert(which + 1, genes[which]);
}

/// Take one gene out of the genome.
///
/// The counterweight to duplication, and what keeps the cap from being a one-way door: a
/// lineage that has filled its genome can shrink and grow again, so being saturated is a
/// condition rather than a fate.
///
/// Nothing is re-ordered on the way out. Removing a gene can bring a gene *behind* it to life,
/// because the one in front is no longer answering first, and that is a real evolutionary
/// event - a silent copy becoming the working gene without anything having changed inside it.
fn delete_a_gene(genes: &mut Vec<Gene>, rng: &mut ChaCha8Rng) {
    if genes.is_empty() {
        return;
    }

    genes.remove(rng.random_range(0..genes.len()));
}

/// Put a gene nobody has ever seen before into the genome, at a position drawn uniformly.
///
/// The only operator that produces material a lineage did not already have some version of,
/// which makes it the one that stops a population being trapped inside the states its founder
/// happened to be given.
///
/// SPEC section 7 says "insert a fully random gene" and does not say where it goes. It goes
/// anywhere, with equal probability, rather than on the end: appending would make every
/// inserted gene the last one consulted, so it could only ever fire on a state no existing
/// gene claims, and insertion would be an operator that mostly inserts silence.
///
/// At the cap it does nothing, for the reason in this module's documentation.
fn insert_a_random_gene(
    genes: &mut Vec<Gene>,
    cap: usize,
    limits: &LimitsConfig,
    rng: &mut ChaCha8Rng,
) {
    if genes.len() >= cap {
        return;
    }

    let where_to = rng.random_range(0..=genes.len());
    genes.insert(where_to, Gene::random(rng, limits));
}

/// Swap one pair of neighbouring genes.
///
/// A mutation that changes no number anywhere and can still change the organism, because
/// development takes the *first* gene that answers to a cell and where a gene sits therefore
/// decides whether it is ever consulted. It is also how a gene that has been silent for a
/// thousand generations gets its turn: one swap puts it in front of the gene that was
/// shadowing it.
///
/// # Where its rate comes from, and why that took a change to SPEC
///
/// It fires on `mutation.reorder_rate`, like every other operator here. That was not true when
/// this module was written: SPEC section 7 listed six operators and gave five of them a rate,
/// reordering was written as *"Reordering — swap two adjacent genes"* with nothing beside it,
/// and `[mutation]` in section 3 had no key for one — the same shape of gap Phase 1 recorded
/// as Q1, where section 2 referred to a cap on ticks per second that section 3 had no key for.
/// So the number lived here as a `REORDER_RATE` constant, the way `genome.rs` declares the four
/// bounds SPEC does not give, and Phase 3 raised it as Q10.
///
/// **The key was added, because an operator with no rate cannot be switched off.** A constant
/// meant reordering fired on one reproduction in fifty during every other operator's test,
/// which is a rate every one of those tests had to be written around; it meant the `bloom`,
/// `famine` and `slow` profiles could not touch it; and it meant the one operator whose whole
/// job is to decide which genes are *expressed* was the one nobody could turn down. It is
/// **0.02** — the rate SPEC gives duplication and deletion, the other two operators that
/// rearrange a genome rather than change what is written in it. Reordering is the gentlest of
/// the three: it creates nothing, destroys nothing, and undoes itself if it happens twice in
/// the same place, so if any of them can afford to be common it is this one. Putting it below
/// the others would mean silent genes accumulating faster than a lineage can ever bring one
/// forward, and expressing a silent gene is the *second* half of duplicate-then-diverge.
///
/// A genome of fewer than two genes has no pair to swap and is left alone.
fn swap_two_adjacent_genes(genes: &mut [Gene], rng: &mut ChaCha8Rng) {
    if genes.len() < 2 {
        return;
    }

    let left = rng.random_range(0..genes.len() - 1);
    genes.swap(left, left + 1);
}

/// Append a second copy of the entire genome to itself.
///
/// The same move as [`duplicate_a_gene`] applied to everything at once, and neutral for the
/// same reason: every gene in the appended half sits behind an identical gene in the original
/// half, so the body that grows is the body that grew before. What the lineage gains is a
/// complete spare set, free to drift.
///
/// A genome more than half the cap **fails entirely** rather than appending as much as fits.
/// Half a copy of a genome is not a genome, and appending one would be a different operator
/// from the one SPEC section 7 describes - it would keep the front of the genome, which is the
/// expressed part, and drop the back, which is the drifting part.
fn duplicate_the_whole_genome(genes: &mut Vec<Gene>, cap: usize) {
    if genes.len() * 2 > cap {
        return;
    }

    genes.extend_from_within(..);
}

/// ⭐⭐ How often a re-drawn `trigger_state` lands on a state some cell of the body is in,
/// rather than anywhere in the sixty-four.
///
/// **Three quarters.** The argument for it being large is the whole of this module's section on
/// the two biases: a `trigger_state` that names a state no cell is in is a gene that fires
/// nowhere, and at a uniform draw that was the outcome **97.8%** of the time. A bias at or below
/// a half would leave the miss the ordinary case and would be a change nobody could measure.
///
/// The argument for it not being **one** is the part worth writing down, because a replacement is
/// the tempting thing to write. Two things go if the uniform tail goes.
///
/// A gene could then never be pointed at a state nothing occupies, which is one of the two ways a
/// gene goes *silent* - and SPEC section 7 is explicit that non-firing genes are not waste but
/// *"exactly where duplication finds raw material to diverge"*. An operator that cannot switch a
/// gene off is an operator that has quietly deleted the neutral half of the genome.
///
/// And it is the dial against the failure this change is most likely to produce. If every cell in
/// every body became developmentally live, bodies would run straight into
/// `limits.max_cells_per_organism` and every organism in the world would be a 64-cell blob. A
/// quarter of re-draws going anywhere at all is what keeps a genome's addressing loose.
///
/// A quarter also gives a plain reading: **one re-draw in four still goes anywhere**, so a lineage
/// is four mutations rather than one from pointing a gene outside everything it currently is.
///
/// # ⭐⭐ And the blob was measured rather than feared
///
/// Three 300,000-tick runs of the shipped world, seed 42, differing only in this number and in
/// [`CHILD_ONTO_AN_ANSWERED_STATE`]:
///
/// | | uniform | **0.75 / 0.25** | 1.00 / 0.50 |
/// | --- | --- | --- | --- |
/// | grown cells in a state their genome names | 4.6% | **17.7%** | 25.4% |
/// | largest body in the world, of a cap of 64 | 32 | **17** | **64** |
/// | bodies at the cap | 0.00% | **0.00%** | **0.11%** |
/// | mean cells | 6.62 | 6.09 | 5.81 |
///
/// **At one the failure begins**, and it begins where taking the tail off predicts: with no tail no
/// gene can ever be switched off by being pointed at nothing, so bodies reach the cap and sit on
/// it. At three quarters nothing does — the largest body in the whole run was 17 cells. That is a
/// better reason for this number than the arithmetic it was first chosen by, and it is why it is
/// three quarters rather than four fifths or nine tenths.
const TRIGGER_ONTO_AN_OCCUPIED_STATE: f64 = 0.75;

/// ⭐⭐ How often a re-drawn `child_state` or `new_state` lands on a state some gene of the genome
/// already answers to, rather than anywhere in the sixty-four.
///
/// **A quarter — deliberately the opposite way round from
/// [`TRIGGER_ONTO_AN_OCCUPIED_STATE`]**, and the asymmetry is the whole of the decision.
///
/// This field is the identity a gene *hands out*. A quarter of re-draws landing on a name some
/// rule already answers to is what closes the addressing loop from the other end - a daughter
/// given a live name is a daughter development can carry on with, which is how a body gets deeper
/// than two cells - and it is deliberately the *minority* case, because the three quarters that go
/// anywhere are what keeps the space of identities **open**. A genome that could only ever hand out
/// names it already answers to could never mint a state nothing yet names, so no new body part
/// could ever be invented and a lineage would be trapped inside the three or four states its
/// founder was given. That is a worse failure than small bodies, and it is the one this number
/// exists to prevent.
///
/// So the two together read: **a rule reaches for a cell that exists; a name is mostly new.** The
/// first is what makes a duplicated gene fire; the second is what makes there be somewhere new for
/// the next duplicate to fire.
///
/// # ⚠️ Neither of these is a configuration key
///
/// `behaviour.rs`'s `LIGHT_REFERENCE` is the precedent and the reason is the same one. A setting
/// in `[mutation]` is a thing a person turns while watching a world - how often mutation happens,
/// how large a step it takes - and it goes into the document a run is replayed from. These two are
/// neither: they are a property of the *operator's own distribution*, they mean nothing to anybody
/// setting up an experiment, and a run whose archived configuration carried them would be a run in
/// which what a `state` addresses had been quietly redefined by a slider. `mutation.point_rate` is
/// the dial that already turns this whole operator down.
const CHILD_ONTO_AN_ANSWERED_STATE: f64 = 0.25;

/// The alphabet of developmental states a genome writes, in the two halves that mean different
/// things: the states its cells end up **in**, and the states its genes **answer to**.
///
/// One bit per state, so a genome's whole alphabet is two 64-bit words and reading it is one pass
/// over the gene list with no allocation. See this module's documentation for why it is read off
/// the genome rather than off a developed body.
#[derive(Clone, Copy)]
struct Alphabet {
    /// A bit set for every state a cell of the body this genome grows could be in.
    ///
    /// The `child_state` of every dividing gene and the `new_state` of every differentiating one,
    /// because those are the only two ways a cell's state is ever written — **and state 0**, which
    /// SPEC section 7's development puts the seed cell in without any gene having to name it.
    ///
    /// ⚠️ **The seed cell's state is what makes this work at all.** Without it a genome whose genes
    /// hand out only state 5 would draw every `trigger_state` onto 5, nothing would answer to the
    /// seed cell, and every body in that lineage would be one cell.
    occupied: u64,

    /// A bit set for every state some gene of this genome triggers on: the names the program
    /// answers to.
    answered: u64,
}

impl Alphabet {
    /// Read a genome's alphabet off its genes.
    fn of(genes: &[Gene]) -> Self {
        // SPEC section 7: development begins with one photocyte in state 0. Every body in the
        // world therefore has a cell in state 0, whatever its genome says.
        let mut occupied = bit(State::ZERO);
        let mut answered = 0;

        for gene in genes {
            occupied |= bit(gene.child_state) | bit(gene.new_state);
            answered |= bit(gene.trigger_state);
        }

        Self { occupied, answered }
    }

    /// A state for a gene to answer to: one some cell is in, three times in four.
    ///
    /// See [`TRIGGER_ONTO_AN_OCCUPIED_STATE`].
    fn trigger(self, rng: &mut ChaCha8Rng) -> State {
        if rng.random_bool(TRIGGER_ONTO_AN_OCCUPIED_STATE) {
            one_of(self.occupied, rng)
        } else {
            State::random(rng)
        }
    }

    /// A state for a gene to hand out: one some gene answers to, one time in four.
    ///
    /// See [`CHILD_ONTO_AN_ANSWERED_STATE`].
    fn handed_out(self, rng: &mut ChaCha8Rng) -> State {
        if rng.random_bool(CHILD_ONTO_AN_ANSWERED_STATE) {
            one_of(self.answered, rng)
        } else {
            State::random(rng)
        }
    }
}

/// Which bit of an [`Alphabet`]'s two words stands for this state.
const fn bit(state: State) -> u64 {
    1 << state.get()
}

/// One of the states in a mask, drawn uniformly from the ones that are there.
///
/// The `n`th set bit, found by clearing the lowest set bit `n` times — `left & (left - 1)` is the
/// standard idiom for that and it is exact under CLAUDE.md's release-mode overflow checking,
/// because the loop runs strictly fewer times than there are bits to clear and so never sees a
/// word that has run out of them.
///
/// # Panics
///
/// If the mask is empty, because there is then no state to answer with. **Neither of the two masks
/// can be**, and both reasons are structural rather than hopeful: `Alphabet::occupied` always
/// carries state 0, and `Alphabet::answered` carries one bit per gene, while the only caller is a
/// point mutation, which by construction is looking at a gene of a genome that has one.
fn one_of(states: u64, rng: &mut ChaCha8Rng) -> State {
    let mut left = states;
    for _ in 0..rng.random_range(0..states.count_ones()) {
        left &= left - 1;
    }

    State::new(u8::try_from(left.trailing_zeros()).expect("a state is one of sixty-four"))
}

/// Change exactly one of a gene's sixteen fields: perturb it if it is a number, re-draw it
/// if it is a choice from a fixed set.
///
/// Which of the two a field gets is not a judgement call - it is what the field *is*. There
/// is no sense in which a cell kind is 0.12 away from another cell kind, and no sense in
/// which nudging a `bool` means anything, so those are re-drawn. The six genuine numbers are
/// nudged, and held inside the bounds `genome.rs` declares; see this module's documentation
/// for why they are held rather than let wander, and why the two that are angles wrap instead.
///
/// ⭐⭐ **Three of the discrete fields are states, and two of the three are re-drawn from a
/// mixture rather than uniformly.** `trigger_state` mostly lands on a state some cell is in;
/// `child_state` and `new_state` mostly land anywhere. See [`Alphabet`] and this module's
/// documentation for the measurement that forced it and for why the two want opposite biases.
/// Everything else here re-draws uniformly exactly as SPEC section 7 says.
///
/// The step window's two ends are re-drawn *independently*, which means one point mutation
/// can put a gene's `min_step` above its `max_step` and switch the gene off entirely. That is
/// deliberate and `genome.rs` says why: a gene that can never fire is not broken, it is
/// neutral material, and material is what duplication diverges from. Because the two ends are
/// drawn independently, the same operator switches such a gene back on again - so the
/// difference between a silent gene and a live one is one mutation in either direction.
fn point_mutate(
    gene: &mut Gene,
    alphabet: Alphabet,
    sigma: f32,
    limits: &LimitsConfig,
    rng: &mut ChaCha8Rng,
) {
    // Re-drawn step numbers stay inside the run's actual budget for the reason `Gene::random`
    // gives: a gene naming step 200 in a run that takes sixteen is a gene that has been
    // switched off by arithmetic rather than by selection.
    let last = crate::genome::last_step(limits);

    match rng.random_range(0..FIELDS_IN_A_GENE) {
        0 => gene.trigger_state = alphabet.trigger(rng),
        1 => gene.min_step = rng.random_range(0..=last),
        2 => gene.max_step = rng.random_range(0..=last),
        3 => gene.action = Action::ALL[rng.random_range(0..Action::ALL.len())],
        4 => gene.angle = wrapped_onto_the_circle(gene.angle + gaussian(rng, sigma)),
        5 => gene.adhere = rng.random(),
        6 => gene.child_state = alphabet.handed_out(rng),
        7 => gene.child_kind = CellKind::ALL[rng.random_range(0..CellKind::ALL.len())],
        8 => {
            gene.rest_length =
                (gene.rest_length + gaussian(rng, sigma)).clamp(0.0, MAX_REST_LENGTH);
        }
        9 => gene.stiffness = (gene.stiffness + gaussian(rng, sigma)).clamp(0.0, MAX_STIFFNESS),
        10 => gene.new_kind = CellKind::ALL[rng.random_range(0..CellKind::ALL.len())],
        11 => gene.new_state = alphabet.handed_out(rng),
        12 => gene.osc_freq = (gene.osc_freq + gaussian(rng, sigma)).clamp(0.0, MAX_OSC_FREQ),
        13 => gene.osc_phase = (gene.osc_phase + gaussian(rng, sigma)).rem_euclid(TAU),
        14 => {
            gene.sensor_gain =
                (gene.sensor_gain + gaussian(rng, sigma)).clamp(-MAX_SENSOR_GAIN, MAX_SENSOR_GAIN);
        }
        _ => gene.sensor_target = SensorTarget::ALL[rng.random_range(0..SensorTarget::ALL.len())],
    }
}

/// Bring a direction back onto the circle, into the half-turn either side of straight ahead
/// that `Gene::random` draws from.
///
/// Used for `angle`, and `osc_phase` is wrapped the same way onto a whole turn. See this
/// module's documentation for why those two are wrapped where the other four numbers are
/// clamped: a barrier across a circle is a barrier across nothing.
fn wrapped_onto_the_circle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// A number drawn from a normal distribution of mean zero and standard deviation `sigma`:
/// SPEC section 7's `N(0, point_sigma)`.
///
/// This is the Marsaglia polar method, and it is written out here rather than imported for
/// the reason in this module's documentation - `rand_distr` would cost this crate two
/// transitive dependencies to save ten lines.
///
/// How it works, since it is short enough to read: throw a dart at the square from `-1` to
/// `1` in both directions, and throw again if it missed the circle inside that square. A dart
/// that landed inside gives, in one go, both a uniformly random *direction* and - through the
/// logarithm of how far out it landed - a distance distributed the way a normal distribution
/// needs. Multiplying the two back together gives a number that is normally distributed, and
/// the rejection is what makes the direction uniform without a sine or a cosine anywhere.
///
/// The loop always ends. A dart lands inside the circle about seventy-eight times in a
/// hundred, so a fifth throw is needed about once in five hundred draws and a tenth about
/// once in a million.
///
/// ⚠️ The logarithm here joins the sine and cosine in `development.rs` on the short list of
/// arithmetic that IEEE 754 does **not** pin to a single answer, so two versions of a maths
/// library may legitimately differ in the last bit. Everything else in this simulation would
/// replay identically on any machine. Phase 8's archive and Phase 9's port to the graphics
/// card both have to check that rather than assume it.
fn gaussian(rng: &mut ChaCha8Rng, sigma: f32) -> f32 {
    loop {
        let x: f32 = rng.random_range(-1.0..=1.0);
        let y: f32 = rng.random_range(-1.0..=1.0);
        let square = x.mul_add(x, y * y);

        // Outside the circle is a miss; dead in the centre has no direction to give and its
        // logarithm is not a number.
        if square > 0.0 && square < 1.0 {
            return sigma * x * (-2.0 * square.ln() / square).sqrt();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Vec2;
    use crate::config::{RawLimits, RawMutation, spec_defaults};
    use crate::development::{Body, develop};
    use crate::genome::{Action, Gene, SensorTarget, State};
    use crate::rng::WorldRng;
    use proptest::prelude::*;
    use std::f32::consts::FRAC_PI_2;

    /// SPEC's shipped limits, checked by `config.rs`'s gate on the way through.
    fn spec_limits() -> LimitsConfig {
        spec_defaults()
            .validate()
            .expect("SPEC's own defaults must be a configuration the program accepts")
            .limits
    }

    /// SPEC's limits with one of them changed, through `config.rs`'s gate exactly as the
    /// shipped numbers go.
    fn limits_with(change: impl FnOnce(&mut RawLimits)) -> LimitsConfig {
        let mut raw = spec_defaults();
        change(&mut raw.limits);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
            .limits
    }

    /// SPEC's mutation rates with some of them changed.
    ///
    /// Almost every test below turns exactly one operator up to certainty and the rest off,
    /// because an operator being tested next to five others firing at their shipped rates is
    /// an operator whose test would be about something else on two runs in a hundred. Going
    /// through `validate` rather than building the record by hand is what keeps every rate a
    /// rate the program would genuinely accept.
    fn mutation_with(change: impl FnOnce(&mut RawMutation)) -> MutationConfig {
        let mut raw = spec_defaults();
        change(&mut raw.mutation);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
            .mutation
    }

    /// Every operator switched off. The starting point for turning exactly one back on.
    ///
    /// All six of them, which it could not say before `mutation.reorder_rate` existed: until
    /// then reordering had no key and went on firing on one reproduction in fifty whatever a
    /// test had turned down to zero. See [`swap_two_adjacent_genes`].
    fn nothing_happens() -> MutationConfig {
        mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        })
    }

    /// A gene that fires on the seed cell and does nothing but stop it.
    ///
    /// The same fixture `development.rs` uses, and for the same reason: every test here cares
    /// about two or three of a gene's sixteen fields and needs the other thirteen to be
    /// *something* that does not grow a body behind the test's back.
    fn a_quiet_gene() -> Gene {
        Gene {
            trigger_state: State::ZERO,
            min_step: 0,
            max_step: u8::MAX,
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
        }
    }

    /// Genes that can be told apart at a glance, so "the order was kept" and "this one is a
    /// copy of that one" are claims about something visible.
    fn numbered_genes(how_many: u8) -> Vec<Gene> {
        (0..how_many)
            .map(|n| Gene {
                trigger_state: State::new(n),
                ..a_quiet_gene()
            })
            .collect()
    }

    /// One organism's own generator, which is the only place a mutation may draw from.
    fn organism_rng(seed: u64, serial: u64) -> ChaCha8Rng {
        WorldRng::from_seed(seed).new_organism_stream(serial)
    }

    /// How many springs each cell of a body has.
    ///
    /// A body's *shape* is a list of positions, but its **structure** is which cells are
    /// joined to which, and this is how that gets counted. A cell with one spring is the end
    /// of something; a cell with two is in the middle of something; a cell with three is a
    /// **branch**, and a body made only of chains cannot contain one however long the chains
    /// are or wherever they point.
    fn springs_per_cell(body: &Body) -> Vec<usize> {
        let mut count = vec![0; body.cells.len()];
        for spring in &body.springs {
            count[spring.a] += 1;
            count[spring.b] += 1;
        }
        count
    }

    /// A point mutation moves one of a gene's numbers, and the size of the move is
    /// `N(0, point_sigma)`.
    ///
    /// Three separate claims, and they are separate because each is satisfied by an
    /// implementation that gets the other two wrong.
    ///
    /// **One field, not sixteen.** Of twenty thousand mutations of the same one-gene genome,
    /// about one in sixteen should have moved the `angle` - because a gene that is hit has
    /// exactly one of its sixteen fields changed. See this module's documentation for why
    /// SPEC's sentence is read that way; the short version is that the other reading turns a
    /// duplicated gene into a freshly-random one before it can diverge, and takes the point
    /// of the project with it.
    ///
    /// **The right distribution.** The moves that did happen must have a mean of zero and a
    /// standard deviation of `point_sigma`, and roughly sixty-eight per cent of them must
    /// fall within one standard deviation of zero. That last figure is what tells a Gaussian
    /// apart from the far easier thing to write by accident - a uniform spread of the same
    /// width would put only fifty-eight per cent inside one deviation, which is nowhere near
    /// the tolerance here. Together they say the hand-rolled polar method in [`gaussian`] is
    /// actually normal and is actually scaled by the configured sigma.
    ///
    /// **The bounds hold.** A gene sitting exactly on `MAX_STIFFNESS` is mutated two thousand
    /// times and never leaves the range, and does sometimes come back below it - so the
    /// clamp bounds the value rather than freezing the field. That is this module's answer to
    /// the question `genome.rs` left open, and the reason is in the module documentation: the
    /// stiffness bound is the edge of the physics' arithmetic, not a matter of taste.
    ///
    /// And with `point_rate` at zero nothing changes at all, without which every number above
    /// could be produced by an operator that fires far too often.
    #[test]
    fn a_point_mutation_perturbs_a_numeric_field() {
        let limits = spec_limits();
        let sigma = 0.12_f32; // SPEC's shipped point_sigma.
        let rates = mutation_with(|rates| {
            rates.point_rate = 1.0;
            rates.point_sigma = f64::from(sigma);
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });

        // An angle of zero, far from the ends of its range, so nothing here is measuring a
        // wrap-around instead of a perturbation.
        let genome = Genome::new(vec![a_quiet_gene()], &limits);
        let mut rng = organism_rng(42, 0);

        const SAMPLES: u32 = 20_000;
        let mut moves: Vec<f64> = Vec::new();
        for _ in 0..SAMPLES {
            let angle = mutate(&genome, &rates, &limits, &mut rng).genes()[0].angle;
            if angle.abs() > 0.0 {
                moves.push(f64::from(angle));
            }
        }

        // Sixteen fields, one of them chosen: 1,250 of 20,000, and a spread of about 34
        // either way. The band is wide enough to be about the design rather than the seed.
        assert!(
            (1_050..=1_450).contains(&moves.len()),
            "{} of {SAMPLES} point mutations moved the angle. One in sixteen - about 1,250 - \
             is what a mutation that changes a single field of the gene it hits produces; \
             far more than that means every field is being re-drawn at once, which turns a \
             duplicated gene into a random one before it can diverge",
            moves.len()
        );

        let count = u32::try_from(moves.len()).expect("a sample count of thousands is a word");
        let mean = moves.iter().sum::<f64>() / f64::from(count);
        let variance =
            moves.iter().map(|m| (m - mean) * (m - mean)).sum::<f64>() / f64::from(count);
        let deviation = variance.sqrt();
        let within_one = moves.iter().filter(|m| m.abs() <= f64::from(sigma)).count();
        let inside =
            f64::from(u32::try_from(within_one).expect("a sample count of thousands is a word"))
                / f64::from(count);

        assert!(
            mean.abs() < 0.02,
            "the average point mutation moved the angle by {mean}, so mutation has a \
             direction: every lineage would drift the same way whatever selection wanted"
        );
        assert!(
            (deviation - f64::from(sigma)).abs() < 0.015,
            "point mutations had a standard deviation of {deviation} where point_sigma asked \
             for {sigma}, so the configured mutation strength is not the mutation strength"
        );
        assert!(
            (0.62..=0.74).contains(&inside),
            "{inside} of the moves fell within one standard deviation of zero. A normal \
             distribution puts 0.68 there and a uniform one of the same width puts 0.58, so \
             this is not the bell curve SPEC section 7 asked for"
        );

        // The bounds this module decided to hold drifting genes inside.
        let at_the_wall = Genome::new(
            vec![Gene {
                stiffness: MAX_STIFFNESS,
                ..a_quiet_gene()
            }],
            &limits,
        );
        let mut came_back_down = false;
        for _ in 0..2_000 {
            let stiffness = mutate(&at_the_wall, &rates, &limits, &mut rng).genes()[0].stiffness;
            assert!(
                (0.0..=MAX_STIFFNESS).contains(&stiffness),
                "a point mutation walked stiffness to {stiffness}, outside 0..={MAX_STIFFNESS}. \
                 That bound is a hundredth of the 14,400 at which the physics' own integrator \
                 diverges at a sixtieth of a second, and the two orders of magnitude are \
                 there precisely so that a drifting lineage cannot walk to the cliff"
            );
            came_back_down |= stiffness < MAX_STIFFNESS;
        }
        assert!(
            came_back_down,
            "a gene held at the stiffness bound never moved off it, so the field is frozen \
             rather than bounded"
        );

        // Without this the whole test could be the work of an operator that ignores its rate.
        let never = nothing_happens();
        let mut quiet_rng = organism_rng(42, 1);
        for _ in 0..64 {
            assert_eq!(
                mutate(&genome, &never, &limits, &mut quiet_rng),
                genome,
                "a genome mutated with every rate at zero came back changed"
            );
        }
    }

    /// Three genes whose triggers and whose handed-out states have **nothing in common**.
    ///
    /// The fixture the two distribution tests below are both written against, and the disjointness
    /// is the whole of what makes them readable: a re-drawn state that comes back in `{1, 2, 3}`
    /// can only have come from the *answered* half of the alphabet, and one that comes back in
    /// `{0, 20..=25}` can only have come from the *occupied* half. Anything else came from the
    /// uniform tail.
    fn two_disjoint_alphabets() -> Vec<Gene> {
        (0..3_u8)
            .map(|n| Gene {
                // answered: 1, 2, 3
                trigger_state: State::new(1 + n),
                action: Action::Divide,
                // occupied: 20, 22, 24 and 21, 23, 25 — and 0, which the seed cell is in
                child_state: State::new(20 + 2 * n),
                new_state: State::new(21 + 2 * n),
                ..a_quiet_gene()
            })
            .collect()
    }

    /// Every state written anywhere in `two_disjoint_alphabets`, plus the seed cell's.
    const WHOLE_ALPHABET: [u8; 10] = [0, 1, 2, 3, 20, 21, 22, 23, 24, 25];

    /// Which of a mutant's three state fields moved, and where each landed.
    ///
    /// A re-draw that lands on the value that was already there is invisible, and that is fine
    /// and is accounted for where the expected proportions are worked out: the parent's own
    /// states are, by construction, in the *other* half of the alphabet from the one each field
    /// is biased towards, so the invisible draws are all draws this test would have counted as
    /// misses.
    fn where_the_states_landed(parent: &[Gene], child: &[Gene]) -> (Vec<u8>, Vec<u8>) {
        let mut triggers = Vec::new();
        let mut handed_out = Vec::new();

        for (was, now) in parent.iter().zip(child) {
            if was.trigger_state != now.trigger_state {
                triggers.push(now.trigger_state.get());
            }
            if was.child_state != now.child_state {
                handed_out.push(now.child_state.get());
            }
            if was.new_state != now.new_state {
                handed_out.push(now.new_state.get());
            }
        }

        (triggers, handed_out)
    }

    /// Twenty thousand reproductions with nothing but point mutation firing, and every state
    /// field that moved, sorted into the two halves it could have come from.
    fn state_redraws(parent: &[Gene], limits: &LimitsConfig) -> (Vec<u8>, Vec<u8>) {
        let rates = mutation_with(|rates| {
            rates.point_rate = 1.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let genome = Genome::new(parent.to_vec(), limits);
        let mut rng = organism_rng(42, 0);

        let mut triggers = Vec::new();
        let mut handed_out = Vec::new();
        for _ in 0..20_000 {
            let child = mutate(&genome, &rates, limits, &mut rng);
            let (mine, theirs) = where_the_states_landed(parent, child.genes());
            triggers.extend(mine);
            handed_out.extend(theirs);
        }

        (triggers, handed_out)
    }

    /// What share of these states is in that set.
    fn share_inside(landed: &[u8], set: &[u8]) -> f64 {
        let inside = landed.iter().filter(|state| set.contains(state)).count();
        let inside = u32::try_from(inside).expect("a sample count of thousands is a word");
        let total = u32::try_from(landed.len()).expect("a sample count of thousands is a word");

        f64::from(inside) / f64::from(total)
    }

    /// ⭐⭐ **A re-drawn `trigger_state` lands on a state some cell of the body actually is
    /// in** — three times in four, and not always.
    ///
    /// This is the half of the change that makes duplicate-and-diverge pay. SPEC section 7 rests
    /// the entire genome design on *"duplicating a gene and changing its `trigger_state` creates a
    /// new body part"*, and measured over 6.46 million cell-observations of the shipped world only
    /// **2.2%** of grown cells sat in a state their own genome named. A copy pointed at a state no
    /// cell is in fires nowhere, so the operator the project is built on was landing on nothing
    /// ninety-eight times in a hundred.
    ///
    /// The fixture's two halves are disjoint on purpose — see [`two_disjoint_alphabets`] — so
    /// every re-draw can be attributed. Three claims, and the second and third are why this is a
    /// **mixture** rather than a replacement:
    ///
    /// **It mostly lands where cells are.** `0.75 + 0.25 × 7/64 = 0.777` of all draws, which is
    /// `0.780` of the ones that visibly moved.
    ///
    /// **It does not always.** A quarter of re-draws still go anywhere at all, which is what
    /// leaves a gene able to be switched *off* — SPEC section 7 calls non-firing genes the raw
    /// material duplication feeds on — and what stops every cell in every body becoming
    /// developmentally live and every organism becoming a 64-cell blob.
    ///
    /// **And it can still leave the genome's alphabet entirely**, which is the same claim seen
    /// from outside: about a fifth of re-draws name a state the genome does not mention anywhere.
    ///
    /// The mask itself is checked first, because every proportion below is a fact about it.
    #[test]
    fn a_re_drawn_trigger_state_lands_on_a_state_some_cell_is_in() {
        let limits = spec_limits();
        let parent = two_disjoint_alphabets();
        let alphabet = Alphabet::of(&parent);

        let occupied: Vec<u8> = (0..State::COUNT)
            .filter(|state| alphabet.occupied & (1 << state) != 0)
            .collect();
        assert_eq!(
            occupied,
            vec![0, 20, 21, 22, 23, 24, 25],
            "the states a body could be in are the ones its genes hand out, and state 0, which \
             SPEC section 7's development puts the seed cell in without any gene naming it. \
             Leave state 0 out and a genome whose genes hand out only state 5 would draw every \
             trigger onto 5, nothing would answer to the seed cell, and every body in that \
             lineage would be a single cell"
        );

        let answered: Vec<u8> = (0..State::COUNT)
            .filter(|state| alphabet.answered & (1 << state) != 0)
            .collect();
        assert_eq!(answered, vec![1, 2, 3], "the states its genes answer to");

        let (triggers, _) = state_redraws(&parent, &limits);
        assert!(
            triggers.len() > 3_000,
            "only {} trigger states moved in twenty thousand reproductions of a three-gene \
             genome, where one field in sixteen of every gene is re-drawn - so this test has \
             almost no evidence in it",
            triggers.len()
        );

        let landed_on_a_cell = share_inside(&triggers, &occupied);
        assert!(
            (0.72..=0.84).contains(&landed_on_a_cell),
            "{landed_on_a_cell} of re-drawn trigger states named a state some cell is in, where \
             the mixture asks for 0.78. A uniform draw over the sixty-four would give 7/64 = \
             0.11, which is the 2.2% that made development stop at nearly every cell it visited"
        );

        let left_the_alphabet = 1.0 - share_inside(&triggers, &WHOLE_ALPHABET);
        assert!(
            (0.12..=0.30).contains(&left_the_alphabet),
            "{left_the_alphabet} of re-drawn trigger states named a state this genome does not \
             mention anywhere, where the quarter that is drawn uniformly asks for about 0.21. \
             Nought would mean this is a replacement rather than a mixture: a gene could never be \
             pointed at a state nothing occupies, so it could never be switched off, and the \
             neutral material SPEC section 7 says duplication feeds on would have been deleted \
             by an operator"
        );
    }

    /// ⭐⭐ **A re-drawn `child_state` or `new_state` can still name a state nothing answers
    /// to** — three times in four, which is the opposite way round from the field above.
    ///
    /// The two fields are not the same kind of thing and this is the test that says so. A
    /// `trigger_state` reaches *for* a cell; a `child_state` is the identity a gene **hands out**,
    /// and it is the only way the set of addressable identities ever grows. Bias it the way the
    /// trigger is biased and a genome could only ever hand out names it already answers to: no
    /// state nothing yet names could ever be minted, no new body part could ever be invented, and
    /// a lineage would be trapped inside the three or four states its founder happened to be
    /// given. **That is a worse failure than small bodies**, because small bodies are slow and a
    /// closed alphabet is the design not working at all.
    ///
    /// So the minority case is the biased one, and it is there for a real reason rather than as a
    /// hedge: `0.25 + 0.75 × 3/64 = 0.285` of these re-draws hand a daughter a name some rule
    /// already answers to, which is a daughter development can carry on with, and that is the
    /// other end of how a body gets deeper than two cells.
    #[test]
    fn a_re_drawn_child_state_can_still_name_a_state_nothing_answers_to() {
        let limits = spec_limits();
        let parent = two_disjoint_alphabets();
        let answered = [1_u8, 2, 3];

        let (_, handed_out) = state_redraws(&parent, &limits);
        assert!(
            handed_out.len() > 6_000,
            "only {} handed-out states moved in twenty thousand reproductions, and there are two \
             such fields per gene - so this test has almost no evidence in it",
            handed_out.len()
        );

        let named_a_rule = share_inside(&handed_out, &answered);
        assert!(
            (0.24..=0.34).contains(&named_a_rule),
            "{named_a_rule} of re-drawn child and new states named a state some gene already \
             answers to, where the mixture asks for 0.29. This is deliberately the minority case: \
             it is what lets a body grow past its first daughter, and it must stay a minority or \
             the set of identities a lineage can address stops growing"
        );

        let minted_something_new = 1.0 - share_inside(&handed_out, &WHOLE_ALPHABET);
        assert!(
            minted_something_new > 0.55,
            "only {minted_something_new} of re-drawn child and new states named a state this \
             genome does not mention anywhere. A lineage that cannot hand out a name nothing yet \
             answers to can never invent a body part, and the genome collapses onto the closed set \
             of states it started with - which is a worse outcome than the small bodies this whole \
             change is about"
        );
    }

    /// ⭐⭐ **A lineage now finds the gene it was already carrying.**
    ///
    /// The consequence the whole change is for, made concrete. The genome here is the shape every
    /// genome in this world has been in since Phase 3: **a perfectly good growth gene that nothing
    /// is ever in the state of**. Its first gene turns the seed cell into state 1 and stops, so
    /// the body is one cell; its second gene would divide a cell in state 40 into more cells in
    /// state 40, for ever, and no cell is ever in state 40. The two are one number apart and the
    /// number is a name.
    ///
    /// Two mutations reach it and both are re-draws of a state: point the second gene's
    /// `trigger_state` at a state a cell is in, or make the first gene hand out the name the
    /// second answers to. **Under a uniform re-draw each is one chance in sixty-four**, on one
    /// field in sixteen. Under the mixture the first is one in five and the second one in seven,
    /// because the alphabet this genome writes holds four states and two.
    ///
    /// The criterion is a body of **more than two** cells rather than more than one, and that is
    /// deliberate: the other way this genome can grow is a re-drawn `action`, which turns its first
    /// gene into a divider and gives a body of exactly two. Ruling that out is what leaves the
    /// count a count of the addressing.
    ///
    /// Measured over a thousand lineages of twenty-four point mutations each: **207** find it,
    /// against **68** under the uniform re-draw this replaces — the same fixture, the same seed,
    /// the same generations, with only the three match arms in [`point_mutate`] put back. Three
    /// times as many, and eleven standard deviations apart, so the band below sits between the two
    /// and far enough from both to be about the operator rather than about the seed.
    ///
    /// ⚠️ **Three times rather than the thirty the arithmetic above suggests**, and the difference
    /// is worth knowing: a lineage of twenty-four mutations has many roads to a larger body that
    /// have nothing to do with addressing — a re-drawn action, a re-drawn step window, a second
    /// mutation rescuing the first — and 68 of the 1,000 find one of those. What the mixture adds
    /// is the 139 that find the *gene they were carrying*.
    #[test]
    fn a_lineage_now_finds_a_body_that_uniform_re_draws_did_not() {
        let limits = spec_limits();
        let only_point_mutations = mutation_with(|rates| {
            rates.point_rate = 1.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });

        let stunted = Genome::new(
            vec![
                // Turns the seed cell into state 1 on the first step, and nothing answers to 1.
                Gene {
                    trigger_state: State::ZERO,
                    min_step: 0,
                    max_step: 0,
                    action: Action::Differentiate,
                    new_kind: CellKind::Photocyte,
                    new_state: State::new(1),
                    child_state: State::new(55),
                    ..a_quiet_gene()
                },
                // A growth gene with nothing to grow on.
                Gene {
                    trigger_state: State::new(40),
                    min_step: 0,
                    max_step: 15,
                    action: Action::Divide,
                    adhere: true,
                    child_state: State::new(40),
                    child_kind: CellKind::Photocyte,
                    rest_length: 8.0,
                    stiffness: 10.0,
                    ..a_quiet_gene()
                },
            ],
            &limits,
        );
        assert_eq!(
            develop(&stunted, &limits).cells.len(),
            1,
            "this genome is supposed to grow a single cell while carrying a gene that would fill \
             a body, so that what the lineages below find is the connection between the two"
        );

        const LINEAGES: u64 = 1_000;
        let mut found = 0_u32;
        for lineage in 0..LINEAGES {
            let mut rng = organism_rng(42, lineage);
            let mut genome = stunted.clone();
            let mut largest = 1;

            for _ in 0..12 {
                genome = mutate(&genome, &only_point_mutations, &limits, &mut rng);
                largest = largest.max(develop(&genome, &limits).cells.len());
            }

            found += u32::from(largest > 2);
        }

        assert!(
            (140..=350).contains(&found),
            "{found} of {LINEAGES} lineages grew a body of more than two cells in twenty-four \
             point mutations. The uniform re-draw this replaces manages 68, because a gene and a \
             cell had to agree about one number out of sixty-four; the mixture measures 207. Far \
             below the band means the bias is not reaching the operator, and far above it means \
             the re-draw has stopped being a mixture - a genome whose every gene fires is a body \
             that runs straight into max_cells_per_organism"
        );
    }

    /// ⭐ Duplication copies one gene and puts the copy immediately after it — and the
    /// organism this produces is, for the moment, exactly the organism it came from.
    ///
    /// This is the operator the whole genome design exists for, and the second half of that
    /// sentence is the part worth dwelling on. First-match-wins means a copy sitting directly
    /// behind its original can *never* fire: anything the copy answers to, the original
    /// answers to first. So a duplication changes the genome and changes nothing about the
    /// body — it is invisible to selection on the day it happens.
    ///
    /// That is precisely what makes it useful. A copy that did something would be a copy
    /// selection could punish, and it would be trimmed back out. A copy that does nothing is
    /// free, and free is the condition under which a piece of a genome can wander somewhere
    /// new. `a_duplicated_gene_that_diverges_is_a_new_body_part` is where it wanders.
    ///
    /// **After, not before.** The copy goes behind the original rather than in front of it,
    /// and the two are not interchangeable: in front, the *copy* would be the expressed gene
    /// and the original the silent one, so the gene under selection would be the one that had
    /// just been made rather than the one that has been working. Behind is what leaves the
    /// organism unchanged.
    ///
    /// The genome that comes back is checked against every genome that *would* be the parent
    /// with one gene copied next to itself, rather than by looking for a duplicate somewhere
    /// in it. An operator that copied a gene to the far end would satisfy the looser check
    /// and would be a different operator.
    ///
    /// Finally, at the cap the genome comes back **identical**, which is a stronger claim
    /// than "the same length": an implementation that inserted the copy and then trimmed the
    /// genome back to size would pass a length check while eating the gene at the far end —
    /// the neutral material this whole operator feeds on. See SPEC section 7.
    #[test]
    fn gene_duplication_copies_a_gene_next_to_itself() {
        let limits = spec_limits();
        let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap fits in a word");
        let parent = numbered_genes(5);

        // Every genome that counts as "the parent with one gene copied next to itself".
        let copied_next_to_itself: Vec<Vec<Gene>> = (0..parent.len())
            .map(|at| {
                let mut copy = parent.clone();
                copy.insert(at + 1, parent[at]);
                copy
            })
            .collect();

        let mut chosen = vec![0_u32; parent.len()];
        let mut rng = organism_rng(42, 0);
        for _ in 0..1_000 {
            let mut genes = parent.clone();
            duplicate_a_gene(&mut genes, cap, &mut rng);

            let at = copied_next_to_itself
                .iter()
                .position(|candidate| *candidate == genes)
                .expect(
                    "duplication produced a genome that is not the parent with one of its \
                     genes copied immediately after itself",
                );
            chosen[at] += 1;
        }

        for (gene, times) in chosen.iter().enumerate() {
            assert!(
                *times > 0,
                "gene {gene} was never the one copied in a thousand duplications, so the \
                 operator copies a fixed gene rather than a random one and most of a genome \
                 is unreachable by duplication"
            );
        }

        // The organism is unchanged by having been duplicated.
        let mut genes = parent.clone();
        duplicate_a_gene(&mut genes, cap, &mut rng);
        assert_eq!(
            develop(&Genome::new(genes, &limits), &limits),
            develop(&Genome::new(parent.clone(), &limits), &limits),
            "duplicating a gene changed the body, so the copy is being expressed instead of \
             sitting silently behind its original — which makes every duplication something \
             selection can see and punish"
        );

        // And the operator is actually wired into the mutation as a whole. Every other rate is
        // zero, reordering included, so the genome that comes back has to be one of the
        // genomes the operator above is allowed to produce - not merely one holding the right
        // genes in some order.
        let rates = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 1.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let genome = Genome::new(parent.clone(), &limits);
        let child = mutate(&genome, &rates, &limits, &mut rng);

        assert!(
            copied_next_to_itself
                .iter()
                .any(|candidate| candidate == child.genes()),
            "a reproduction did not produce the parent with one of its genes copied \
             immediately after itself, so the operator above is not the one a reproduction \
             reaches"
        );

        // At the cap. A genome that is full comes back exactly as it went in - identical,
        // rather than merely the same length or holding the same genes.
        let tight = limits_with(|limits| limits.max_genes = 4);
        let full = Genome::new(numbered_genes(4), &tight);
        for _ in 0..64 {
            assert_eq!(
                mutate(&full, &rates, &tight, &mut rng),
                full,
                "a genome at max_genes came back changed after a duplication. At the cap a \
                 lengthening mutation fails; it does not truncate, because truncating eats the \
                 far end of the genome, which is where the material duplication feeds on lives"
            );
        }
    }

    /// ⭐⭐ **The one the project rests on.** A gene is duplicated; the copy is pointed at a
    /// different developmental state; and the organism grows a limb it did not have.
    ///
    /// CLAUDE.md's decision log opens by saying that a fixed list of numbers can only ever
    /// evolve a better single cell, because there is no slot in it for a thing that does not
    /// exist yet, and that duplicate-then-diverge is what lets a lineage *gain* structure.
    /// This test is that claim, made concrete enough to fail.
    ///
    /// # The organism before
    ///
    /// Four genes, each firing on one step only, each handing its daughter a fresh state. The
    /// result is a **stalk**: a photocyte buds a photocyte, which buds a photocyte, which buds
    /// a photocyte, and the last of them buds a single **myocyte at right angles** — one
    /// contractile cell on the end of a four-cell stem. Five cells, four springs, and every
    /// cell joined to at most two others: a chain, with a bend at the far end.
    ///
    /// The tip gene is the interesting one. It is the only gene that turns, and the only gene
    /// that makes a cell of a different kind, so it is the closest thing this organism has to
    /// a description of an organ: *"grow a muscle out sideways from here"*.
    ///
    /// # What is done to it
    ///
    /// Two mutations, in the order SPEC section 7 applies them, and both are operators from
    /// this file rather than edits invented for the test.
    ///
    /// First, the real duplication operator runs until it copies the tip gene. The genome is
    /// then five genes and the body is **exactly the same body** — the copy sits behind its
    /// original where first-match-wins can never reach it. Nothing has happened yet, and that
    /// is the point: this is a change no selection pressure anywhere can see.
    ///
    /// Then one field of the copy changes: its `trigger_state`, from the state of the stalk's
    /// tip to the state of its *first* segment. That is one point mutation — the operator
    /// above, landing on one of sixteen fields — and it is written out by hand rather than
    /// waited for, because a uniform re-draw finds this particular state one time in
    /// sixty-four. What is being demonstrated is the consequence of the divergence, not the
    /// arrival of it.
    ///
    /// # The organism after
    ///
    /// Six cells. The stalk is still there, unchanged, with its myocyte still on the end. But
    /// the first segment of the stalk — which in the parent was a plain length of stem with a
    /// cell above and a cell below — now **also** buds a myocyte out sideways, into a space
    /// the parent's body had nothing anywhere near.
    ///
    /// So the mutant has:
    ///
    /// - a cell with **three** springs on it, where every cell of the parent had at most two.
    ///   The parent is a chain. The mutant has a **fork**, and a fork is not a longer chain or
    ///   a differently-bent chain — it is a structure a chain cannot be.
    /// - **two** myocytes where the parent had one, in the same relationship to their stems.
    ///   The organ was not invented from nothing; it was *repeated*, which is what the whole
    ///   design predicts and is how essentially all real biological complexity was built.
    ///
    /// And it took one copy and one changed field to get there. That is the bet the project
    /// is making, and this is it paying out.
    #[test]
    fn a_duplicated_gene_that_diverges_is_a_new_body_part() {
        let limits = spec_limits();
        let arm = 10.0;

        // The stalk: three plain segments and a myocyte budded sideways off the end.
        let segment = |generation: u8, turn: f32, kind: CellKind| Gene {
            trigger_state: State::new(generation),
            min_step: generation,
            max_step: generation,
            action: Action::Divide,
            angle: turn,
            adhere: true,
            child_state: State::new(generation + 1),
            child_kind: kind,
            rest_length: arm,
            stiffness: 1.0,
            ..a_quiet_gene()
        };
        let stalk = vec![
            segment(0, 0.0, CellKind::Photocyte),
            segment(1, 0.0, CellKind::Photocyte),
            segment(2, 0.0, CellKind::Photocyte),
            segment(3, FRAC_PI_2, CellKind::Myocyte),
        ];
        const TIP: usize = 3;

        let parent = Genome::new(stalk.clone(), &limits);
        let before = develop(&parent, &limits);

        assert_eq!(
            before.cells.len(),
            5,
            "the stalk is four segments and a muscle"
        );
        assert_eq!(
            before
                .cells
                .iter()
                .filter(|cell| cell.kind == CellKind::Myocyte)
                .count(),
            1,
            "the parent has exactly one muscle, on the end of its stalk"
        );
        assert!(
            springs_per_cell(&before).iter().all(|joins| *joins <= 2),
            "the parent's body is supposed to be a chain, and a chain has no cell joined to \
             three others: {:?}",
            springs_per_cell(&before)
        );

        // --- the duplication, by the real operator -----------------------------------
        //
        // It copies a gene at random, so it is asked repeatedly until it copies the tip gene.
        // Asking rather than pinning a seed is what keeps this test about the operator: a
        // fixed seed would have to be re-chosen every time anything upstream of it changed
        // the order numbers are drawn in.
        let mut copied = stalk.clone();
        copied.insert(TIP + 1, stalk[TIP]);

        let duplication_only = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 1.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let duplicated = (0..64_u64)
            .find_map(|serial| {
                let mut rng = organism_rng(42, serial);
                let child = mutate(&parent, &duplication_only, &limits, &mut rng);
                (child.genes() == copied).then_some(child)
            })
            .expect(
                "sixty-four reproductions never once copied the tip gene next to itself, so \
                 there is nothing here to diverge and the project's central operator is not \
                 working",
            );

        assert_eq!(
            develop(&duplicated, &limits),
            before,
            "duplicating the tip gene changed the body. The copy is supposed to be silent - \
             invisible to selection, and therefore free - until something diverges it"
        );

        // --- the divergence: one field of the copy ------------------------------------
        let mut diverged = duplicated.genes().to_vec();
        diverged[TIP + 1].trigger_state = State::new(1);
        assert_eq!(
            diverged[TIP + 1],
            Gene {
                trigger_state: State::new(1),
                ..stalk[TIP]
            },
            "the diverged gene differs from the one it was copied from in more than the one \
             field a point mutation changes, so this test is doing more than it claims"
        );

        let after = develop(&Genome::new(diverged, &limits), &limits);

        // --- what the body gained ------------------------------------------------------
        assert_eq!(
            after.cells.len(),
            before.cells.len() + 1,
            "the diverged copy grew no cell at all, so it fires nowhere and nothing has been \
             gained"
        );

        let forks: Vec<usize> = springs_per_cell(&after)
            .into_iter()
            .enumerate()
            .filter(|(_, joins)| *joins > 2)
            .map(|(cell, _)| cell)
            .collect();
        assert_eq!(
            forks,
            vec![1],
            "the mutant body has no cell joined to three others, so it is still a chain. A \
             chain that grew a cell is a longer chain; what duplicate-and-diverge is supposed \
             to produce is a body plan the parent could not have - and a branch is one"
        );

        let muscles: Vec<Vec2> = after
            .cells
            .iter()
            .filter(|cell| cell.kind == CellKind::Myocyte)
            .map(|cell| cell.offset)
            .collect();
        assert_eq!(
            muscles.len(),
            2,
            "the parent had one muscle and the mutant should have two: the same organ, grown \
             a second time somewhere else, which is the whole claim"
        );

        // The new muscle stands off the first segment of the stalk, at right angles, exactly
        // as the original stands off the last one.
        let new_muscle = Vec2::new(arm, arm);
        assert!(
            muscles.iter().any(|at| (*at - new_muscle).length() < 1e-4),
            "no cell grew at {new_muscle:?}, where the diverged gene asks for one: {muscles:?}"
        );
        assert!(
            before
                .cells
                .iter()
                .all(|cell| (cell.offset - new_muscle).length() > arm / 2.0),
            "the parent already had a cell where the mutant's new muscle is, so the mutant is \
             not reaching anywhere its parent could not"
        );

        // And the stalk is still the stalk. Divergence added a part; it did not rearrange the
        // organism into a different one.
        for cell in &before.cells {
            assert!(
                after.cells.contains(cell),
                "a cell of the parent's body is missing from the mutant's, so this mutation \
                 replaced structure rather than adding to it"
            );
        }
    }

    /// ⭐⭐ **Every state a body reaches is one its genome writes down.**
    ///
    /// The claim that makes reading the alphabet off the *gene list* legitimate rather than merely
    /// cheap. The exact answer to "which states are occupied" is what a development pass would
    /// report, and a pass costs the whole of `develop` per reproduction on top of the one
    /// reproduction already does for the child. What is used instead is a **superset** of it, and
    /// the reason it is one is structural: a cell's state is written in exactly two places — a
    /// gene's `child_state` when it is budded and a gene's `new_state` when it is re-made — and
    /// the one cell neither of those touches is the seed cell, which SPEC section 7 puts in state
    /// 0.
    ///
    /// So [`Alphabet::occupied`] can *over*-report — by naming a state that is written down and
    /// never reached, because of a step window or because a gene in front of it answers first —
    /// and it can never under-report. That is the right direction: a `trigger_state` drawn onto a
    /// written-but-unreached state is a gene that does not fire, which is an ordinary silent gene;
    /// a state that a cell is in and the alphabet had not heard of would be a cell the operator
    /// could never point anything at, and this test would be the only thing that could ever say so.
    ///
    /// Five hundred genomes, each built from genes the real [`Gene::random`] drew and then squeezed
    /// onto a four-state alphabet so that genes actually fire — a genome of genes drawn over all
    /// sixty-four states usually grows one cell and would make this test a comparison of empty
    /// bodies — and each of them checked again after eight generations of the real mutation
    /// operators, because the claim has to survive the thing that changes genomes.
    #[test]
    fn every_state_a_body_reaches_is_one_its_genome_writes_down() {
        let limits = spec_limits();
        let rates = mutation_with(|rates| {
            rates.point_rate = 1.0;
            rates.duplication_rate = 0.5;
            rates.deletion_rate = 0.2;
            rates.insertion_rate = 0.5;
            rates.reorder_rate = 0.5;
            rates.genome_duplication_rate = 0.1;
        });
        let mut rng = organism_rng(42, 0);
        let mut multicellular = 0_u32;

        for _ in 0..500 {
            let genes: Vec<Gene> = (0..4)
                .map(|_| {
                    let drawn = Gene::random(&mut rng, &limits);
                    Gene {
                        trigger_state: State::new(rng.random_range(0..4_u8)),
                        child_state: State::new(rng.random_range(0..4_u8)),
                        new_state: State::new(rng.random_range(0..4_u8)),
                        ..drawn
                    }
                })
                .collect();
            let mut genome = Genome::new(genes, &limits);

            for generation in 0..=8 {
                let alphabet = Alphabet::of(genome.genes());
                let body = develop(&genome, &limits);
                multicellular += u32::from(body.cells.len() > 1);

                for cell in &body.cells {
                    assert!(
                        alphabet.occupied & bit(cell.state) != 0,
                        "generation {generation}: a cell of this body is in state {}, which its \
                         own genome writes down nowhere. Every state a cell can be in is either \
                         state 0, which development puts the seed cell in, or some gene's \
                         child_state or new_state - so an alphabet read off the gene list is a \
                         superset of what a body occupies, and this says it is",
                        cell.state.get()
                    );
                }

                genome = mutate(&genome, &rates, &limits, &mut rng);
            }
        }

        assert!(
            multicellular > 1_000,
            "only {multicellular} of the 4,500 bodies grown here had more than one cell in them, \
             so this test has been comparing seed cells with seed cells"
        );
    }

    /// Deletion takes out one gene and leaves the rest where they were.
    ///
    /// The counterweight to duplication, and the reason the cap is survivable: a lineage that
    /// has filled its genome can still shrink, and having shrunk can grow again. Without
    /// deletion the cap would be a one-way door and a saturated lineage would be frozen.
    ///
    /// Order matters as much here as it does for duplication. First-match-wins means removing
    /// a gene can bring a gene *behind* it to life — the silent copy that has been drifting
    /// for a thousand generations is one deletion away from being the gene that fires — so an
    /// implementation that quietly re-sorted what was left would be destroying information
    /// that took a lineage its whole history to accumulate.
    ///
    /// An empty genome is a genome with nothing to delete, and that is checked here because
    /// it is the one input this operator could crash on rather than decline.
    #[test]
    fn gene_deletion_removes_one() {
        let limits = spec_limits();
        let parent = numbered_genes(5);

        let with_one_missing: Vec<Vec<Gene>> = (0..parent.len())
            .map(|gone| {
                let mut left = parent.clone();
                left.remove(gone);
                left
            })
            .collect();

        let mut chosen = vec![0_u32; parent.len()];
        let mut rng = organism_rng(42, 0);
        for _ in 0..1_000 {
            let mut genes = parent.clone();
            delete_a_gene(&mut genes, &mut rng);

            let gone = with_one_missing
                .iter()
                .position(|candidate| *candidate == genes)
                .expect(
                    "deletion produced a genome that is not the parent with exactly one gene \
                     taken out of it and the others left in their order",
                );
            chosen[gone] += 1;
        }

        for (gene, times) in chosen.iter().enumerate() {
            assert!(
                *times > 0,
                "gene {gene} was never the one deleted in a thousand deletions, so the \
                 operator removes a fixed gene rather than a random one"
            );
        }

        // Nothing to delete, and nothing to crash on.
        let mut empty: Vec<Gene> = Vec::new();
        delete_a_gene(&mut empty, &mut rng);
        assert!(empty.is_empty(), "a gene appeared in an empty genome");

        // Wired into a reproduction.
        let rates = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 1.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let child = mutate(
            &Genome::new(parent.clone(), &limits),
            &rates,
            &limits,
            &mut rng,
        );
        assert_eq!(
            child.genes().len(),
            parent.len() - 1,
            "mutation as a whole deletes nothing, so a genome can only ever grow"
        );

        // And a genome can be deleted away to nothing without anything going wrong. A lineage
        // whose genome has emptied is a single photocyte that still reproduces - SPEC section
        // 7's genome with no genes - rather than an error.
        let mut dwindling = Genome::new(parent, &limits);
        for _ in 0..16 {
            dwindling = mutate(&dwindling, &rates, &limits, &mut rng);
        }
        assert!(
            dwindling.genes().is_empty(),
            "a genome deleted from sixteen times over kept five genes"
        );
        assert_eq!(
            develop(&dwindling, &limits).cells.len(),
            1,
            "a genome with nothing left in it should grow SPEC section 7's single seed cell"
        );
    }

    /// Insertion puts a gene nobody has seen before into the genome, anywhere in it.
    ///
    /// This is the operator that supplies *novelty* rather than variations on what a lineage
    /// already has, and it is the one that keeps a population from being trapped: a genome
    /// whose genes all answer to the same three states can reach a fourth only by drawing one.
    ///
    /// **Anywhere, not on the end.** SPEC section 7 says "insert a fully random gene" and does
    /// not say where, which matters because first-match-wins makes position meaning. Appending
    /// would make every inserted gene the *last* thing consulted, so it could only ever be
    /// expressed on a state no existing gene claims - insertion would be an operator that
    /// mostly inserts silence. A uniformly chosen position is the reading with no thumb on the
    /// scale: sometimes the new gene takes over a state, sometimes it lands behind a gene that
    /// already answers to it and waits.
    ///
    /// The genes that were already there must all still be there, in their order, because an
    /// operator that inserted a gene *over* one would be a deletion and an insertion at once
    /// and no test that counted genes would notice.
    ///
    /// At the cap it fails, and the genome comes back identical rather than merely the same
    /// length - see `gene_duplication_copies_a_gene_next_to_itself` for why that distinction
    /// is the whole of SPEC section 7's rule.
    #[test]
    fn gene_insertion_adds_a_random_one() {
        let limits = spec_limits();
        let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap fits in a word");
        let parent = numbered_genes(5);
        let mut rng = organism_rng(42, 0);

        let mut positions = vec![0_u32; parent.len() + 1];
        let mut arrivals: Vec<Gene> = Vec::new();
        for _ in 0..1_000 {
            let mut genes = parent.clone();
            insert_a_random_gene(&mut genes, cap, &limits, &mut rng);

            assert_eq!(
                genes.len(),
                parent.len() + 1,
                "insertion did not add exactly one gene"
            );

            // The one position where the genome stopped matching the parent is where the new
            // gene went in; everything on either side of it must be untouched.
            let at = (0..genes.len())
                .find(|i| parent.get(*i) != Some(&genes[*i]))
                .expect("a genome one gene longer than its parent must differ somewhere");
            let mut without = genes.clone();
            let arrived = without.remove(at);
            assert_eq!(
                without, parent,
                "taking the new gene back out did not leave the parent's genes in the order \
                 they were in, so insertion moved or overwrote something as well as adding"
            );

            positions[at] += 1;
            arrivals.push(arrived);
        }

        for (slot, times) in positions.iter().enumerate() {
            assert!(
                *times > 0,
                "no gene was ever inserted at position {slot} of a thousand tries, so \
                 insertion cannot reach every part of a genome and where a gene lands is not \
                 uniform"
            );
        }

        assert!(
            arrivals.windows(2).any(|pair| pair[0] != pair[1]),
            "a thousand insertions all inserted the same gene, so 'a fully random gene' is one \
             fixed gene"
        );
        for gene in &arrivals {
            assert!(
                gene.max_step < u8::try_from(limits.max_dev_steps.get()).unwrap_or(u8::MAX)
                    && gene.min_step <= gene.max_step,
                "an inserted gene names a step window of {}..={} that this run never reaches, \
                 so insertion mostly inserts nothing",
                gene.min_step,
                gene.max_step
            );
        }

        // Wired into a reproduction.
        let rates = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 1.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let genome = Genome::new(parent.clone(), &limits);
        let child = mutate(&genome, &rates, &limits, &mut rng);
        assert_eq!(
            child.genes().len(),
            parent.len() + 1,
            "mutation as a whole inserts nothing, so no lineage can gain a gene it did not \
             already have a copy of"
        );

        // At the cap.
        let tight = limits_with(|limits| limits.max_genes = 4);
        let full = Genome::new(numbered_genes(4), &tight);
        for _ in 0..64 {
            assert_eq!(
                mutate(&full, &rates, &tight, &mut rng),
                full,
                "a genome at max_genes gained a gene, or was truncated to make room for one"
            );
        }
    }

    /// Reordering swaps one pair of neighbouring genes, and nothing else moves.
    ///
    /// It is a real mutation because of first-match-wins:
    /// `the_first_matching_gene_wins_so_gene_order_carries_information` in `development.rs`
    /// shows the same two genes in two orders growing two different organisms. Where a gene
    /// sits is part of what a genome *says*, so shuffling is a way of saying something else
    /// without changing a single number.
    ///
    /// It is also the operator that brings the silent back to life. A gene that has been
    /// sitting behind another one — drifting, costing nothing, invisible to selection — is one
    /// swap away from being the gene that fires. Duplication provides the material and this is
    /// what eventually reads it out.
    ///
    /// Two things are checked. The first is the operator itself: exactly one adjacent pair
    /// changes places, every pair is reachable, and nothing is added or lost. The second is
    /// that a reproduction actually performs it, about `mutation.reorder_rate` of the time —
    /// which is the only way to notice a wire that was never connected, since a missing
    /// operator here breaks no other test in this file. `reordering_can_be_switched_off` is
    /// the other end of the same claim.
    #[test]
    fn reordering_swaps_two_adjacent_genes() {
        let limits = spec_limits();
        let parent = numbered_genes(5);

        let with_one_pair_swapped: Vec<Vec<Gene>> = (0..parent.len() - 1)
            .map(|left| {
                let mut swapped = parent.clone();
                swapped.swap(left, left + 1);
                swapped
            })
            .collect();

        let mut chosen = vec![0_u32; parent.len() - 1];
        let mut rng = organism_rng(42, 0);
        for _ in 0..1_000 {
            let mut genes = parent.clone();
            swap_two_adjacent_genes(&mut genes, &mut rng);

            let pair = with_one_pair_swapped
                .iter()
                .position(|candidate| *candidate == genes)
                .expect(
                    "reordering produced a genome that is not the parent with exactly one \
                     neighbouring pair changed places",
                );
            chosen[pair] += 1;
        }

        for (pair, times) in chosen.iter().enumerate() {
            assert!(
                *times > 0,
                "the pair at position {pair} was never swapped in a thousand reorderings, so \
                 most of a genome's order is unreachable"
            );
        }

        // A genome with nothing to swap is left alone rather than crashed on.
        for short in 0..2_u8 {
            let mut few = numbered_genes(short);
            swap_two_adjacent_genes(&mut few, &mut rng);
            assert_eq!(
                few,
                numbered_genes(short),
                "a genome of {short} gene(s) came back changed by a swap it cannot perform"
            );
        }

        // Wired into a reproduction, and firing at the rate SPEC section 3 ships. Every other
        // rate is zero here, so anything that happens below is this operator and nothing else.
        let shipped = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.genome_duplication_rate = 0.0;
        });
        let genome = Genome::new(parent.clone(), &limits);
        const REPRODUCTIONS: u32 = 4_000;
        let mut reordered = 0_u32;
        for serial in 0..u64::from(REPRODUCTIONS) {
            let mut own_rng = organism_rng(7, serial);
            if mutate(&genome, &shipped, &limits, &mut own_rng).genes() != parent {
                reordered += 1;
            }
        }

        // 4,000 reproductions at one in fifty is eighty, give or take nine. The band is four
        // deviations wide either way, so it is about the rate rather than about the seed.
        assert!(
            (45..=125).contains(&reordered),
            "{reordered} of {REPRODUCTIONS} reproductions came back with their genes in a \
             different order, where a reorder_rate of {} asks for about 80. Zero means the \
             operator is never reached by a reproduction; far more means every genome is \
             being shuffled every generation",
            shipped.reorder_rate
        );
    }

    /// ⭐ Reordering can be **switched off**, which is the whole reason
    /// `mutation.reorder_rate` was added to SPEC section 3.
    ///
    /// Until it existed, this operator was the one thing in the file firing at a rate nobody
    /// could change: a `REORDER_RATE` constant, one reproduction in fifty, during every other
    /// operator's test. Every claim in this module about a genome at its cap had to be phrased
    /// in terms of which genes it *held* rather than what order they were in, the `bloom`,
    /// `famine` and `slow` profiles could not reach it, and the operator that decides which
    /// genes are expressed at all was the only one an experiment could not turn down.
    ///
    /// So: four thousand reproductions of a five-gene genome with every rate at zero, and not
    /// one of them may come back in a different order. The old constant would have shuffled
    /// about eighty of them, which is what makes this a test of the configured value actually
    /// reaching the operator rather than a test that nothing happens when nothing is asked
    /// for.
    ///
    /// The second half is what stops that being vacuous. The same genome, the same loop, the
    /// same everything-else-at-zero, and `reorder_rate` at certainty: now *every* reproduction
    /// comes back with a pair changed places. The key is a dial the operator reads, rather
    /// than a value that happens to be ignored in one direction.
    #[test]
    fn reordering_can_be_switched_off() {
        let limits = spec_limits();
        let parent = numbered_genes(5);
        let genome = Genome::new(parent.clone(), &limits);
        const REPRODUCTIONS: u64 = 4_000;

        let never = nothing_happens();
        for serial in 0..REPRODUCTIONS {
            let mut rng = organism_rng(7, serial);
            assert_eq!(
                mutate(&genome, &never, &limits, &mut rng),
                genome,
                "a genome mutated with reorder_rate at zero came back with its genes \
                 rearranged, so the operator is still firing at a rate of its own and the \
                 configuration cannot switch it off"
            );
        }

        let always = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 1.0;
            rates.genome_duplication_rate = 0.0;
        });
        for serial in 0..REPRODUCTIONS {
            let mut rng = organism_rng(7, serial);
            let child = mutate(&genome, &always, &limits, &mut rng);

            assert_ne!(
                child, genome,
                "a genome mutated with reorder_rate at one came back untouched, so the rate \
                 above is not the number the operator is reading"
            );
            assert_eq!(
                child.genes().len(),
                parent.len(),
                "reordering changed how many genes the genome holds"
            );
        }
    }

    /// Whole-genome duplication appends a second copy of everything — or, if that will not
    /// fit, does nothing whatever.
    ///
    /// The rarest operator SPEC gives, by a factor of twenty-five, and the largest. In real
    /// biology it is the event behind whole classes of complexity: every gene suddenly has a
    /// spare, and every spare is free to wander. Here it is the same move as gene duplication
    /// with the volume turned all the way up — the appended copy sits entirely behind the
    /// original, so first-match-wins gives it nothing to do and **the body does not change at
    /// all**. What changes is that the lineage now has twice as much material that costs it
    /// nothing to carry.
    ///
    /// **A genome more than half the cap fails entirely rather than appending as much as
    /// fits.** That is this module's reading of SPEC section 7's rule and it is worth being
    /// plain about why: half a copy of a genome is not a genome. Appending one would be a
    /// *different* operator from the one SPEC describes — it would take the front of the
    /// genome, which is the expressed part, and leave the back, which is the drifting part,
    /// and it would do so silently at exactly the moment a lineage was most successful.
    #[test]
    fn whole_genome_duplication_appends_a_full_copy() {
        let limits = spec_limits();
        let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap fits in a word");
        let parent = numbered_genes(5);

        let mut doubled = parent.clone();
        duplicate_the_whole_genome(&mut doubled, cap);
        assert_eq!(
            doubled,
            [parent.clone(), parent.clone()].concat(),
            "a whole-genome duplication is the genome, and then the genome again"
        );
        assert_eq!(
            develop(&Genome::new(doubled, &limits), &limits),
            develop(&Genome::new(parent.clone(), &limits), &limits),
            "doubling the whole genome changed the body, so the appended copy is being \
             expressed rather than carried"
        );

        // Nothing doubled is still nothing.
        let mut empty: Vec<Gene> = Vec::new();
        duplicate_the_whole_genome(&mut empty, cap);
        assert!(empty.is_empty());

        // Wired into a reproduction.
        let rates = mutation_with(|rates| {
            rates.point_rate = 0.0;
            rates.duplication_rate = 0.0;
            rates.deletion_rate = 0.0;
            rates.insertion_rate = 0.0;
            rates.reorder_rate = 0.0;
            rates.genome_duplication_rate = 1.0;
        });
        let mut rng = organism_rng(42, 0);
        let child = mutate(
            &Genome::new(parent.clone(), &limits),
            &rates,
            &limits,
            &mut rng,
        );
        assert_eq!(
            child.genes().len(),
            parent.len() * 2,
            "mutation as a whole never doubles a genome"
        );

        // More than half the cap: the whole operator fails. Three genes with room for five
        // would need six, so nothing at all should happen - not five genes, which is what an
        // implementation that appended "as much as fits" would leave behind.
        let tight = limits_with(|limits| limits.max_genes = 5);
        let three = Genome::new(numbered_genes(3), &tight);
        for _ in 0..64 {
            assert_eq!(
                mutate(&three, &rates, &tight, &mut rng),
                three,
                "a genome of three in a world that allows five came back changed after a \
                 whole-genome duplication. Half a copy of a genome is not a genome, and \
                 appending one is a different operator from the one SPEC section 7 describes"
            );
        }

        // Exactly half the cap still fits, because a cap you cannot reach is a lower cap with
        // nobody able to tell.
        let exactly_half = Genome::new(numbered_genes(2), &limits_with(|l| l.max_genes = 4));
        let room_for_four = limits_with(|limits| limits.max_genes = 4);
        assert_eq!(
            mutate(&exactly_half, &rates, &room_for_four, &mut rng)
                .genes()
                .len(),
            4,
            "a genome of two in a world that allows four refused to double, so the cap is one \
             gene tighter than the configuration says"
        );
    }

    /// An offspring's mutations depend on its parent's genome and its own serial number, and
    /// on nothing else in the world.
    ///
    /// Not on how many organisms were born before it, not on what order the world happened to
    /// visit them in, and — this is the one that matters — not on how many of the machine's
    /// sixteen threads Phase 4 decides to run them on. `rng.rs` proves that property for
    /// *numbers*; this proves it for the thing the numbers are used for, which is not the same
    /// claim: an implementation that drew from a generator handed round the simulation would
    /// pass every test in `rng.rs` and fail here.
    ///
    /// The evidence is the same organism's offspring produced four ways — twice from one
    /// world, once from a separately built world with the same seed, and once from a world
    /// where sixty-three other organisms have been reproducing hard in between. All four must
    /// be the same organism.
    ///
    /// Then a whole lineage rather than a single birth: a hundred generations of successive
    /// mutation, replayed, must come out identical down to the last gene. That is what makes
    /// a run watchable a second time, which SPEC section 2 says is the point of the seed.
    ///
    /// And the half without which none of it means anything: different organisms must get
    /// *different* mutations. An implementation that handed every organism the same generator
    /// would satisfy every claim above perfectly and would be a population of one organism
    /// wearing four thousand costumes.
    #[test]
    fn mutation_is_deterministic_from_the_organisms_own_stream() {
        let limits = spec_limits();

        // Every operator turned up, so that "the same" and "different" are both about
        // something substantial rather than about two genomes that were never mutated.
        let busy = mutation_with(|rates| {
            rates.point_rate = 1.0;
            rates.duplication_rate = 0.5;
            rates.deletion_rate = 0.5;
            rates.insertion_rate = 0.5;
            rates.reorder_rate = 0.5;
            rates.genome_duplication_rate = 0.1;
        });
        let parent = Genome::new(numbered_genes(6), &limits);

        let world = WorldRng::from_seed(20_260_731);
        let elsewhere = WorldRng::from_seed(20_260_731);
        let child_of = |rng: &mut ChaCha8Rng| mutate(&parent, &busy, &limits, rng);

        let first = child_of(&mut world.new_organism_stream(7));
        let again = child_of(&mut world.new_organism_stream(7));
        let from_elsewhere = child_of(&mut elsewhere.new_organism_stream(7));

        // Sixty-three other organisms live eventful lives, in an order chosen to be awkward.
        for serial in (0..64).rev().filter(|serial| *serial != 7) {
            let mut theirs = world.new_organism_stream(serial);
            let mut theirs_now = parent.clone();
            for _ in 0..32 {
                theirs_now = mutate(&theirs_now, &busy, &limits, &mut theirs);
            }
        }
        let after_the_crowd = child_of(&mut world.new_organism_stream(7));

        assert_eq!(
            first, again,
            "one organism reproduced twice into two organisms"
        );
        assert_eq!(
            first, from_elsewhere,
            "the same organism in two identically-seeded worlds had different offspring"
        );
        assert_eq!(
            first, after_the_crowd,
            "an organism's offspring changed because other organisms reproduced first, so the \
             run depends on the order the world is walked in and Phase 4 cannot spread it over \
             the machine's cores"
        );

        // A whole lineage, not a single birth.
        let lineage = |serial: u64| -> Vec<Genome> {
            let mut rng = world.new_organism_stream(serial);
            let mut now = parent.clone();
            (0..100)
                .map(|_| {
                    now = mutate(&now, &busy, &limits, &mut rng);
                    now.clone()
                })
                .collect()
        };
        assert_eq!(
            lineage(11),
            lineage(11),
            "a hundred generations of one lineage played twice came out differently, so \
             nothing interesting that happens in a run can ever be watched again"
        );

        // Without this the whole test is satisfied by handing everybody the same numbers.
        let offspring: Vec<Genome> = (0..32)
            .map(|serial| child_of(&mut world.new_organism_stream(serial)))
            .collect();
        for (serial, child) in offspring.iter().enumerate() {
            for (other_serial, other) in offspring.iter().enumerate().skip(serial + 1) {
                assert_ne!(
                    child, other,
                    "organisms {serial} and {other_serial} produced identical offspring from \
                     the same parent, so every lineage mutates in lockstep and the population \
                     is one organism in many costumes"
                );
            }
        }
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The tests above use genomes written by hand, each built to show one operator working.
    // This one uses genomes written by the machine and mutated ten thousand times over, which
    // is the only way to see what an operator that compounds actually does.
    // ---------------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4))]

        /// ⭐ **The critical one.** Ten thousand mutations at rates that would explode a
        /// genome, and the cap still holds and every field is still a number the rest of the
        /// simulation can use.
        ///
        /// CLAUDE.md marks `max_genes` as the one cap that must never be raised without a
        /// metabolic cost per gene, and the reason is arithmetic rather than taste: gene
        /// duplication compounds, and whole-genome duplication compounds faster. A lineage
        /// that duplicates faster than selection punishes it grows a genome into the megabytes
        /// and takes the process down with it, and no amount of memory outruns an exponential.
        /// SPEC section 15 asks for exactly this test by name.
        ///
        /// Every rate here is at certainty, including the whole-genome duplication that ships
        /// at 0.0008. That is not a plausible configuration; it is the worst one, which is the
        /// point. A genome under this treatment reaches the cap within the first handful of
        /// generations and then spends the remaining ten thousand pressed against it, which is
        /// precisely the state a real saturated lineage would be in and the state in which an
        /// off-by-one in the cap would show.
        ///
        /// Three claims, checked after **every** mutation rather than at the end, because a
        /// genome that spent one generation at 129 genes and came back is a genome that
        /// overran a wall CLAUDE.md says must hold "even if the simulation code is wrong":
        ///
        /// **The length never passes the cap.** **Every field is inside the bounds
        /// `genome.rs` declares** - so a gene ten thousand mutations old is still a gene the
        /// physics can be handed, with a stiffness the integrator will not diverge on and a
        /// spring length that cannot reach round the world. **And nothing is ever an
        /// infinity or a not-a-number**, which is the failure that would spread silently: one
        /// such value in one gene puts a cell at an impossible position and the whole world's
        /// arithmetic goes with it.
        ///
        /// The last assertion is what stops the test being vacuous - the genome must actually
        /// have reached the cap, rather than the cap having held over a genome that never grew.
        /// And the body is grown at the end, because a genome the mutation operators are happy
        /// with is only useful if development is happy with it too.
        #[test]
        fn the_genome_cap_holds_under_ten_thousand_mutations(seed: u64, founders in 0_usize..8) {
            let limits = spec_limits();
            let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap is a word");
            let last_step = u8::try_from(limits.max_dev_steps.get() - 1)
                .expect("config.rs caps the development budget at 255");

            // Everything at once, as hard as the configuration will allow.
            let explosive = mutation_with(|rates| {
                rates.point_rate = 1.0;
                rates.point_sigma = 1.0;
                rates.duplication_rate = 1.0;
                rates.deletion_rate = 1.0;
                rates.insertion_rate = 1.0;
                rates.reorder_rate = 1.0;
                rates.genome_duplication_rate = 1.0;
            });

            let mut rng = organism_rng(seed, 0);
            let founding: Vec<Gene> = (0..founders)
                .map(|_| Gene::random(&mut rng, &limits))
                .collect();
            let mut genome = Genome::new(founding, &limits);

            for generation in 0..10_000 {
                genome = mutate(&genome, &explosive, &limits, &mut rng);

                prop_assert!(
                    genome.genes().len() <= cap,
                    "generation {}: a genome of {} genes in a world that allows {cap}. \
                     Duplication is exponential and this cap is the only thing between it and \
                     the machine's memory",
                    generation,
                    genome.genes().len(),
                );

                for gene in genome.genes() {
                    prop_assert!(gene.trigger_state.get() < State::COUNT);
                    prop_assert!(gene.child_state.get() < State::COUNT);
                    prop_assert!(gene.new_state.get() < State::COUNT);
                    prop_assert!(
                        gene.min_step <= last_step && gene.max_step <= last_step,
                        "generation {}: a gene names steps {}..={} in a run of {}, so \
                         mutation has switched it off by arithmetic",
                        generation, gene.min_step, gene.max_step, limits.max_dev_steps,
                    );

                    prop_assert!(
                        (-PI..=PI).contains(&gene.angle),
                        "generation {}: an angle of {} is off the circle",
                        generation, gene.angle,
                    );
                    prop_assert!(
                        (0.0..=TAU).contains(&gene.osc_phase),
                        "generation {}: a phase of {} is off the circle",
                        generation, gene.osc_phase,
                    );
                    prop_assert!(
                        (0.0..=MAX_REST_LENGTH).contains(&gene.rest_length),
                        "generation {}: a rest length of {}. Past {MAX_REST_LENGTH} a body \
                         can grow wide enough to reach round the world, and SPEC section 8 \
                         warns that a spring across the seam hauls its cells through it",
                        generation, gene.rest_length,
                    );
                    prop_assert!(
                        (0.0..=MAX_STIFFNESS).contains(&gene.stiffness),
                        "generation {}: a stiffness of {}. Past {MAX_STIFFNESS} the physics' \
                         own integrator diverges at a sixtieth of a second, and the lineage \
                         explodes rather than fails",
                        generation, gene.stiffness,
                    );
                    prop_assert!((0.0..=MAX_OSC_FREQ).contains(&gene.osc_freq));
                    prop_assert!(gene.sensor_gain.abs() <= MAX_SENSOR_GAIN);

                    prop_assert!(
                        gene.angle.is_finite()
                            && gene.rest_length.is_finite()
                            && gene.stiffness.is_finite()
                            && gene.osc_freq.is_finite()
                            && gene.osc_phase.is_finite()
                            && gene.sensor_gain.is_finite(),
                        "generation {}: a gene holds an infinity or a not-a-number, which \
                         puts a cell at an impossible position and takes the world's \
                         arithmetic with it",
                        generation,
                    );
                }
            }

            prop_assert_eq!(
                genome.genes().len(),
                cap,
                "ten thousand mutations at every rate turned to certainty did not fill the \
                 genome, so this test has been watching a cap that was never approached",
            );

            // A genome the operators are happy with has to be one development is happy with.
            let body = develop(&genome, &limits);
            prop_assert!(
                body.cells.len()
                    <= usize::try_from(limits.max_cells_per_organism.get()).unwrap_or(usize::MAX),
                "a genome ten thousand mutations old grew a body of {} cells",
                body.cells.len(),
            );
        }
    }
}
