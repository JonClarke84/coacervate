//! The gene, and the genome.
//!
//! See `SPEC.md` section 7 for the rules this implements.
//!
//! This is the module CLAUDE.md's decision log opens by calling *"the single most important
//! decision in the project"*, so it is worth saying plainly what the decision was and what
//! it buys.
//!
//! An organism could have been described by a fixed list of numbers - how big, how fast, how
//! hungry. That design can only ever evolve a *better single cell*, because there is no slot
//! in it for a thing that does not exist yet. You cannot mutate a fourth leg onto a creature
//! whose description has three leg-shaped holes in it.
//!
//! What is here instead is a **variable-length list of rules**. Each rule says "a cell in
//! developmental state *s*, at some point between step *a* and step *b*, does *this*", and
//! `development.rs` runs them to grow a body. Because a rule is keyed on a *state* rather
//! than on a body part, copying a rule and changing the state it answers to produces a rule
//! that fires somewhere new - which is to say, **a new body part**. Duplicate, then diverge.
//! That single operator is the mechanism behind essentially all real biological complexity,
//! and every decision in this file is downstream of wanting it to work.
//!
//! # Why a gene is a fixed-size record with unused fields in it
//!
//! A gene carries the parameters for all three actions at once, so a gene whose action is
//! `Terminate` still has an `angle` and a `child_kind` sitting unread inside it. That looks
//! wasteful and is deliberate, for two reasons.
//!
//! The first is mechanical: SPEC section 7 asks for fixed-size records so a genome packs
//! into a flat array, which is what the Phase 9 port to the graphics card needs. A record
//! whose size depended on which action it held could not be stored that way.
//!
//! The second is evolutionary, and it is the more important of the two. A point mutation
//! that changes only the *action* of a gene turns it into a working gene of a different
//! kind, because the parameters that action needs were already there and were already being
//! carried along - drifting, unseen, unselected - while the gene did something else. Fields
//! that are ignored today are the raw material of the gene that fires tomorrow. Trimming
//! them away would make every action change a leap into freshly-random parameters, and a
//! leap is far less likely to land somewhere useful than a step.
//!
//! # Where the bounds in this file come from, and where they do not
//!
//! Two of them are read straight off the specification: a developmental state is `0..=63`
//! (SPEC section 6) and a genome holds at most `max_genes` genes (SPEC section 7, and
//! CLAUDE.md's one critical cap).
//!
//! The rest - how long a spring may be, how stiff, how fast a myocyte may oscillate, how
//! strongly a sensocyte may respond - are **not in SPEC at all**. They exist because
//! [`Gene::random`] has to draw a number from somewhere, and "somewhere" has to be written
//! down. Each is justified where it is declared, and each is a number a later phase may
//! reasonably want to move. None of them is enforced on a gene built by hand; they are the
//! bounds of the *draw*, not invariants of the type, because a mutation that walks a gene
//! outside them is a thing Group C gets to decide about rather than a thing this module
//! forbids in advance.

use crate::cell::CellKind;
use crate::config::LimitsConfig;
use crate::physics::DT;
// `RngExt` is what supplies `random` and `random_range`; `rand`'s `Rng` is the narrower
// trait beneath it and brings neither.
use rand::RngExt;
use rand::rngs::ChaCha8Rng;
use std::f32::consts::{PI, TAU};

/// A cell's developmental identity: which of a genome's rules answer to it.
///
/// SPEC section 6 gives the range as `0..=63` and this type is what makes that true rather
/// than hoped for. It is six bits, and a number with more bits than that in it does not
/// name a state at all.
///
/// # Why the constructor takes any byte and keeps only six bits
///
/// The alternative - refusing a value above 63 - would put a failure case into the middle
/// of the mutation operators, which are the one place in the project that produces states
/// in bulk and the one place a failure case has nothing useful to do with itself. SPEC
/// section 7's point mutation "re-draws discrete fields uniformly", and a re-draw that can
/// come back holding an error is a re-draw with a retry loop around it.
///
/// Keeping the low six bits is also exactly uniform, which a clamp would not be. There are
/// 256 bytes and 64 states, so each state is the answer to precisely four of them: a
/// uniformly-drawn byte gives a uniformly-drawn state, with no state made four times more
/// likely by being at the end of the range the way clamping would.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct State(u8);

impl State {
    /// How many developmental identities there are. SPEC section 6's `0..=63`, counted.
    pub const COUNT: u8 = 64;

    /// The state every organism starts in: the seed cell's identity, before development has
    /// done anything at all.
    pub const ZERO: Self = Self(0);

    /// The state named by the low six bits of this byte. See the type's documentation for
    /// why the other two bits are dropped rather than refused.
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value % Self::COUNT)
    }

    /// The number this state is, which is always in `0..=63`.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// A state drawn uniformly from all sixty-four.
    ///
    /// Used by [`Gene::random`] and, from Group C, by the point mutation that re-draws a
    /// discrete field.
    #[must_use]
    pub fn random(rng: &mut ChaCha8Rng) -> Self {
        Self::new(rng.random())
    }
}

/// What a gene does to the cell it fires on.
///
/// Three actions, and between them they are the whole of development: make a cell, change a
/// cell, stop a cell. SPEC section 7 lists them in this order and the order is not
/// decoration - Group C's point mutation re-draws an action uniformly from this list, so
/// inserting a fourth in the middle would change what every existing genome means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Grow a daughter cell beside this one, and optionally spring the two together. This
    /// is the only action that makes a body larger than one cell.
    Divide,
    /// Change this cell's kind and state where it stands. No new cell.
    Differentiate,
    /// Make this cell inert: it stays part of the body and fires no further genes.
    Terminate,
}

impl Action {
    /// Every action there is, in SPEC section 7's order, so that a test can walk the whole
    /// set and a mutation can draw from it.
    pub const ALL: [Self; 3] = [Self::Divide, Self::Differentiate, Self::Terminate];
}

