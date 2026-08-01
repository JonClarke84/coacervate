//! What cells actually do: SPEC section 6's table, made real.
//!
//! See `SPEC.md` sections 6 and 9 for the rules this implements.
//!
//! Everything before this file builds a world and puts bodies in it. Nothing in it is alive:
//! a cell has a size and an upkeep and a position, and no way to earn anything. This is the
//! **income** side of the ledger - the four kinds of cell that bring energy in or spend it on
//! moving - and it is deliberately built before the costs, because a world where everything
//! starves is a world in which no test can tell a balance problem from a missing feature.
//!
//! # What each kind of cell does, in one line each
//!
//! | Kind | What it does here |
//! | --- | --- |
//! | `Photocyte` | Draws energy out of the tile it is standing on, in proportion to what that tile holds and to how much of the light reaches it past whatever is above. |
//! | `Devorocyte` | Drains anything it is touching that is not its own body: dead biomass at the full rate, living tissue at whatever that tissue's toughness leaves. |
//! | `Myocyte` | Works the rest length of the springs it is attached to, up and down, and pays for the work. |
//! | `Sensocyte` | Reports how lopsided its surroundings are in one of light, dead biomass or other organisms. |
//! | `Sclerocyte`, `Gonocyte` | Nothing at all. Both earn their place elsewhere - one by being hard to eat, the other by being what Group C requires before an organism can reproduce. |
//!
//! **Nothing here decides anything.** No cell chooses a target, moves towards one, or knows
//! that another organism exists as anything other than a thing that is or is not its own. A
//! devorocyte in contact with foreign biomass drains it; that sentence is the whole of
//! predation. Whether hunting, fleeing, herding, armour or a herbivore/predator split ever
//! appears has to come out of *where a genome puts its cells and how it moves them*, and
//! CLAUDE.md's decision log is emphatic about why: coding any of it in would be answering in
//! advance the question the project exists to ask.
//!
//! # Two passes, and why it is not one
//!
//! Every behaviour here is worked out in a **read-only** pass over the whole crowd, and only
//! then applied. That costs a second walk and buys three things.
//!
//! It makes the answer independent of the order cells are visited in. A photocyte that read a
//! tile another photocyte had already drawn down this tick would earn a different amount
//! depending on which of the two came first, and two devorocytes biting the same organism
//! would split it by whoever the loop reached first. Neither is *wrong* exactly - the order is
//! fixed, so a run still reproduces - but both make an organism's income depend on where in
//! the arena its neighbours happen to live, which is a hidden variable nobody would think to
//! look for when a lineage does something strange.
//!
//! It keeps everything a cell reads consistent with everything else it reads. A tick is a
//! snapshot: every cell sees the world as it was at the start of the pass, not a world
//! half-way through being changed by its neighbours.
//!
//! And it is what makes running the population across the machine's cores possible later
//! without changing the result, which SPEC section 2 goes out of its way to keep available.
//!
//! # Every movement goes through the ledger, and that is not the same as the books balancing
//!
//! Phase 2 learned this twice and SPEC section 5 states it: **a conservation check cannot see
//! energy that was never declared, only energy declared wrongly.** An organism handed energy
//! that no account was told about leaves all five accounts exactly as they were, so the
//! invariant is perfectly happy while a body stands in the world holding energy nobody
//! counted.
//!
//! So there is no arithmetic anywhere in this file that adds to an organism's energy without
//! a matching ledger movement in the same breath, and the tests do not assert that the books
//! balance - they assert that the **accounts moved**, and by how much.
//!
//! # What is deliberately not here
//!
//! Upkeep, death and detritus decay live in `metabolism.rs`, and reproduction is Group C.
//! This file has one outgoing payment in it - the work a myocyte does - and it is here
//! because SPEC section 6 puts it in the same row of the same table as the thing that pays
//! for it.

use crate::cell::{Cell, CellKind, Vec2};
use crate::config::Config;
use crate::genome::{Gene, MAX_REST_LENGTH, SensorTarget};
use crate::grid::Grid;
use crate::ledger::Ledger;
use crate::organism::Organism;
use crate::physics::{DT, Spring, wrapped_offset};

/// SPEC section 6: a photocyte "harvests from the field tile it occupies, rate ∝ local
/// energy × exposure". This is the constant of proportionality that SPEC's "∝" leaves open.
///
/// A photocyte standing in full light on a tile at SPEC section 3's default ceiling of 8
/// therefore draws 0.08 a tick, against the 0.004 an unshaded photocyte costs to keep - twenty
/// times its own upkeep, which is the headroom a body needs in order to afford the cells that
/// do not feed it.
///
/// # What it does *not* decide, which is the surprising part
///
/// A lone photocyte's income at equilibrium is **not** set by this number. It draws its tile
/// down until what it takes equals what the light puts back, and what the light puts back is
/// `influx`; so its steady income is the influx and nothing else. What this rate decides is
/// how far down the tile is pulled to get there, and therefore how much of the standing stock
/// a body can turn into growth before the water around it runs thin.
///
/// Where it bites hardest is **competition**. Two organisms on one tile take shares in
/// proportion to their rates, so exposure decides who gets the light exactly when the world is
/// crowded - which is the moment it matters.
const HARVEST_RATE: f64 = 0.01;

/// How far apart two cells can be and still have anything to do with one another.
///
/// Two of the widest radius in the world, because that is the largest a pair's radii can sum
/// to. It is the same number `physics.rs` sizes its own neighbour search from, and it answers
/// both of the questions this file asks about a pair of cells - are they touching, and does
/// one stand in the other's light - because both are the same question about two discs
/// overlapping. Light falls straight down from `y = 0`, so a cell's shadow is exactly as wide
/// as the cell is and no wider.
const REACH: f32 = 2.0 * CellKind::LARGEST_RADIUS;

/// How far below a cell its shadow still reaches at all.
///
/// **Twice the longest limb a genome can grow.** SPEC gives no number for this - it gives no
/// occlusion model at all - so it is anchored to the one length in the project that already
/// means "how far one cell can be from the cell it grew from": `genome.rs`'s
/// [`MAX_REST_LENGTH`].
///
/// The reasoning is in [`Behaviour::shade`], and the short version is that the thing occlusion
/// has to be able to tell apart is a daughter placed *beside* its parent from one placed
/// *beneath* it. Shorter than one limb and a daughter directly below its parent would already
/// be in clear water, so occlusion would discriminate between nothing. Very much longer and a
/// photocyte's income would be decided by whatever happened to be drifting through the water
/// column above it rather than by the shape of its own body.
const SHADOW_DEPTH: f32 = 2.0 * MAX_REST_LENGTH;

/// What a devorocyte drains per tick out of one thing it is touching, before that thing's
/// toughness is taken off.
///
/// SPEC section 6 gives a devorocyte's function and no rate, so this is a Phase 4 number. It
/// is **one rate for both halves of the job**: dead biomass has no toughness, so a grain of
/// detritus gives up the whole of it, while a living cell gives up whatever its toughness
/// leaves. That is the whole difference between scavenging and predation in this model, which
/// is the point - nothing distinguishes the two strategies except what the victim is made of
/// and how hard it is to reach.
///
/// The size of it was chosen against the two things it has to sit between. A devorocyte costs
/// 0.009 a tick to keep, so a bite worth 0.05 is worth taking - about five times its own
/// upkeep from a single contact, and four times what a photocyte earns off a tile at
/// equilibrium. That is what makes SPEC section 10's claim true rather than merely stated: *a
/// body is a denser package of energy than the soup*, so eating one is genuinely a better
/// strategy under some conditions. And a sclerocyte's 0.9 toughness cuts it to 0.005, which is
/// **below** a devorocyte's upkeep - so armour does not slow a predator down, it makes the
/// attempt cost more than it returns.
///
/// Whether any lineage ever discovers either strategy is not decided here and must not be.
const DEVOUR_RATE: f64 = 0.05;

/// How far a sensocyte's world extends.
///
/// **The longest limb a genome can grow**, from `genome.rs`. A sensocyte senses about as far
/// as the body it belongs to can reach, which is the distance at which knowing something is
/// there is any use: a signal from further off than a body can act on in the next few ticks is
/// a signal it can do nothing with, and one from nearer than a limb's length would be a sense
/// of touch rather than a sense at all.
const SENSE_RANGE: f32 = MAX_REST_LENGTH;

/// How wide a grain of detritus is, for deciding whether a devorocyte is touching it.
///
/// SPEC gives detritus a position and an energy and no size at all. A grain has to have some
/// width or "on contact" cannot be answered, and one world unit - a third of a cell - is the
/// smallest thing that is still a thing. Group B kept it: a grain is one cell's worth of a
/// corpse, so being smaller than the cell it came from is the right shape for it.
const DETRITUS_RADIUS: f32 = 1.0;

/// What a light gradient is measured against: the half-signal mark for a sensocyte tuned to
/// [`SensorTarget::Light`].
///
/// ⭐ **Phase 7, and the reason a light sensor was worth nothing before it.** See [`sense`]
/// for the normalisation this replaces and what it cost.
///
/// Two hundredths is not a round number chosen to be one. It is **the background gradient of
/// the shipped world**, worked out from SPEC section 4's own formula rather than measured: a
/// tile's ceiling is `cap × (1 - gradient × depth)`, so at a `cap` of 8, a `gradient` of 0.75
/// and 144 rows the light falls by `8 × 0.75 / 144 = 0.042` a row, and the central difference
/// [`light_gradient`] takes is half of what lies two rows apart - which is that figure again.
/// The field settles at about half its ceiling once there is a population eating out of it,
/// so a sensocyte in open water at equilibrium reads about **0.02**.
///
/// Putting the reference *there* is what makes the number mean something: a signal of a half
/// is "the ordinary gradient of open water", below that is flatter than usual, and a tile
/// something has been grazing - which is ten to a hundred times steeper - runs up towards one
/// without ever reaching it. It is deliberately not a configuration key. A cell has no way to
/// know what `light.cap` is set to, and a sensor whose meaning moved with the weather would be
/// one whose evolved gain meant something different after every change of conditions.
const LIGHT_REFERENCE: f32 = 0.02;

/// Everything one pass of behaviour is allowed to touch that is alive.
///
/// Grouped into one argument rather than passed as six, because six references threaded
/// through a call is a place to get two of them the wrong way round.
pub(crate) struct Living<'a> {
    /// The living cells, packed together in slot order - `world.rs`'s crowd.
    pub cells: &'a mut [Cell],

    /// The adhesions between them, with endpoints numbered into `cells`.
    ///
    /// Mutable because a myocyte's whole function is to change a rest length. These are
    /// rebuilt from the arena at the start of every tick, so what is written here is this
    /// tick's contraction and never accumulates: the length a gene asked for is still in the
    /// arena, untouched.
    pub springs: &'a mut [Spring],

    /// Which organism each of those cells belongs to.
    pub owner: &'a [usize],

    /// Who is in each slot, and nothing at all in the slots that are free.
    pub organisms: &'a mut [Option<Organism>],

    /// The dead biomass on its way down - see [`Detritus`]. `metabolism.rs` fills it and
    /// empties it; this pass only eats from it and smells it.
    pub detritus: &'a mut [Detritus],
}

/// A grain of dead biomass, sinking.
///
/// A position and a quantity of energy, and nothing else - which turned out to be the whole of
/// it. Group A defined it this way as a placeholder, so that a devorocyte had something to
/// bite and a sensocyte something to smell; Group B, which makes them, sinks them and rots
/// them away, needed nothing added.
///
/// It lives here rather than in `metabolism.rs` because this is the file that *reads* one.
/// `metabolism.rs` makes grains and takes them away again, and `world.rs` owns the arena they
/// live in, but the questions asked of a grain - is a mouth touching it, which way do the dead
/// lie from here - are all asked in this file, and the type belongs with the questions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detritus {
    /// Where it is.
    pub pos: Vec2,

    /// How much energy it still holds. Counted in the ledger's `detritus` account.
    pub energy: f64,
}

/// How many buckets of a given size fit across a span of world, at least one.
///
/// Rounded down, so every bucket is at least a full reach across and two things within a reach
/// of one another are always either in the same bucket or in neighbouring ones. Exactly the
/// argument `physics.rs` makes about its own hash.
fn buckets_across(span: f32, reach: f32) -> u32 {
    let exact = f64::from(span) / f64::from(reach);

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value has been rounded down to a whole number and then clamped into \
                  the range of the type it is becoming, so the conversion neither truncates \
                  nor loses a sign - the clamp is what makes it total"
    )]
    let buckets = exact.floor().clamp(1.0, f64::from(u32::MAX)) as u32;

    buckets
}

/// Which of a row (or column) of buckets a coordinate falls in.
///
/// The clamp is what makes this total: a cell resting exactly on the floor has `y` equal to
/// the world's height, which multiplied out lands one past the last bucket.
fn bucket_along(coordinate: f32, per_unit: f64, last: f64) -> usize {
    let exact = f64::from(coordinate) * per_unit;

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value has been rounded down to a whole number and then clamped \
                  between nought and the index of the last bucket, so the conversion is \
                  exact and in range for any coordinate whatever"
    )]
    let bucket = exact.floor().clamp(0.0, last) as usize;

    bucket
}

/// How many buckets either side of its own a search of this reach has to look at.
fn buckets_within(distance: f32, per_unit: f64, most: usize) -> usize {
    let exact = (f64::from(distance) * per_unit).ceil();

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a distance is never negative and the value is clamped to the number of \
                  buckets in the world before it is converted, so the conversion is exact \
                  and in range"
    )]
    let buckets = exact.clamp(0.0, f64::from(u32::MAX)) as usize;

    buckets.min(most)
}

/// A uniform grid of buckets over the world, holding which of a set of things are in each.
///
/// The same structure and the same counting sort as the one in `physics.rs`, and it is a
/// second copy rather than a shared one for a reason worth stating: the physics asks one
/// question - "who is close enough to touch me" - and asks it of a hash it rebuilds inside its
/// own tick, whereas this file asks four questions of four different shapes and has to ask
/// them *before* the physics has run. A single structure answering both would be a structure
/// with a lifetime split across two modules and a query shape general enough to be nobody's.
///
/// # It indexes positions rather than cells, and that is Group B's doing
///
/// Group A built it over the crowd. Group B has a second population to search - the drift of
/// dead biomass - and two of this file's questions are asked of that instead: which grains a
/// devorocyte is touching, and which way the dead lie from a sensocyte. Written against
/// `&[Cell]` this structure could not be pointed at either, and the alternative was a third
/// copy of the same counting sort, which is how a project ends up with three versions of one
/// rule and no idea which of them is right. So [`Neighbourhood::rebuild`] takes a count and a
/// way of asking where the nth thing is, and neither the sort nor the search knows what it is
/// sorting.
///
/// What is *not* duplicated is the rule about the world wrapping sideways:
/// [`crate::physics::wrapped_offset`] is shared, because two versions of that would eventually
/// disagree at the join and nothing would say so.
///
/// # Why the buckets are the size they are
///
/// One reach of the collision force across - the widest two cells' radii summed. That is the
/// narrowest of the three questions asked of it, so it is the one that decides: a wider bucket
/// would hand the contact search a great many candidates it has no use for, and every question
/// asked of a bucketed world can look at more buckets but cannot look at less than one.
struct Neighbourhood {
    cols: usize,
    rows: usize,

    /// Buckets per world unit, across and down, and the index of the last in a row and in a
    /// column. Precomputed so that placing a cell is a multiplication rather than a division.
    across_per_unit: f64,
    down_per_unit: f64,
    last_col: f64,
    last_row: f64,

    /// Which bucket each thing landed in, from the most recent rebuild.
    bucket_of: Vec<usize>,

    /// Where each bucket's run begins in `order`, one entry longer than there are buckets so
    /// the last run needs no special case.
    starts: Vec<usize>,

    /// Working room for the counting sort: each bucket's next free slot as it fills.
    cursor: Vec<usize>,

    /// Everything indexed, in bucket order. This is the sorted result a search reads.
    order: Vec<usize>,

    /// How many things went into the most recent rebuild.
    ///
    /// Kept only so that an index over *nothing* can be recognised in one comparison, and it
    /// earns its place: a rebuild clears and prefix-sums one entry per bucket, which for the
    /// default world is fifty thousand of them, on every tick, whether or not there is
    /// anything to sort into them. The drift is empty for the whole of a world that has not
    /// killed anything yet - which is every tick of the field-only conservation tests, and the
    /// opening stretch of every real run - and paying a fifty-thousand-entry sweep for it
    /// **doubled the cost of a hundred thousand ticks**, from 27 seconds to 52. Measured.
    count: usize,
}

impl Neighbourhood {
    /// Lay buckets over the world a configuration describes, once, with room for `capacity`
    /// things in them.
    fn new(config: &Config, capacity: usize) -> Self {
        let cols_across = buckets_across(config.world.width, REACH);
        let rows_down = buckets_across(config.world.height, REACH);
        let cols = usize::try_from(cols_across).expect("a bucket count fits in a machine word");
        let rows = usize::try_from(rows_down).expect("a bucket count fits in a machine word");

        Self {
            cols,
            rows,
            across_per_unit: f64::from(cols_across) / f64::from(config.world.width),
            down_per_unit: f64::from(rows_down) / f64::from(config.world.height),
            last_col: f64::from(cols_across - 1),
            last_row: f64::from(rows_down - 1),
            bucket_of: vec![0; capacity],
            starts: vec![0; cols * rows + 1],
            cursor: vec![0; cols * rows],
            order: vec![0; capacity],
            count: 0,
        }
    }

