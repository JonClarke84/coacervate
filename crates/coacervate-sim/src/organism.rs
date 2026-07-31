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

use crate::cell::{Cell, Vec2};
use crate::config::{LimitsConfig, WorldConfig};
use crate::development::Body;
use crate::genome::Genome;
use crate::physics::{Spring, wrapped};
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
    /// reaching zero, or age passing a limit the genome decides. `metabolism.rs` derives that
    /// limit from what the body costs to run.
    ///
    /// It answers a second question too, and `reproduction.rs` is where the reasoning is: an
    /// organism only breeds once it has been alive for a tick. A body born during this tick has
    /// nought here, and a body that was already in the world has at least one, so the two can be
    /// told apart without anything having to keep a list of who is new.
    age: u64,

    /// Which organism this is, for as long as the run lasts.
    ///
    /// Not the slot. A slot is a place in the arena and is handed on to whoever is born there
    /// next; a serial is minted once, never reused, and is what names this organism's private
    /// sequence of random numbers. Two organisms that lived in the same slot at different
    /// times are different organisms and must not draw the same numbers.
    serial: u64,

    /// Which organism this one was born to, or nothing at all if it was seeded from outside.
    ///
    /// A serial rather than a slot, and that is the whole reason this field can exist at all: a
    /// slot is a place and is handed on to whoever is born there next, so a parent recorded as
    /// a slot would silently come to name whichever stranger moved in after the parent died -
    /// which for most organisms happens well within their own lifetime.
    ///
    /// # Why "no parent" is `None` rather than a zero
    ///
    /// Because serials start at nought, so a zero meaning "founder" is the same value as "the
    /// child of the very first organism in the run". Nothing about that fails. A lineage tree
    /// simply comes out with every root joined to one arbitrary body, and SPEC section 11's
    /// species naming would descend the whole world from a single founder that happened to be
    /// seeded first.
    ///
    /// SPEC section 12 is what wants this: hue from lineage. There is no lineage without a link
    /// back, and until now an organism had a name and a genome and no way of saying where it
    /// came from - two bodies could be twins or strangers and nothing in the world could tell.
    parent: Option<u64>,

    /// A fingerprint of the genome this organism is running. See [`Genome::hash`].
    ///
    /// Taken once, when the organism is made, rather than worked out when it is asked for. A
    /// genome does not change during a life - reproduction copies the *program* and mutates the
    /// copy - so there is nothing for a stored fingerprint to fall out of step with, and the
    /// alternative is walking up to 128 genes of sixteen fields apiece for every organism on
    /// every frame.
    ///
    /// ⚠️ **It is not what a lineage is identified by from a distance**, and Group B found out
    /// why by drawing a frame that way. A fingerprint does not drift; it jumps. Any mutation at
    /// all - a change to one gene's angle in the fourth decimal - rerolls every bit of it, so a
    /// parent and its child were as far apart as two strangers. See [`marker`](Organism::marker),
    /// which is what carries a lineage's identity instead, and `docs/frames/phase5-groupb.png`,
    /// which is what a renderer reading this one produced.
    ///
    /// What it *is* good for is the two things a fingerprint is right for: telling whether a
    /// genome is still the one it came from, and being the well-mixed number that decides which
    /// way a newborn's marker moves.
    genome_hash: u64,

    /// ⭐ Where this lineage stands on a circle it walks slowly round.
    ///
    /// A number between nought and one, wrapping at both ends. **It is inherited rather than
    /// computed**: an offspring takes its parent's and shifts it by a small amount, larger when
    /// the genome changed more - see [`drifted_marker`]. A founder is given one by
    /// [`founding_marker`] and nothing else ever puts one here.
    ///
    /// SPEC section 11 spells out the property this exists for and the reason the obvious
    /// alternative fails: *"**inherited, not computed**… Lineages drift as they drift genetically
    /// — you can literally watch speciation happen, because a splitting lineage comes apart as a
    /// gradient rather than as a jump."*
    ///
    /// # ⚠️ This is a fact about descent and it is deliberately nothing else
    ///
    /// CLAUDE.md: *"The simulation crate must not know that rendering exists."* So it is named
    /// for what it is - **a marker a lineage carries and passes on with a small error** - and not
    /// for what SPEC section 11 happens to use it as. The same number would serve a lineage tree,
    /// a species clustering, or a printed report on a machine with no screen in it. What a
    /// renderer does with it is the renderer's business, and `coacervate-render`'s `scene.rs` is
    /// the one file in the project that knows.
    marker: f32,

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
    ///
    /// `parent` is the serial of the organism this one was born to, or nothing at all if it was
    /// seeded from outside. There are only two callers - `world.rs`'s [`World::seed`], which
    /// always passes nothing, and `reproduction.rs`, which always passes a parent - so the
    /// distinction between a founder and a child is made at the two doors an organism can come
    /// through and nowhere else.
    ///
    /// The genome's fingerprint is taken here rather than asked for, so that there is no moment
    /// at which an organism exists with a fingerprint that is not its genome's.
    ///
    /// [`World::seed`]: crate::world::World::seed
    #[must_use]
    pub(crate) fn new(
        genome: Genome,
        energy: f64,
        serial: u64,
        parent: Option<u64>,
        marker: f32,
        cells: usize,
        springs: usize,
    ) -> Self {
        Self {
            genome_hash: genome.hash(),
            genome,
            energy,
            age: 0,
            serial,
            parent,
            marker,
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

    /// Which organism it was born to, or nothing at all if it was seeded from outside.
    #[must_use]
    pub fn parent(&self) -> Option<u64> {
        self.parent
    }

    /// A fingerprint of the genome it is running. See [`Genome::hash`].
    #[must_use]
    pub fn genome_hash(&self) -> u64 {
        self.genome_hash
    }

    /// Where its lineage stands on the circle it walks slowly round. See [`Organism::marker`].
    #[must_use]
    pub fn marker(&self) -> f32 {
        self.marker
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
    /// This is why the field is private and why there is no setter. The only caller is
    /// `behaviour.rs`, which does every one of its movements in one place for exactly this
    /// reason. The two ways an organism *starts* holding something - being seeded, and being
    /// born - both hand the figure to [`Organism::new`] instead, so there is no moment at which
    /// an organism exists holding nothing and is then topped up.
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

/// How far round the circle a lineage moves when its whole growth program has changed.
///
/// ⭐ **The number that decides whether speciation can be seen**, and both directions of getting
/// it wrong are easy to picture. Too large and a parent and its child land as far apart as two
/// strangers, which is Group B's fingerprint arrived at by a longer route. Too small and a
/// lineage that has been diverging for twenty thousand ticks is still exactly where it was
/// seeded, so a population that has split into two is one point.
///
/// A twelfth of the circle, and it is the *most* a single birth can move - the step is this
/// multiplied by how much of the genome actually changed and by a direction between minus one
/// and one, so the average birth that mutates at all moves about a twentieth of the way round
/// and the great majority of births, which copy the genome exactly, move nothing.
///
/// Measured against a real run rather than chosen: at SPEC section 3's shipped mutation rates
/// about a sixth of births change the genome, so a lineage twenty thousand ticks old has taken a
/// few dozen steps of a random walk and its descendants occupy roughly a tenth of the circle -
/// a tight cluster with a gradient across it, well clear of the next lineage's.
pub const MARKER_DRIFT: f32 = 0.083;

/// The 64-bit golden-ratio step: 2^64 divided by φ, rounded to an odd number.
///
/// Multiplying a counter by this and reading the top of the product walks round the circle in
/// steps of 0.618, which is the additive recurrence that spreads a sequence of whole numbers as
/// evenly as anything can. [`founding_marker`] uses it so that the founders of a run are put
/// round the circle as far from one another as they can be got, for any number of them.
const GOLDEN_STEP: u64 = 0x9e37_79b9_7f4a_7c15;

/// A place on the circle, from the top sixteen bits of a number.
///
/// The *top*, because both callers are handed a number whose low bits are the least mixed:
/// FNV-1a's avalanche is weakest there, and an additive recurrence's bottom bits simply count.
fn on_the_wheel(bits: u64) -> f32 {
    let top = u16::try_from(bits >> 48).expect("the top sixteen bits of a u64 are a u16");

    f32::from(top) / (f32::from(u16::MAX) + 1.0)
}

/// A position brought back onto a circle of circumference one.
///
/// ⚠️ The second line is the same guard `camera.rs`'s `wrapped` carries and it is not tidying:
/// `f32::rem_euclid` of a value a hair below nought returns the modulus itself once the
/// subtraction rounds, and a marker of exactly one is a place off the end of the wheel.
fn round_the_wheel(place: f32) -> f32 {
    let inside = place.rem_euclid(1.0);

    if inside >= 1.0 { 0.0 } else { inside }
}

/// The marker a founder is seeded with.
///
/// From the serial rather than from the genome, and that is not a detail: `founding.rs` seeds
/// every founder of a run with **the same genome**, so a marker taken from the growth program
/// would put all eight of them on one point and leave nothing to tell the eight colonies they
/// grow into apart by afterwards.
///
/// The golden-ratio recurrence spaces them: two founders come out half the circle apart, three a
/// third, and any number of them are as far from one another as that number can be got. Nothing
/// after this ever consults a serial again - descent is what moves a marker from here on.
#[must_use]
pub fn founding_marker(serial: u64) -> f32 {
    on_the_wheel(serial.wrapping_mul(GOLDEN_STEP))
}

/// The marker an offspring carries: its parent's, moved a little.
///
/// ⭐ **SPEC section 11's sentence, and the correction it carries.** *"Colour is inherited, not
/// computed: an offspring takes its parent's hue and shifts it by a small amount, larger when the
/// genome changed more."* And, in as many words, why the obvious alternative is wrong: *"A hash
/// does not drift; it jumps. Any mutation at all reseeds it, so every child is a completely
/// unrelated colour from its parent. It was built that way and looked at, and the result is
/// confetti."*
///
/// So there are three parts, and each does one job:
///
/// - **The parent's marker** is where the child starts. This is the whole of "inherited": a
///   child of a child of a founder is somewhere near that founder, however far the genome has
///   travelled in between, and a lineage is therefore a *region* of the circle rather than a
///   scatter over it.
/// - **The divergence** is how far it moves - [`Genome::divergence_from`], which is nought for
///   the great majority of births, since most copies come out identical. A lineage that is not
///   changing does not move at all.
/// - **The offspring's fingerprint** is which way. It has to be *some* well-mixed number or every
///   lineage in the world would walk the same way round the circle at a speed set only by how
///   fast it was mutating, and two unrelated lineages that had mutated equally would arrive at
///   the same place. The fingerprint of the genome the child is actually running is the one
///   number to hand that is different for every child and costs nothing to get.
///
/// # ⚠️ Nothing here draws a random number, and that is the point
///
/// Every input is a value that already exists at the moment a birth happens: the parent's marker,
/// the two genomes, and the fingerprint [`Organism::new`] is about to take anyway. **Drawing a
/// step from the parent's stream would have been the obvious way to write this and it would have
/// moved every golden vector in the project** - one extra draw per birth shifts every mutation
/// after it in that lineage, so a run would stay perfectly deterministic and become
/// deterministically different, and `docs/PHASE4.md`'s figures would silently stop describing the
/// program that produced them. `a_run_produces_what_it_produced_before_group_a` is the test that
/// exists to catch exactly that, and it passes across this change unaltered.
#[must_use]
pub fn drifted_marker(parent: f32, divergence: f32, offspring_hash: u64) -> f32 {
    // Between minus one and just under one, so a lineage's marker takes a walk rather than a
    // march: successive generations are as likely to move back as on.
    let direction = on_the_wheel(offspring_hash).mul_add(2.0, -1.0);

    round_the_wheel(divergence.mul_add(MARKER_DRIFT * direction, parent))
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

/// Where a body is, as one position, in a world that joins up sideways.
///
/// ⚠️ **SPEC section 8 warns about this by name and it is worth quoting**: *"A body straddling
/// the seam has no single position. Wrapping is handled per pair, so the physics is fine, but
/// averaging cell positions to find a body's centre will put that centre in the middle of the
/// world for a body sitting on the join. Species clustering, rendering and the inspector all
/// need to know this."*
///
/// The failure is not subtle once you have seen it. A two-celled body with one cell two units
/// inside the left-hand edge and one two units inside the right is an entirely ordinary body -
/// its cells are four units apart, the physics measures them the short way round, the spring
/// between them sits at rest. Average the two `x` coordinates and the answer is x = 1024 in a
/// world 2048 wide: the middle of the ocean, a thousand units from either cell and from
/// anything the body is touching. A camera following it would sit somewhere the body has never
/// been, and a species clustering on positions would file a lineage living on the seam beside
/// one living nowhere near it.
///
/// # The answer is an angle, because a horizontal position is a point on a circle
///
/// The world joins up sideways, so `x` is not a number on a line. It is a **bearing**, and the
/// mean of several bearings is not the mean of the numbers naming them - north-by-a-degree and
/// north-less-a-degree average to *south* if you treat 1 and 359 as ordinary numbers.
///
/// So each cell's `x` is read as an angle round the world, the unit vectors pointing at those
/// angles are averaged, and the direction of the result is turned back into an `x`. Cells
/// either side of the seam point in nearly the same direction, so their average points that way
/// too, and the centre comes back on the seam where it belongs. It is the standard circular
/// mean and it needs no anchor cell, no "which way round is shorter" decision, and no special
/// case for the seam - a body in open water gets the same answer the plain average would give
/// it.
///
/// The depth is left as a plain average, because SPEC section 8 closes the world top and
/// bottom. There is no join to fall over there.
///
/// # ⚠️ This uses trigonometry, so it must never be read back into the simulation
///
/// `docs/PHASE3.md`'s open question Q8 records that trigonometry and logarithms are the
/// operations IEEE 754 does not pin, so two toolchains may disagree in the last bits of a sine.
/// That is harmless here and would not be harmless anywhere else: **nothing computed by this
/// function may be fed back into a tick.** It is for looking at a world - a camera, an
/// inspector, the clustering that names species - and every one of those is a reading rather
/// than a cause. If a later phase ever wants a body's centre to *decide* something the physics
/// sees, this is not the function to use.
///
/// # The two degenerate answers
///
/// A body of no cells - which is what an empty slot has - answers the origin. There is nothing
/// else it could say, and refusing would put a failure case into every caller for a question
/// that has no wrong answer.
///
/// Cells spread evenly all the way round the world average to no direction at all, and the
/// answer is then the left-hand edge. That is arbitrary and it is *deterministically* arbitrary,
/// which is the only property that matters: such a body genuinely has no centre, and the same
/// world must give the same answer twice. A body cannot reach that state - `genome.rs` bounds a
/// chain of 64 cells at under 900 world units against a world 2048 wide - but the species
/// clustering this will be shared with can hand it any set of positions at all.
#[must_use]
pub fn body_centre(cells: &[Cell], width: f32) -> Vec2 {
    use std::f64::consts::TAU;

    if cells.is_empty() {
        return Vec2::ZERO;
    }

    let width = f64::from(width);
    let mut eastward = 0.0f64;
    let mut northward = 0.0f64;
    let mut depth = 0.0f64;

    for cell in cells {
        let bearing = f64::from(cell.pos.x) / width * TAU;
        eastward += bearing.cos();
        northward += bearing.sin();
        depth += f64::from(cell.pos.y);
    }

    let count = f64::from(u32::try_from(cells.len()).expect("a body is not four billion cells"));

    // ⚠️ `atan2` answers between -π and π, **not** between nought and 2π, so the bearing comes
    // back as a position between minus half the world and plus half of it and `wrapped` is what
    // brings it inside. It is the one line of this that looks like tidying and is not: without
    // it a body sitting exactly on the seam comes out a hair *below* nought - measured at
    // -2.8e-16 - which is a position outside the world, and everything downstream of here, the
    // spatial hash and the camera's own wrap included, is written on the assumption that no such
    // position exists. `physics.rs` gives the same warning about the same value.
    //
    // It answers zero for a direction of nothing at all, which is the degenerate case above.
    let bearing = northward.atan2(eastward);

    Vec2::new(
        wrapped(narrowed(bearing / TAU * width), narrowed(width)),
        narrowed(depth / count),
    )
}

/// Turn a quantity worked out at full precision into one the world stores.
///
/// The same conversion, for the same reason, as the ones in `grid.rs` and `behaviour.rs`: the
/// standard library offers no `TryFrom` between the two sizes of number, so a project that
/// forbids lossy casts writes it out with the reasoning attached. Everything that goes through
/// this one is a position inside a world a few thousand units across, so there is nothing here
/// to overflow; what it gives up is digits, which is what SPEC section 2 asks of everything the
/// simulation holds.
fn narrowed(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the values are positions inside a world a few thousand units across, so \
                  there is nothing here to overflow; the lost digits are what SPEC section 2 \
                  asks for"
    )]
    let narrow = value as f32;
    narrow
}

/// Write a grown body into the stretch of the world's arenas its slot owns, with its seed cell
/// at `at`.
///
/// There are two ways an organism arrives in the world - somebody seeds it, or a parent has
/// one - and this is the part they share. It lives here because it is entirely about the slot
/// arithmetic this module owns: the same range of the same two arrays, whichever way the body
/// got there. Written out twice it would be two places to fix on the day a slot changes shape,
/// and one of them would be missed.
///
/// # Turning a shape into a position
///
/// `development.rs` grows a body as a set of offsets from its seed cell, which is a shape and
/// not a place. Every cell lands at `at` plus its own offset, brought inside the world by SPEC
/// section 8's boundaries: sideways it joins up, and top and bottom it stops. The clamp is
/// against cell *centres*, which is the literal reading SPEC section 8 gives and says it is
/// giving - a cell resting on the floor has half of itself below the world.
///
/// Doing it here rather than leaving it to the physics' first tick matters for a birth in a way
/// it never did for a seeding: a newborn is placed beside its parent, and its parent may be
/// standing on the seam. A daughter cell budded eastward from a body at the world's right-hand
/// edge belongs at the left-hand edge, and a body laid down without this would have cells
/// outside the world for one tick - long enough for the neighbour search, which assumes
/// otherwise.
///
/// # Nothing here is told whether the slot was free
///
/// It is the caller's business, and both callers check. `world.rs` writes a seeded body into a
/// free slot before it knows whether the water can pay for it, deliberately, because the
/// alternative is a spare copy of every body for the sake of the seedings that fail.
pub(crate) fn lay_out(
    slot: usize,
    body: &Body,
    at: Vec2,
    world: &WorldConfig,
    limits: &LimitsConfig,
    cells: &mut [Cell],
    springs: &mut [Spring],
) {
    let first_cell = cell_slot(slot, limits).start;
    let first_spring = spring_slot(slot, limits).start;

    for (local, grown) in body.cells.iter().enumerate() {
        let put = at + grown.offset;
        let mut cell = Cell::new(
            grown.kind,
            Vec2::new(wrapped(put.x, world.width), put.y.clamp(0.0, world.height)),
        );
        cell.state = grown.state.get();
        cells[first_cell + local] = cell;
    }

    for (local, spring) in body.springs.iter().enumerate() {
        springs[first_spring + local] = *spring;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellKind;
    use crate::config::spec_defaults;
    use rand::Rng;

    /// The centre a naive reader would compute: the plain average of the cells' positions,
    /// with no idea that the world joins up.
    ///
    /// Written out here so that the test below can say what it is *rejecting* rather than only
    /// what it accepts. Every assertion about the seam compares against this number, so a
    /// [`body_centre`] that quietly went back to averaging would fail with the two figures side
    /// by side.
    fn naive_centre(cells: &[Cell]) -> Vec2 {
        let count = f32::from(u16::try_from(cells.len()).expect("a body is not 65,536 cells"));
        let total = cells
            .iter()
            .fold(Vec2::ZERO, |running, cell| running + cell.pos);

        total.scaled(1.0 / count)
    }

    /// Two cells standing where you put them, which is all [`body_centre`] reads.
    fn cells_at(places: &[(f32, f32)]) -> Vec<Cell> {
        places
            .iter()
            .map(|&(x, y)| Cell::new(CellKind::Photocyte, Vec2::new(x, y)))
            .collect()
    }

    /// ⭐ **A4.** A body sitting on the seam has its centre **on the seam**, and not in the
    /// middle of the world.
    ///
    /// SPEC section 8 warns about this in as many words: *"A body straddling the seam has no
    /// single position. Wrapping is handled per pair, so the physics is fine, but averaging
    /// cell positions to find a body's centre will put that centre in the middle of the world
    /// for a body sitting on the join. Species clustering, rendering and the inspector all need
    /// to know this."*
    ///
    /// It is worth being concrete about what the failure looks like, because it is not a crash
    /// and it is not subtle once you see it. A two-celled body with one cell just inside the
    /// left-hand edge and one just inside the right-hand edge is a perfectly ordinary body -
    /// its cells are four units apart, the physics measures them the short way round, the
    /// spring between them is at rest. Average the two x coordinates and the answer is the
    /// middle of the world, a thousand units from either cell and from anything else the body
    /// is touching. A camera following that body would sit somewhere the body has never been; a
    /// species clustering on it would put a lineage living on the seam in the same group as one
    /// living in the middle of the ocean.
    ///
    /// # What is done instead, and why the answer is an angle
    ///
    /// The world joins up sideways, so a horizontal position is not a number on a line - it is
    /// a **position on a circle**, and the mean of positions on a circle is not the mean of the
    /// numbers naming them. The standard answer is to treat each `x` as an angle, average the
    /// unit vectors pointing at those angles, and take the direction of the result. Two cells
    /// either side of the seam point in nearly the same direction, so their average points that
    /// way too, and the answer comes back on the seam where it belongs.
    ///
    /// The depth is left as a plain average, because SPEC section 8 closes the world top and
    /// bottom. There is no seam to fall over there.
    ///
    /// # Every case here is checked against the naive answer as well
    ///
    /// A test that only checked the seam case would pass against something that always answered
    /// zero. So the ordinary case has to *agree* with the plain average - a body in open water
    /// has an unremarkable centre and the wrapping must not disturb it - and the seam case has
    /// to disagree with it, by most of the width of the world.
    #[test]
    fn a_body_has_a_position_that_wraps() {
        let width = 2048.0;

        // --- open water: the wrapping must not disturb an ordinary body ---
        let ordinary = cells_at(&[(100.0, 200.0), (108.0, 220.0), (116.0, 240.0)]);
        let centre = body_centre(&ordinary, width);

        assert!(
            (centre - naive_centre(&ordinary)).length() < 0.01,
            "a body in open water has its centre at {centre:?} and the plain average is {:?}; \
             wrapping is only supposed to change the answer at the seam",
            naive_centre(&ordinary)
        );

        // --- the seam: two cells four units apart, with the join between them ---
        let straddling = cells_at(&[(width - 2.0, 500.0), (2.0, 500.0)]);
        let centre = body_centre(&straddling, width);
        let naive = naive_centre(&straddling);

        assert!(
            (0.0..width).contains(&centre.x),
            "the centre is at x = {}, which is outside a world {width} wide",
            centre.x
        );
        assert!(
            centre.x.min(width - centre.x) < 0.5,
            "a body with one cell two units inside the left edge and one two units inside the \
             right has its centre at x = {}. The plain average says {} - the middle of the \
             world, a thousand units from either cell - and this answer is no better",
            centre.x,
            naive.x
        );
        assert!(
            (centre.y - 500.0).abs() < 0.01,
            "both cells are at depth 500 and the centre came out at {}",
            centre.y
        );
        assert!(
            (centre.x - naive.x).abs() > width * 0.4,
            "the wrapped centre and the plain average agree to within {}, so whatever is \
             being computed is still an average of the coordinates",
            (centre.x - naive.x).abs()
        );

        // --- and the same body, walked round the world, keeps the same shape of answer ---
        //
        // A body two units either side of the *quarter* mark is nowhere near the seam, so the
        // plain average is right for it. Shifting the seam case by a quarter of the world must
        // give exactly that answer shifted by a quarter, or the wrap is only being handled at
        // one particular place.
        let elsewhere = cells_at(&[(width * 0.25 - 2.0, 500.0), (width * 0.25 + 2.0, 500.0)]);
        let there = body_centre(&elsewhere, width);

        assert!(
            (there.x - width * 0.25).abs() < 0.5,
            "the same body a quarter of the way round the world has its centre at {}, and a \
             quarter of {width} is {}",
            there.x,
            width * 0.25
        );

        // --- a whole body across the join, at the positions `lay_out` gives it ---
        //
        // A three-celled chain eight units between cells, put down with its seed cell four
        // units inside the right-hand edge. Its coordinates are run through [`wrapped`], which
        // is the same call `lay_out` makes, so these are the numbers a real body would actually
        // hold: 2044, 4 and 12. Two of the three are on the far side of the world from the
        // first, and the answer has to be a few units past the join rather than anywhere near
        // the average of those figures.
        let seed = width - 4.0;
        let chain = cells_at(&[
            (wrapped(seed, width), 600.0),
            (wrapped(seed + 8.0, width), 600.0),
            (wrapped(seed + 16.0, width), 600.0),
        ]);
        let centre = body_centre(&chain, width);

        assert!(
            centre.x.min(width - centre.x) < 20.0,
            "a three-celled chain laid down four units inside the right-hand edge has cells \
             at {:?} and its centre came out at x = {}",
            chain.iter().map(|cell| cell.pos.x).collect::<Vec<f32>>(),
            centre.x
        );

        // --- and the degenerate cases answer rather than crash ---
        assert_eq!(
            body_centre(&[], width),
            Vec2::ZERO,
            "a slot with nobody in it has no cells, and asking where its body is has to \
             answer something rather than divide by nothing"
        );
        let one = cells_at(&[(1234.0, 567.0)]);
        assert!(
            (body_centre(&one, width) - Vec2::new(1234.0, 567.0)).length() < 0.01,
            "a body of one cell is centred on that cell"
        );
    }

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
        let mut organism = Organism::new(
            Genome::new(Vec::new(), &limits),
            0.0,
            7,
            None,
            founding_marker(7),
            1,
            0,
        );

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