/// What a sensocyte is looking for.
///
/// SPEC section 6 gives a sensocyte a gradient to sample and says which gradient is
/// "determined by its gene"; this is that choice. Nothing reads it until Phase 4, when
/// sensing exists at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorTarget {
    /// The energy in the resource field - which is to say, where the light is.
    Light,
    /// Dead biomass on its way down.
    Detritus,
    /// The cells of *other* organisms. What a predator would want to find and what prey
    /// would want to avoid, and the gene does not say which - see [`Gene::sensor_gain`].
    ForeignBiomass,
}

impl SensorTarget {
    /// Every target there is, in SPEC section 6's order, for the same two reasons as
    /// [`Action::ALL`].
    pub const ALL: [Self; 3] = [Self::Light, Self::Detritus, Self::ForeignBiomass];
}

/// One condition-action rule: SPEC section 7's `Gene`, field for field.
///
/// Everything is public because a gene has no invariant of its own to protect. It is a
/// record of what a cell in some state should do, and the rules that act on it live in
/// `development.rs`, which reads it, and in Group C's mutation, which perturbs it.
///
/// A gene whose `min_step` is above its `max_step` can never fire, and that is allowed
/// rather than prevented. Such a gene is not broken - it is neutral genetic material, which
/// SPEC section 7 is explicit about wanting: non-firing genes are exactly where duplication
/// finds something to diverge from, and a type that refused to hold one would be a type
/// that threw the raw material away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gene {
    // --- the condition ---
    /// Fires on cells whose developmental identity is this.
    pub trigger_state: State,

    /// The earliest development step at which this may fire.
    pub min_step: u8,

    /// The latest. Both ends are included, so `min_step == max_step` is a gene that fires
    /// on exactly one step - which is how a growth program gets a sense of *when*.
    pub max_step: u8,

    // --- the action ---
    /// Which of the three things it does.
    pub action: Action,

    // --- Divide ---
    /// Where the daughter goes, in radians from the parent's own axis. See
    /// `development.rs` for what a cell's axis is, which SPEC does not say.
    pub angle: f32,

    /// Whether the daughter stays physically attached to its parent.
    ///
    /// **This one boolean is the origin of multicellularity in the model.** False and the
    /// daughter drifts off as an independent cell of the same organism; true and a spring
    /// joins the two, and a lineage that keeps saying true grows a body.
    pub adhere: bool,

    /// The daughter's developmental identity, and so which genes will fire on it next.
    pub child_state: State,

    /// What the daughter is made of.
    pub child_kind: CellKind,

    /// How far apart the spring holds parent and daughter, if the daughter adheres.
    pub rest_length: f32,

    /// How hard that spring pulls per unit it is stretched by.
    pub stiffness: f32,

    // --- Differentiate ---
    /// What the cell becomes.
    pub new_kind: CellKind,

    /// The identity it takes on, and so which genes fire on it afterwards. A gene whose
    /// `new_state` equals its own `trigger_state` fires again on the same cell every step
    /// for the rest of development, changing nothing; see `development.rs` for why that is
    /// harmless.
    pub new_state: State,

    // --- behaviour, for the kinds that have any (Phase 4) ---
    /// How fast a myocyte works its springs, in cycles per second.
    pub osc_freq: f32,

    /// Where in that cycle it starts, in radians.
    ///
    /// Useless in a single cell and the whole of swimming in several: two myocytes
    /// contracting a quarter of a cycle apart make a travelling wave, and two contracting
    /// together only make a body that pulses.
    pub osc_phase: f32,

    /// How strongly a sensocyte responds to what it senses, **and in which direction**.
    ///
    /// The sign is the load-bearing part. A positive gain steers towards the thing and a
    /// negative one steers away, so attraction and avoidance are the same gene with one
    /// number's sign flipped - which puts fleeing exactly one point mutation away from
    /// hunting, in either direction. Split into a magnitude and a separate "towards or
    /// away" flag, the crossing would need two mutations at once and would essentially
    /// never happen.
    pub sensor_gain: f32,

    /// What it is sensing.
    pub sensor_target: SensorTarget,
}

/// The longest a gene may ask a spring to hold two cells apart, when one is drawn at random.
///
/// Two of the widest cells in the world touch at `2 x LARGEST_RADIUS`, so this is twice the
/// width of a body's own cells: enough gap for genuinely spread-out, branching shapes -
/// which SPEC section 6 says photocytes want, since a cell shaded by the cell above it
/// harvests nothing - and no more.
///
/// The upper end matters more than it looks. SPEC section 8 warns that springs "have no
/// length limit" and are not found by the neighbour search, so a spring between two cells on
/// opposite sides of the world will quietly haul them together *through the seam*.
/// Development can only avoid creating one by never growing a body wide enough to reach
/// round, and this bound is what makes that true: at most 64 cells in a chain, each at most
/// this far from the last, is under 900 world units - comfortably inside half the width of
/// SPEC's default 2048-unit world, which is the distance at which the wrap starts choosing
/// the wrong way round.
pub const MAX_REST_LENGTH: f32 = 4.0 * CellKind::LARGEST_RADIUS;

/// The stiffest a gene may make a spring, when one is drawn at random.
///
/// This is a limit of the arithmetic rather than of biology. SPEC section 8 integrates the
/// physics with a fixed step of a sixtieth of a second, and an explicit integrator of that
/// kind diverges on a spring stiffer than `4 / dt²` - 14,400 at this tick rate - whatever
/// the drag. A body at anything approaching that number is not a stiff organism, it is a
/// numerical explosion.
///
/// The bound here is a hundredth of that, which leaves two orders of magnitude of headroom
/// against a mutation walking a lineage towards the cliff, and still comfortably contains
/// SPEC's own `collision_stiffness` of 40 - the one stiffness in the project that has a
/// value chosen by a person.
pub const MAX_STIFFNESS: f32 = 0.01 * 4.0 / (DT * DT);

/// The fastest a gene may ask a myocyte to oscillate, in cycles per second, when one is
/// drawn at random.
///
/// The tick is a sixtieth of a second, so an oscillation above thirty cycles a second cannot
/// be represented at all - it is sampled fewer than twice per cycle and comes out as some
/// slower rhythm that nothing in the genome asked for. Five is a sixth of that limit, which
/// leaves every drawn frequency sampled at least a dozen times per cycle.
///
/// Nothing reads this until Phase 4, which owns the number and may well want it lower: at
/// SPEC section 8's drag of 0.92 the water is thick enough that a fast flutter moves a body
/// less than a slow stroke does.
pub const MAX_OSC_FREQ: f32 = 5.0;