    /// Sort `count` things into their buckets, throwing away whatever was here before.
    ///
    /// `at` says where the nth of them is, and is the whole of what this needs to know about
    /// them: the crowd hands it a cell's position and the drift hands it a grain's.
    fn rebuild(&mut self, count: usize, at: impl Fn(usize) -> Vec2) {
        assert!(
            count <= self.bucket_of.len(),
            "the behaviour pass was built with room for {} of these and was handed {count}",
            self.bucket_of.len()
        );

        // An index over nothing is left untouched rather than swept clear, and [`Neighbourhood::near`]
        // knows not to read it. See the note on `count`.
        self.count = count;
        if count == 0 {
            return;
        }

        let buckets = self.cols * self.rows;
        let (cols, across, down) = (self.cols, self.across_per_unit, self.down_per_unit);
        let (last_col, last_row) = (self.last_col, self.last_row);

        self.starts[..=buckets].fill(0);

        for index in 0..count {
            let pos = at(index);
            let bucket =
                bucket_along(pos.y, down, last_row) * cols + bucket_along(pos.x, across, last_col);

            self.bucket_of[index] = bucket;
            self.starts[bucket + 1] += 1;
        }

        for bucket in 1..=buckets {
            self.starts[bucket] += self.starts[bucket - 1];
        }
        self.cursor[..buckets].copy_from_slice(&self.starts[..buckets]);

        let Self {
            bucket_of,
            cursor,
            order,
            ..
        } = self;
        for (index, &bucket) in bucket_of[..count].iter().enumerate() {
            order[cursor[bucket]] = index;
            cursor[bucket] += 1;
        }
    }

    /// Hand back everything indexed inside the box `half_width` either side of `at`, `above`
    /// up and `below` down of it.
    ///
    /// Generous: what comes back is everything in every bucket the box touches, most of which
    /// is further away than was asked for. That is the bargain the whole structure makes, and
    /// the caller does the precise test - one subtraction and a comparison, paid to avoid
    /// asking the question of everything in the world.
    ///
    /// Whatever is at `at`, if it is in the index, comes back too. Callers skip it themselves
    /// rather than being handed a special case for it.
    ///
    /// The order is fixed and repeatable: down the rows from the top, across the columns from
    /// the left, and in ascending index within each bucket. That is what lets a sum built out
    /// of what this returns be the same sum in two runs of the same seed.
    fn near(
        &self,
        at: Vec2,
        half_width: f32,
        above: f32,
        below: f32,
        mut visit: impl FnMut(usize),
    ) {
        // Nothing was sorted, so the buckets say nothing and must not be read: what is in them
        // is whatever the last rebuild that had something to sort left behind.
        if self.count == 0 {
            return;
        }

        let col = bucket_along(at.x, self.across_per_unit, self.last_col);
        let row = bucket_along(at.y, self.down_per_unit, self.last_row);

        // Sideways the world joins up, so the columns are walked modulo the world's width -
        // and the count is capped at the number of columns there are, because a search wider
        // than the world would otherwise visit the same column twice and count everything in
        // it twice with it.
        let sideways = buckets_within(half_width, self.across_per_unit, self.cols);
        let columns = (2 * sideways + 1).min(self.cols);
        let leftmost = (col + self.cols - sideways % self.cols) % self.cols;

        // Downwards there is no wrap: the world has a surface and a floor, and the search
        // simply stops at them.
        let top = row.saturating_sub(buckets_within(above, self.down_per_unit, self.rows));
        let bottom =
            (row + buckets_within(below, self.down_per_unit, self.rows)).min(self.rows - 1);

        for band in top..=bottom {
            for step in 0..columns {
                let bucket = band * self.cols + (leftmost + step) % self.cols;

                for &other in &self.order[self.starts[bucket]..self.starts[bucket + 1]] {
                    visit(other);
                }
            }
        }
    }
}

/// Everything the behaviour pass needs that is not the world itself, allocated once.
///
/// CLAUDE.md: a simulation that cannot allocate cannot leak. Every array here is built at the
/// size the configuration implies - one entry per cell the world could ever hold, or one per
/// organism - and none of them can grow. At SPEC section 3's defaults that is 256,000 cells at
/// twenty bytes apiece and four thousand organisms at thirty-two, so about **5 MB**, and the
/// two neighbourhoods cost 5 MB each. Sixteen megabytes all told, against the 61 MB `world.rs`
/// accounts for and CLAUDE.md's resident target of 2 GB.
pub struct Behaviour {
    /// SPEC section 3's `metabolism.movement_cost`: what a unit of work costs to do.
    movement_cost: f64,

    /// SPEC section 3's `[behaviour]` table: how hard a muscle works with nothing telling it
    /// otherwise, and how much of its rest length a fully-driven one works through.
    ///
    /// ⭐⭐ Both were constants in this file until Phase 7's Group H, and the second of them was
    /// **the** number deciding whether a muscle was worth owning: a body's speed goes as
    /// roughly the cube of it. See [`crate::config::BehaviourConfig`] for the measurement and
    /// for why the stroke stops at one.
    resting_amplitude: f32,
    stroke: f32,

    /// How wide the world is, for measuring the short way round it.
    width: f32,

    /// Which cells are near which.
    hash: Neighbourhood,

    /// Which *grains* are near which, which is the same structure over the other population.
    ///
    /// A second index rather than one shared with the cells, because the two are searched
    /// with different questions and a bucket holding both would hand every contact test a pile
    /// of candidates of the wrong kind. It is built at the drift's full capacity - one grain
    /// per cell the world can hold - so it can never be handed more than it has room for.
    drift: Neighbourhood,

    /// How much each photocyte is about to ask its tile for.
    ///
    /// Filled in the read-only pass and spent in the commit pass. This is the array that
    /// makes two photocytes on one tile take shares of what was there at the start of the
    /// tick rather than of whatever the earlier one left.
    want: Vec<f64>,

    /// What each organism has taken in this tick, and what has been taken out of it.
    ///
    /// Kept per organism rather than applied as it happens, so that a victim being eaten is
    /// the victim it was at the start of the tick however many things are eating it.
    gained: Vec<f64>,
    lost: Vec<f64>,

    /// How much every devorocyte in the world between them wants out of each organism, and
    /// what fraction of that it is actually going to get.
    ///
    /// See [`Behaviour::feed`]: this is the one place where several callers reach for the same
    /// energy at once, and these two arrays are what stop the answer depending on which of
    /// them the loop happened to visit first.
    demand: Vec<f64>,
    share: Vec<f64>,

    /// What each sensocyte is reporting, between nought and one.
    signal: Vec<f32>,

    /// What the sensocytes adhered to each cell are reporting between them, and how many of
    /// them there are - which together are SPEC section 9's "mean of connected Sensocyte
    /// outputs, or 0 if none".
    sensed: Vec<f32>,
    sensors: Vec<u32>,

    /// What each cell gained or lost over the tick, for the renderer to brighten it by.
    ///
    /// Accumulated here and written onto the cells at the end, because a cell can be paid and
    /// bitten in the same tick and SPEC section 6 asks for the *net*.
    flow: Vec<f32>,
}

impl Behaviour {
    /// Build the working room a configuration implies.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let cells = crate::physics::cell_capacity(config);
        let slots = usize::try_from(config.limits.max_organisms.get())
            .expect("a population cap fits in a machine word");

        Self {
            movement_cost: f64::from(config.metabolism.movement_cost),
            resting_amplitude: config.behaviour.resting_amplitude,
            stroke: config.behaviour.stroke,
            width: config.world.width,
            hash: Neighbourhood::new(config, cells),
            drift: Neighbourhood::new(config, cells),
            want: vec![0.0; cells],
            gained: vec![0.0; slots],
            lost: vec![0.0; slots],
            demand: vec![0.0; slots],
            share: vec![0.0; slots],
            signal: vec![0.0; cells],
            sensed: vec![0.0; cells],
            sensors: vec![0; cells],
            flow: vec![0.0; cells],
        }
    }

    /// Take `metabolism.movement_cost` and the `[behaviour]` table again, on a running world.
    ///
    /// The three numbers in this module that a configuration decides and SPEC section 3 does
    /// not lock. `width` is `[world]`'s and every array here was sized from `[limits]`, so
    /// neither moves. See [`crate::world::World::retune`].
    pub fn retune(&mut self, config: &Config) {
        self.movement_cost = f64::from(config.metabolism.movement_cost);
        self.resting_amplitude = config.behaviour.resting_amplitude;
        self.stroke = config.behaviour.stroke;
    }

    /// Let every cell do what its kind does, for one tick.
    ///
    /// Read everything, then change everything. See the module documentation for why that is
    /// two walks rather than one.
    pub(crate) fn run(
        &mut self,
        living: Living<'_>,
        grid: &mut Grid,
        ledger: &mut Ledger,
        ticks: u64,
    ) {
        let Living {
            cells,
            springs,
            owner,
            organisms,
            detritus,
        } = living;
        let population = cells.len();

        self.gained[..organisms.len()].fill(0.0);
        self.lost[..organisms.len()].fill(0.0);
        self.flow[..population].fill(0.0);
        self.hash.rebuild(population, |index| cells[index].pos);
        self.drift
            .rebuild(detritus.len(), |index| detritus[index].pos);

        self.look(cells, grid, detritus, owner, organisms);
        self.contract(cells, springs, owner, organisms, ledger, ticks);
        self.eat(cells, owner, grid, ledger);
        self.feed(cells, owner, organisms, detritus, ledger);
        self.settle(cells, organisms);
    }

    /// The read-only half: work out what every cell is about to do, changing nothing.
    ///
    /// The cells arrive grouped by organism, in slot order, so the table of which gene answers
    /// to which state is rebuilt once per body rather than searched for once per cell.
    fn look(
        &mut self,
        cells: &[Cell],
        grid: &Grid,
        detritus: &[Detritus],
        owner: &[usize],
        organisms: &[Option<Organism>],
    ) {
        let Self {
            width,
            hash,
            drift,
            want,
            signal,
            ..
        } = self;
        let around = Surroundings {
            hash,
            drift,
            cells,
            detritus,
            grid,
            owner,
            width: *width,
        };

        for (index, cell) in cells.iter().enumerate() {
            // Shading is worked out only where somebody is about to eat. It is the most
            // expensive question in the pass - a box of fifteen buckets per cell - and a
            // sclerocyte standing in a shadow does nothing differently for being in one.
            want[index] = if cell.kind == CellKind::Photocyte {
                let tile = grid.tile_at(cell.pos);

                HARVEST_RATE * f64::from(grid.tiles()[tile]) * f64::from(shade(&around, index))
            } else {
                0.0
            };

            signal[index] = 0.0;
            if cell.kind != CellKind::Sensocyte {
                continue;
            }

            let Some(organism) = organisms[owner[index]].as_ref() else {
                continue;
            };
            // What a sensocyte is tuned to is the `sensor_target` of the gene that made it a
            // sensocyte. See [`Behaviour::contract`] for the argument, which is one argument
            // for both kinds that have any behaviour at all.
            let Some(gene) = cell.gene else {
                continue;
            };

            signal[index] = sense(
                organism.genome().genes()[usize::from(gene)].sensor_target,
                &around,
                index,
            );
        }
    }

    /// Myocytes work their springs, and their organisms pay for the work.
    ///
    /// SPEC section 9's controller, applied once per spring rather than once per myocyte,
    /// because a spring has only one rest length and can have a muscle on both ends. Where it
    /// does, the two contractions are **averaged** - which is order-independent, and which
    /// means two muscles pulling against each other cancel rather than one of them winning by
    /// being looked at second.
    ///
    /// # ⭐⭐ Where a myocyte's rhythm comes from: **the gene that built it**
    ///
    /// `osc_freq`, `osc_phase` and `sensor_gain` live on a *gene*, and a cell is not a gene.
    /// SPEC section 7 never says in as many words which gene a grown cell's behaviour comes
    /// from, but it puts those three fields, and `sensor_target`, in **the same fixed record**
    /// as `child_kind` and `new_kind`. The natural reading of one record is that it describes
    /// one thing: a gene that divides a parent into a myocyte says how that myocyte oscillates.
    /// So a cell carries the position of the gene that made it what it is - see
    /// [`crate::development::develop`] - and this reads it.
    ///
    /// **Phase 4 decided the other way and the evidence is that it connected almost nothing.**
    /// It looked a cell's behaviour up by matching its `state` against `trigger_state`, which
    /// is development's own first-match-wins rule with the step window taken off, on the
    /// grounds that a state is what a genome uses to say what a cell *is*. Measured over
    /// 120,000 ticks of the shipped world, over every cell of every body except the seed cell
    /// it started as: **0.05% were in a state their own genome named.** Not one myocyte,
    /// devorocyte or sclerocyte in the population was. A state is one of 64, a genome of that
    /// age holds about three genes, `trigger_state` is not where mutation spends its time, and
    /// development scatters daughters across the whole range through `child_state` - so a
    /// muscle was overwhelmingly likely to be grown into a state nothing in its own genome was
    /// listening to. **Anatomically present and behaviourally disconnected**: a muscle with no
    /// nerve to it. Three separate changes to what movement was *worth* - the anisotropic
    /// water, the drifting light and the stroke - all came back null against a code path the
    /// world took about once in every two hundred thousand spring-ticks.
    ///
    /// What the old rule offered and this one does not is that a duplicated gene could take
    /// over an existing cell's behaviour by naming its state. What this one keeps is the thing
    /// duplication is actually for: duplicate a dividing gene, point the copy at another state,
    /// and the new body part it grows arrives **with its own rhythm** - because the rhythm
    /// travels with the gene instead of being looked up afterwards. Gene order still decides
    /// which gene builds a cell, so order still carries information.
    ///
    /// A cell no gene speaks for has no behaviour at all: no frequency, no gain. It must not
    /// fall back on some default rhythm, or a lineage would be swimming to a tune nobody
    /// selected. Under this rule that case is **unreachable in a grown body** - only a seed
    /// cell can lack a gene and a seed cell is always a photocyte, which
    /// `development.rs`'s `a_cell_with_no_gene_is_the_seed_cell_and_needs_none` proves - so
    /// the fallback is a rule about a case rather than the ordinary path it used to be.
    ///
    /// # What the work is
    ///
    /// **Force through distance**: the tension already in the spring, times how far this
    /// tick's contraction moved its rest length. SPEC section 6 says the cost is
    /// `movement_cost × work done` and leaves work undefined, and the definition matters more
    /// than the constant: a flat charge per myocyte would be indistinguishable from upkeep and
    /// would select on how many muscles a body had rather than on what it did with them.
    ///
    /// A muscle that is not moving therefore pays exactly nothing, and one working against a
    /// stiffer spring pays proportionally more.
    ///
    /// The distance is worked out from the controller itself rather than remembered from last
    /// tick - the rest length is a closed-form function of the time, so last tick's is a
    /// subtraction rather than a number that has to be stored per spring and kept in step. The
    /// one approximation in it is that the amplitude is taken as it is *now* for both, so a
    /// tick in which the sensors changed sharply mis-measures the distance by whatever the
    /// amplitude moved. That is a hundredth of a tick's swing at worst and it costs nothing to
    /// be wrong about.
    fn contract(
        &mut self,
        cells: &[Cell],
        springs: &mut [Spring],
        owner: &[usize],
        organisms: &[Option<Organism>],
        ledger: &mut Ledger,
        ticks: u64,
    ) {
        let Self {
            width,
            movement_cost,
            resting_amplitude,
            stroke,
            signal,
            sensed,
            sensors,
            lost,
            flow,
            ..
        } = self;
        let (width, movement_cost) = (*width, *movement_cost);
        let drive = Drive {
            resting: *resting_amplitude,
            stroke: *stroke,
        };

        // SPEC section 9's "mean of connected Sensocyte outputs, or 0 if none". Connected
        // means **adhered**, one spring away and no further: a myocyte hears the sensocytes it
        // is joined to and nothing else. Anything wider - the whole body, say - would give
        // every muscle in an organism the same number, and a body whose muscles all hear the
        // same thing can pulse but cannot turn.
        sensed[..cells.len()].fill(0.0);
        sensors[..cells.len()].fill(0);
        for spring in springs.iter() {
            for (from, to) in [(spring.a, spring.b), (spring.b, spring.a)] {
                if cells[from].kind == CellKind::Sensocyte {
                    sensed[to] += signal[from];
                    sensors[to] += 1;
                }
            }
        }

        let seconds = elapsed(ticks);
        let a_tick_ago = seconds - f64::from(DT);

        for spring in springs.iter_mut() {
            let slot = owner[spring.a];
            let Some(organism) = organisms[slot].as_ref() else {
                continue;
            };

            let mut now = 0.0f32;
            let mut a_moment_ago = 0.0f32;
            let mut muscles = 0u32;

            for end in [spring.a, spring.b] {
                if cells[end].kind != CellKind::Myocyte {
                    continue;
                }
                // The gene that made this cell a myocyte is the gene that says how it moves.
                // A cell carrying an index into its own organism's genome is an invariant
                // `development.rs` establishes and nothing afterwards can disturb: a body and
                // the genome it was grown from are made and replaced together.
                let Some(which) = cells[end].gene else {
                    continue;
                };
                let gene = &organism.genome().genes()[usize::from(which)];

                let heard = if sensors[end] > 0 {
                    sensed[end] / narrowed(f64::from(sensors[end]))
                } else {
                    0.0
                };
                let (freq, phase) = (f64::from(gene.osc_freq), f64::from(gene.osc_phase));

                now += contraction(drive, gene, heard, seconds.mul_add(freq, phase));
                a_moment_ago += contraction(drive, gene, heard, a_tick_ago.mul_add(freq, phase));
                muscles += 1;
            }

            if muscles == 0 {
                continue;
            }
            let each = if muscles > 1 { 0.5 } else { 1.0 };
            let (now, a_moment_ago) = (now * each, a_moment_ago * each);

            let base = spring.rest_length;
            spring.rest_length = base * now;

            let apart = wrapped_offset(cells[spring.a].pos, cells[spring.b].pos, width).length();
            let tension = f64::from(spring.stiffness) * f64::from(apart - spring.rest_length);
            let moved = f64::from(base) * f64::from(now - a_moment_ago);
            let cost = movement_cost * tension.abs() * moved.abs();

            if cost <= 0.0 {
                continue;
            }

            ledger.spend(cost);
            lost[slot] += cost;
            for end in [spring.a, spring.b] {
                if cells[end].kind == CellKind::Myocyte {
                    flow[end] -= narrowed(cost) * each;
                }
            }
        }
    }

    /// Photocytes take what they asked for, out of the field and into the books.
    ///
    /// What each one gets is what the tile *actually* gave up rather than what was wanted -
    /// [`Grid::harvest`] hands the realised figure back, and a tile with less in it than was
    /// asked for pays what it has. Nothing here can drive a tile below nothing, which is what
    /// makes the field the one account nobody has to keep a running total of.
    fn eat(&mut self, cells: &[Cell], owner: &[usize], grid: &mut Grid, ledger: &mut Ledger) {
        for (index, cell) in cells.iter().enumerate() {
            let wanted = self.want[index];
            if wanted <= 0.0 {
                continue;
            }

            let tile = grid.tile_at(cell.pos);
            let taken = grid.harvest(ledger, tile, wanted);

            self.gained[owner[index]] += taken;
            self.flow[index] += narrowed(taken);
        }
    }

    /// Devorocytes take what they are touching: dead biomass out of the drift, living biomass
    /// out of whoever it belongs to.
    ///
    /// # Two walks over the same contacts, and the second one is why
    ///
    /// A living victim is the only thing in the world that several mouths can be drawing on at
    /// once while it is also the thing being drawn down. Written as one walk, the first
    /// devorocyte the loop happened to reach would take a full bite and the second would find
    /// what was left, so an organism's dinner would depend on where in the arena its rival
    /// lived - a hidden variable nobody would think to look for.
    ///
    /// So the first walk only *asks*: it adds up what every mouth wants out of each organism.
    /// Then each victim's share is worked out once - all of it if there is enough, and
    /// otherwise the fraction of it there is - and the second walk takes bites scaled by that.
    /// Every predator gets the same proportion of what it asked for, the victim gives up
    /// exactly what it had, and nobody is left holding a debt.
    ///
    /// Detritus is clamped in visiting order instead, and the asymmetry is deliberate rather
    /// than an oversight. A grain is one small packet with at most a mouth or two against it,
    /// where an organism is a pool that bites in half a dozen places all draw on; and sharing
    /// grains proportionally would need an arena sized to a detritus population that Group B
    /// has not decided on. What is guaranteed either way is the part that matters: a grain
    /// gives what it has and is never taken below nothing.
    fn feed(
        &mut self,
        cells: &[Cell],
        owner: &[usize],
        organisms: &[Option<Organism>],
        detritus: &mut [Detritus],
        ledger: &mut Ledger,
    ) {
        let Self {
            width,
            hash,
            drift,
            gained,
            lost,
            demand,
            share,
            flow,
            ..
        } = self;
        let width = *width;

        demand[..organisms.len()].fill(0.0);
        bites(hash, cells, owner, width, |_, victim, _, wanted| {
            demand[victim] += wanted;
        });

        for (slot, organism) in organisms.iter().enumerate() {
            // An organism already in the red has nothing left to give. Its energy is floored
            // at nothing rather than used as it stands, because a negative share would turn
            // every mouth on it into a source of energy rather than a drain on one.
            let held = organism.as_ref().map_or(0.0, Organism::energy).max(0.0);

            share[slot] = if demand[slot] > held {
                held / demand[slot]
            } else {
                1.0
            };
        }

        bites(
            hash,
            cells,
            owner,
            width,
            |mouth, victim, bitten, wanted| {
                let taken = wanted * share[victim];
                if taken <= 0.0 {
                    return;
                }

                ledger.predate(taken);
                gained[owner[mouth]] += taken;
                lost[victim] += taken;
                flow[mouth] += narrowed(taken);
                flow[bitten] -= narrowed(taken);
            },
        );

        // Scavenging, which is the same rate against something that cannot be hurt by it. A
        // grain is taken in visiting order and clamped at what it holds - see this method's
        // documentation for why this half is not shared out the way the half above is.
        //
        // Through the drift's own bucket index, not along the whole drift. Group A wrote the
        // walk because there was nothing in the world to walk over; a world with a hundred
        // thousand grains in it and a thousand mouths would have made a tick a hundred million
        // measurements long, and would have done it silently.
        let reach = CellKind::LARGEST_RADIUS + DETRITUS_RADIUS;
        for (mouth, cell) in cells.iter().enumerate() {
            if cell.kind != CellKind::Devorocyte {
                continue;
            }

            drift.near(cell.pos, reach, reach, reach, |grain| {
                let morsel = &mut detritus[grain];
                let apart = wrapped_offset(cell.pos, morsel.pos, width).length();
                if apart >= cell.radius + DETRITUS_RADIUS {
                    return;
                }

                let taken = DEVOUR_RATE.min(morsel.energy);
                if taken <= 0.0 {
                    return;
                }

                ledger.scavenge(taken);
                morsel.energy -= taken;
                gained[owner[mouth]] += taken;
                flow[mouth] += narrowed(taken);
            });
        }
    }

    /// Put the tick's income and outgoings onto the organisms that earned and spent them.
    ///
    /// Once, at the end, and in one place. Every ledger movement above has already happened;
    /// this is the other half of each of them, and keeping the two halves apart is what makes
    /// it possible to look at this file and see that there is no route by which an organism's
    /// energy changes without an account changing with it.
    fn settle(&mut self, cells: &mut [Cell], organisms: &mut [Option<Organism>]) {
        for (slot, organism) in organisms.iter_mut().enumerate() {
            let Some(organism) = organism.as_mut() else {
                continue;
            };

            organism.gain(self.gained[slot]);
            organism.lose(self.lost[slot]);
        }

        for (index, cell) in cells.iter_mut().enumerate() {
            cell.energy_flow = self.flow[index];
        }
    }
}