/// The strongest a gene may make a sensocyte's response, in either direction, when one is
/// drawn at random.
///
/// The magnitude is arbitrary and Phase 4 owns it - what a gain multiplies *into* does not
/// exist yet. What is not arbitrary is that the draw is symmetric about zero, so a random
/// sensocyte is as likely to be repelled by what it senses as attracted to it. See
/// [`Gene::sensor_gain`].
pub const MAX_SENSOR_GAIN: f32 = 1.0;

impl Gene {
    /// A gene with every field drawn at random.
    ///
    /// Group C's insertion operator is the caller that matters: SPEC section 7 inserts "a
    /// fully random gene" as one of its six mutations, and this is that gene.
    ///
    /// # Why the step window is drawn from the configured budget
    ///
    /// `min_step` and `max_step` are bytes and could hold anything up to 255, but a run only
    /// takes `max_dev_steps` of them - sixteen, by default. Drawn over the full range of the
    /// type, a random gene would name a window that development never reaches about
    /// nineteen times in twenty, and independent draws would put the two ends the wrong way
    /// round in half of what was left. Gene insertion would then be an operator that almost
    /// always inserts nothing, which is not the operator SPEC asked for.
    ///
    /// So the window is drawn inside the run's actual budget, and `max_step` is drawn from
    /// `min_step` upwards so the two ends are never inverted. A random gene can therefore
    /// always fire. Genes that can *never* fire are still perfectly legal - see [`Gene`] -
    /// they simply arrive by mutation of a live gene rather than fully formed.
    #[must_use]
    pub fn random(rng: &mut ChaCha8Rng, limits: &LimitsConfig) -> Self {
        let last_step = last_step(limits);
        let min_step = rng.random_range(0..=last_step);

        Self {
            trigger_state: State::random(rng),
            min_step,
            max_step: rng.random_range(min_step..=last_step),
            action: Action::ALL[rng.random_range(0..Action::ALL.len())],
            angle: rng.random_range(-PI..=PI),
            adhere: rng.random(),
            child_state: State::random(rng),
            child_kind: CellKind::ALL[rng.random_range(0..CellKind::ALL.len())],
            rest_length: rng.random_range(0.0..=MAX_REST_LENGTH),
            stiffness: rng.random_range(0.0..=MAX_STIFFNESS),
            new_kind: CellKind::ALL[rng.random_range(0..CellKind::ALL.len())],
            new_state: State::random(rng),
            osc_freq: rng.random_range(0.0..=MAX_OSC_FREQ),
            osc_phase: rng.random_range(0.0..=TAU),
            sensor_gain: rng.random_range(-MAX_SENSOR_GAIN..=MAX_SENSOR_GAIN),
            sensor_target: SensorTarget::ALL[rng.random_range(0..SensorTarget::ALL.len())],
        }
    }
}

/// The last development step a run will actually reach, as a gene would name it.
///
/// Development runs steps `0..max_dev_steps`, so the last one is one less than the budget.
/// The conversion cannot fail: `config.rs` refuses a budget above 255 precisely because a
/// gene's step numbers are bytes and a step beyond that could not be named by any gene.
///
/// Shared with `mutation.rs`, which re-draws a gene's step window and has to draw it from the
/// same place [`Gene::random`] does. A point mutation that could name a step outside the run's
/// budget would be an operator that switches genes off by arithmetic.
pub(crate) fn last_step(limits: &LimitsConfig) -> u8 {
    let budget = u8::try_from(limits.max_dev_steps.get())
        .expect("config.rs caps the development budget at 255, which is one byte");

    budget - 1
}

/// An organism's whole growth program: SPEC section 7's ordered, variable-length list of
/// genes.
///
/// # Order is information
///
/// Development takes the **first** gene that matches a cell, so where a gene sits in this
/// list decides whether it ever fires. That makes swapping two genes a real mutation with a
/// real effect on the body, and it makes a gene sitting behind another one *silent* rather
/// than absent - carried along, mutating freely, costing nothing, and one reordering away
/// from being expressed. SPEC section 7 calls that "a feature, not waste", and it is where
/// duplication finds material that is already several mutations away from what the organism
/// currently does.
///
/// # The cap is not a suggestion
///
/// CLAUDE.md marks `max_genes` as the one cap that must never be raised without adding a
/// metabolic cost per gene, and the reason is arithmetic: gene duplication is the operator
/// this whole design exists for, and duplication compounds. A lineage that duplicates faster
/// than selection punishes it doubles its genome every few generations, and there is no
/// number of gigabytes that stops an exponential - only a wall does.
///
/// So the cap is enforced *by construction*. There is no way to hold a `Genome` longer than
/// the configuration allows, because the only way to make one is through
/// [`Genome::new`], which drops the excess. Group C's mutation operators inherit the
/// guarantee without having to remember it.
#[derive(Debug, Clone, PartialEq)]
pub struct Genome {
    genes: Vec<Gene>,
}

impl Genome {
    /// A genome holding these genes, and at most `max_genes` of them.
    ///
    /// Anything past the cap is dropped from the end rather than refused. Refusing would
    /// mean a failure case in the middle of every mutation operator, and the operators have
    /// nothing sensible to do with one: an organism whose genome came out one gene too long
    /// is not an error to report to anybody, it is an organism with a shorter genome. What
    /// matters is only that the wall is real and that it is in one place.
    #[must_use]
    pub fn new(mut genes: Vec<Gene>, limits: &LimitsConfig) -> Self {
        let cap = usize::try_from(limits.max_genes.get()).expect("a gene cap fits in a word");

        genes.truncate(cap);
        Self { genes }
    }

    /// The genes, in the order development will try them.
    #[must_use]
    pub fn genes(&self) -> &[Gene] {
        &self.genes
    }

    /// A fingerprint of this growth program: the same number for the same genes, for ever.
    ///
    /// Two genomes that differ anywhere have to come out as two numbers, two genomes that are the
    /// same have to come out as one, and - crucially - they have to come out as the *same* one
    /// next year.
    ///
    /// # ⚠️ What this is not
    ///
    /// **It is not what identifies a lineage.** Group B built SPEC section 12's per-lineage
    /// identity out of this, drew a frame, and looked at it; SPEC section 11 now records the
    /// result in as many words: *"A hash does not drift; it jumps. Any mutation at all reseeds
    /// it, so every child is completely unrelated to its parent… the result is confetti."* What
    /// identifies a lineage is `organism.rs`'s `marker`, which is **inherited** and shifted by a
    /// small amount rather than computed.
    ///
    /// What this *is* for is telling whether a genome is still the one it came from - and, at a
    /// birth, being the well-mixed number that decides **which way** round the circle the
    /// offspring's marker moves. Both want exactly the property a hash has and a marker must not:
    /// that any difference at all shows up.
    ///
    /// # ⚠️ Why this is written out rather than borrowed from `std`
    ///
    /// Because the standard library's `DefaultHasher` says in its own documentation that its
    /// algorithm is not specified and may change between Rust releases. The direction every
    /// lineage in a run drifted would then come out differently the next time `rustup update` was
    /// run, and every frame dumped, every screenshot kept and - once Phase 8's replay log
    /// exists - every archived run would stop matching the world it came from, with nothing
    /// anywhere to say why. `grid.rs` writes out its own mixer for the same reason: a number that
    /// has to mean one thing for ever is one this project writes down.
    ///
    /// This is FNV-1a, which is short enough to read, fast enough not to matter, and specified
    /// by two constants that are in the source below. The value it produces for a fixed genome
    /// is pinned as a golden vector in this module's tests.
    ///
    /// # What goes in, and in what order
    ///
    /// The number of genes first, then each gene's sixteen fields in the order SPEC section 7
    /// declares them. Whole numbers go in as themselves; the enumerations go in as their
    /// position in `ALL`, which is the order SPEC lists them and the order Group C's point
    /// mutation draws from; the floats go in as their bit patterns, because that is the only
    /// exact reading of a float there is.
    ///
    /// **The count goes first and that is not decoration.** `docs/PHASE1.md` recorded it as an
    /// obligation before there was anything to hash: a byte-stream hash cannot tell `[a, b]`
    /// followed by nothing from `[a]` followed by `[b]`, so any variable-length sequence writes
    /// its length before its elements - and gene duplication and deletion are precisely the
    /// operators that change this one's length. Phase 8's snapshot digest will hash several
    /// such sequences one after another, which is exactly the case that obligation describes.
    ///
    /// # One thing it is deliberately stricter about than `==` is
    ///
    /// A float's bit pattern distinguishes `0.0` from `-0.0`, which compare equal. Two genomes
    /// differing only that way would compare equal and fingerprint differently. That is the
    /// harmless direction for a change detector - it can report a change that did not matter,
    /// never miss one that did - and the alternative is a branch on every one of the six floats
    /// in every gene for a case that costs a hue nobody would notice.
    #[must_use]
    pub fn hash(&self) -> u64 {
        let mut digest = Digest::new();

        digest.count(self.genes.len());
        for gene in &self.genes {
            digest.byte(gene.trigger_state.get());
            digest.byte(gene.min_step);
            digest.byte(gene.max_step);
            digest.choice(&Action::ALL, gene.action);
            digest.float(gene.angle);
            digest.byte(u8::from(gene.adhere));
            digest.byte(gene.child_state.get());
            digest.choice(&CellKind::ALL, gene.child_kind);
            digest.float(gene.rest_length);
            digest.float(gene.stiffness);
            digest.choice(&CellKind::ALL, gene.new_kind);
            digest.byte(gene.new_state.get());
            digest.float(gene.osc_freq);
            digest.float(gene.osc_phase);
            digest.float(gene.sensor_gain);
            digest.choice(&SensorTarget::ALL, gene.sensor_target);
        }

        digest.finish()
    }

    /// How much of this genome is not the other one, between nothing at all and all of it.
    ///
    /// ⭐ **This is what makes an offspring's lineage marker move by a *small* amount rather
    /// than by a fixed one.** SPEC section 11: *"an offspring takes its parent's hue and shifts
    /// it by a small amount, **larger when the genome changed more**"*. Something has to say how
    /// much more, and this is that number: the share of the longer genome's gene positions at
    /// which the two disagree.
    ///
    /// Nought means the copy came out identical - which is what most births produce, since SPEC
    /// section 3's operators fire on well under a fifth of them - and a lineage that is not
    /// changing therefore does not move either. One means nothing survived in place.
    ///
    /// # Position by position, and an insertion at the front counts as everything
    ///
    /// A gene is compared with the gene at the same index, and genes past the end of the shorter
    /// genome all count as changed. So a gene inserted at the front comes out as a divergence of
    /// one, because every gene after it has moved.
    ///
    /// That looks harsh for one operator and it is the honest answer for this genome. SPEC
    /// section 7's development takes the **first** gene whose trigger matches, so a gene put in
    /// at the front is read before everything behind it and can change what the body grows into
    /// entirely. An alignment that quietly forgave the shift would be reporting that nothing much
    /// happened at the one moment when a great deal might have.
    ///
    /// SPEC section 11 describes a finer measure for Phase 7's clustering - matched genes
    /// contributing their *scaled numeric difference* rather than a whole unit - and this is
    /// deliberately not that. A marker that drifts is a rough record of how far a lineage has
    /// travelled; it does not have to tell a small point mutation from a large one, only to move
    /// further when more of the program is different.
    #[must_use]
    pub fn divergence_from(&self, other: &Self) -> f32 {
        let longer = self.genes.len().max(other.genes.len());
        if longer == 0 {
            return 0.0;
        }

        let shared = self.genes.len().min(other.genes.len());
        let changed = (0..shared)
            .filter(|&at| self.genes[at] != other.genes[at])
            .count()
            + (longer - shared);

        // Both are gene counts, which `Genome::new` caps at `limits.max_genes` - 128 in SPEC
        // section 3's defaults and never more than a `u16` could hold, so both conversions are
        // exact and the division is a single rounding.
        let changed = f32::from(u16::try_from(changed).expect("a genome is capped at 128 genes"));
        let longer = f32::from(u16::try_from(longer).expect("a genome is capped at 128 genes"));

        changed / longer
    }
}