/// Hand every mouthful of *living* biomass in the world to `bite`, as `(the devorocyte doing
/// the biting, the slot the victim is in, the victim's cell, how much is wanted)`.
///
/// **The whole of predation is here, and it is two lines of rule.** A devorocyte takes from
/// anything it is touching that does not belong to its own organism, at a rate the victim's
/// toughness reduces.
///
/// There is deliberately **nothing else**: no seeking, no choosing between a soft cell and a
/// hard one, no preference for a full victim over an empty one, and no notion of a predator at
/// all. CLAUDE.md's decision log and SPEC section 10 both insist on this - *whether a
/// herbivore/predator split appears is one of the genuinely interesting outcomes and coding it
/// in would be answering the question in advance.* Everything that could make a lineage a
/// hunter has to come out of where its body puts its devorocytes and how it moves them, which
/// is what a genome actually controls.
///
/// A body cannot eat itself: a mouthful is only ever taken from a *different* slot. Not for
/// the sake of the books - `biomass → biomass` balances perfectly - but because an organism
/// with a devorocyte against its own gonocyte would be shuffling its own energy round in a
/// circle for nothing, which is free energy, and the mutation operators would find it.
///
/// Called twice per tick with two different closures. See [`Behaviour::feed`] for why once is
/// not enough.
fn bites(
    hash: &Neighbourhood,
    cells: &[Cell],
    owner: &[usize],
    width: f32,
    mut bite: impl FnMut(usize, usize, usize, f64),
) {
    for (mouth, cell) in cells.iter().enumerate() {
        if cell.kind != CellKind::Devorocyte {
            continue;
        }

        hash.near(cell.pos, REACH, REACH, REACH, |other| {
            if owner[other] == owner[mouth] {
                return;
            }

            let meal = cells[other];
            let apart = wrapped_offset(cell.pos, meal.pos, width).length();
            if apart >= cell.radius + meal.radius {
                return;
            }

            let toughness = f64::from(meal.kind.toughness());
            bite(mouth, owner[other], other, DEVOUR_RATE * (1.0 - toughness));
        });
    }
}

/// How much of the light reaching the surface still reaches this cell, between nought and
/// one.
///
/// ⭐⭐ **SPEC says occlusion happens and does not say how. This is the model, and it is the
/// most consequential thing decided in Phase 4 Group A.** SPEC section 6: a photocyte is
/// *"occluded by cells above it — this is what rewards spread-out, branching body plans over
/// compact blobs"*. Without it, every arrangement of a given number of photocytes earns
/// exactly the same amount, and the project's central premise - that a body's *shape* is
/// something evolution discovers - has nothing whatever pushing on it.
///
/// # The model in one line
///
/// Every cell above this one lays down a little **optical depth**, they add up, and what gets
/// through is `1 / (1 + total)`. One blocker directly overhead and touching contributes an
/// optical depth of one, so it halves the light.
///
/// A blocker's contribution is the product of two tapers:
///
/// - **sideways**: `1 - across / (my radius + its radius)`, so a blocker directly over the
///   cell counts fully, one just far enough out that the two discs no longer overlap counts
///   for nothing, and everything between is in proportion.
/// - **downwards**: `1 - drop / SHADOW_DEPTH`, for a blocker anywhere strictly above and
///   within that depth. Level with, or below, is nothing.
///
/// # Why the shadow is exactly as wide as the cell casting it
///
/// Because that is what a disc does to light falling straight down. The alternative - a
/// shadow with some other width - would need a reason, and the only reason available would be
/// "because it makes the pressure stronger", which is tuning the answer rather than modelling
/// the thing.
///
/// What the *taper* is for is different, and it is not cosmetic. A hard-edged shadow makes a
/// cell's income a step function of where it is: a body could sit a hundredth of a unit from
/// the edge of a shadow and a mutation that moved it a hundredth further would double its
/// income. Evolution climbs slopes; it cannot see cliffs. With the taper, a mutation that
/// nudges a limb's angle by a hair changes what that limb earns by a hair, which is what makes
/// the shape of a body something a lineage can *find* rather than stumble onto.
///
/// # Why the shadow fades with depth, and why that is the load-bearing decision
///
/// A shadow that never faded would be the physically simpler thing to write: light travels in
/// straight lines, so a cell at the surface would darken the entire column of water beneath it
/// for ever. It was rejected, for a reason that matters more than the physics.
///
/// **Occlusion is here to reward a body's own shape.** With an unfading shadow, a photocyte's
/// income is decided by everything that happens to be floating above it in the whole water
/// column - and in a crowded world that is overwhelmingly *other organisms*, drifting past on
/// currents the lineage has no control over. Shape would still matter, but it would be a small
/// signal buried in a large amount of noise, and selection cannot act on what it cannot
/// distinguish. Fading over `SHADOW_DEPTH` makes shading a **local** effect at roughly the
/// scale of a body, which is the scale the thing being selected lives at.
///
/// It is also not a fudge physically. Water scatters, and a shadow in real water blurs out and
/// fills in over a short distance rather than persisting to the sea floor. The fade is linear
/// rather than exponential because a linear fade reaches zero at a definite depth, which turns
/// the search into a bounded box and keeps this whole pass linear in the population - and
/// because SPEC's own Q8 flags the transcendental functions as the ones the arithmetic
/// standard does not pin, and this file already has to use one for the myocyte.
///
/// # Why anything above shades, and not only the cell's own body
///
/// **Because light does not know whose cell it is.** The alternative - a body shaded only by
/// itself - would be a rule stating that light passes through strangers, which has no
/// justification beyond being easier to compute, and it is not even that: the search is a box
/// around the cell either way and skipping the foreigners in it would be extra work rather
/// than less.
///
/// What it buys is a second pressure that a self-only model would have thrown away: an
/// organism at the surface shades everything beneath it, so crowding at the top of the world
/// is genuinely costly and being underneath a neighbour is genuinely worse than being beside
/// one. That is a canopy, it is what light competition actually looks like, and SPEC section 4
/// already asks for movement to have a payoff.
///
/// Note what it does *not* do: the light still falls on the tiles regardless. Occlusion
/// changes how much of a tile a cell can take, never how much the tile is given. A shaded
/// world does not go dark; the energy simply goes to whoever is in a position to take it.
///
/// # Why what gets through is `1 / (1 + depth)` rather than `1 - depth`
///
/// Because subtraction runs out. A photocyte under three cells would be at exactly zero, and a
/// fourth cell above it would cost nothing at all - so at precisely the point a lineage is
/// most buried, every mutation would look equally good and there would be no gradient leading
/// out. `1 / (1 + depth)` never reaches nothing, so every cell that is added overhead costs
/// something and every cell that moves out of the way is worth something, however deep in a
/// blob the photocyte started.
///
/// # What it costs
///
/// A box one [`REACH`] wide and [`SHADOW_DEPTH`] tall, looked up in the bucket index - so
/// fifteen buckets, which at any realistic density is a dozen or so candidates. Written the
/// obvious way, as "compare this cell with every other cell", it would be the square of the
/// population and the simulation would stop being able to run overnight; SPEC section 8 makes
/// exactly this argument about collisions and it applies here unchanged.
fn shade(around: &Surroundings<'_>, index: usize) -> f32 {
    let here = around.cells[index];
    let mut optical_depth = 0.0f32;

    around
        .hash
        .near(here.pos, REACH, SHADOW_DEPTH, 0.0, |other| {
            if other == index {
                return;
            }

            let blocker = around.cells[other];

            // Strictly above, and within the depth a shadow reaches. Two cells at exactly the same
            // height shade each other not at all, which is the only answer that is symmetric -
            // there is no fact about which of them is on top.
            let drop = here.pos.y - blocker.pos.y;
            if drop <= 0.0 || drop >= SHADOW_DEPTH {
                return;
            }

            // The short way round the world, because a body straddling the join is an ordinary
            // body and its cells shade one another exactly as they would anywhere else.
            let across = wrapped_offset(blocker.pos, here.pos, around.width).x.abs();
            let overlap = here.radius + blocker.radius;
            if across >= overlap {
                return;
            }

            optical_depth += (1.0 - across / overlap) * (1.0 - drop / SHADOW_DEPTH);
        });

    1.0 / (1.0 + optical_depth)
}

/// Everything the read-only pass looks at, gathered up.
///
/// Handed round as one thing rather than as six separate references, because six of them
/// threaded through a call is a place to get two the wrong way round - and because everything
/// in it is read-only, which is the property the whole pass rests on.
struct Surroundings<'a> {
    hash: &'a Neighbourhood,
    drift: &'a Neighbourhood,
    cells: &'a [Cell],
    detritus: &'a [Detritus],
    grid: &'a Grid,
    owner: &'a [usize],
    width: f32,
}

/// What a sensocyte is reporting, between nought and one.
///
/// SPEC section 9: *"Each `Sensocyte` outputs a normalised gradient magnitude toward its
/// `sensor_target`, sampled from the resource grid or from nearby foreign biomass."* SPEC
/// gives no formula, so the shape of this is a Phase 4 decision.
///
/// # It is a gradient, which is a different thing from how much is nearby
///
/// The number is built out of a **direction**: every source within reach contributes a pull
/// towards itself, they are added up as vectors, and the length of what is left is the signal.
/// A sensocyte with prey equally on both sides therefore reports **nothing**, which is
/// correct and is the whole point - there is no direction to go, and a signal that rose
/// whenever anything was near would drive a muscle hardest exactly where movement is no use.
///
/// # How it is normalised, and what the two references mean
///
/// `lopsided / (lopsided + reference)`, which maps anything from nought upwards onto nought to
/// one without a clamp in it. A clamp would be a cliff, and a cliff is a place where a
/// mutation stops changing anything.
///
/// For **detritus** and **foreign biomass** the reference is one, and one is the pull of a
/// single source right against the sensocyte - so "one thing, touching" is the half-signal
/// mark, two are two thirds, and a crowd approaches but never reaches saturation.
///
/// For **light** the reference is [`LIGHT_REFERENCE`], a fixed quantity of energy, which makes
/// the signal an *absolute* gradient: how much the light changes across a tile, against how
/// much it changes across a tile in the world as the light alone leaves it.
///
/// ⭐ **That is Phase 7's correction, and the thing it replaced is the reason nothing had ever
/// used a light sensor.** The reference used to be the energy of the tile the sensocyte was
/// standing in, which made the signal *relative*: how much the light changes across a tile as
/// a fraction of how much light there is. It reads well in principle - the same gradient is
/// worth more in dim water, so a cell deep down is more sensitive - and in the shipped world
/// the divisor is about **four units** against a gradient of about **two hundredths**, so
/// every reading came back a couple of thousandths and a sensocyte's whole output was smaller
/// than the rounding on the amplitude it drives. Measured: 0.0025 for the background gradient,
/// and 0.05 to 0.31 beside a tile something had been grazing.
///
/// Dividing by a fixed reference instead puts the background gradient near the middle of the
/// range and a grazed tile near the top of it, which is a signal a gene can be selected on.
/// The price is the property that was nice about the old one: the light is now read the same
/// way at every depth, and a lineage wanting to be more sensitive in the dark has to evolve
/// the gain for it.
fn sense(target: SensorTarget, around: &Surroundings<'_>, index: usize) -> f32 {
    let here = around.cells[index];

    let (lopsided, reference) = match target {
        SensorTarget::Light => (light_gradient(around.grid, here.pos), LIGHT_REFERENCE),
        SensorTarget::Detritus => (drift_gradient(around, here.pos), 1.0),
        SensorTarget::ForeignBiomass => (crowd_gradient(around, index), 1.0),
    };

    if lopsided <= 0.0 {
        return 0.0;
    }

    lopsided / (lopsided + reference)
}

/// The pull one source exerts on a sensocyte: which way it lies, faded out to nothing at the
/// edge of what can be sensed.
///
/// Nothing at all from a source exactly on top of the sensocyte, because a thing in the same
/// place as you is not in any direction from you, and the arithmetic that would say otherwise
/// is a division by nought.
fn towards(from: Vec2, to: Vec2, width: f32) -> Option<Vec2> {
    let offset = wrapped_offset(from, to, width);
    let apart = offset.length();

    if apart <= 0.0 || apart >= SENSE_RANGE {
        return None;
    }

    Some(offset.scaled((1.0 - apart / SENSE_RANGE) / apart))
}

/// How lopsided the *other organisms* around a cell are.
///
/// Its own body does not count. A sensocyte that could smell the cells it is joined to would
/// read a large constant that never changed, and the thing it is for - finding what is *not*
/// itself - would be buried under it.
fn crowd_gradient(around: &Surroundings<'_>, index: usize) -> f32 {
    let here = around.cells[index];
    let mut lopsided = Vec2::ZERO;

    around
        .hash
        .near(here.pos, SENSE_RANGE, SENSE_RANGE, SENSE_RANGE, |other| {
            if around.owner[other] == around.owner[index] {
                return;
            }

            if let Some(pull) = towards(here.pos, around.cells[other].pos, around.width) {
                lopsided += pull;
            }
        });

    lopsided.length()
}

/// How lopsided the dead biomass around a point is.
///
/// Asked of the drift's own bucket index rather than of the whole drift, which is Group B's
/// correction to Group A: a walk over every grain in the world cost nothing while nothing
/// made any, and became the population times the drift the moment something did. Everything
/// past [`SENSE_RANGE`] contributes nothing - `towards` says so - so the bucketed answer is
/// the same answer, found by looking at a few dozen grains instead of a hundred thousand.
fn drift_gradient(around: &Surroundings<'_>, at: Vec2) -> f32 {
    let mut lopsided = Vec2::ZERO;

    around
        .drift
        .near(at, SENSE_RANGE, SENSE_RANGE, SENSE_RANGE, |grain| {
            if let Some(pull) = towards(at, around.detritus[grain].pos, around.width) {
                lopsided += pull;
            }
        });

    lopsided.length()
}

/// How fast the light changes around a point.
///
/// A central difference over the four tiles around the one the cell is standing in - the
/// coarsest possible answer, and the only one available: the resource field *is* tiles, and
/// there is nothing finer in it to sample. Sideways it wraps, because the world does;
/// vertically it stops at the surface and the floor, where the one-sided difference that
/// results is half of the real slope and the honest answer to a question with nothing on one
/// side of it.
fn light_gradient(grid: &Grid, at: Vec2) -> f32 {
    let tile = grid.tile_at(at);
    let (cols, rows) = (grid.cols(), grid.rows());
    let (col, row) = (tile % cols, tile / cols);
    let tiles = grid.tiles();

    let west = tiles[row * cols + (col + cols - 1) % cols];
    let east = tiles[row * cols + (col + 1) % cols];
    let over = tiles[row.saturating_sub(1) * cols + col];
    let under = tiles[(row + 1).min(rows - 1) * cols + col];

    Vec2::new((east - west) * 0.5, (under - over) * 0.5).length()
}

/// The two numbers SPEC section 9's controller is driven by, carried together so they cannot
/// be read from different configurations.
///
/// ⭐⭐ **Phase 7's Group H.** Both were written into this file as constants - the 0.3 an
/// unsensed muscle contracts at and the 0.4 of its rest length a driven one works through - and
/// the second was measured to be the only lever in the project that makes swimming worth doing.
/// See [`crate::config::BehaviourConfig`].
#[derive(Clone, Copy)]
struct Drive {
    resting: f32,
    stroke: f32,
}

/// SPEC section 9's controller, for one myocyte at one moment: what its spring's rest length
/// is a multiple of.
///
/// ```text
/// amplitude = clamp(resting_amplitude + sensor_gain × signal, 0.0, 1.0)
/// rest_len  = base_rest × (1 + amplitude × stroke × sin(t × osc_freq + osc_phase))
/// ```
///
/// Written out from the specification rather than rearranged; the two coefficients are SPEC
/// section 3's `[behaviour]` table, which shipped as the constants 0.3 and 0.4 until Phase 7's
/// Group H. The `angle` is the whole of SPEC's `t × osc_freq + osc_phase`, worked out by the
/// caller at 64 bits because `t` is a running total over the life of a run and the phase of a
/// long run would otherwise be noise.
///
/// # `sensor_gain` is the **myocyte's**, not the sensocyte's
///
/// SPEC section 9 puts the whole of this block under "Each `Myocyte` oscillates its springs'
/// rest length", and then says separately that a sensocyte "outputs a normalised gradient
/// magnitude" - an output with no gain in it. So the gain that appears here belongs to the
/// gene that answers to the *myocyte's* state. (`genome.rs` describes the field as how
/// strongly a sensocyte responds, which is the other reading; the pseudo-code is what is
/// implemented.)
///
/// It is also much the more useful of the two. One sensocyte can drive several myocytes, and
/// with the gain on the myocyte each of them decides for itself what to make of the same
/// signal - so a body can have one side excited and the other inhibited by one sensor, which
/// is a turn. With the gain on the sensocyte, every muscle hearing it would respond the same
/// way and a body could only speed up and slow down.
fn contraction(drive: Drive, gene: &Gene, signal: f32, angle: f64) -> f32 {
    let amplitude = signal
        .mul_add(gene.sensor_gain, drive.resting)
        .clamp(0.0, 1.0);

    amplitude.mul_add(drive.stroke * narrowed(angle.sin()), 1.0)
}

/// How many simulated seconds a run has taken.
///
/// SPEC section 2 fixes a tick at a sixtieth of a second, so this is only a multiplication -
/// except that a tick count is a 64-bit whole number and there is no lossless conversion from
/// one of those to a 64-bit float. Splitting it in half and putting the halves back together
/// is that conversion written out: exact for any run of fewer than nine quadrillion ticks,
/// which is four and a half million years of running at sixty ticks a second.
///
/// It is worked out at 64 bits and stays there until the sine is taken, because it is a
/// running total over the life of a run: at 32 bits an overnight run's phase would be rounded
/// away entirely, and every myocyte in the world would end up oscillating to whatever rhythm
/// the rounding left rather than the one its genome asked for.
fn elapsed(ticks: u64) -> f64 {
    let high = u32::try_from(ticks >> 32).expect("the top half of a u64 is a u32");
    let low = u32::try_from(ticks & 0xFFFF_FFFF).expect("the bottom half of a u64 is a u32");

    f64::from(high).mul_add(4_294_967_296.0, f64::from(low)) * f64::from(DT)
}