/// FNV-1a, hand-rolled, so that the numbers it produces are the same numbers for ever.
///
/// See [`Genome::hash`] for why this is written out instead of taken from the standard library.
/// The two constants are the published 64-bit ones and must never be edited: they are what
/// makes a colour recorded tonight the same colour next year.
///
/// Every operation is explicitly a wrapping one, exactly as `grid.rs`'s mixer is, because
/// CLAUDE.md turns integer overflow checking on in release builds and a hash is nothing but
/// deliberate overflow.
struct Digest(u64);

impl Digest {
    /// FNV's 64-bit offset basis: what the hash holds before anything has been fed to it.
    ///
    /// Named rather than written inline because a test needs it: an empty sequence that fed
    /// *nothing* to the hash would come back holding exactly this, which is how the length
    /// prefix is observed from outside.
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

    /// FNV's 64-bit prime.
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    /// One byte: exclusive-or it in, then multiply.
    ///
    /// That order round is what makes this FNV-1**a** rather than FNV-1, and it is the variant
    /// with the better avalanche behaviour on short inputs - which is what a genome is.
    fn byte(&mut self, value: u8) {
        self.0 = (self.0 ^ u64::from(value)).wrapping_mul(Self::PRIME);
    }

    /// How many things are about to follow. See [`Genome::hash`] for why this exists.
    ///
    /// Eight bytes, least significant first, so that the answer does not depend on the machine
    /// the run happened on.
    ///
    /// # Panics
    ///
    /// If asked to record a count larger than a 64-bit number, which no machine can hold a
    /// sequence of.
    fn count(&mut self, value: usize) {
        let value = u64::try_from(value).expect("a sequence fits in a 64-bit count");

        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    /// One 32-bit number, as the bits it actually is.
    ///
    /// The bit pattern rather than any decimal reading of it, because a fingerprint has to
    /// distinguish two numbers that differ in their last place, and every decimal rendering
    /// there is rounds.
    fn float(&mut self, value: f32) {
        for byte in value.to_bits().to_le_bytes() {
            self.byte(byte);
        }
    }

    /// One value of a small fixed set, as its position in that set.
    ///
    /// The position rather than anything the type itself provides, and the set is passed in so
    /// that the position is the one SPEC's list gives. Those lists are already load-bearing -
    /// Group C's point mutation draws uniformly from them, so inserting an entry in the middle
    /// changes what every existing genome's mutations do - and this makes the fingerprint agree
    /// with them rather than with the order somebody happened to write the enumeration in.
    ///
    /// # Panics
    ///
    /// If the value is not in the set it was given, which would mean a variant had been added
    /// to an enumeration and left out of its `ALL`.
    fn choice<T: PartialEq + std::fmt::Debug>(&mut self, all: &[T], value: T) {
        let position = all
            .iter()
            .position(|candidate| *candidate == value)
            .expect("every variant of an enumeration is in its own ALL");

        self.byte(u8::try_from(position).expect("no enumeration here has 256 variants"));
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::spec_defaults;
    use proptest::prelude::*;
    use rand::SeedableRng;

    /// A configuration built from SPEC's defaults with one thing changed about it.
    ///
    /// Several tests below need a different cap from the shipped one - a genome that may
    /// hold four genes, a run that takes three development steps - and every one of them
    /// wants the number to have been through `config.rs`'s gate rather than assembled by
    /// hand, because that gate is where the ceilings live.
    fn limits_with(change: impl FnOnce(&mut crate::config::RawLimits)) -> LimitsConfig {
        let mut raw = spec_defaults();
        change(&mut raw.limits);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
            .limits
    }

    /// A gene is what SPEC section 7 says it is, field for field.
    ///
    /// A transcription check, and it earns its place the same way `cell.rs`'s does. SPEC
    /// section 7's struct is sixteen fields, several of them neighbours of the same type
    /// carrying quite different meanings: `child_state` and `new_state` are both states,
    /// `child_kind` and `new_kind` are both kinds, and one pair belongs to dividing while
    /// the other belongs to differentiating. Wire a pair the wrong way round and nothing
    /// fails - bodies simply grow with the wrong cells in them, for ever, and every balance
    /// decision made afterwards is made against a model nobody agreed to.
    ///
    /// The last three claims are about the shape of the record rather than its contents.
    ///
    /// Three actions and three sensor targets, counted, because both lists are drawn from
    /// uniformly by Group C's point mutation - a list that has quietly gained a fourth entry
    /// changes the odds on every existing genome.
    ///
    /// An array of genes is exactly as large as that many genes, which is SPEC section 7's
    /// "fixed-size records so they pack into flat arrays" stated in the one way that can be
    /// checked. A gene that had grown a heap pointer inside it would still satisfy this, so
    /// what this really catches is a gene that had grown a *variable* size.
    ///
    /// And a state is six bits: 64 is not a state, it is state 0 wearing a seventh bit.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "every number here is the literal it was built from, so the comparison has \
                  to be with the number itself; an approximate match would accept two \
                  neighbouring fields having been swapped"
    )]
    fn a_gene_has_the_fields_spec_section_7_gives_it() {
        let gene = Gene {
            trigger_state: State::new(5),
            min_step: 2,
            max_step: 9,
            action: Action::Divide,
            angle: 0.75,
            adhere: true,
            child_state: State::new(6),
            child_kind: CellKind::Myocyte,
            rest_length: 7.5,
            stiffness: 12.0,
            new_kind: CellKind::Sclerocyte,
            new_state: State::new(11),
            osc_freq: 1.5,
            osc_phase: 0.25,
            sensor_gain: -0.5,
            sensor_target: SensorTarget::Detritus,
        };

        assert_eq!(gene.trigger_state, State::new(5));
        assert_eq!(gene.min_step, 2);
        assert_eq!(gene.max_step, 9);
        assert_eq!(gene.action, Action::Divide);
        assert_eq!(gene.angle, 0.75);
        assert!(gene.adhere, "the one boolean multicellularity is made of");
        assert_eq!(gene.child_state, State::new(6));
        assert_eq!(gene.child_kind, CellKind::Myocyte);
        assert_eq!(gene.rest_length, 7.5);
        assert_eq!(gene.stiffness, 12.0);
        assert_eq!(gene.new_kind, CellKind::Sclerocyte);
        assert_eq!(gene.new_state, State::new(11));
        assert_eq!(gene.osc_freq, 1.5);
        assert_eq!(gene.osc_phase, 0.25);
        assert_eq!(
            gene.sensor_gain, -0.5,
            "a sensor gain must be able to be negative, because that is the whole of \
             avoidance"
        );
        assert_eq!(gene.sensor_target, SensorTarget::Detritus);

        assert_eq!(
            Action::ALL.len(),
            3,
            "SPEC section 7 gives three actions; a fourth would change what every existing \
             genome's point mutations do"
        );
        assert_eq!(SensorTarget::ALL.len(), 3, "SPEC section 6 gives three");

        assert_eq!(
            size_of::<[Gene; 8]>(),
            8 * size_of::<Gene>(),
            "genes no longer pack into a flat array, which is what SPEC section 7 asks of \
             them and what the Phase 9 port to the graphics card needs"
        );

        assert_eq!(State::COUNT, 64, "SPEC section 6 gives states as 0..=63");
        assert_eq!(
            State::new(64),
            State::ZERO,
            "a state is six bits, so the sixty-fifth is the first one again"
        );
    }