/// Turn a quantity worked out at full precision into one a cell can carry.
///
/// The same conversion, for the same reason, as the one in `grid.rs`: the standard library
/// offers no `TryFrom` between the two sizes of number, so a project that forbids lossy casts
/// has to write it out once with the reasoning attached. Everything that goes through it is a
/// small quantity of energy or a length in world units, so there is nothing here to overflow;
/// what it gives up is digits, which is what SPEC section 2 asks of everything stored on a
/// cell.
fn narrowed(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the values are small quantities of energy and lengths in world units, so \
                  there is nothing here to overflow; the lost digits are what SPEC section 2 \
                  asks for"
    )]
    let narrow = value as f32;
    narrow
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellKind;
    use crate::config::{Config, RawConfig, spec_defaults};
    use crate::genome::{Action, Gene, Genome, SensorTarget, State};
    use std::f32::consts::FRAC_PI_2;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    ///
    /// # The light is this file's own, and deliberately not the shipped one
    ///
    /// Every test here is about a *rate* or a *shape* - what a photocyte takes out of the tile
    /// it is on, how much of a cell's light the cell above it blocks, what a mouth drains from
    /// what it is touching - and every one of them wants full water to measure against, which
    /// [`Scene::fill_with_light`] provides by ticking the grid a thousand times.
    ///
    /// How many ticks that takes is `light.cap / light.influx`, which is a number Group D
    /// changed by a factor of twelve when it tuned the ecology. Left reading the shipped
    /// value, every scene in this file would have been a fifth full instead of full, and the
    /// arithmetic these tests exist to pin would have moved with a setting that has nothing to
    /// do with any of them. So the light is set here, at the value the whole file was measured
    /// against, and a future tuning of the shipped ecology cannot reach it.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        raw.light.influx = 0.012;
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// A small world to try one behaviour in: some bodies, the water they stand in, and the
    /// books that watch them.
    ///
    /// Built by hand rather than through [`crate::world::World`] on purpose. Every test here
    /// is about a *shape* - a cell directly above another, two cells touching, a sensocyte
    /// beside a gradient - and growing a genome that produces an exact shape is a second
    /// puzzle standing between the test and what it is trying to say. `world.rs` is where the
    /// wiring is proved; this is where the arithmetic is.
    struct Scene {
        config: Config,
        grid: Grid,
        ledger: Ledger,
        cells: Vec<Cell>,
        springs: Vec<Spring>,
        /// The springs as their genes made them, which is what a tick starts from.
        relaxed: Vec<Spring>,
        owner: Vec<usize>,
        organisms: Vec<Option<Organism>>,
        detritus: Vec<Detritus>,
        behaviour: Behaviour,
        ticks: u64,
    }

    impl Scene {
        /// An empty scene: water, no bodies.
        fn new(config: &Config) -> Self {
            let grid = Grid::new(config);
            let ledger = Ledger::new(grid.total_energy());

            Self {
                config: config.clone(),
                grid,
                ledger,
                cells: Vec::new(),
                springs: Vec::new(),
                relaxed: Vec::new(),
                owner: Vec::new(),
                organisms: Vec::new(),
                detritus: Vec::new(),
                behaviour: Behaviour::new(config),
                ticks: 0,
            }
        }

        /// Let the light fall until the tiles are full, exactly as a real run must before
        /// anything can be seeded into it. A world starts dark.
        fn fill_with_light(&mut self) {
            for _ in 0..1_000 {
                self.grid.tick(&mut self.ledger);
            }
        }

        /// Put a body in the next free slot, with its energy taken out of the field the way
        /// `World::seed` takes it, and hand back the slot.
        ///
        /// The genome matters only where a test is about a gene - a photocyte harvesting
        /// needs no rule to tell it to.
        ///
        /// ⭐ **A cell is given the gene that built it, not a developmental state**, which is
        /// Phase 7's change to where behaviour comes from. `development.rs` is what stamps
        /// that on in a real body; a scene builds its bodies by hand, so it says so by hand.
        /// A cell's `state` is left at nought throughout this file, because after that change
        /// nothing in this module reads one.
        fn add(
            &mut self,
            genome: Genome,
            energy: f64,
            body: &[(CellKind, Vec2, Option<u8>)],
        ) -> usize {
            let slot = self.organisms.len();

            for &(kind, at, gene) in body {
                let mut cell = Cell::new(kind, at);
                cell.gene = gene;
                self.cells.push(cell);
                self.owner.push(slot);
            }

            // Out of the tile the seed cell is standing on, through the same door a photocyte
            // eats through - so that a scene never holds energy the books were not told about.
            // A hair's tolerance rather than none: a tile is a 32-bit number and what it gives
            // up is never exactly what was asked of it. `Grid::harvest` hands back the
            // realised figure and that is what the organism is given, exactly as
            // `World::seed` does it.
            let mut taken = 0.0;
            if energy > 0.0 {
                let tile = self.grid.tile_at(body[0].1);
                taken = self.grid.harvest(&mut self.ledger, tile, energy);
                assert!(
                    (taken - energy).abs() < 1e-6,
                    "this scene's field holds {taken} under the body and was asked for {energy}"
                );
            }

            let serial = u64::try_from(slot).expect("a slot number fits in a serial");
            self.organisms.push(Some(Organism::new(
                genome,
                taken,
                serial,
                None,
                crate::organism::founding_marker(serial),
                body.len(),
                0,
            )));

            slot
        }

        /// Put a grain of dead biomass in the water, holding energy that came out of the
        /// field the long way round - through a body and out the other side.
        ///
        /// Nothing in Group A makes detritus, so a scene has to make its own; and it makes it
        /// by the route a real one will take, so that the `detritus` account holds what the
        /// drift holds rather than a number nobody was told about.
        /// The energy comes out of a far corner of the world rather than out of the tile the
        /// grain is lying on, which is both more realistic and quietly necessary: a grain is
        /// dead biomass from something that ate somewhere else, and a test that dug a hole
        /// under every grain it dropped would be putting a light gradient there for a
        /// sensocyte to find by accident.
        fn drop_detritus(&mut self, pos: Vec2, energy: f64) -> f64 {
            let tile = self.grid.tile_at(Vec2::ZERO);
            let taken = self.grid.harvest(&mut self.ledger, tile, energy);
            assert!(
                (taken - energy).abs() < 1e-6,
                "this scene's field holds {taken} under a grain asking for {energy}"
            );

            self.ledger.die(taken);
            self.detritus.push(Detritus { pos, energy: taken });

            taken
        }

        /// Fill the water with a real drift: `count` grains on an even lattice across the
        /// whole world, each holding `each`.
        ///
        /// Paid for out of the tiles in turn rather than all out of one, because twenty
        /// thousand grains is more than any single tile holds and a scene that dug one tile
        /// into the ground would be a scene with a light gradient in it.
        fn drop_a_drift(&mut self, across: u32, down: u32, each: f64) {
            let tiles = self.grid.cols() * self.grid.rows();
            let (width, height) = (self.config.world.width, self.config.world.height);

            for step in 0..across * down {
                let taken = self.grid.harvest(
                    &mut self.ledger,
                    usize::try_from(step).expect("a grain count fits in a word") % tiles,
                    each,
                );
                self.ledger.die(taken);
                self.detritus.push(Detritus {
                    pos: Vec2::new(
                        width * narrowed(f64::from(step % across) / f64::from(across)),
                        height * narrowed(f64::from(step / across) / f64::from(down)),
                    ),
                    energy: taken,
                });
            }
        }

        /// Everything the drift is holding between it.
        fn drift_energy(&self) -> f64 {
            self.detritus.iter().map(|grain| grain.energy).sum()
        }

        /// Take energy out of one tile and let it leave the world, so there is a hole in the
        /// water for something to notice.
        ///
        /// The energy goes to `dissipated` rather than to nobody, which is the difference
        /// between a test that digs a hole and a test that quietly invents a leak - and a leak
        /// in a scene is exactly the failure SPEC section 5 says an invariant cannot see.
        fn eat_out(&mut self, at: Vec2, amount: f64) {
            let tile = self.grid.tile_at(at);
            let taken = self.grid.harvest(&mut self.ledger, tile, amount);

            self.ledger.spend(taken);
        }

        /// Join two of the most recently added body's cells, by their position in the scene.
        fn adhere(&mut self, a: usize, b: usize, rest_length: f32, stiffness: f32) {
            self.springs.push(Spring {
                a,
                b,
                rest_length,
                stiffness,
            });
            self.relaxed.push(Spring {
                a,
                b,
                rest_length,
                stiffness,
            });

            let slot = self.owner[a];
            let organism = self.organisms[slot]
                .as_mut()
                .expect("a spring belongs to an organism that exists");
            let springs = organism.springs() + 1;
            let cells = organism.cells();
            let (genome, energy) = (organism.genome().clone(), organism.energy());
            let serial = u64::try_from(slot).expect("a slot number fits in a serial");
            self.organisms[slot] = Some(Organism::new(
                genome,
                energy,
                serial,
                None,
                crate::organism::founding_marker(serial),
                cells,
                springs,
            ));
        }

        /// One tick of behaviour, and nothing else.
        ///
        /// The springs are put back to the lengths their genes asked for before every tick,
        /// which is not tidying up: it is what `World::gather` does, and it is what makes a
        /// myocyte's contraction *this tick's* contraction rather than something that
        /// compounds. A body whose rest lengths were oscillated on top of last tick's
        /// oscillation would wind itself up until it tore apart.
        fn run(&mut self) {
            self.springs.copy_from_slice(&self.relaxed);

            self.behaviour.run(
                Living {
                    cells: &mut self.cells,
                    springs: &mut self.springs,
                    owner: &self.owner,
                    organisms: &mut self.organisms,
                    detritus: &mut self.detritus,
                },
                &mut self.grid,
                &mut self.ledger,
                self.ticks,
            );
            self.ticks += 1;
        }

        /// What the organism in this slot is holding.
        fn energy(&self, slot: usize) -> f64 {
            self.organisms[slot]
                .as_ref()
                .expect("this scene put an organism in that slot")
                .energy()
        }

        /// A genome with no rules in it, for the bodies whose behaviour comes from their kind
        /// rather than from a gene.
        fn no_genome(&self) -> Genome {
            Genome::new(Vec::new(), &self.config.limits)
        }
    }

    /// ⭐ **A1.** A photocyte draws energy out of the tile it is standing on, at a rate that
    /// follows how much that tile is holding - and it does it through `Grid::harvest`, so the
    /// books cannot be gone round.
    ///
    /// SPEC section 6 gives a photocyte's whole function in one line: it "harvests from the
    /// field tile it occupies, rate ∝ local energy × exposure". This is the first half of
    /// that; `a_photocyte_is_occluded_by_cells_above_it` is the second.
    ///
    /// # Why the assertions are about the *tile* and not only the organism
    ///
    /// Because a version of this that checked the organism had gained energy, and that the
    /// books still balanced, would pass perfectly against an implementation that conjured the
    /// energy out of nothing. SPEC section 5 is explicit: the invariant cannot see energy that
    /// was never declared. So what is measured is that the tile went **down** by what the
    /// organism went **up** by, that the `biomass` account moved by the same amount, and that
    /// no other tile in the world was touched.
    ///
    /// # And why a second photocyte on a poorer tile has to earn less
    ///
    /// "Rate ∝ local energy" is the part that makes the light gradient mean something. An
    /// implementation that harvested a flat amount per tick would satisfy every other claim
    /// here and would quietly remove the reason to be anywhere in particular: depth would stop
    /// costing anything, and SPEC section 4's insistence that "the gradient is what gives
    /// movement a reason to exist" would be false.
    #[test]
    fn a_photocyte_harvests_from_the_tile_it_occupies() {
        let mut scene = Scene::new(&config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.limits.max_organisms = 4;
            raw.limits.max_cells_per_organism = 8;
        }));
        scene.fill_with_light();

        // One photocyte near the surface and one near the floor, far enough apart that
        // neither is standing anywhere near the other.
        let genome = scene.no_genome();
        let bright = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Photocyte, Vec2::new(40.0, 12.0), None)],
        );
        let dim = scene.add(
            genome,
            0.0,
            &[(CellKind::Photocyte, Vec2::new(200.0, 132.0), None)],
        );

        let shallow_tile = scene.grid.tile_at(Vec2::new(40.0, 12.0));
        let deep_tile = scene.grid.tile_at(Vec2::new(200.0, 132.0));
        let held = f64::from(scene.grid.tiles()[shallow_tile]);
        let tiles_before: Vec<f32> = scene.grid.tiles().to_vec();
        let biomass_before = scene.ledger.biomass();

        scene.run();

        let earned = scene.energy(bright);
        let earned_deep = scene.energy(dim);

        assert!(
            earned > 0.0,
            "a photocyte standing on a tile holding {held} earned nothing at all"
        );
        // A hair's tolerance rather than none, and the hair is the point. What the organism
        // is holding is what the *tile* gave up, not what was asked of it - a tile is a
        // 32-bit number and this is not, so taking a quantity out of one does not lower it by
        // exactly that quantity. `grid.rs` hands back the realised figure precisely so that
        // the organism and the books hold the same number rather than nearly the same one.
        assert!(
            (earned - HARVEST_RATE * held).abs() < 1e-6,
            "a photocyte on a tile holding {held} took {earned}, and the rate is meant to \
             follow what the tile is holding"
        );

        // The tile paid for it, and only that tile.
        let mut paid = 0.0;
        for (tile, (before, after)) in tiles_before.iter().zip(scene.grid.tiles()).enumerate() {
            let fell = f64::from(*before) - f64::from(*after);
            if tile == shallow_tile || tile == deep_tile {
                paid += fell;
            } else {
                assert!(
                    fell.abs() < f64::EPSILON,
                    "tile {tile} has no photocyte anywhere near it and gave up {fell}"
                );
            }
        }
        assert!(
            (paid - (earned + earned_deep)).abs() < 1e-12,
            "the two tiles gave up {paid} and the two organisms are holding {} between them, \
             so the difference has been invented or destroyed",
            earned + earned_deep
        );
        assert!(
            (scene.ledger.biomass() - biomass_before - (earned + earned_deep)).abs() < 1e-12,
            "the biomass account moved by {} while the organisms gained {}",
            scene.ledger.biomass() - biomass_before,
            earned + earned_deep
        );

        // Depth costs. A tile at the floor holds a quarter of what one at the surface does at
        // SPEC's default gradient, so the photocyte down there earns proportionally less.
        assert!(
            earned_deep < earned * 0.6,
            "a photocyte at the floor earned {earned_deep} against {earned} at the surface, \
             so harvesting is not following the local energy and depth costs nothing"
        );
    }

    /// Two photocytes sharing a tile take equal shares of what was in it when the tick began,
    /// rather than the first one served taking its share of the whole and the second taking
    /// its share of the remainder.
    ///
    /// The claim the whole two-pass shape of this file exists to make, on the simplest case
    /// there is. Written as one walk - read a tile, take from it, move on - the answer would
    /// depend on which of the two the loop happened to reach first, and an organism's income
    /// would quietly depend on which slot of the arena its rival was living in. The order is
    /// fixed either way, so a run would still reproduce; it would just be reproducing
    /// something nobody chose.
    ///
    /// The two are placed level with one another so that neither shades the other, which
    /// leaves the tile as the only thing they are sharing.
    #[test]
    fn two_photocytes_on_one_tile_share_what_was_there_when_the_tick_began() {
        let mut scene = Scene::new(&evenly_lit());
        scene.fill_with_light();
        let genome = scene.no_genome();

        let first = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Photocyte, Vec2::new(41.0, 72.0), None)],
        );
        let second = scene.add(
            genome,
            0.0,
            &[(CellKind::Photocyte, Vec2::new(46.0, 72.0), None)],
        );

        let tile = scene.grid.tile_at(Vec2::new(41.0, 72.0));
        assert_eq!(
            tile,
            scene.grid.tile_at(Vec2::new(46.0, 72.0)),
            "the two photocytes are meant to be standing on one tile, so this test is not \
             looking at what it thinks it is"
        );
        let held = f64::from(scene.grid.tiles()[tile]);

        scene.run();

        assert!(
            (scene.energy(first) - scene.energy(second)).abs() < 1e-12,
            "the two took {} and {} out of one tile, so whichever the loop reached first got \
             its share of a fuller tile",
            scene.energy(first),
            scene.energy(second)
        );
        assert!(
            (scene.energy(first) - HARVEST_RATE * held).abs() < 1e-6,
            "each of the two took {} out of a tile holding {held}, rather than the {} its \
             rate asks for",
            scene.energy(first),
            HARVEST_RATE * held
        );
    }

    /// A world with the depth gradient and the blotchiness turned off, so that every tile
    /// holds exactly the same and the only thing that can make two photocytes earn different
    /// amounts is what is standing over them.
    ///
    /// Both are confounds rather than complications. A body arranged vertically spans several
    /// rows of tiles and a body arranged horizontally spans one, so with the gradient left in
    /// place a comparison between the two shapes would be measuring the gradient as much as
    /// the shadow - and the blotchiness would put a different ceiling under every cell.
    fn evenly_lit() -> Config {
        config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.light.gradient = 0.0;
            raw.light.patchiness = 0.0;
            raw.limits.max_organisms = 8;
            raw.limits.max_cells_per_organism = 8;
        })
    }

    /// What one photocyte at `at` earns in a tick, with `blockers` standing in the water
    /// around it - each in an organism of its own, so nothing is being shaded only by its own
    /// body unless a test says so.
    fn earnings(at: Vec2, blockers: &[(CellKind, Vec2)]) -> f64 {
        let mut scene = Scene::new(&evenly_lit());
        scene.fill_with_light();

        let genome = scene.no_genome();
        let eater = scene.add(genome.clone(), 0.0, &[(CellKind::Photocyte, at, None)]);
        for &(kind, where_it_is) in blockers {
            scene.add(genome.clone(), 0.0, &[(kind, where_it_is, None)]);
        }

        scene.run();
        scene.energy(eater)
    }

    /// ⭐⭐ **A2.** A photocyte is shaded by whatever is standing above it, and that is the
    /// only reason a body has any reason to be a particular shape.
    ///
    /// SPEC section 6, on the photocyte: *"**Occluded by cells above it** — this is what
    /// rewards spread-out, branching body plans over compact blobs."* Without it, every
    /// arrangement of a given number of photocytes earns exactly the same, a blob is as good
    /// as a frond, and the project's central bet - that shape *emerges* rather than being
    /// designed - has nothing to push on it.
    ///
    /// SPEC says occlusion happens and says nothing whatever about how to compute it. The
    /// model was chosen in Phase 4 and is written out in full on [`Behaviour::shade`]; this
    /// test is where each of its decisions is nailed down, so that changing one of them
    /// breaks a named claim rather than quietly changing what bodies are worth growing.
    ///
    /// # The claims, in the order they are made below
    ///
    /// **Nothing above, nothing lost.** A photocyte in clear water earns the full rate, so
    /// occlusion is a deduction from something rather than the whole of the income.
    ///
    /// **Straight overhead is the worst case.** A cell six units above takes nearly half the
    /// light. The number is pinned rather than merely "less", because "less" is satisfied by a
    /// shadow so faint that no lineage would ever be selected on it.
    ///
    /// **The shadow has a width, and it is the width of the two cells.** A blocker moved
    /// sideways until the discs no longer overlap casts nothing at all, and one moved half way
    /// there casts half. That taper is what makes the pressure *smooth*: a mutation that
    /// nudges a limb's angle by a hair changes its income by a hair, and a shadow with a hard
    /// edge would give a body a cliff to fall off instead of a slope to climb.
    ///
    /// **Light comes from the surface.** SPEC section 2 puts the origin at the top-left with
    /// +Y downwards, so a cell *below* a photocyte casts nothing on it. Get this the wrong way
    /// up and the whole pressure inverts: bodies would be rewarded for hanging their
    /// photocytes underneath everything else.
    ///
    /// **The shadow fades with depth.** A blocker far above is nearly transparent. This is the
    /// decision that keeps occlusion a *body-scale* effect rather than a global one - see
    /// [`Behaviour::shade`] for why that matters more than it sounds.
    ///
    /// **Light does not care whose cell it is.** A blocker belonging to somebody else shades
    /// exactly as hard as one belonging to the same body.
    ///
    /// **Shade accumulates, and never quite finishes.** Each extra cell overhead costs
    /// something, and a photocyte at the bottom of a stack still earns a little - so there is
    /// always a gradient for selection to climb, however badly buried a cell is.
    ///
    /// The consequence of all of that -- that a spread-out body outearns a compact one -- has a
    /// test of its own next door, because it is the claim SPEC actually makes and it should
    /// fail under its own name.
    #[test]
    fn a_photocyte_is_occluded_by_cells_above_it() {
        let at = Vec2::new(40.0, 72.0);
        let clear = earnings(at, &[]);
        assert!(clear > 0.0, "a photocyte in clear water earned nothing");

        // Straight overhead, six units up: a sclerocyte's 3.4 and a photocyte's 3.0 overlap
        // completely, and six units is a little under a quarter of the depth a shadow reaches,
        // so 1 - 6/27.2 of a full cell's worth of shade stands between this photocyte and the
        // surface. That is an optical depth of 0.779, and 1/(1 + 0.779) of the light gets
        // through.
        let overhead = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(40.0, 66.0))]);
        assert!(
            ((overhead / clear) - 0.562).abs() < 0.002,
            "a photocyte with a cell six units directly above it kept {} of its income, \
             against the 0.562 this model gives",
            overhead / clear
        );

        // The shadow is exactly as wide as the two cells are, and it tapers rather than
        // stopping dead. Half way out, half the shade.
        let half_out = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(43.2, 66.0))]);
        assert!(
            half_out > overhead && half_out < clear,
            "a blocker half way out of the shadow took {} of the light, against {} for one \
             directly overhead and nothing at all for one clear of it - so the edge of a \
             shadow is a cliff rather than a slope, and a body has nothing to climb",
            1.0 - half_out / clear,
            1.0 - overhead / clear
        );
        let beside = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(47.0, 66.0))]);
        assert!(
            (beside - clear).abs() < 1e-12,
            "a blocker seven units to the side still shades a photocyte, and the two cells \
             are 6.4 units wide between them - so the shadow is wider than the thing casting \
             it"
        );

        // Light comes from y = 0. A cell underneath casts nothing.
        let underneath = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(40.0, 78.0))]);
        assert!(
            (underneath - clear).abs() < 1e-12,
            "a cell six units *below* a photocyte shaded it, so the world is lit from the \
             floor and every pressure in this model is upside down"
        );

        // And it fades with distance, so a shadow is a local thing.
        let far_above = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(40.0, 72.0 - 25.0))]);
        assert!(
            far_above > overhead && far_above > clear * 0.9,
            "a blocker twenty-five units up cost {} of the light against {} for one six \
             units up, and a shadow that reaches that far is a statement about the whole \
             water column rather than about a body's own shape",
            1.0 - far_above / clear,
            1.0 - overhead / clear
        );
        let out_of_range = earnings(at, &[(CellKind::Sclerocyte, Vec2::new(40.0, 72.0 - 30.0))]);
        assert!(
            (out_of_range - clear).abs() < 1e-12,
            "a blocker thirty units up still shades, and a shadow is meant to have run out \
             by twenty-seven"
        );

        // Light does not know whose cell it is. `earnings` puts every blocker in an organism
        // of its own, so the number above was already a foreign body; this is the same shape
        // grown as one body, and it has to come out the same.
        let own_body = {
            let mut scene = Scene::new(&evenly_lit());
            scene.fill_with_light();
            let genome = scene.no_genome();
            let slot = scene.add(
                genome,
                0.0,
                &[
                    (CellKind::Photocyte, at, None),
                    (CellKind::Sclerocyte, Vec2::new(40.0, 66.0), None),
                ],
            );
            scene.run();
            scene.energy(slot)
        };
        assert!(
            (own_body - overhead).abs() < 1e-12,
            "a cell of the photocyte's own body shaded it by {own_body} and a stranger's by \
             {overhead}, so light is being told which organism it is falling on"
        );

        // Shade accumulates, and a buried photocyte is never quite in the dark.
        let stacked: Vec<(CellKind, Vec2)> = (1..=7u8)
            .map(|above| {
                (
                    CellKind::Sclerocyte,
                    Vec2::new(40.0, 72.0 - f32::from(above) * 3.4),
                )
            })
            .collect();
        let buried = earnings(at, &stacked);
        assert!(
            buried < overhead,
            "seven cells overhead shaded no more than one, so a body pays nothing for \
             piling up"
        );
        assert!(
            buried > 0.0,
            "a photocyte under seven cells earns exactly nothing, so there is no longer any \
             gradient for selection to find its way out of a blob by"
        );
    }

    /// ⭐⭐ **The reason occlusion exists.** Four photocytes spread out sideways earn far more
    /// than the same four stacked on top of one another.
    ///
    /// SPEC section 6 gives this as the *purpose* of occluding photocytes - it "rewards
    /// spread-out, branching body plans over compact blobs" - so it gets a test of its own
    /// rather than being the last claim of a longer one. If shading is ever weakened or
    /// removed, this is the line in the output that should say so, because this is the
    /// sentence that stops being true.
    ///
    /// What it would mean for it to fail is worth stating plainly: every arrangement of a
    /// given number of photocytes would be worth exactly the same, a blob would be as good as
    /// a frond, and there would be nothing whatever in this simulation selecting on the shape
    /// of a body. The project's premise is that shape emerges. This is the pressure it emerges
    /// under.
    ///
    /// The two bodies are the same four cells at the same depth in water with no gradient and
    /// no blotchiness in it, so the only difference between them is which of them are standing
    /// in each other's light. Measured: **1.6 times**, and the assertion is at 1.4 so that
    /// tuning the shadow has some room before this has to be argued about.
    #[test]
    fn a_spread_out_body_earns_more_than_a_compact_one() {
        let spread = {
            let mut scene = Scene::new(&evenly_lit());
            scene.fill_with_light();
            let genome = scene.no_genome();
            let body: Vec<(CellKind, Vec2, Option<u8>)> = (0..4u8)
                .map(|n| {
                    (
                        CellKind::Photocyte,
                        Vec2::new(40.0 + f32::from(n) * 8.0, 72.0),
                        None,
                    )
                })
                .collect();
            let slot = scene.add(genome, 0.0, &body);
            scene.run();
            scene.energy(slot)
        };
        let blob = {
            let mut scene = Scene::new(&evenly_lit());
            scene.fill_with_light();
            let genome = scene.no_genome();
            let body: Vec<(CellKind, Vec2, Option<u8>)> = (0..4u8)
                .map(|n| {
                    (
                        CellKind::Photocyte,
                        Vec2::new(140.0, 60.0 + f32::from(n) * 8.0),
                        None,
                    )
                })
                .collect();
            let slot = scene.add(genome, 0.0, &body);
            scene.run();
            scene.energy(slot)
        };

        assert!(
            spread > blob * 1.4,
            "four photocytes spread sideways earned {spread} and the same four stacked up \
             earned {blob} - and if those two are close, then every arrangement of a body is \
             worth the same and nothing in this simulation has any reason to have a shape"
        );
    }

    /// ⭐ **A3.** A devorocyte touching a grain of dead biomass drains it, and the grain loses
    /// exactly what the devorocyte gains.
    ///
    /// SPEC section 6: a devorocyte, "on contact, drains energy from detritus". SPEC section
    /// 5 names the movement - `detritus → biomass` - and this is where it happens.
    ///
    /// # Nothing in the world makes detritus yet, and this test still means something
    ///
    /// Group B is where a death produces detritus and where detritus sinks and decays. What
    /// Group A owes is the *mouth*: a devorocyte that can find dead biomass and take energy
    /// out of it without going round the books. So [`Detritus`] is defined here as the
    /// smallest thing that can be bitten - a position and a quantity - and the drift is handed
    /// in by the caller. `world.rs` hands in an empty one and will until Group B fills it.
    ///
    /// The alternative was to defer the test until there was something to eat, which would
    /// have left a devorocyte with half its function written and no way to try the other half
    /// until a phase that has quite enough of its own difficulty in it.
    ///
    /// # What is asserted
    ///
    /// That the grain went **down** by what the organism went **up** by, and that the ledger's
    /// two accounts moved with them - not merely that the books balance, which they would do
    /// just as happily if a devorocyte were inventing its dinner. That contact is required, so
    /// a devorocyte a body-width away from a meal gets nothing. And that a grain with less in
    /// it than a full bite gives what it has and stops, rather than being taken below nothing
    /// and owing the difference back.
    #[test]
    fn a_devorocyte_drains_detritus_on_contact() {
        let mut scene = Scene::new(&evenly_lit());
        scene.fill_with_light();
        let genome = scene.no_genome();

        // One devorocyte in reach of a grain, one well clear of its own.
        let feeding = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(40.0, 72.0), None)],
        );
        let hungry = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(120.0, 72.0), None)],
        );
        // And one on a grain holding far less than a bite.
        let scraping = scene.add(
            genome,
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(200.0, 72.0), None)],
        );

        let full_grain = scene.drop_detritus(Vec2::new(42.0, 72.0), 1.0);
        scene.drop_detritus(Vec2::new(125.0, 72.0), 1.0);
        let crumb = scene.drop_detritus(Vec2::new(202.0, 72.0), 0.001);

        let detritus_before = scene.ledger.detritus();
        let biomass_before = scene.ledger.biomass();

        scene.run();

        let eaten = scene.energy(feeding);
        assert!(
            (eaten - DEVOUR_RATE).abs() < 1e-12,
            "a devorocyte in contact with a grain of detritus took {eaten} rather than the \
             {DEVOUR_RATE} a bite is worth"
        );
        assert!(
            (scene.detritus[0].energy - (full_grain - DEVOUR_RATE)).abs() < 1e-12,
            "the grain is holding {} after giving up {eaten}",
            scene.detritus[0].energy
        );
        assert!(
            (scene.energy(hungry)).abs() < f64::EPSILON,
            "a devorocyte five units from the nearest grain ate {}, so contact is not \
             required and a devorocyte feeds on the whole world at once",
            scene.energy(hungry)
        );

        // A grain gives what it has and no more. Nothing may take detritus below nothing:
        // that would be a debt in an account with no running total to notice it.
        assert!(
            (scene.energy(scraping) - crumb).abs() < 1e-12,
            "a devorocyte on a grain holding {crumb} took {}",
            scene.energy(scraping)
        );
        assert!(
            scene.detritus[2].energy >= 0.0 && scene.detritus[2].energy < 1e-12,
            "a grain that was eaten out is holding {}",
            scene.detritus[2].energy
        );

        // And the books moved, in both directions, by the same amount.
        let alive: f64 = (0..3).map(|slot| scene.energy(slot)).sum();
        assert!(
            (scene.ledger.biomass() - biomass_before - alive).abs() < 1e-12,
            "the biomass account moved by {} while the organisms gained {alive}",
            scene.ledger.biomass() - biomass_before
        );
        assert!(
            (detritus_before - scene.ledger.detritus() - alive).abs() < 1e-12,
            "the detritus account fell by {} while the organisms gained {alive}, so the \
             difference has been invented",
            detritus_before - scene.ledger.detritus()
        );
    }

    /// ⭐ **A4.** A devorocyte in contact with another organism's cell drains it, and a tough
    /// cell gives up far less than a soft one.
    ///
    /// SPEC section 6 gives a devorocyte's other half: it drains "from another organism's
    /// cells at a rate reduced by that cell's toughness". SPEC section 10 and CLAUDE.md's
    /// decision log then both say the same thing twice over, and it is the reason this test
    /// looks the way it does:
    ///
    /// > *"Predation is emergent. A living body is a denser package of energy than the
    /// > surrounding soup, so devorocytes contacting foreign cells is simply a better strategy
    /// > under some conditions. Whether a herbivore/predator split appears is one of the
    /// > genuinely interesting outcomes and must never be scripted."*
    ///
    /// # What "never scripted" means in the code, and what this test is guarding
    ///
    /// There is **no targeting anywhere in this file.** Nothing looks for prey, nothing
    /// chooses between a soft cell and a hard one, nothing prefers a full organism to an empty
    /// one, and no organism can tell a predator from anything else. The whole of predation is
    /// the sentence "a devorocyte in contact with foreign biomass drains it", and every
    /// interesting thing that might follow - hunting, herding, armour, a herbivore/predator
    /// split, or none of them - has to come out of *movement* and *shape*, which are what a
    /// genome actually controls.
    ///
    /// A test cannot easily assert the absence of a feature. What it can do is pin the two
    /// numbers that would have to be tampered with to sneak one in, and that is what is here:
    /// the rate depends on the victim's toughness and on nothing else at all, and a devorocyte
    /// takes from whatever it happens to be touching.
    ///
    /// # Why armour has to cost the predator rather than merely slow it down
    ///
    /// A sclerocyte gives up a ninth of what a soft cell does, and the number that matters is
    /// that this is **less than a devorocyte's own upkeep**. Armour that only halved a bite
    /// would be a tax: worth eating through anyway, and a lineage that grew it would have paid
    /// for cells that contribute nothing and bought nothing back. At a ninth, biting a
    /// sclerocyte leaves a predator worse off than not biting at all - which is what SPEC
    /// section 6 means by calling sclerocytes "the answer to predation".
    ///
    /// # And a body cannot eat itself
    ///
    /// A devorocyte adjacent to its own organism's cells takes nothing. Not because eating
    /// yourself would break the books - it moves `biomass → biomass` and balances perfectly -
    /// but because it would be free energy: an organism could grow a devorocyte beside a
    /// gonocyte and shuffle its own energy round in a circle for ever, which is a strategy the
    /// mutation operators would find in an afternoon.
    #[test]
    fn a_devorocyte_drains_a_foreign_cell_at_a_rate_its_toughness_reduces() {
        let mut scene = Scene::new(&evenly_lit());
        scene.fill_with_light();
        let genome = scene.no_genome();

        // Three pairs, each a devorocyte in contact with a foreign cell of a different kind,
        // and each pair far enough from the others to have nothing to do with them.
        let soft_eater = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(40.0, 72.0), None)],
        );
        let soft_prey = scene.add(
            genome.clone(),
            4.0,
            &[(CellKind::Gonocyte, Vec2::new(44.0, 72.0), None)],
        );
        let armoured_eater = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(120.0, 72.0), None)],
        );
        let _armoured_prey = scene.add(
            genome.clone(),
            4.0,
            &[(CellKind::Sclerocyte, Vec2::new(124.0, 72.0), None)],
        );
        let out_of_reach = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(190.0, 72.0), None)],
        );
        let untouched = scene.add(
            genome.clone(),
            4.0,
            &[(CellKind::Gonocyte, Vec2::new(197.0, 72.0), None)],
        );

        // And one organism that is a devorocyte sitting against its own gonocyte.
        let itself = scene.add(
            genome,
            4.0,
            &[
                (CellKind::Devorocyte, Vec2::new(60.0, 20.0), None),
                (CellKind::Gonocyte, Vec2::new(64.0, 20.0), None),
            ],
        );

        let prey_at_first = scene.energy(soft_prey);
        let alone_at_first = scene.energy(untouched);
        let itself_at_first = scene.energy(itself);
        let biomass_before = scene.ledger.biomass();
        scene.run();

        let soft = scene.energy(soft_eater);
        let armoured = scene.energy(armoured_eater);

        assert!(
            soft > 0.0 && armoured > 0.0,
            "a devorocyte in contact with a foreign body took nothing at all"
        );
        assert!(
            ((soft / armoured) - 9.0).abs() < 0.01,
            "a soft cell gave up {soft} and a sclerocyte {armoured}, a ratio of {}, against \
             the nine this toughness table gives",
            soft / armoured
        );
        assert!(
            armoured < f64::from(CellKind::Devorocyte.upkeep()),
            "biting a sclerocyte returns {armoured} against the {} a devorocyte costs to \
             keep, so armour merely slows a predator down instead of making it worse off - \
             and SPEC section 6's claim that sclerocytes are the answer to predation is not \
             true of these numbers",
            CellKind::Devorocyte.upkeep()
        );

        // What one gained, the other lost. This is the one movement in the world with living
        // tissue at both ends, so no total changes - which is exactly why it needs checking
        // from both sides.
        assert!(
            (scene.energy(soft_prey) - (prey_at_first - soft)).abs() < 1e-12,
            "the prey is holding {} after being drained of {soft}",
            scene.energy(soft_prey)
        );
        assert!(
            (scene.ledger.biomass() - biomass_before).abs() < 1e-12,
            "the biomass account moved by {} over a tick in which the only thing that \
             happened was living tissue changing hands",
            scene.ledger.biomass() - biomass_before
        );

        // Contact is required, and a body cannot eat itself.
        assert!(
            scene.energy(out_of_reach).abs() < f64::EPSILON
                && (scene.energy(untouched) - alone_at_first).abs() < f64::EPSILON,
            "a devorocyte seven units from the nearest foreign cell drained it anyway"
        );
        assert!(
            (scene.energy(itself) - itself_at_first).abs() < 1e-12,
            "an organism whose devorocyte sits against its own gonocyte is holding {} \
             rather than the {itself_at_first} it started with, so a body can eat itself and \
             any lineage that grows that pair has found free energy",
            scene.energy(itself)
        );
    }

    /// A victim is never drained past what it is holding, however many things are eating it.
    ///
    /// The one place in this file where two callers reach for the same energy at once, and
    /// therefore the one place where the order the loop happens to visit them in could decide
    /// who gets fed. Both drains are worked out against what the victim held at the *start* of
    /// the tick and then scaled down together, so each takes the same share whichever is
    /// looked at first - and between them they take exactly what was there.
    ///
    /// `Grid::harvest` already refuses to take a tile below nothing for the same reason, and
    /// this is that discipline applied to the other thing in the world that can be eaten. It
    /// matters more than it looks: predators taking more than the victim had would leave that
    /// organism holding a debt, and Group B's death would then move a negative quantity into
    /// the detritus account - which is energy created out of an overdraft.
    #[test]
    fn a_victim_is_not_drained_below_what_it_holds() {
        let mut scene = Scene::new(&evenly_lit());
        scene.fill_with_light();
        let genome = scene.no_genome();

        // Two devorocytes either side of one nearly-empty gonocyte. Each would take 0.045 on
        // its own; between them they want 0.09 and there is a tenth of that to be had.
        let left = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(36.0, 72.0), None)],
        );
        let right = scene.add(
            genome.clone(),
            0.0,
            &[(CellKind::Devorocyte, Vec2::new(44.0, 72.0), None)],
        );
        let prey = scene.add(
            genome,
            0.009,
            &[(CellKind::Gonocyte, Vec2::new(40.0, 72.0), None)],
        );
        let held = scene.energy(prey);

        scene.run();

        assert!(
            scene.energy(prey) >= 0.0 && scene.energy(prey) < 1e-12,
            "a gonocyte holding {held} was drained to {}, so two predators between them took \
             more than there was",
            scene.energy(prey)
        );
        assert!(
            (scene.energy(left) - scene.energy(right)).abs() < 1e-12,
            "the two predators took {} and {}, so the one the loop happened to reach first \
             got the better of it",
            scene.energy(left),
            scene.energy(right)
        );
        assert!(
            (scene.energy(left) + scene.energy(right) - held).abs() < 1e-12,
            "the two predators took {} between them out of an organism holding {held}",
            scene.energy(left) + scene.energy(right)
        );
    }

    /// A gene that says nothing about development and everything about behaviour: how the
    /// cells it built oscillate, and how they respond to what they sense.
    ///
    /// ⚠️ The `state` is which cells this gene *fires on* during development, and since Phase
    /// 7 it has nothing to do with whose behaviour the gene carries - that is decided by which
    /// cells name it in their `gene`, which a scene sets by hand. It is kept as an argument
    /// because a genome of several genes wants them told apart, and
    /// `a_myocyte_takes_its_rhythm_from_the_gene_that_built_it` deliberately points a cell at
    /// a gene whose trigger state nothing in the body is in.
    fn a_behaviour_gene(
        state: u8,
        osc_freq: f32,
        osc_phase: f32,
        sensor_gain: f32,
        sensor_target: SensorTarget,
    ) -> Gene {
        Gene {
            trigger_state: State::new(state),
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
            osc_freq,
            osc_phase,
            sensor_gain,
            sensor_target,
        }
    }

    /// ⭐⭐ **A myocyte takes its rhythm from the gene that built it**, and not from a gene
    /// looked up by the state it happens to be in.
    ///
    /// **This is the change that connected the muscles to the genome**, and the test is written
    /// so that it can only pass under the new rule. The body below is the case the shipped
    /// world is almost entirely made of and the old rule could not reach: a myocyte sitting in
    /// state 44, with **no gene in its genome naming state 44**, built by a gene that names a
    /// state nothing in the body is in. Under the state lookup that muscle is silent - it has
    /// no frequency, no phase and no gain, and its spring is never touched. Under this rule it
    /// works the full stroke, because the gene that said "make a myocyte here" is the gene that
    /// says how that myocyte moves.
    ///
    /// Measured over 120,000 ticks of the shipped world before the change: **0.05% of grown
    /// cells** were in a state their own genome named, and **not one myocyte in the world was**.
    /// See [`crate::development::develop`] for the full argument and `docs/PHASE7.md` for the
    /// count.
    ///
    /// # The second half is what stops this being a rule with a hole in it
    ///
    /// A myocyte with **no gene at all** still does nothing, and pays nothing. That case is not
    /// hypothetical in this file - a scene can build one - and it is unreachable in a real body,
    /// which `development.rs`'s `a_cell_with_no_gene_is_the_seed_cell_and_needs_none` proves:
    /// only a seed cell can lack a gene and a seed cell is always a photocyte. The fallback is
    /// still asserted here rather than assumed, because the alternative to asserting it is a
    /// default rhythm nobody selected.
    #[test]
    fn a_myocyte_takes_its_rhythm_from_the_gene_that_built_it() {
        let settings = evenly_lit();
        let mut scene = Scene::new(&settings);
        scene.fill_with_light();

        // One gene, and it answers to a state no cell in this body is in. The old rule found a
        // myocyte's gene by matching its `state`; this genome offers that rule nothing.
        let genome = Genome::new(
            vec![a_behaviour_gene(7, 3.0, 0.0, 0.0, SensorTarget::Light)],
            &settings.limits,
        );

        let wired = scene.add(
            genome,
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(40.0, 72.0), Some(0)),
                (CellKind::Sclerocyte, Vec2::new(48.0, 72.0), None),
            ],
        );
        scene.adhere(0, 1, 8.0, 10.0);
        scene.cells[0].state = 44;

        // The same body with nothing speaking for the muscle at all.
        let deaf = scene.add(
            Genome::new(
                vec![a_behaviour_gene(7, 3.0, 0.0, 0.0, SensorTarget::Light)],
                &settings.limits,
            ),
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(140.0, 72.0), None),
                (CellKind::Sclerocyte, Vec2::new(148.0, 72.0), None),
            ],
        );
        scene.adhere(2, 3, 8.0, 10.0);
        scene.cells[2].state = 44;

        let (mut longest, mut shortest) = (0.0f32, f32::MAX);
        for _ in 0..130 {
            scene.run();
            longest = longest.max(scene.springs[0].rest_length);
            shortest = shortest.min(scene.springs[0].rest_length);

            assert!(
                (scene.springs[1].rest_length - 8.0).abs() < f32::EPSILON,
                "a myocyte with no gene at all worked its spring to {}, so it is oscillating \
                 to a rhythm nobody selected",
                scene.springs[1].rest_length
            );
        }

        // The full stroke: `resting_amplitude × stroke` of eight units either way, which at
        // the shipped 0.8 and 1.0 is 1.6 to 14.4. The same figures
        // `a_myocyte_oscillates_its_springs_and_pays_for_the_work` records for a muscle the old
        // rule *could* reach.
        assert!(
            (shortest - 1.6).abs() < 0.01 && (longest - 14.4).abs() < 0.01,
            "a myocyte built by a gene worked its spring between {shortest} and {longest}, \
             against the 1.6 to 14.4 SPEC section 9's controller gives - so a cell is not \
             taking its behaviour from the gene that made it"
        );
        assert!(
            scene.energy(wired) < 4.0,
            "a muscle that moved a spring paid nothing for the work"
        );
        assert!(
            (scene.energy(deaf) - 4.0).abs() < f64::EPSILON,
            "a myocyte no gene speaks for paid {} for movement",
            4.0 - scene.energy(deaf)
        );
    }

    /// ⭐⭐ **Phase 7, Group H.** How far a myocyte works its spring is a setting, and both ends
    /// of it come out of the document rather than out of this file.
    ///
    /// SPEC section 9's controller used to have two numbers written into it - the 0.3 an
    /// unsensed muscle contracts at, and the 0.4 of its rest length a fully-driven one works
    /// through - and Group H's measurement is that **the second of them is the only lever in
    /// the project that makes swimming worth doing.** Speed goes as roughly the cube of that
    /// coefficient and as the square root of everything else, so a number nobody could reach
    /// from a settings file was the number that decided whether a muscle was worth owning.
    ///
    /// Three claims, and the third is the one that makes this a test rather than a restatement:
    ///
    /// **The shipped settings give the shipped swing.** `resting_amplitude × stroke` is
    /// `0.8 × 1.0`, so a spring asked to be eight units long is worked between 1.6 and 14.4.
    ///
    /// **A document that says otherwise is obeyed.** The same body under a `[behaviour]` table
    /// holding SPEC's original 0.3 and 0.4 is worked between 7.04 and 8.96, which is what
    /// `a_myocyte_oscillates_its_springs_and_pays_for_the_work` measured before this group -
    /// so the old world is still reachable, and it is reachable *as a configuration* rather
    /// than as a version of the source.
    ///
    /// **⚠️ And at the top of the range a rest length reaches nothing and does not pass it.**
    /// A stroke of one is exactly where `base × (1 − stroke)` hits zero; `config.rs` refuses
    /// anything above, and the reason is that a negative rest length is a spring that pulls at
    /// every phase of its cycle instead of oscillating - a body hauling itself through its own
    /// cells, which looks like very fast swimming and is not swimming at all.
    #[test]
    fn a_myocyte_works_through_the_stroke_the_settings_give_it() {
        let worked = |resting: f64, stroke: f64| {
            let mut settings = evenly_lit();
            settings.behaviour.resting_amplitude = narrowed(resting);
            settings.behaviour.stroke = narrowed(stroke);

            let mut scene = Scene::new(&settings);
            scene.fill_with_light();

            let genome = Genome::new(
                vec![a_behaviour_gene(1, 3.0, 0.0, 0.0, SensorTarget::Light)],
                &settings.limits,
            );
            scene.add(
                genome,
                4.0,
                &[
                    (CellKind::Myocyte, Vec2::new(40.0, 72.0), Some(0)),
                    (CellKind::Sclerocyte, Vec2::new(48.0, 72.0), None),
                ],
            );
            scene.adhere(0, 1, 8.0, 10.0);

            let (mut longest, mut shortest) = (0.0f32, f32::MAX);
            for _ in 0..130 {
                scene.run();
                longest = longest.max(scene.springs[0].rest_length);
                shortest = shortest.min(scene.springs[0].rest_length);
            }

            (shortest, longest)
        };

        let shipped = spec_defaults().behaviour;
        let (shortest, longest) = worked(shipped.resting_amplitude, shipped.stroke);
        assert!(
            (shortest - 1.6).abs() < 0.01 && (longest - 14.4).abs() < 0.01,
            "at the shipped `behaviour` table a spring asked to be eight units long was \
             worked between {shortest} and {longest}, against the 1.6 to 14.4 that a resting \
             amplitude of 0.8 and a stroke of 1.0 give"
        );

        // SPEC section 9 as it was written before Group H, reachable as a configuration.
        let (was_shortest, was_longest) = worked(0.3, 0.4);
        assert!(
            (was_shortest - 7.04).abs() < 0.01 && (was_longest - 8.96).abs() < 0.01,
            "the world as it shipped until Group H worked the same spring between \
             {was_shortest} and {was_longest}, and 7.04 to 8.96 is what it measured - so the \
             old world is no longer reachable from a settings file"
        );

        // The top of the range, where the rest length reaches nothing exactly.
        let (bottom, _) = worked(1.0, 1.0);
        assert!(
            (0.0..0.01).contains(&bottom),
            "at the top of both settings the shortest a spring asked to be was {bottom}, and \
             a stroke of one is defined as the point where that reaches nought without \
             passing it - anything below is a spring that pulls at every phase of its own \
             cycle. (It stops a thousandth short because the sine is sampled sixty times a \
             second and never lands exactly on its trough.)"
        );
    }

    /// ⭐ **A5.** A myocyte works the rest length of its springs up and down, and its organism
    /// pays for the work it does.
    ///
    /// SPEC section 9's reactive controller, exactly as written there:
    ///
    /// ```text
    /// signal    = mean of connected Sensocyte outputs, or 0 if none
    /// amplitude = clamp(resting_amplitude + sensor_gain × signal, 0.0, 1.0)
    /// rest_len  = base_rest × (1 + amplitude × stroke × sin(t × osc_freq + osc_phase))
    /// ```
    ///
    /// This is that with no sensocytes anywhere, so the signal is nought and the amplitude is
    /// the bare `behaviour.resting_amplitude` - which at the shipped 0.8 and a `stroke` of 1.0
    /// makes the swing 0.8 of the rest length either way.
    /// `a_sensocyte_reports_a_gradient_towards_its_target` is where the signal starts moving,
    /// and `a_myocyte_works_through_the_stroke_the_settings_give_it` is where the two
    /// coefficients themselves are pinned.
    ///
    /// # The cost has to be a real quantity, and this is most of what the test is about
    ///
    /// SPEC section 6 says a myocyte "costs `movement_cost` × work done" and does not say what
    /// work is. Written as a flat charge per myocyte per tick it would be indistinguishable
    /// from upkeep, and there would be no selection pressure whatever towards moving
    /// *efficiently* - a lineage that thrashed uselessly and one that swam well would pay the
    /// same. So work here is the physical quantity: **force through distance**, the tension
    /// already in the spring multiplied by how far this tick's contraction moved its rest
    /// length.
    ///
    /// Two consequences are asserted below, and both are the point. A myocyte that does not
    /// oscillate pays **exactly nothing** - not almost nothing - because it moves nothing. And
    /// a stiffer spring costs proportionally more to work, so a lineage that grows powerful
    /// muscle pays for the power rather than getting it free.
    ///
    /// # And what is spent leaves the world
    ///
    /// SPEC section 5: movement moves `biomass → dissipated`. The claim below is not that the
    /// books balance but that the `dissipated` account went **up** by exactly what the
    /// organisms went **down** by - because a version of this that quietly deducted the energy
    /// from an organism and told nobody would balance perfectly and be wrong.
    #[test]
    fn a_myocyte_oscillates_its_springs_and_pays_for_the_work() {
        let settings = evenly_lit();
        let mut scene = Scene::new(&settings);
        scene.fill_with_light();

        let genome = Genome::new(
            vec![a_behaviour_gene(1, 3.0, 0.0, 0.0, SensorTarget::Light)],
            &settings.limits,
        );
        let inert = Genome::new(Vec::new(), &settings.limits);

        // A myocyte on one end of a spring and a sclerocyte on the other. Nothing in this
        // scene harvests, so every change in an organism's energy is the muscle.
        let swimmer = scene.add(
            genome.clone(),
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(40.0, 72.0), Some(0)),
                (CellKind::Sclerocyte, Vec2::new(48.0, 72.0), None),
            ],
        );
        scene.adhere(0, 1, 8.0, 10.0);

        // The same body with a stiffer spring, which has to cost proportionally more.
        let stiff = scene.add(
            genome,
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(140.0, 72.0), Some(0)),
                (CellKind::Sclerocyte, Vec2::new(148.0, 72.0), None),
            ],
        );
        scene.adhere(2, 3, 8.0, 20.0);

        // And the same body again with no myocyte in it at all.
        let still = scene.add(
            inert,
            4.0,
            &[
                (CellKind::Sclerocyte, Vec2::new(40.0, 20.0), None),
                (CellKind::Sclerocyte, Vec2::new(48.0, 20.0), None),
            ],
        );
        scene.adhere(4, 5, 8.0, 10.0);

        let dissipated_before = scene.ledger.dissipated();
        let (mut longest, mut shortest) = (0.0f32, f32::MAX);

        // A hundred and thirty ticks is a little over one full cycle at three radians a
        // second, so the whole swing is seen.
        for _ in 0..130 {
            scene.run();
            longest = longest.max(scene.springs[0].rest_length);
            shortest = shortest.min(scene.springs[0].rest_length);
            assert!(
                (scene.springs[2].rest_length - 8.0).abs() < f32::EPSILON,
                "a spring with no myocyte on either end was worked to {}",
                scene.springs[2].rest_length
            );
        }

        // The swing is `amplitude × behaviour.stroke` of the rest length either way, and with
        // no sensocyte the amplitude is `behaviour.resting_amplitude` - so at the shipped
        // 0.8 and 1.0 that is 0.8 of eight units, which is 6.4.
        //
        // ⚠️ **Re-recorded in Phase 7's Group H, and both previous figures are kept.** It read
        // **7.04 to 8.96** while SPEC section 9's two coefficients were the constants 0.3 and
        // 0.4, which is a swing of 0.96 - about a seventh of one cell's width, worked by a
        // muscle costing three and a half times what a photocyte costs to keep. Group H
        // measured that a body's speed goes as roughly the **cube** of that swing, and the
        // whole of Group H is this number.
        assert!(
            (longest - 14.4).abs() < 0.01 && (shortest - 1.6).abs() < 0.01,
            "the spring was worked between {shortest} and {longest}, against the 1.6 to \
             14.4 SPEC section 9's controller gives a rest length of eight at the shipped \
             resting amplitude"
        );

        let spent = 4.0 - scene.energy(swimmer);
        let spent_stiff = 4.0 - scene.energy(stiff);

        // ⚠️ **Phase 7 moved this figure by a factor of fifteen hundred, and deliberately.**
        // It read 2.790 over the hundred and thirty ticks, which is 0.0215 a tick for a spring
        // of stiffness ten worked at the bare amplitude - **one and a half times a myocyte's
        // own upkeep of 0.014, for one spring, at rest, with no sensor driving it.** A body
        // that swam therefore paid several times its own cost of living to do so, and nothing
        // it could have found on the other side would have covered that: the whole energy of a
        // tile at the shipped `cap` and `influx` is worth about 3e-4 to cross, so `movement_cost`
        // at 0.15 was roughly a thousand times break-even. Swimming could not pay whatever it
        // discovered. `config/default.toml` now ships **0.0001**, and this is that number: 0.00186
        // over the same hundred and thirty ticks, which is **1.4e-5 a tick**, or a thousandth of
        // a myocyte's upkeep.
        //
        // It is still an *upper* bound on what a real muscle pays: nothing moves in this scene,
        // so the spring never gets to relieve its own tension the way it would with the physics
        // running.
        //
        // ⚠️ **Re-recorded a second time in Group H, and both earlier figures are kept above.**
        // 0.00186 → **0.0827** over the same hundred and thirty ticks, which is the swing
        // going from 0.12 of the rest length to 0.8: work is force through distance and both
        // halves of it scale with the swing, so a stroke 6.7 times larger costs 44 times as
        // much. **That is the cost side of Group H and it is affordable**, which is the whole
        // question the number is here to answer: 6.4e-4 a tick against a myocyte's own upkeep
        // of 0.014 is **four and a half per cent** of what the cell costs to keep, for the
        // hardest-worked spring in the scene, with nothing allowed to move.
        assert!(
            (spent - 0.0827).abs() < 1e-4,
            "a myocyte spent {spent} over a hundred and thirty ticks, against the 0.0827 \
             recorded here"
        );
        assert!(
            (4.0 - scene.energy(still)).abs() < f64::EPSILON,
            "a body with no myocyte in it paid {} for movement",
            4.0 - scene.energy(still)
        );
        assert!(
            ((spent_stiff / spent) - 2.0).abs() < 0.01,
            "working a spring of twice the stiffness cost {} times as much rather than \
             twice, so the charge is not following the force the muscle is working against",
            spent_stiff / spent
        );

        // What was spent left the world.
        assert!(
            (scene.ledger.dissipated() - dissipated_before - (spent + spent_stiff)).abs() < 1e-12,
            "the dissipated account rose by {} while the organisms went down by {}",
            scene.ledger.dissipated() - dissipated_before,
            spent + spent_stiff
        );
    }

    /// A myocyte that does not move pays **exactly** nothing.
    ///
    /// The claim SPEC section 6's "costs `movement_cost` × work done" is worth nothing
    /// without. If a myocyte that holds still were charged anything at all, the charge would
    /// be a second upkeep under another name, and a lineage would be selected on how many
    /// myocytes it had rather than on what it did with them - which is the opposite of the
    /// pressure the cost exists to create.
    ///
    /// Two ways of doing nothing, and both have to come to nought. A myocyte whose gene gives
    /// it no frequency never changes its rest length, so it moves nothing through no distance.
    /// And a myocyte **no gene built** behaves as though it had a frequency of nought rather
    /// than falling back on some default rhythm nobody asked for. Since Phase 7 the second of
    /// those is unreachable in a grown body - see
    /// [`crate::development::develop`] - so it is a rule about the boundary rather than about
    /// the ordinary case it used to describe; a scene can still build one, and this is what
    /// says what happens when it does.
    #[test]
    fn a_myocyte_that_does_nothing_pays_nothing() {
        let settings = evenly_lit();
        let mut scene = Scene::new(&settings);
        scene.fill_with_light();

        let motionless = Genome::new(
            vec![a_behaviour_gene(1, 0.0, 1.0, 0.0, SensorTarget::Light)],
            &settings.limits,
        );
        let silent = Genome::new(Vec::new(), &settings.limits);

        let held = scene.add(
            motionless,
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(40.0, 72.0), Some(0)),
                (CellKind::Sclerocyte, Vec2::new(48.0, 72.0), None),
            ],
        );
        scene.adhere(0, 1, 8.0, 10.0);

        let unwired = scene.add(
            silent,
            4.0,
            &[
                (CellKind::Myocyte, Vec2::new(140.0, 72.0), None),
                (CellKind::Sclerocyte, Vec2::new(148.0, 72.0), None),
            ],
        );
        scene.adhere(2, 3, 8.0, 10.0);

        let dissipated_before = scene.ledger.dissipated();
        for _ in 0..100 {
            scene.run();
        }

        assert!(
            (scene.energy(held) - 4.0).abs() < f64::EPSILON,
            "a myocyte with no frequency paid {} over a hundred ticks, so holding a shape \
             costs the same as swimming and there is nothing to be gained by being still",
            4.0 - scene.energy(held)
        );
        assert!(
            (scene.energy(unwired) - 4.0).abs() < f64::EPSILON,
            "a myocyte no gene built paid {}, so it is oscillating to some rhythm that is not \
             in its genome",
            4.0 - scene.energy(unwired)
        );
        assert!(
            (scene.ledger.dissipated() - dissipated_before).abs() < f64::EPSILON,
            "a hundred ticks in which nothing moved still dissipated {}",
            scene.ledger.dissipated() - dissipated_before
        );

        // A rest length that is held rather than worked is held at what the gene asked for,
        // shifted once by the phase and then left there.
        //
        // ⚠️ Worked out from the settings rather than written down. Phase 7's Group H moved
        // both coefficients into `[behaviour]`, and a number here would have been a second,
        // silent copy of the shipped stroke that a retune would leave behind.
        let swing = scene.config.behaviour.resting_amplitude * scene.config.behaviour.stroke;
        assert!(
            (scene.springs[0].rest_length - 8.0 * (1.0 + swing * 1.0_f32.sin())).abs() < 1e-4,
            "a myocyte at a standstill is holding its spring at {} rather than at the one \
             length its phase puts it at",
            scene.springs[0].rest_length
        );
    }

    /// ⭐ The drift is **searched** rather than scanned, and this test is the reason the whole
    /// of Group B could not leave it alone.
    ///
    /// Group A defined a grain of detritus and let a devorocyte eat one, and did it the only
    /// way it could: by walking the whole drift for every mouth, and again for every sensocyte
    /// tuned to detritus. That cost nothing whatever while nothing in the world made a grain.
    /// Group B is what makes grains, and the moment it does, both of those walks become the
    /// population **times** the drift.
    ///
    /// # What the failure would have looked like, which is why it is worth a test
    ///
    /// Not a wrong answer. Both readings give identical results - a scan and a bucketed search
    /// find the same grains, because everything past the reach contributes nothing either way.
    /// What changes is only how long a tick takes, and it changes in a way nobody notices
    /// until an overnight run stops being an overnight run: a thousand bodies in a world with
    /// a hundred thousand grains in it is a hundred million distance measurements per tick,
    /// for a result that a spatial index gets in a few hundred.
    ///
    /// So this test is a *scale* test rather than a correctness one, and it passes either way.
    /// Twenty thousand grains and a hundred and fifty cells, ticked five times, is 15 million
    /// measurements the slow way and a few thousand the fast one. **Measured, 31 July 2026,
    /// debug build: 0.28 s scanning the drift and 0.05 s searching it.**
    ///
    /// The ratio is the thing rather than the two figures. At this size the scan is only five
    /// times slower; the point is that its cost is the drift's *length*, so twenty times the
    /// grains is twenty times the work, while the search looks at whatever is in nine buckets
    /// however full the world is. A default world can hold 256,000 grains.
    ///
    /// The assertions are what makes it worth running at all: at this scale the mouths still
    /// take exactly what they are touching, the drift gives up exactly what they take, and the
    /// books agree with both.
    #[test]
    fn a_crowded_drift_is_searched_rather_than_scanned() {
        let settings = config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.light.gradient = 0.0;
            raw.light.patchiness = 0.0;
            // The drift is built to hold one grain per cell the world can contain, so a world
            // that can hold twenty thousand grains is a world with room for that many cells.
            raw.limits.max_organisms = 400;
            raw.limits.max_cells_per_organism = 64;
        });
        let mut scene = Scene::new(&settings);
        scene.fill_with_light();
        scene.drop_a_drift(200, 100, 0.0004);

        // A hundred mouths and fifty noses, on a coarse lattice of their own so that every one
        // of them has grains around it.
        let mouths: Vec<(CellKind, Vec2, Option<u8>)> = (0..100u32)
            .map(|index| {
                (
                    CellKind::Devorocyte,
                    Vec2::new(
                        f32::from(u8::try_from(index % 10).expect("under ten")) * 25.0 + 5.0,
                        f32::from(u8::try_from(index / 10).expect("under ten")) * 14.0 + 5.0,
                    ),
                    None,
                )
            })
            .collect();
        let noses: Vec<(CellKind, Vec2, Option<u8>)> = (0..50u32)
            .map(|index| {
                (
                    CellKind::Sensocyte,
                    Vec2::new(
                        f32::from(u8::try_from(index % 10).expect("under ten")) * 25.0 + 15.0,
                        f32::from(u8::try_from(index / 10).expect("under five")) * 28.0 + 12.0,
                    ),
                    Some(0),
                )
            })
            .collect();

        let genome = Genome::new(
            vec![a_behaviour_gene(1, 0.0, 0.0, 1.0, SensorTarget::Detritus)],
            &settings.limits,
        );
        let eaters = scene.add(scene.no_genome(), 0.0, &mouths);
        scene.add(genome, 0.0, &noses);

        let grains = scene.detritus.len();
        let drift_before = scene.drift_energy();
        let detritus_before = scene.ledger.detritus();

        for _ in 0..5 {
            scene.run();
        }

        let eaten = scene.energy(eaters);
        assert!(
            eaten > 0.0,
            "a hundred devorocytes standing in {grains} grains of detritus ate nothing at all"
        );
        assert!(
            (drift_before - scene.drift_energy() - eaten).abs() < 1e-9,
            "the mouths took {eaten} and the drift gave up {}",
            drift_before - scene.drift_energy()
        );
        assert!(
            (detritus_before - scene.ledger.detritus() - eaten).abs() < 1e-9,
            "the mouths took {eaten} and the detritus account moved by {}",
            detritus_before - scene.ledger.detritus()
        );
        assert!(
            scene.drift_energy() > 0.0,
            "a hundred mouths cleared a drift of {grains} grains in five ticks, so this test \
             is not measuring a crowded drift"
        );
    }

    /// One body, built so that everything a sensocyte hears comes straight back out as a
    /// number that can be read off a spring.
    ///
    /// A myocyte and a sensocyte, adhered. The myocyte's frequency is nought and its phase is
    /// a quarter turn, so its sine is exactly one and the whole of SPEC section 9's
    /// controller collapses to `rest_len = base × (1 + amplitude × 0.4)` - which makes the
    /// amplitude, and therefore the signal, something a test can measure rather than infer.
    ///
    /// A frequency of nought also means the muscle does no work and pays nothing, so the body
    /// can be seeded holding nothing and the field is left exactly as the light made it. That
    /// matters for the light-sensing case, where a tile the body had eaten out of would be a
    /// gradient the test put there by accident.
    /// ⚠️ **`resting` is a parameter rather than the shipped setting**, and Phase 7's Group H
    /// made it one. What these scenes measure is the *signal*, read back through the one thing
    /// in the world that shows it - a myocyte's amplitude - and SPEC section 9 clamps that
    /// amplitude into `0..=1`. So a scene built at the shipped resting amplitude of 0.8 has only
    /// two tenths of room above it, and every reading strong enough to matter would come back
    /// flattened against the top of the clamp: a body four units from a neighbour and one eight
    /// units away would read *the same number*, and the test would be measuring the clamp.
    ///
    /// Each caller therefore says what it needs. Nought, for the tests about what a sensocyte
    /// reports, so the amplitude read back **is** the signal times the gain with nothing else in
    /// it; a half, for the test about the sign, so both directions have exactly the same room.
    fn a_body_that_senses(resting: f64, target: SensorTarget, gain: f32) -> Scene {
        let settings = config(|raw| {
            raw.world.width = 256.0;
            raw.world.height = 144.0;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 18;
            raw.light.gradient = 0.0;
            raw.light.patchiness = 0.0;
            raw.limits.max_organisms = 8;
            raw.limits.max_cells_per_organism = 8;
            raw.behaviour.resting_amplitude = resting;
        });
        let mut scene = Scene::new(&settings);
        scene.fill_with_light();

        let genome = Genome::new(
            vec![
                a_behaviour_gene(1, 0.0, FRAC_PI_2, gain, SensorTarget::Light),
                a_behaviour_gene(2, 0.0, 0.0, 0.0, target),
            ],
            &settings.limits,
        );
        scene.add(
            genome,
            0.0,
            &[
                (CellKind::Myocyte, Vec2::new(40.0, 72.0), Some(0)),
                (CellKind::Sensocyte, Vec2::new(46.0, 72.0), Some(1)),
            ],
        );
        scene.adhere(0, 1, 8.0, 10.0);

        scene
    }

    /// How hard the myocyte in [`a_body_that_senses`] is working, read back off its spring.
    ///
    /// Divided by the scene's own `behaviour.stroke` rather than by a constant, so that what
    /// comes back is SPEC section 9's *amplitude* whatever the settings do - which is what every
    /// claim below is about.
    fn amplitude(scene: &Scene) -> f32 {
        (scene.springs[0].rest_length / 8.0 - 1.0) / scene.config.behaviour.stroke
    }

    /// ⭐ **A6.** A sensocyte reports how lopsided its surroundings are in whatever it is tuned
    /// to, as a number between nought and one.
    ///
    /// SPEC section 9: *"Each `Sensocyte` outputs a normalised gradient magnitude toward its
    /// `sensor_target`, sampled from the resource grid or from nearby foreign biomass."* SPEC
    /// section 7 gives the three things it can be tuned to and says the choice is a gene's.
    ///
    /// The signal is not visible from outside this file, and that is on purpose - it is not
    /// state, it is something worked out and used within a tick. So it is measured the way the
    /// rest of the world measures it: through a myocyte adhered to the sensocyte, whose
    /// amplitude the signal drives and whose spring's rest length that amplitude sets. Which
    /// means this test also proves the wiring - that a myocyte hears the sensocytes it is
    /// joined to at all.
    ///
    /// # Why it is a **gradient** rather than how much is nearby, and why that is the whole
    /// difference
    ///
    /// A sensocyte with foreign bodies equally on both sides reports **nothing**, even though
    /// it is surrounded. That is the claim that makes this a direction-finder rather than a
    /// proximity alarm: a body cannot steer towards or away from something that is evenly
    /// spread, and a signal that rose whenever anything was near would drive a muscle hardest
    /// exactly where movement is least use.
    ///
    /// # The three targets are genuinely different things
    ///
    /// Light comes off the resource grid, which is what SPEC means by "the resource field"; the
    /// other two are sums over things in the water. So a sensocyte tuned to light is
    /// *unmoved* by a body drifting past it, and one tuned to foreign biomass is unmoved by
    /// the water going dark - and each is checked here, because a version that quietly sensed
    /// the same thing for all three targets would satisfy every other claim.
    #[test]
    fn a_sensocyte_reports_a_gradient_towards_its_target() {
        // Nothing to sense: nothing read. Every scene here is built at a resting amplitude of
        // nought, so what comes back is the signal itself - see `a_body_that_senses`.
        let mut empty = a_body_that_senses(0.0, SensorTarget::ForeignBiomass, 1.0);
        empty.run();
        assert!(
            amplitude(&empty).abs() < 1e-5,
            "a sensocyte in empty water read {} rather than nothing",
            amplitude(&empty)
        );

        // A foreign body to one side.
        let mut nearby = a_body_that_senses(0.0, SensorTarget::ForeignBiomass, 1.0);
        let stranger = nearby.no_genome();
        nearby.add(
            stranger,
            0.0,
            &[(CellKind::Sclerocyte, Vec2::new(54.0, 72.0), None)],
        );
        nearby.run();
        assert!(
            amplitude(&nearby) > 0.05,
            "a sensocyte eight units from a foreign body read {}, against the nothing it \
             reads in empty water - so either it cannot smell a neighbour or the signal is \
             too faint to select on",
            amplitude(&nearby)
        );

        // Closer is stronger.
        let mut closer = a_body_that_senses(0.0, SensorTarget::ForeignBiomass, 1.0);
        let stranger = closer.no_genome();
        closer.add(
            stranger,
            0.0,
            &[(CellKind::Sclerocyte, Vec2::new(50.0, 72.0), None)],
        );
        closer.run();
        assert!(
            amplitude(&closer) > amplitude(&nearby),
            "a body four units away read {} and one eight units away read {}, so the signal \
             carries no sense of how far off a thing is",
            amplitude(&closer),
            amplitude(&nearby)
        );

        // ⭐ And a gradient has a direction: the same body on both sides cancels.
        let mut surrounded = a_body_that_senses(0.0, SensorTarget::ForeignBiomass, 1.0);
        let stranger = surrounded.no_genome();
        surrounded.add(
            stranger.clone(),
            0.0,
            &[(CellKind::Sclerocyte, Vec2::new(54.0, 72.0), None)],
        );
        surrounded.add(
            stranger,
            0.0,
            &[(CellKind::Sclerocyte, Vec2::new(38.0, 72.0), None)],
        );
        surrounded.run();
        assert!(
            amplitude(&surrounded).abs() < 1e-5,
            "a sensocyte with a foreign body equally either side of it read {} rather than \
             nothing, so it is reporting how much is nearby rather than which way it is - \
             and a body cannot steer on that",
            amplitude(&surrounded)
        );

        // Detritus, and a sensocyte tuned to light that is unmoved by it.
        let mut smelling = a_body_that_senses(0.0, SensorTarget::Detritus, 1.0);
        smelling.drop_detritus(Vec2::new(54.0, 72.0), 1.0);
        smelling.run();
        assert!(
            amplitude(&smelling) > 0.05,
            "a sensocyte tuned to detritus, eight units from a grain of it, read {}",
            amplitude(&smelling)
        );

        let mut looking = a_body_that_senses(0.0, SensorTarget::Light, 1.0);
        looking.drop_detritus(Vec2::new(54.0, 72.0), 1.0);
        looking.run();
        assert!(
            amplitude(&looking).abs() < 1e-5,
            "a sensocyte tuned to light read {} beside a grain of detritus, so the target on \
             its gene is not deciding what it senses",
            amplitude(&looking)
        );

        // Light, off the resource grid. Even water first, then a tile eaten out beside it.
        let mut even = a_body_that_senses(0.0, SensorTarget::Light, 1.0);
        even.run();
        assert!(
            amplitude(&even).abs() < 1e-5,
            "a sensocyte in water that is the same everywhere read {}, and there is no \
             gradient there to read",
            amplitude(&even)
        );

        let mut patchy = a_body_that_senses(0.0, SensorTarget::Light, 1.0);
        patchy.eat_out(Vec2::new(36.0, 72.0), 7.0);
        patchy.run();
        assert!(
            amplitude(&patchy) > 0.05,
            "a sensocyte with the next tile along eaten nearly empty read {}, so it cannot \
             see the light gradient it is standing in",
            amplitude(&patchy)
        );

        let mut unmoved = a_body_that_senses(0.0, SensorTarget::ForeignBiomass, 1.0);
        unmoved.eat_out(Vec2::new(36.0, 72.0), 7.0);
        unmoved.run();
        assert!(
            amplitude(&unmoved).abs() < 1e-5,
            "a sensocyte tuned to foreign biomass read {} beside a tile that had been eaten \
             out, so all three targets are sensing the same thing",
            amplitude(&unmoved)
        );
    }

    /// ⭐ Both **attraction and avoidance** are one mutation apart, because the sign of
    /// `sensor_gain` is the whole of the difference between them.
    ///
    /// `genome.rs` calls the sign the load-bearing part of that field and SPEC section 9 gives
    /// the reason: *"Because `sensor_gain` is signed and evolvable, both attraction and
    /// avoidance are reachable by mutation, and phototaxis, detritus-seeking and
    /// predator-avoidance are all discoverable rather than coded."* If only one sign did
    /// anything - if the arithmetic clamped one direction away, say - then half of the
    /// behaviours SPEC hopes to see would be unreachable, and nothing would announce it.
    ///
    /// So what is asserted is that the response is **symmetric about the resting amplitude**:
    /// the same surroundings drive the muscle as far above 0.3 with a positive gain as below
    /// it with the negative of the same gain, and both are strictly inside the clamp so
    /// neither is being flattened against an end of the range.
    ///
    /// # What "attraction" and "avoidance" actually are here, which is worth being plain about
    ///
    /// Neither is a steering command; there is no such thing in this model. A signal changes
    /// how hard a muscle works, and a muscle changes the shape of a body. A body whose
    /// sensocytes sit at different places reads different signals at each of them, its muscles
    /// therefore work by different amounts, and it turns. Which way it turns depends on how the
    /// body is put together - so the *same* sign of gain is attraction in one body plan and
    /// avoidance in another, and flipping the sign reverses whichever it was.
    ///
    /// That is exactly the property that makes taxis discoverable rather than designed:
    /// nothing in this file knows which way anything is, and a lineage that finds phototaxis
    /// will have found it by growing a shape that turns the right way, not by being told to.
    #[test]
    fn both_attraction_and_avoidance_are_reachable_for_a_sensocyte() {
        // Half way up SPEC section 9's clamp, so the two directions have exactly the same
        // room. At the shipped resting amplitude of 0.8 the positive direction would flatten
        // against the top of the clamp and this test would be measuring that rather than the
        // sign - see `a_body_that_senses`.
        let resting = 0.5;

        let drawn_to = {
            let mut scene = a_body_that_senses(resting, SensorTarget::ForeignBiomass, 1.0);
            let stranger = scene.no_genome();
            scene.add(
                stranger,
                0.0,
                &[(CellKind::Sclerocyte, Vec2::new(54.0, 72.0), None)],
            );
            scene.run();
            amplitude(&scene)
        };
        let put_off = {
            let mut scene = a_body_that_senses(resting, SensorTarget::ForeignBiomass, -1.0);
            let stranger = scene.no_genome();
            scene.add(
                stranger,
                0.0,
                &[(CellKind::Sclerocyte, Vec2::new(54.0, 72.0), None)],
            );
            scene.run();
            amplitude(&scene)
        };

        let resting = 0.5f32;
        assert!(
            drawn_to > resting && put_off < resting,
            "a positive gain drove the muscle to {drawn_to} and a negative one to {put_off}, \
             either side of {resting} - and if they are not either side of it then one of the \
             two directions is unreachable however long a lineage mutates"
        );
        assert!(
            ((drawn_to - resting) - (resting - put_off)).abs() < 1e-5,
            "the two gains moved the muscle by {} and {} respectively, so one direction \
             responds more strongly than the other and selection would find it first",
            drawn_to - resting,
            resting - put_off
        );
        assert!(
            put_off > 0.0 && drawn_to < 1.0,
            "one of the two responses has been flattened against an end of SPEC section 9's \
             clamp, so this test is measuring the clamp rather than the sign"
        );
    }
}