    /// A genome is an ordered list of genes, and it cannot be longer than the configuration
    /// allows.
    ///
    /// The order half is checked by reading the genes back out in the order they went in -
    /// which sounds too obvious to test until you remember that first-match-wins is the
    /// entire reason gene position carries information. A genome that sorted or de-
    /// duplicated its genes would still be a perfectly good list and would destroy that.
    ///
    /// The cap half is the one CLAUDE.md calls critical. It is checked at three sizes -
    /// under the cap, exactly at it, and far past it - because the failure that matters is
    /// not "the cap is missing" but "the cap is one out", and only the boundary shows that.
    ///
    /// The cap used here is four rather than SPEC's 128, so that "far past it" is a genome
    /// this test can write out and a reader can count.
    #[test]
    fn a_genome_is_a_list_of_genes_and_is_capped() {
        let limits = limits_with(|limits| limits.max_genes = 4);

        // Genes distinguishable from one another at a glance, so that "the order was kept"
        // is a claim about something visible.
        let numbered: Vec<Gene> = (0..10)
            .map(|n| Gene {
                trigger_state: State::new(n),
                ..a_terminating_gene()
            })
            .collect();

        let short = Genome::new(numbered[..3].to_vec(), &limits);
        assert_eq!(
            short.genes().len(),
            3,
            "a genome under the cap keeps all of it"
        );
        for (position, gene) in short.genes().iter().enumerate() {
            assert_eq!(
                gene.trigger_state,
                State::new(u8::try_from(position).expect("three fits in a byte")),
                "the genes came back in a different order from the one they went in, so \
                 gene position - which is what first-match-wins turns into information - \
                 does not survive being stored"
            );
        }

        let exact = Genome::new(numbered[..4].to_vec(), &limits);
        assert_eq!(
            exact.genes().len(),
            4,
            "a genome exactly at the cap must be allowed; a cap you cannot reach is a cap \
             one lower with nobody able to tell which"
        );

        let bloated = Genome::new(numbered.clone(), &limits);
        assert_eq!(
            bloated.genes().len(),
            4,
            "a genome longer than the cap was stored at its full length, and gene \
             duplication is exponential"
        );
        assert_eq!(
            bloated.genes()[0].trigger_state,
            State::new(0),
            "the excess was taken off the front rather than the end"
        );
    }

    /// The simplest possible gene: one that fires on the seed cell and stops it.
    ///
    /// The tests above care about one or two fields each and need the other fourteen to be
    /// *something*. This is that something, and it is deliberately a `Terminate` so that a
    /// gene built from it and left otherwise alone grows nothing unexpected.
    fn a_terminating_gene() -> Gene {
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

    /// ⭐ **A5.** A genome has a fingerprint that is the same number for the same genes, this
    /// year and next year, and a different one for anything else.
    ///
    /// SPEC section 12 wants an organism's hue to come from its lineage and to **drift as the
    /// lineage drifts genetically**, so that speciation is something you can watch happen
    /// rather than something a panel tells you about afterwards. That needs a number that
    /// summarises a genome, and it has three requirements which are easy to state and easy to
    /// get wrong.
    ///
    /// # 1. It must not change when the toolchain does
    ///
    /// ⚠️ **This is why the standard library's `DefaultHasher` is not used, and it is not a
    /// matter of taste.** Its documentation says outright that the algorithm is not specified
    /// and may change between Rust releases. A hue computed from it would silently shift the
    /// colour of every lineage in the project the next time `rustup update` was run - and every
    /// frame dumped, every screenshot kept and, once Phase 8 exists, every archived run would
    /// stop matching the world it came from, with nothing anywhere to say why.
    ///
    /// So this is FNV-1a, written out, with both of its constants in the source. It is the same
    /// decision `grid.rs` takes about the blotchiness of the light and for the same reason: a
    /// number that has to mean the same thing for ever is one this project writes down rather
    /// than borrows.
    ///
    /// The golden vector at the bottom of this test is what actually enforces it, and
    /// `docs/PHASE1.md`'s rule applies: **if that line ever fails, investigate. Do not paste in
    /// the new number.**
    ///
    /// # 2. Order has to matter
    ///
    /// Development takes the *first* gene that matches a cell, so where a gene sits in the list
    /// decides whether it ever fires - which makes SPEC section 7's reordering a real mutation
    /// with a real effect on the body. A fingerprint that combined the genes commutatively
    /// (adding them up, or exclusive-oring them together) would be blind to it, and a lineage
    /// that reordered its genome into a different creature would keep exactly the colour it
    /// had.
    ///
    /// # 3. The length has to go in before the genes do
    ///
    /// `docs/PHASE1.md` recorded this as an obligation for whoever built the first digest:
    /// *"A byte-stream hash cannot distinguish `[a, b]` + `[]` from `[a]` + `[b]`. Any
    /// variable-length sequence must write its element count before its elements. Gene
    /// duplication and deletion are precisely the operators that change those lengths."*
    ///
    /// Within a single genome of fixed-size records the ambiguity does not bite today, and the
    /// prefix goes in anyway, for two reasons. Phase 8's snapshot digest will hash several of
    /// these sequences one after another, which is exactly the case the obligation describes.
    /// And it is *observable now*, which is what the first assertion below rests on: an empty
    /// genome that fed nothing to the hash would come back holding FNV's offset basis
    /// unchanged, and it does not, because its length went in first.
    #[test]
    fn a_genomes_hash_is_stable_and_ordered_and_length_prefixed() {
        let limits = limits_with(|limits| limits.max_genes = 8);
        let gene = |n: u8| Gene {
            trigger_state: State::new(n),
            ..a_terminating_gene()
        };

        // 3. The length prefix, observed. A genome of no genes still fed something to the
        //    hash, so the answer is not the number the hash started from.
        assert_ne!(
            Genome::new(Vec::new(), &limits).hash(),
            Digest::OFFSET_BASIS,
            "an empty genome hashed nothing at all, so no length was written before the \
             genes - and a digest built this way cannot tell two sequences from one"
        );

        // 1. The same genes give the same number, and different genes do not.
        let one = Genome::new(vec![gene(1), gene(2)], &limits);
        let same = Genome::new(vec![gene(1), gene(2)], &limits);
        let changed = Genome::new(vec![gene(1), gene(3)], &limits);

        assert_eq!(
            one.hash(),
            same.hash(),
            "two genomes with the same genes in them have different fingerprints, so a hue \
             taken from this would differ between two identical clones"
        );
        assert_ne!(
            one.hash(),
            changed.hash(),
            "a point mutation on one gene left the fingerprint alone, so a lineage could \
             drift genetically without its colour drifting with it"
        );

        // 2. Order matters, because to development it does.
        let reordered = Genome::new(vec![gene(2), gene(1)], &limits);
        assert_ne!(
            one.hash(),
            reordered.hash(),
            "reordering a genome left the fingerprint alone. Reordering is one of SPEC \
             section 7's mutation operators and it changes which genes fire, so a lineage \
             that reordered itself into a different creature would keep the colour it had"
        );

        // Length changes it too, which is what duplication and deletion do.
        let duplicated = Genome::new(vec![gene(1), gene(1)], &limits);
        let deleted = Genome::new(vec![gene(1)], &limits);
        assert_ne!(
            duplicated.hash(),
            deleted.hash(),
            "duplicating a genome's only gene left the fingerprint alone"
        );
        assert_ne!(one.hash(), deleted.hash());

        // Every one of a gene's sixteen fields reaches the fingerprint. A field left out of
        // the hash is a whole dimension of the genome that lineages could drift along
        // invisibly - and the ones most likely to be forgotten are the behaviour fields at the
        // end, which nothing in development reads at all.
        let base = a_terminating_gene();
        let variants = [
            (
                "trigger_state",
                Gene {
                    trigger_state: State::new(7),
                    ..base
                },
            ),
            (
                "min_step",
                Gene {
                    min_step: 3,
                    ..base
                },
            ),
            (
                "max_step",
                Gene {
                    max_step: 4,
                    ..base
                },
            ),
            (
                "action",
                Gene {
                    action: Action::Divide,
                    ..base
                },
            ),
            ("angle", Gene { angle: 1.5, ..base }),
            (
                "adhere",
                Gene {
                    adhere: true,
                    ..base
                },
            ),
            (
                "child_state",
                Gene {
                    child_state: State::new(9),
                    ..base
                },
            ),
            (
                "child_kind",
                Gene {
                    child_kind: CellKind::Myocyte,
                    ..base
                },
            ),
            (
                "rest_length",
                Gene {
                    rest_length: 6.5,
                    ..base
                },
            ),
            (
                "stiffness",
                Gene {
                    stiffness: 11.0,
                    ..base
                },
            ),
            (
                "new_kind",
                Gene {
                    new_kind: CellKind::Gonocyte,
                    ..base
                },
            ),
            (
                "new_state",
                Gene {
                    new_state: State::new(13),
                    ..base
                },
            ),
            (
                "osc_freq",
                Gene {
                    osc_freq: 2.5,
                    ..base
                },
            ),
            (
                "osc_phase",
                Gene {
                    osc_phase: 0.75,
                    ..base
                },
            ),
            (
                "sensor_gain",
                Gene {
                    sensor_gain: -0.25,
                    ..base
                },
            ),
            (
                "sensor_target",
                Gene {
                    sensor_target: SensorTarget::Detritus,
                    ..base
                },
            ),
        ];
        let plain = Genome::new(vec![base], &limits).hash();

        assert_eq!(
            variants.len(),
            16,
            "SPEC section 7 gives a gene sixteen fields and {} are checked here",
            variants.len()
        );
        for (field, gene) in variants {
            assert_ne!(
                Genome::new(vec![gene], &limits).hash(),
                plain,
                "changing a gene's {field} left the fingerprint alone, so that field is not \
                 being hashed and a lineage can drift along it without changing colour"
            );
        }

        // ---------------------------------------------------------------------------------
        // ⭐ The golden vector. See this test's documentation: **investigate, do not paste.**
        //
        // A run recorded tonight and looked at next year has to come back the same colour,
        // and the only thing that can promise that is a number written down on the day.
        // ---------------------------------------------------------------------------------
        assert_eq!(
            Genome::new(Vec::new(), &limits).hash(),
            0xa8c7_f832_281a_39c5,
            "the fingerprint of the empty genome has moved"
        );
        assert_eq!(
            one.hash(),
            0x9944_92ab_6df4_05d0,
            "the fingerprint of a fixed pair of genes has moved, so every lineage in the \
             project has changed colour and no frame dumped before today matches the world \
             it was taken from"
        );
    }

    /// ⭐ **D5's other half.** How much of one genome is not another one, as a fraction.
    ///
    /// The fingerprint above answers *whether* a genome changed and can say nothing about how
    /// much - that is the whole of what makes it the wrong thing to identify a lineage by. SPEC
    /// section 11 asks for an offspring's marker to shift *"by a small amount, larger when the
    /// genome changed more"*, so something has to be able to say **more**, and this is it.
    ///
    /// The last claim is the one worth writing down: a gene inserted at the front comes out as a
    /// divergence of one, because every gene behind it has moved and SPEC section 7's
    /// development reads the first gene that matches. It is not an oversight in an alignment; it
    /// is what shifting the whole reading frame of a growth program actually costs.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "every figure here is a small whole number of genes divided by another, which \
                  is exact in binary at these sizes - a quarter, a fifth, one and nought - and a \
                  tolerance would wave through a divergence measure that counted the wrong genes"
    )]
    fn a_genome_can_say_how_much_of_another_one_it_is_not() {
        let limits = limits_with(|limits| limits.max_genes = 8);
        let gene = |n: u8| Gene {
            trigger_state: State::new(n),
            ..a_terminating_gene()
        };

        let four = Genome::new(vec![gene(1), gene(2), gene(3), gene(4)], &limits);

        // Nothing changed. This is what most births produce, and it is what keeps a whole
        // colony's markers in one small stretch of the circle.
        assert_eq!(
            four.divergence_from(&four.clone()),
            0.0,
            "a genome differs from an exact copy of itself"
        );
        assert_eq!(
            Genome::new(Vec::new(), &limits).divergence_from(&Genome::new(Vec::new(), &limits)),
            0.0,
            "two genomes with no genes in them differ, which is a division by nothing waiting \
             to happen"
        );

        // One gene of four, changed in place.
        let one_of_four = Genome::new(vec![gene(1), gene(9), gene(3), gene(4)], &limits);
        assert_eq!(
            four.divergence_from(&one_of_four),
            0.25,
            "a point mutation on one gene of four is not a quarter of the genome"
        );

        // The same mutation on a genome of one is the whole of it, which is the property that
        // makes an elaborate lineage's marker steadier than a plain one's.
        let single = Genome::new(vec![gene(1)], &limits);
        assert_eq!(
            single.divergence_from(&Genome::new(vec![gene(9)], &limits)),
            1.0
        );

        // A gene added at the end is one position of five.
        let longer = Genome::new(vec![gene(1), gene(2), gene(3), gene(4), gene(5)], &limits);
        assert_eq!(
            four.divergence_from(&longer),
            0.2,
            "appending a fifth gene to four is not one position of five"
        );
        assert_eq!(
            longer.divergence_from(&four),
            four.divergence_from(&longer),
            "a genome is a different distance from another than that one is from it"
        );

        // ⚠️ And one inserted at the *front* moves everything behind it.
        let shifted = Genome::new(vec![gene(9), gene(1), gene(2), gene(3), gene(4)], &limits);
        assert_eq!(
            four.divergence_from(&shifted),
            1.0,
            "a gene inserted at the front of a genome left most of it reading as unchanged, \
             which is a claim that the growth program is mostly the same one - and development \
             takes the first gene that matches, so it may not be"
        );
    }

    // ---------------------------------------------------------------------------------
    // Properties
    // ---------------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Whatever seed it is drawn from, and whatever step budget the run has, a random
        /// gene is a usable gene.
        ///
        /// This is the test the mutation operators lean on. Group C inserts random genes
        /// into living genomes, and a random gene with a state outside `0..=63` or a step
        /// window running past the end of development is not a mutation - it is a gene that
        /// silently does nothing, in an operator whose whole purpose is to do something.
        ///
        /// Four claims, and the last two are the ones that would not be missed if they
        /// broke:
        ///
        /// Every state is in range, which the type guarantees and this confirms is not
        /// being bypassed. Every number is finite and inside the bounds this module
        /// declares. The step window is the right way round *and* inside the run's budget,
        /// so the gene can actually fire. And the gain is genuinely signed - across enough
        /// draws both signs must appear, because a generator that quietly produced only
        /// positive gains would leave avoidance unreachable by insertion and nothing else
        /// in the project would notice.
        #[test]
        fn a_random_gene_is_always_within_bounds(seed: u64, budget in 1u32..=255) {
            let limits = limits_with(|limits| limits.max_dev_steps = budget);
            let last = u8::try_from(budget - 1).expect("a budget of at most 255 is a byte");
            let mut rng = ChaCha8Rng::seed_from_u64(seed);

            let mut seen_negative_gain = false;
            let mut seen_positive_gain = false;

            for _ in 0..64 {
                let gene = Gene::random(&mut rng, &limits);

                prop_assert!(gene.trigger_state.get() < State::COUNT);
                prop_assert!(gene.child_state.get() < State::COUNT);
                prop_assert!(gene.new_state.get() < State::COUNT);

                prop_assert!(
                    gene.min_step <= gene.max_step,
                    "a random gene was born with its step window inverted, so it can \
                     never fire: {}..={}",
                    gene.min_step,
                    gene.max_step,
                );
                prop_assert!(
                    gene.max_step <= last,
                    "a random gene named step {} in a run that only takes {}",
                    gene.max_step,
                    budget,
                );

                prop_assert!((-PI..=PI).contains(&gene.angle));
                prop_assert!((0.0..=MAX_REST_LENGTH).contains(&gene.rest_length));
                prop_assert!((0.0..=MAX_STIFFNESS).contains(&gene.stiffness));
                prop_assert!((0.0..=MAX_OSC_FREQ).contains(&gene.osc_freq));
                prop_assert!((0.0..=TAU).contains(&gene.osc_phase));
                prop_assert!(gene.sensor_gain.abs() <= MAX_SENSOR_GAIN);

                seen_negative_gain |= gene.sensor_gain < 0.0;
                seen_positive_gain |= gene.sensor_gain > 0.0;
            }

            prop_assert!(
                seen_negative_gain && seen_positive_gain,
                "sixty-four random genes did not produce sensor gains of both signs, so \
                 insertion can reach attraction or avoidance but not both"
            );
        }
    }
}
