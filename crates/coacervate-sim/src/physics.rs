//! What moves the cells.
//!
//! See `SPEC.md` section 8 for the rules this implements.
//!
//! One tick of this module is four lines of arithmetic done to every cell in the world:
//! add up what is pushing on it, let that change how fast it is going, let that change
//! where it is, and put it back inside the world if it has left. Everything else in the
//! file is either working out *which* cells are pushing on which - which is nearly all of
//! the difficulty - or the reasoning about why each of those four lines is written the way
//! it is.
//!
//! # The water is thick, and that decides almost everything
//!
//! A cell is a few millionths of a metre across, and at that size water behaves the way
//! treacle behaves to us. Nothing coasts. A bacterium that stops swimming stops moving in
//! considerably less than the width of its own body, and SPEC section 8 is explicit that
//! this is both the physically right model and the numerically convenient one.
//!
//! It is expressed as `drag`: the proportion of a cell's velocity that survives a tick, 0.92
//! by default. Measured, that means a cell shoved hard enough to cross sixty world units in
//! a second travels **eleven and a half units** and stops - not quite two of its own
//! body-widths. Every journey an organism makes, it makes under power, and the moment a
//! myocyte stops contracting the body is stationary.
//!
//! That has a consequence worth stating plainly for Phase 4, because it is the sort of
//! thing that reads as a bug later: **momentum is not a strategy here.** Nothing can build
//! up speed and glide, no body can be flung anywhere useful, and a burst of movement is
//! worth exactly the distance it is paid for. If a lineage ever appears to be coasting,
//! something is wrong with the drag rather than clever about the lineage.
//!
//! # Two forces, and only two
//!
//! **Springs** hold a body together. Phase 3 creates one whenever a gene divides a cell
//! into a daughter that stays attached, and from then on it pulls the two towards a rest
//! length that a myocyte can oscillate - which is the only source of locomotion in the
//! entire simulation.
//!
//! **Collisions** keep bodies out of each other. Two cells that overlap push apart in
//! proportion to how far they overlap, and cells that are merely near each other do
//! nothing. There is no attraction anywhere in this module: things stick together because
//! there is a spring between them, and for no other reason.
//!
//! A pair joined by a spring is exempt from the collision, which SPEC section 8 says in one
//! word - "non-adhered" - and which is easy to skim past. It matters because a myocyte's
//! whole function is to pull a spring *shorter* than the cells it joins. Let the collision
//! act on an adhered pair too and the two forces balance at the width of the cells,
//! contraction achieves nothing, and locomotion silently ceases to exist while every test
//! about springs still passes.
//!
//! # The neighbour search is the whole performance story
//!
//! Finding which cells overlap by comparing each with every other costs the square of the
//! population: four thousand cells is eight million comparisons a tick, forty thousand is
//! eight hundred million, and somewhere in between the simulation stops being something you
//! can leave running overnight. SPEC section 8 calls the fix "the single most important
//! performance decision on the CPU side", and it is: bucket the world into squares a little
//! wider than the largest cell, and a cell need only look at the nine buckets around it.
//!
//! [`SpatialHash`] is that, built out of dense arrays and a counting sort rather than a map
//! from bucket to cells - deliberately, because a map's iteration order is not something
//! the standard library promises, and neighbours arriving in a different order means forces
//! summed in a different order, which means different roundings, which means the same seed
//! producing a different run. That is a failure with no symptom until a recording no longer
//! replays.
//!
//! # The world joins up sideways and stops top and bottom
//!
//! Both halves come from SPEC section 8 and both have reasons. Sideways it wraps, so there
//! is no edge to hug: a wall is a place with neighbours on one side only, which is worth
//! evolving towards for reasons that have nothing to do with the ecology being studied.
//! Top and bottom it stops, because depth is what the light gradient is made of and a world
//! with no shallowest and deepest place has no gradient in it.
//!
//! The wrap is the single easiest thing in this file to half-implement. Moving a cell from
//! one edge to the other is obvious and gets written; making *distances* wrap is neither
//! obvious nor visible when it is missing. Without it the join becomes an invisible wall
//! that cells never see across, and what that looks like from the outside is a body tearing
//! itself in half when it happens to drift over a particular column, months later, in a run
//! nobody was watching.
//!
//! # Where the arithmetic could go wrong, and why it does not
//!
//! Positions are moved by the velocity that the tick's force has *already* been added to,
//! rather than by the velocity from the start of the tick. That is what "semi-implicit"
//! means and it is most of why this is stable: it damps rather than amplifies, so a stiff
//! collision corrects an overlap instead of overshooting it a little further every tick.
//!
//! The measurement is in `physics_is_stable_under_a_pile_up`, and the headline is that a
//! crowd of sixty-four cells stacked nearly on top of one another settles at any collision
//! stiffness up to somewhere between **3,240 and 3,280**. SPEC section 3 ships **40.0**, so
//! the shipped world runs about eighty times below the edge. That is comfortable rather
//! than marginal: the setting is exposed as a live slider, and it would have to be dragged
//! through two orders of magnitude before anything misbehaved.
//!
//! # What is deliberately not here
//!
//! **Energy.** This module never touches the ledger. Movement costs energy from Phase 4,
//! when there is an organism to charge it to; wiring it up now would mean inventing an
//! owner for the cost, and `ledger.rs` is built on the principle that every transfer has
//! two named ends.
//!
//! **Organisms.** There is no body type. Cells are a flat array and springs are a flat list
//! of index pairs into it, which is what Phase 3 will fill and what a Phase 9 shader wants
//! to read. Which cells belong to which organism is not a question this module asks.
//!
//! **Behaviour.** A myocyte oscillating its springs, a sensocyte reading a gradient - those
//! are Phases 3 and 4. What is here is only the mechanics they will act through.
//!
//! # An open question this module answers rather than asks
//!
//! `spring_damping` is listed in SPEC section 3 with no description and mentioned in
//! section 8 only in passing, so it has never had stated semantics. Here it is a resistance
//! to two adhered cells moving *along* the spring between them, in proportion to how fast
//! they are separating - the ordinary meaning of the word, and the one that resists
//! stretching without resisting swinging, so a body can still bend and turn. At the shipped
//! drag of 0.92 the surrounding water is already thick enough that a spring cannot really
//! oscillate, so the setting does very little; it becomes the only thing that can bring a
//! body to rest as drag is turned towards 1. See `a_spring_pulls_towards_its_rest_length`.

use crate::cell::{Cell, CellKind, Vec2};
use crate::config::Config;

/// How far apart two cells can be and still be touching.
///
/// Two of the widest radius in the world, because that is the largest a pair's radii can
/// sum to. It is the reach of every force in this module except a spring, and springs are
/// a list rather than a search, so this one number decides how large the neighbour search
/// has to look.
const REACH: f32 = 2.0 * CellKind::LARGEST_RADIUS;

/// How many cells the arenas here are built for.
///
/// Every organism holds at most `max_cells_per_organism` cells and there are at most
/// `max_organisms` of them, so this is the most cells that can exist at once. CLAUDE.md: a
/// simulation that cannot allocate cannot leak.
///
/// `world.rs` builds its cell and spring arenas from this same function rather than working
/// the number out again, so the arena a caller fills and the arrays this module sizes to
/// match it cannot drift apart.
pub(crate) fn cell_capacity(config: &Config) -> usize {
    let organisms = usize::try_from(config.limits.max_organisms.get())
        .expect("a population cap fits in a machine word");
    let cells = usize::try_from(config.limits.max_cells_per_organism.get())
        .expect("a body-size cap fits in a machine word");
    organisms * cells
}

/// How many buckets fit across a span of world, given how far a force reaches.
///
/// The count is rounded *down*, so the buckets that result are each at least a full reach
/// across. That is the property the whole search depends on: two cells within a reach of
/// one another are then either in the same bucket or in neighbouring ones, so looking at
/// the eight buckets around a cell's own is looking everywhere it could possibly be
/// touching something.
///
/// A world too narrow to hold a single bucket gets one, which is a world where every cell
/// is every other cell's neighbour. That is the honest answer for a world smaller than a
/// cell, and it is still correct - only slower, in a world with no room to be slow in.
fn buckets_across(span: f32, reach: f32) -> u32 {
    let exact = f64::from(span) / f64::from(reach);

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value has been rounded down to a whole number and then clamped into \
                  the range of the type it is becoming, so the conversion neither \
                  truncates nor loses a sign - the clamp is what makes it total"
    )]
    let buckets = exact.floor().clamp(1.0, f64::from(u32::MAX)) as u32;

    buckets
}

/// Which of a row (or column) of buckets a coordinate falls in.
///
/// `per_unit` is how many buckets there are per world unit, worked out once when the hash
/// is built, and `last` is the index of the final bucket. Multiplying by a precomputed
/// scale rather than dividing by a bucket width means the buckets divide the world
/// exactly, which is the same trick `grid.rs` uses to lay its noise lattice down, so the
/// join where the world wraps falls precisely on a bucket boundary rather than a hair
/// either side of one.
///
/// The clamp is not defensive tidying. A cell resting exactly on the floor has `y` equal to
/// the world's height, which multiplied out lands one past the last bucket; without the
/// clamp that is an index off the end of the array. It also makes the conversion below
/// total, which is what allows it to be a conversion at all.
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

/// A uniform grid of buckets laid over the world, holding which cells are in each.
///
/// SPEC section 8 calls this "the single most important performance decision on the CPU
/// side", and the reason is arithmetic rather than taste. Asking which cells are touching
/// by comparing every cell with every other costs the square of the population: at four
/// thousand cells that is eight million comparisons a tick, at forty thousand it is eight
/// hundred million, and the simulation stops being able to run overnight somewhere in
/// between. Bucketing the world first turns the same question into "who else is in my
/// bucket, and in the eight around it", which costs about the population itself.
///
/// # Why it is built out of dense arrays and a counting sort
///
/// The obvious way to write a spatial hash is a map from bucket to a list of cells. That is
/// banned in this crate - see `clippy.toml` - and the ban is the point rather than an
/// inconvenience. SPEC section 2 requires a run to reproduce exactly from its seed, and a
/// map's iteration order is not something the standard library promises; it varies between
/// program runs by design. Neighbours coming back in a different order means forces added
/// up in a different order, which means different roundings, which means the same seed
/// producing a different world. Nothing announces it. The run simply stops matching its own
/// recording.
///
/// So the cells are sorted into their buckets by counting: count how many land in each,
/// add those counts up to find where each bucket's run of cells begins, then place every
/// cell at its bucket's next free slot. Two passes over the cells and one over the buckets,
/// no allocation, and cells come back in ascending order within each bucket every single
/// time, because that is the order they were placed in.
///
/// # The world wraps sideways, and that is where this goes wrong
///
/// Buckets divide the world's width a whole number of times, so the join where the world
/// wraps falls exactly on a bucket edge and the first column of buckets is genuinely next
/// to the last. A cell one unit from the left-hand edge is a unit away from a cell one unit
/// from the right-hand edge, and both are found by the same search. Treat the edges as
/// walls instead and nothing crashes: cells near the join simply stop noticing one another,
/// and a body straddling it tears itself apart while the middle of the world behaves
/// perfectly. `the_spatial_hash_finds_the_same_neighbours_as_checking_everything` is what
/// stands between that and a long evening.
///
/// Downwards there is no wrap, because the world has a surface and a floor. The search
/// simply stops at the top and bottom rows.
struct SpatialHash {
    /// How many buckets across the world is, and how many down.
    cols: usize,
    rows: usize,

    /// Buckets per world unit, across and down. Precomputed so that placing a cell is a
    /// multiplication rather than a division.
    across_per_unit: f64,
    down_per_unit: f64,

    /// The index of the last bucket in a row and in a column, as a number the placement
    /// arithmetic can clamp against.
    last_col: f64,
    last_row: f64,

    /// Which bucket each cell landed in, from the most recent rebuild.
    bucket_of: Vec<usize>,

    /// Where each bucket's run of cells begins in `order`. One entry longer than there are
    /// buckets, so a bucket's run is always `starts[bucket]..starts[bucket + 1]` with no
    /// special case for the last one.
    starts: Vec<usize>,

    /// Working room for the counting sort: each bucket's next free slot as it fills.
    cursor: Vec<usize>,

    /// Every cell, in bucket order. This is the sorted result the search reads.
    order: Vec<usize>,
}

impl SpatialHash {
    /// Lay buckets over the world a configuration describes.
    ///
    /// Everything is allocated here and never resized. The default world is 2048 by 1152
    /// units with a reach of 6.8, so it comes out as 301 by 169 buckets - about fifty
    /// thousand of them, and around a megabyte all told, against the four thousand
    /// organisms the same configuration allows.
    fn new(config: &Config) -> Self {
        let cols_across = buckets_across(config.world.width, REACH);
        let rows_down = buckets_across(config.world.height, REACH);
        let cols = usize::try_from(cols_across).expect("a bucket count fits in a machine word");
        let rows = usize::try_from(rows_down).expect("a bucket count fits in a machine word");
        let capacity = cell_capacity(config);

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
        }
    }

    /// Sort every cell into its bucket, throwing away whatever was here before.
    ///
    /// Called once a tick, before any force is worked out, because a bucketing that is one
    /// tick out of date is a bucketing that misses exactly the pairs that have just moved
    /// into contact.
    fn rebuild(&mut self, cells: &[Cell]) {
        assert!(
            cells.len() <= self.bucket_of.len(),
            "the physics was built with room for {} cells and was handed {}",
            self.bucket_of.len(),
            cells.len()
        );

        let buckets = self.cols * self.rows;
        let (cols, across, down) = (self.cols, self.across_per_unit, self.down_per_unit);
        let (last_col, last_row) = (self.last_col, self.last_row);

        // Nothing of the previous tick survives: every count starts at nought.
        self.starts[..=buckets].fill(0);

        for (index, cell) in cells.iter().enumerate() {
            let col = bucket_along(cell.pos.x, across, last_col);
            let row = bucket_along(cell.pos.y, down, last_row);
            let bucket = row * cols + col;

            self.bucket_of[index] = bucket;
            self.starts[bucket + 1] += 1;
        }

        // Running totals: each bucket's run begins where all the earlier buckets' runs
        // finished.
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
        for (index, &bucket) in bucket_of[..cells.len()].iter().enumerate() {
            order[cursor[bucket]] = index;
            cursor[bucket] += 1;
        }
    }

    /// The columns of buckets to search either side of a given one.
    ///
    /// Three of them, wrapping round the world's edge, except in a world so narrow that
    /// three columns would mean visiting the same column twice. That is not a hypothetical
    /// tidiness: a bucket visited twice hands back every cell in it twice, and a pair of
    /// cells pushed apart twice is a world where the collision force silently doubles in
    /// small worlds and nowhere else.
    fn columns_around(&self, col: usize) -> ([usize; 3], usize) {
        match self.cols {
            1 => ([0, 0, 0], 1),
            2 => ([0, 1, 0], 2),
            wide => ([(col + wide - 1) % wide, col, (col + 1) % wide], 3),
        }
    }

    /// Hand back every cell that is close enough to this one to be worth measuring.
    ///
    /// "Close enough" means in one of the nine buckets around it, which is a generous
    /// answer: most of what comes back is further away than a cell can reach. That is the
    /// bargain the whole structure makes. Rejecting a candidate is one subtraction and a
    /// comparison, and it is being paid to avoid asking the question of every cell in the
    /// world.
    ///
    /// The order is fixed and repeatable: down the rows from the top, across the columns
    /// from the left, and in ascending cell index within each bucket. A cell never sees
    /// itself.
    fn for_each_neighbour(&self, cell: usize, mut visit: impl FnMut(usize)) {
        let bucket = self.bucket_of[cell];
        let (columns, wide) = self.columns_around(bucket % self.cols);

        let here = bucket / self.cols;
        let first = here.saturating_sub(1);
        let last = (here + 1).min(self.rows - 1);

        for row in first..=last {
            for &col in &columns[..wide] {
                let near = row * self.cols + col;

                for &other in &self.order[self.starts[near]..self.starts[near + 1]] {
                    if other != cell {
                        visit(other);
                    }
                }
            }
        }
    }
}

/// One tick of simulated time, in seconds.
///
/// SPEC section 2 fixes it at a sixtieth of a second and decouples it entirely from how
/// fast the machine can compute ticks. That is what makes a run reproducible: a tick is the
/// same slice of simulated time on a fast machine and a slow one, so the answer does not
/// depend on the hardware.
pub const DT: f32 = 1.0 / 60.0;

/// An adhesion between two cells: what holds a body together.
///
/// Phase 3 makes these, one per daughter cell that a gene chose to keep attached, with the
/// rest length and stiffness that gene carries. This module only resolves them. They are a
/// flat list of index pairs rather than anything owned by the cells, because that is the
/// shape both the counting sort below and a Phase 9 shader want to read.
///
/// Two springs are equal when all four of their numbers are, which `development.rs` needs
/// in order to compare one grown body against another - and comparing whole bodies is how
/// `a_body_is_a_pure_function_of_its_genome` states the promise the museum will rest on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// The two cells it joins, by their position in the cell array.
    pub a: usize,
    pub b: usize,

    /// How far apart it would rather they were. A myocyte oscillates this, from Phase 3,
    /// and that oscillation is the only source of locomotion in the world.
    pub rest_length: f32,

    /// How hard it pulls per unit it is stretched by.
    pub stiffness: f32,
}

/// The shortest way from one point to another in a world that joins up sideways.
///
/// The one piece of arithmetic in this module that it would be easy to leave out and never
/// notice. Two cells a unit apart either side of the join are a unit apart; measured with a
/// plain subtraction they are a whole world apart, so they ignore each other completely.
/// Nothing crashes, and the world grows an invisible wall down one column that organisms
/// will eventually be found clustering against.
///
/// Rounding the offset to whole worlds and taking that many away is a way of writing it
/// that has no cases in it, which matters: written as a pair of `if`s, one of them is only
/// reached by cells near one particular edge and is therefore the half that gets tested by
/// accident and the half that does not.
///
/// Shared with `behaviour.rs`, which measures how far a cell is to the side of the one it is
/// shading and has to answer that question by the same rule the physics measures a collision
/// by. Two versions of "the world wraps" would be two rules about one thing, and the one
/// written down twice is the one that ends up disagreeing with itself at the join.
pub(crate) fn wrapped_offset(from: Vec2, to: Vec2, width: f32) -> Vec2 {
    let across = to.x - from.x;

    Vec2::new(across - width * (across / width).round(), to.y - from.y)
}

/// Bring a coordinate back inside a world that joins up sideways.
///
/// The second line looks redundant and is not. A coordinate a hair below nought comes back
/// from the remainder as a value so close to the world's width that, at 32 bits, it rounds
/// to exactly the width - a position that is one past the right-hand edge rather than just
/// inside it, which is the one value everything downstream assumes cannot happen.
///
/// Shared with `world.rs`, which places a newly-seeded body and has to put its cells inside
/// the world by the same rule the physics will keep them there by. Two versions of "the world
/// wraps" would be two rules about one thing, and the one that is written down twice is the
/// one that ends up disagreeing with itself at the join.
pub(crate) fn wrapped(coordinate: f32, width: f32) -> f32 {
    let inside = coordinate.rem_euclid(width);

    if inside < width { inside } else { 0.0 }
}

/// Which way one cell lies from another, as a direction of length one.
///
/// Two cells at exactly the same point have no direction between them, and it does happen -
/// Phase 3 places a daughter cell beside its parent, and "beside" can round to "on top of".
/// Something has to break the tie, and what matters is only that it is broken the same way
/// every run, because SPEC section 2 requires a run to reproduce exactly. Sideways is as
/// good as any other answer and is the one that is written down.
fn direction(offset: Vec2, distance: f32) -> Vec2 {
    if distance > 0.0 {
        offset.scaled(1.0 / distance)
    } else {
        Vec2::new(1.0, 0.0)
    }
}

/// Whether a spring joins these two cells.
///
/// Reads the index built by [`Physics::index_bonds`]: a cell's adhesions are a short run of
/// entries, and a body's cells have a handful each, so this is a walk over three or four
/// numbers rather than a search.
fn adhered(starts: &[usize], partners: &[usize], cell: usize, other: usize) -> bool {
    partners[starts[cell]..starts[cell + 1]].contains(&other)
}

/// Everything needed to move a crowd of cells for one tick, allocated once.
///
/// See the module documentation for what a tick does. What this type is, structurally, is
/// six arrays and five numbers copied out of the configuration: no state that carries from
/// one tick to the next, so ticking the same cells twice from the same starting point gives
/// the same answer both times.
pub struct Physics {
    /// The size of the world. Across, it wraps; down, it stops.
    width: f32,
    height: f32,

    /// SPEC section 3's `[physics]` table.
    drag: f32,
    collision_stiffness: f32,
    spring_damping: f32,

    /// Which cells are near which.
    hash: SpatialHash,

    /// What is pushing on each cell this tick. Cleared at the start of every tick, so
    /// nothing from the last one leaks into this one.
    forces: Vec<Vec2>,

    /// Where each cell's run of adhered partners begins in `bond_partners`, one entry
    /// longer than there are cells so the last run needs no special case.
    bond_starts: Vec<usize>,

    /// Working room for the counting sort that builds that index.
    bond_cursor: Vec<usize>,

    /// Every adhesion, twice - once from each end - grouped by cell.
    ///
    /// Two entries per spring, and there can be no more springs than cells: SPEC section
    /// 7 creates a spring only when a gene divides a cell into a daughter that stays
    /// attached, and a daughter is created once. So room for two per cell is room for
    /// every spring the world can hold, however Phase 3 arranges them.
    bond_partners: Vec<usize>,
}

impl Physics {
    /// Build the physics a configuration describes.
    ///
    /// Every array is allocated here at the size the configuration implies and never
    /// resized, which is CLAUDE.md's rule for every arena in the project: a simulation that
    /// cannot allocate cannot leak. At SPEC section 3's defaults - four thousand organisms
    /// of up to sixty-four cells - that is room for 256,000 cells and about twelve
    /// megabytes, most of it the two adhesion arrays.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let capacity = cell_capacity(config);

        Self {
            width: config.world.width,
            height: config.world.height,
            drag: config.physics.drag,
            collision_stiffness: config.physics.collision_stiffness,
            spring_damping: config.physics.spring_damping,
            hash: SpatialHash::new(config),
            forces: vec![Vec2::ZERO; capacity],
            bond_starts: vec![0; capacity + 1],
            bond_cursor: vec![0; capacity],
            bond_partners: vec![0; capacity * 2],
        }
    }

    /// Take the `[physics]` table again, on a running world.
    ///
    /// SPEC section 3 does not lock `[physics]`, so drag, stiffness and damping can all be
    /// turned mid-run. Nothing here is a size, so nothing here is allocated: `width` and
    /// `height` come from `[world]`, which is locked, and every array was built from
    /// `[limits]`, which is locked too. See [`crate::world::World::retune`].
    pub fn retune(&mut self, config: &Config) {
        self.drag = config.physics.drag;
        self.collision_stiffness = config.physics.collision_stiffness;
        self.spring_damping = config.physics.spring_damping;
    }

    /// Move every cell on by one tick.
    ///
    /// SPEC section 8's four lines, in its order: gather the forces, then let them change
    /// the velocities, then let the velocities change the positions. Nothing here reads or
    /// writes the energy ledger, and that is deliberate rather than forgotten - movement
    /// costs energy only once there is an organism to charge it to, which is Phase 4.
    pub fn step(&mut self, cells: &mut [Cell], springs: &[Spring]) {
        self.hash.rebuild(cells);
        self.index_bonds(cells.len(), springs);

        self.forces[..cells.len()].fill(Vec2::ZERO);
        self.pull_springs(cells, springs);
        self.push_apart(cells);
        self.integrate(cells);
    }

    /// Turn the flat list of springs inside out, so that a cell's adhesions can be found
    /// from the cell.
    ///
    /// The collision pass needs to ask, of a pair of overlapping cells, whether a spring
    /// already joins them - and it asks that of every overlapping pair in the world, so the
    /// answer has to be cheap. Walking the spring list each time would put the population
    /// multiplied by the spring count into the middle of the tick, which is precisely the
    /// cost the spatial hash exists to avoid paying elsewhere.
    ///
    /// So the springs are counted into per-cell runs, exactly as the spatial hash counts
    /// cells into buckets: how many adhesions each cell has, where each cell's run
    /// therefore starts, and then each spring written into both of its cells' runs. Dense
    /// arrays and two passes, with no map anywhere and therefore no iteration order to
    /// depend on.
    ///
    /// Rebuilt every tick rather than kept, because Phase 3 will add and remove springs as
    /// bodies grow and die, and an index that is one tick stale is an index that says two
    /// cells are joined when the spring between them has gone.
    fn index_bonds(&mut self, count: usize, springs: &[Spring]) {
        assert!(
            springs.len() * 2 <= self.bond_partners.len(),
            "the physics was built with room for {} springs and was handed {}",
            self.bond_partners.len() / 2,
            springs.len()
        );

        self.bond_starts[..=count].fill(0);

        for spring in springs {
            assert!(
                spring.a < count && spring.b < count,
                "a spring joins cells {} and {}, and there are only {count} cells",
                spring.a,
                spring.b
            );
            self.bond_starts[spring.a + 1] += 1;
            self.bond_starts[spring.b + 1] += 1;
        }

        for cell in 1..=count {
            self.bond_starts[cell] += self.bond_starts[cell - 1];
        }

        self.bond_cursor[..count].copy_from_slice(&self.bond_starts[..count]);

        for spring in springs {
            self.bond_partners[self.bond_cursor[spring.a]] = spring.b;
            self.bond_cursor[spring.a] += 1;
            self.bond_partners[self.bond_cursor[spring.b]] = spring.a;
            self.bond_cursor[spring.b] += 1;
        }
    }

    /// Hooke's law along every spring, damped.
    ///
    /// A spring stretched past its rest length pulls its two cells together and one
    /// compressed below it pushes them apart, in proportion to how far it is from where it
    /// would rather be. One number, applied to one end and taken from the other, so what
    /// pulls on the first cell cannot disagree with what pulls on the second - the same
    /// shape as `ledger.rs` moving energy between accounts and `grid.rs` moving it between
    /// tiles.
    ///
    /// # What `spring_damping` means, since SPEC does not say
    ///
    /// SPEC section 3 lists it with no description and section 8 only mentions it in
    /// passing, so this is the answer rather than a reading: it is a resistance to the two
    /// cells moving *along* the spring, added to the pull in proportion to how fast they
    /// are separating. That is the ordinary meaning of damping and it has one consequence
    /// worth stating, which is that it resists stretching and not swinging. Two cells
    /// circling one another at a fixed distance feel none of it, so a body can still bend
    /// and turn; what it cannot do is ring like a bell.
    ///
    /// With SPEC's default drag of 0.92 the surrounding water is already so viscous that a
    /// spring barely oscillates at all, so this number does very little at the shipped
    /// configuration. It becomes the only thing that settles a body as drag is turned up
    /// towards one, which is what `a_spring_pulls_towards_its_rest_length` demonstrates.
    fn pull_springs(&mut self, cells: &[Cell], springs: &[Spring]) {
        let (width, damping) = (self.width, self.spring_damping);

        for spring in springs {
            let here = &cells[spring.a];
            let there = &cells[spring.b];

            let offset = wrapped_offset(here.pos, there.pos, width);
            let distance = offset.length();
            let along = direction(offset, distance);
            let separating = (there.vel - here.vel).dot(along);

            let pull = along
                .scaled(spring.stiffness * (distance - spring.rest_length) + damping * separating);

            self.forces[spring.a] += pull;
            self.forces[spring.b] -= pull;
        }
    }

    /// Push overlapping cells apart, unless a spring already joins them.
    ///
    /// The repulsion is Hooke's law again, with the world's `collision_stiffness` and a
    /// rest length of "just touching": two cells overlapping by one unit push each other
    /// apart with a force of `collision_stiffness`, and two cells that are merely close
    /// push not at all. There is no attraction and no stickiness - a body is held together
    /// by its springs, and this is only what stops two bodies occupying the same water.
    ///
    /// # Adhered pairs are exempt, and it is not a detail
    ///
    /// SPEC section 8 says "overlapping *non-adhered* cells", and the reason is that a
    /// myocyte's whole function is to pull its springs shorter than the cells they join.
    /// Let the collision act on an adhered pair as well and the two forces fight: the body
    /// cannot compress below the width of its own cells, contraction does nothing, and the
    /// only means of locomotion in the world quietly stops working while every test about
    /// springs still passes.
    ///
    /// # Each pair is handled once
    ///
    /// The neighbour search reports a pair from both ends, so acting only on the half with
    /// the larger index means each pair is measured once and the force applied to both
    /// cells together. Handling it from both ends instead would be a subtler bug than it
    /// sounds: it does not double the repulsion, it applies it twice from opposite
    /// directions, which is the same thing only when the arithmetic is exact.
    fn push_apart(&mut self, cells: &[Cell]) {
        let Self {
            width,
            collision_stiffness,
            hash,
            forces,
            bond_starts,
            bond_partners,
            ..
        } = self;
        let (width, stiffness) = (*width, *collision_stiffness);

        for (index, cell) in cells.iter().enumerate() {
            hash.for_each_neighbour(index, |other| {
                if other < index || adhered(bond_starts, bond_partners, index, other) {
                    return;
                }

                let offset = wrapped_offset(cell.pos, cells[other].pos, width);
                let distance = offset.length();
                let overlap = cell.radius + cells[other].radius - distance;

                if overlap > 0.0 {
                    let push = direction(offset, distance).scaled(stiffness * overlap);
                    forces[index] -= push;
                    forces[other] += push;
                }
            });
        }
    }

    /// Let the forces move the cells: SPEC section 8's last two lines.
    ///
    /// Semi-implicit Euler, which means the position is moved by the velocity the force has
    /// *already* been added to rather than the one from the start of the tick. That one
    /// ordering is most of why this is stable at a stiffness two hundred times what the
    /// world ships with - see `physics_is_stable_under_a_pile_up`.
    ///
    /// Drag is a proportion of velocity kept per tick rather than a force, which is SPEC
    /// section 8's "velocity ≈ force": at 0.92 a cell keeps eight per cent less of its
    /// motion every tick, so it stops within a fraction of a second of being left alone.
    /// That is what a cell-sized object in water actually does, and it is also far kinder
    /// to the arithmetic than momentum would be.
    ///
    /// # The edges of the world
    ///
    /// Sideways there is no edge: a cell that walks off one side arrives at the other, and
    /// SPEC section 8 wants it that way so that no part of the world is a corner worth
    /// hiding in.
    ///
    /// Top and bottom there is. A cell pressed into the surface or the floor stops there,
    /// and **its downward motion is taken away rather than reversed**. A bounce would be
    /// elastic, which is wrong for a world this viscous, and it would also hand the cell
    /// back energy the physics never accounted for - a pile of cells resting on the floor
    /// would simmer for ever instead of settling.
    fn integrate(&mut self, cells: &mut [Cell]) {
        let (width, height, drag) = (self.width, self.height, self.drag);

        for (cell, force) in cells.iter_mut().zip(&self.forces) {
            cell.vel = (cell.vel + force.scaled(DT)).scaled(drag);
            cell.pos += cell.vel.scaled(DT);

            cell.pos.x = wrapped(cell.pos.x, width);

            if cell.pos.y < 0.0 {
                cell.pos.y = 0.0;
                cell.vel.y = 0.0;
            } else if cell.pos.y > height {
                cell.pos.y = height;
                cell.vel.y = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, RawConfig, spec_defaults};
    use proptest::prelude::*;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// A handful of world shapes, chosen for the awkwardness of their bucket counts rather
    /// than for realism.
    ///
    /// The default world is the last of them. The others are there because the number of
    /// buckets across a world changes what the neighbour search has to do: three or more
    /// columns is the ordinary case, two means the same column sits on both sides of
    /// every cell, and one means there are no sides at all. Each of those is a separate
    /// piece of code and the ordinary case is the only one anybody tests by accident.
    const SHAPES: [(f32, f32); 5] = [
        (6.0, 6.0),
        (13.0, 9.0),
        (15.0, 40.0),
        (100.0, 60.0),
        (2048.0, 1152.0),
    ];

    /// The shortest way from one cell to another in a world that wraps sideways, worked out
    /// the slow and obvious way.
    ///
    /// Written from SPEC section 8's description rather than borrowed from the module
    /// above, for the reason `grid.rs` writes the light profile out twice: a test that
    /// calls the code it is testing is a test that agrees with whatever that code happens
    /// to do. This tries all three places the far cell could be - where it is, one world to
    /// the left, and one world to the right - and keeps whichever is nearest. That is what
    /// "the world wraps" means, spelled out.
    fn spec_separation(from: Vec2, to: Vec2, width: f32) -> Vec2 {
        let mut across = to.x - from.x;

        for candidate in [across - width, across + width] {
            if candidate.abs() < across.abs() {
                across = candidate;
            }
        }

        Vec2::new(across, to.y - from.y)
    }

    /// Whether two cells are overlapping, which is the only question the neighbour search
    /// exists to answer.
    fn touching(here: &Cell, there: &Cell, width: f32) -> bool {
        spec_separation(here.pos, there.pos, width).length() < here.radius + there.radius
    }

    /// The cells every crowd contains, wherever the random ones happen to land.
    ///
    /// Five positions across and five down, and the interesting ones are the extremes.
    /// Across: hard against the left-hand edge, a little in from it, the middle, a little
    /// in from the right-hand edge, and as close to the right-hand edge as a 32-bit number
    /// can get without reaching it. The pairs that matter are the ones at opposite ends,
    /// which are a whisker apart *through the seam* and a whole world apart if you forget
    /// the seam is there. Down: the surface, the floor, and three depths between - a cell
    /// resting exactly on the floor being the one that lands a bucket past the end of the
    /// array if the placement arithmetic is not clamped.
    ///
    /// Sclerocytes, because they are the widest cell in the world and therefore the ones
    /// whose reach the buckets are sized for.
    fn edge_cases(width: f32, height: f32) -> Vec<Cell> {
        let outermost = width * (1.0 - f32::EPSILON);
        let across = [
            0.0,
            (width * 0.05).min(outermost),
            (width * 0.5).min(outermost),
            (width * 0.95).min(outermost),
            outermost,
        ];
        let down = [0.0, height * 0.05, height * 0.5, height * 0.95, height];

        let mut cells = Vec::new();
        for depth in down {
            for offset in across {
                cells.push(Cell::new(
                    CellKind::Sclerocyte,
                    Vec2::new(offset, depth.min(height)),
                ));
            }
        }
        cells
    }

    /// A world shape, and a crowd of cells scattered over it.
    ///
    /// The crowd is always the edge cases plus up to a hundred and eighty cells of random
    /// kind dropped anywhere. Random alone would find the seam eventually and only
    /// eventually; the fixed ones put a cell on it every single run.
    fn shaped_crowd() -> impl Strategy<Value = ((f32, f32), Vec<Cell>)> {
        (0usize..SHAPES.len()).prop_flat_map(|which| {
            let (width, height) = SHAPES[which];

            prop::collection::vec(
                (0.0f32..width, 0.0f32..height, 0usize..CellKind::ALL.len()),
                0..180,
            )
            .prop_map(move |scattered| {
                let mut cells = edge_cases(width, height);
                cells.extend(
                    scattered
                        .into_iter()
                        .map(|(x, y, kind)| Cell::new(CellKind::ALL[kind], Vec2::new(x, y))),
                );
                ((width, height), cells)
            })
        })
    }

    /// Every pair of cells the slow way, so the fast way has something to be wrong against.
    fn every_touching_pair(cells: &[Cell], width: f32) -> Vec<(usize, usize)> {
        let mut pairs = Vec::new();

        for near in 0..cells.len() {
            for far in near + 1..cells.len() {
                if touching(&cells[near], &cells[far], width) {
                    pairs.push((near, far));
                }
            }
        }

        pairs
    }

    /// Two cells of one kind, this far apart on a horizontal line, in open water.
    fn pair(kind: CellKind, apart: f32) -> Vec<Cell> {
        vec![
            Cell::new(kind, Vec2::new(500.0, 500.0)),
            Cell::new(kind, Vec2::new(500.0 + apart, 500.0)),
        ]
    }

    /// How far apart the first two cells are, measured the slow way round the world.
    fn gap(cells: &[Cell], width: f32) -> f32 {
        spec_separation(cells[0].pos, cells[1].pos, width).length()
    }

    /// How much motion there is in a crowd, all told.
    ///
    /// Every cell has a mass of one - SPEC section 8's force equation has no mass in it at
    /// all - so this is the sum of the squares of the speeds, halved. What it is for is
    /// answering "is this settling or is it simmering", and only its trend matters.
    fn kinetic_energy(cells: &[Cell]) -> f32 {
        cells.iter().map(|cell| cell.vel.dot(cell.vel)).sum::<f32>() * 0.5
    }

    /// Sixty-four cells stacked nearly on top of one another in a small world.
    ///
    /// Spaced 1.4 apart when they are 6.0 wide, so every cell overlaps its neighbours by
    /// three quarters of itself and the middle of the pile overlaps eight cells at once.
    /// This is not a contrived worst case: Phase 3 grows a body by placing daughter cells
    /// beside their parents, and a mutation that places several in the same spot is one
    /// point mutation away at all times.
    fn pile_up() -> Vec<Cell> {
        let mut cells = Vec::new();

        for row in 0u8..8 {
            for col in 0u8..8 {
                cells.push(Cell::new(
                    CellKind::Photocyte,
                    Vec2::new(100.0 + f32::from(col) * 1.4, 60.0 + f32::from(row) * 1.4),
                ));
            }
        }

        cells
    }

    /// Drop that pile into a world of a given collision stiffness and watch it for a while.
    ///
    /// Hands back the cells as they ended up, the most motion there ever was in the crowd,
    /// and how much there was at the end. A pile that settles ends with almost none; a pile
    /// that has gone unstable is still shaking itself to pieces on the last tick.
    fn watch_a_pile(stiffness: f64, ticks: usize) -> (Vec<Cell>, f32, f32) {
        let world = config(|raw| {
            raw.world.width = 200.0;
            raw.world.height = 120.0;
            raw.physics.collision_stiffness = stiffness;
            raw.limits.max_organisms = 1;
            raw.limits.max_cells_per_organism = 64;
        });

        let mut physics = Physics::new(&world);
        let mut cells = pile_up();
        let (mut peak, mut latest) = (0.0f32, 0.0f32);

        for _ in 0..ticks {
            physics.step(&mut cells, &[]);
            latest = kinetic_energy(&cells);
            peak = peak.max(latest);
        }

        (cells, peak, latest)
    }

    /// Cells that are overlapping push each other apart until they are merely touching, and
    /// then stop.
    ///
    /// This is the force that stops two bodies occupying the same water, and it has three
    /// ways to be wrong that all look like something else from the outside.
    ///
    /// **It can pull instead of push**, if the offset between the two cells is measured
    /// with its ends the wrong way round. What that looks like is every crowd in the world
    /// collapsing into a single point, which reads as a mysterious attraction rather than a
    /// swapped subtraction. So the separation is checked on every one of six hundred ticks
    /// rather than only at the end, and the two cells are required never to swap sides.
    ///
    /// **It can fail to stop.** The pair does not halt the instant it stops overlapping,
    /// and that is correct rather than a flaw: the water is viscous, not infinitely thick,
    /// so a pair shoved apart hard carries on coasting while the drag absorbs the motion.
    /// Two photocytes overlapping by four of their six units end up **ten units apart**
    /// rather than six - four units of coasting, which is well inside the eleven and a half
    /// that `motion_is_viscous_not_ballistic` measures a hard shove to be worth. What would
    /// be wrong is not coming to rest at all, so the pair is required to be *stationary* at
    /// the end rather than merely far apart.
    ///
    /// **It can be asymmetric**, moving one cell and not the other, which is what happens
    /// when the force is applied to one end of the pair and not subtracted from the other.
    /// A body would then drift in a direction decided by the order its cells happen to be
    /// stored in.
    ///
    /// The last claim is the one nobody writes: two cells at *exactly* the same point.
    /// There is no direction between them, so a plain division gives not-a-number and both
    /// cells leave the world on the next tick. It happens the first time Phase 3 places a
    /// daughter cell on top of its parent.
    #[test]
    fn overlapping_cells_push_apart() {
        let world = config(|_| {});
        let width = world.world.width;
        let touching = CellKind::Photocyte.radius() * 2.0;
        let mut physics = Physics::new(&world);

        let mut cells = pair(CellKind::Photocyte, 2.0);
        let mut previous = gap(&cells, width);

        for tick in 0..600 {
            physics.step(&mut cells, &[]);
            let now = gap(&cells, width);

            assert!(
                now >= previous - 1e-4,
                "on tick {tick} the overlapping pair closed from {previous} to {now}, so \
                 the collision force is pulling rather than pushing"
            );
            assert!(
                cells[0].pos.x < cells[1].pos.x,
                "on tick {tick} the two cells had swapped sides, so they passed straight \
                 through one another"
            );
            previous = now;
        }

        let settled = gap(&cells, width);
        assert!(
            settled > touching - 0.01,
            "the pair settled {settled} apart when two photocytes are {touching} wide, so \
             they are still overlapping"
        );
        assert!(
            settled < touching + 4.5,
            "the pair started 2 apart and ended up {settled} apart. Coasting a little past \
             {touching} is expected, and was measured at four units, but this is the \
             repulsion throwing them rather than separating them"
        );
        for (index, cell) in cells.iter().enumerate() {
            assert!(
                cell.vel.length() < 1e-4,
                "cell {index} of the separated pair is still moving at {}, so the two are \
                 sailing apart rather than settling",
                cell.vel.length()
            );
        }

        // Equal and opposite: neither cell is favoured by being stored first.
        let leftwards = 500.0 - cells[0].pos.x;
        let rightwards = cells[1].pos.x - 502.0;
        assert!(
            (leftwards - rightwards).abs() < 1e-3,
            "one cell moved {leftwards} and the other {rightwards}, so a pair of cells \
             pushing apart also drifts, in a direction decided by which was stored first"
        );

        // Cells that are not overlapping are not touched at all.
        let mut apart = pair(CellKind::Photocyte, 20.0);
        for _ in 0..600 {
            physics.step(&mut apart, &[]);
        }
        assert!(
            (gap(&apart, width) - 20.0).abs() < 1e-4,
            "two cells twenty apart drifted to {} apart, so something is acting on cells \
             that are not touching",
            gap(&apart, width)
        );

        // And two cells in exactly the same place separate rather than dividing by nothing.
        let mut stacked = pair(CellKind::Photocyte, 0.0);
        for _ in 0..600 {
            physics.step(&mut stacked, &[]);
        }
        for (index, cell) in stacked.iter().enumerate() {
            assert!(
                cell.pos.x.is_finite() && cell.pos.y.is_finite(),
                "cell {index} of a coincident pair left the world entirely, at ({}, {})",
                cell.pos.x,
                cell.pos.y
            );
        }
        assert!(
            gap(&stacked, width) > touching - 0.01,
            "two cells in exactly the same place ended up {} apart, so a coincident pair \
             stays coincident for ever",
            gap(&stacked, width)
        );
    }

    /// A spring brings the two cells it joins to its rest length, whether that means
    /// pulling them together or pushing them apart.
    ///
    /// Springs are what make a body a body rather than a crowd. Everything Phase 3 grows is
    /// held together by these, and the rest length is the number a myocyte oscillates to
    /// swim.
    ///
    /// The second case is the important one and it is not really about springs at all. Its
    /// rest length is two units and the cells are six units wide, so the spring wants them
    /// closer together than they can be without overlapping. If an adhered pair also
    /// repelled one another - which is what happens if SPEC section 8's word "non-adhered"
    /// is skimmed - the two forces balance at six units apart and the spring appears to
    /// work. Every other test here would still pass. What would be broken is contraction:
    /// a myocyte pulling its springs shorter would achieve nothing, and the only means of
    /// locomotion in the world would silently not exist.
    ///
    /// The third case is what `spring_damping` is *for*, and SPEC section 3 does not say -
    /// it is listed with no description at all. At the shipped configuration the water is
    /// so viscous that a spring cannot oscillate anyway, so the setting does nothing
    /// visible and could be anything. Turn the drag off entirely and it becomes the only
    /// thing in the world that can bring a spring to rest: without it the pair rings for
    /// ever, with it the pair settles. That is the semantics, demonstrated rather than
    /// asserted.
    #[test]
    fn a_spring_pulls_towards_its_rest_length() {
        let world = config(|_| {});
        let width = world.world.width;
        let mut physics = Physics::new(&world);

        // Stretched: the spring pulls the pair together.
        let mut stretched = pair(CellKind::Photocyte, 20.0);
        let springs = [Spring {
            a: 0,
            b: 1,
            rest_length: 8.0,
            stiffness: 5.0,
        }];
        for _ in 0..900 {
            physics.step(&mut stretched, &springs);
        }
        assert!(
            (gap(&stretched, width) - 8.0).abs() < 0.01,
            "a spring with a rest length of 8 left its cells {} apart",
            gap(&stretched, width)
        );

        // Compressed below the width of the cells themselves. This settles at the rest
        // length only because an adhered pair does not also collide.
        let mut compressed = pair(CellKind::Photocyte, 10.0);
        let short = [Spring {
            a: 0,
            b: 1,
            rest_length: 2.0,
            stiffness: 5.0,
        }];
        for _ in 0..900 {
            physics.step(&mut compressed, &short);
        }
        assert!(
            (gap(&compressed, width) - 2.0).abs() < 0.01,
            "a spring with a rest length of 2 left its two 6-wide cells {} apart. At about \
             6 the pair is also colliding, which means SPEC section 8's 'non-adhered' has \
             been skipped and no myocyte will ever be able to contract",
            gap(&compressed, width)
        );

        // With the water's drag turned off, damping is the only thing that can stop a
        // spring ringing.
        let mut ringing = pair(CellKind::Photocyte, 12.0);
        let mut settling = pair(CellKind::Photocyte, 12.0);
        let long = [Spring {
            a: 0,
            b: 1,
            rest_length: 8.0,
            stiffness: 5.0,
        }];

        let mut frictionless = Physics::new(&config(|raw| {
            raw.physics.drag = 1.0;
            raw.physics.spring_damping = 0.0;
        }));
        let mut damped = Physics::new(&config(|raw| {
            raw.physics.drag = 1.0;
            raw.physics.spring_damping = 4.0;
        }));

        let (mut ringing_low, mut ringing_high) = (f32::MAX, 0.0f32);
        let (mut damped_low, mut damped_high) = (f32::MAX, 0.0f32);
        for tick in 0..900 {
            frictionless.step(&mut ringing, &long);
            damped.step(&mut settling, &long);

            // Only the last two hundred ticks count: both start by swinging.
            if tick >= 700 {
                ringing_low = ringing_low.min(gap(&ringing, width));
                ringing_high = ringing_high.max(gap(&ringing, width));
                damped_low = damped_low.min(gap(&settling, width));
                damped_high = damped_high.max(gap(&settling, width));
            }
        }

        assert!(
            ringing_high - ringing_low > 3.0,
            "with no drag and no damping the pair should still be swinging, and its \
             separation only varied between {ringing_low} and {ringing_high}, so this test \
             has nothing to compare against"
        );
        assert!(
            damped_high - damped_low < 0.01,
            "with damping the pair should have stopped, and its separation was still \
             varying between {damped_low} and {damped_high}"
        );
        assert!(
            (damped_high - 8.0).abs() < 0.01,
            "the damped pair stopped at {damped_high} rather than at its rest length of 8"
        );
    }

    /// A cell that is given a shove coasts to a halt almost immediately, rather than
    /// sailing on.
    ///
    /// SPEC section 8: at the scale of a cell in water, inertia is nearly irrelevant. That
    /// is physically right - it is what a low Reynolds number means, and it is why bacteria
    /// stop dead the instant they stop swimming - and it is also what makes the arithmetic
    /// here stable enough to run overnight.
    ///
    /// The number that comes out of it is worth knowing when reading anything else in this
    /// module: at SPEC's drag of 0.92, a cell shoved at sixty world units a second travels
    /// **eleven and a half units** before it stops, which is not quite two of its own
    /// body-widths. Everything a cell does, it does under power.
    ///
    /// The second half is the counterfactual, and without it the first half proves nothing:
    /// the same shove in a world with no drag at all carries the cell six hundred units in
    /// the same time. So this is a test about viscosity rather than a test that a small
    /// number is small.
    #[test]
    fn motion_is_viscous_not_ballistic() {
        let world = config(|_| {});
        let mut physics = Physics::new(&world);

        let mut cells = vec![Cell::new(CellKind::Photocyte, Vec2::new(500.0, 500.0))];
        cells[0].vel = Vec2::new(60.0, 0.0);

        for _ in 0..600 {
            physics.step(&mut cells, &[]);
        }

        let coasted = cells[0].pos.x - 500.0;
        let body_width = CellKind::Photocyte.radius() * 2.0;
        assert!(
            coasted < body_width * 2.0,
            "a shoved cell coasted {coasted} units, which is more than two of its own \
             {body_width}-unit body-widths, so the world is not viscous"
        );
        assert!(
            coasted > body_width,
            "a shoved cell travelled only {coasted} units, which is less than its own \
             width - the drag is so heavy that nothing could ever move"
        );
        assert!(
            cells[0].vel.length() < 1e-4,
            "the cell is still moving at {} after six hundred ticks of coasting",
            cells[0].vel.length()
        );

        // The same shove in water with no drag in it at all.
        let mut ballistic = Physics::new(&config(|raw| raw.physics.drag = 1.0));
        let mut sailing = vec![Cell::new(CellKind::Photocyte, Vec2::new(500.0, 500.0))];
        sailing[0].vel = Vec2::new(60.0, 0.0);

        for _ in 0..600 {
            ballistic.step(&mut sailing, &[]);
        }

        let sailed = sailing[0].pos.x - 500.0;
        assert!(
            sailed > coasted * 20.0,
            "with no drag the same shove carried the cell {sailed} units against {coasted} \
             with drag, so the drag setting is barely doing anything"
        );
    }

    /// The world joins up sideways and stops at the surface and the floor.
    ///
    /// SPEC section 8 gives both halves and a reason for each. Wrapping sideways is what
    /// stops an edge-hugging strategy from dominating - a wall is a place with neighbours
    /// on only one side, which is worth evolving towards for reasons that have nothing to
    /// do with the ecology. Closing the world top and bottom is what keeps the light
    /// gradient meaningful: depth has to mean something, so there has to be a shallowest
    /// and a deepest place.
    ///
    /// Four claims, and the third is the one that is easy to have wrong for months.
    ///
    /// A cell that walks off one side arrives at the other, and its position never leaves
    /// the world on the way.
    ///
    /// A cell driven into the surface or the floor stops *exactly* there and its downward
    /// motion is taken away rather than reversed. Exactly, not nearly: the floor is a
    /// definite place, and a cell that ends a hair beyond it is a cell the resource grid
    /// will look up in a row that does not exist.
    ///
    /// **Forces reach across the join.** Two cells either side of it are neighbours and
    /// push apart through it. A wrap that only moves positions and forgets that distances
    /// wrap too passes the first claim perfectly and still leaves an invisible wall down
    /// one column of the world. The claim is put as strongly as it can be: an identical
    /// pair placed in the middle of the world must end up at exactly the same separation,
    /// so the join is required to be *nowhere in particular* rather than merely passable.
    ///
    /// And a cell pushed off the left-hand edge comes back on the right, rather than being
    /// stopped by an edge that is not supposed to exist.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the surface is at y = 0 and the floor at y = height, and a cell resting \
                  on either must be exactly there; a tolerance would let a cell sit a \
                  fraction outside the world, which is a row the resource grid does not have"
    )]
    fn the_world_wraps_sideways_and_is_closed_top_and_bottom() {
        let steady = config(|raw| raw.physics.drag = 1.0);
        let (width, height) = (steady.world.width, steady.world.height);
        let mut physics = Physics::new(&steady);

        // Straight off the right-hand edge, four units a tick, for a hundred ticks.
        let mut travelling = vec![Cell::new(
            CellKind::Photocyte,
            Vec2::new(width - 2.0, 500.0),
        )];
        travelling[0].vel = Vec2::new(240.0, 0.0);

        let mut wrapped_at_least_once = false;
        let mut previous = travelling[0].pos.x;
        for tick in 0..100 {
            physics.step(&mut travelling, &[]);
            let here = travelling[0].pos.x;

            assert!(
                (0.0..width).contains(&here),
                "on tick {tick} the cell was at {here} in a world {width} wide"
            );
            if here < previous {
                wrapped_at_least_once = true;
            }
            previous = here;
        }
        assert!(
            wrapped_at_least_once,
            "a cell driven off the right-hand edge for a hundred ticks never came back on \
             the left, so the world does not wrap"
        );
        assert!(
            (travelling[0].pos.x - 398.0).abs() < 0.1,
            "after travelling 400 units from {} in a world {width} wide the cell is at {}, \
             and it should be at 398",
            width - 2.0,
            travelling[0].pos.x
        );

        // Straight up into the surface.
        let mut rising = vec![Cell::new(CellKind::Photocyte, Vec2::new(500.0, 5.0))];
        rising[0].vel = Vec2::new(0.0, -60.0);
        for _ in 0..100 {
            physics.step(&mut rising, &[]);
        }
        assert_eq!(
            rising[0].pos.y, 0.0,
            "a cell swimming upwards did not stop at the surface"
        );
        assert_eq!(
            rising[0].vel.y, 0.0,
            "a cell held against the surface is still recorded as swimming into it, so it \
             will spring away the moment anything else touches it"
        );

        // Straight down into the floor.
        let mut sinking = vec![Cell::new(
            CellKind::Photocyte,
            Vec2::new(500.0, height - 5.0),
        )];
        sinking[0].vel = Vec2::new(0.0, 60.0);
        for _ in 0..100 {
            physics.step(&mut sinking, &[]);
        }
        assert_eq!(
            sinking[0].pos.y, height,
            "a cell sinking to the bottom did not stop at the floor"
        );
        assert_eq!(
            sinking[0].vel.y, 0.0,
            "a cell resting on the floor is still sinking"
        );

        // Two cells either side of the join push each other apart through it, and they do
        // it exactly as an identical pair in open water does.
        let default_world = config(|_| {});
        let mut ordinary = Physics::new(&default_world);
        let mut straddling = vec![
            Cell::new(CellKind::Photocyte, Vec2::new(1.0, 500.0)),
            Cell::new(CellKind::Photocyte, Vec2::new(width - 1.0, 500.0)),
        ];
        let mut in_open_water = vec![
            Cell::new(CellKind::Photocyte, Vec2::new(1002.0, 500.0)),
            Cell::new(CellKind::Photocyte, Vec2::new(1000.0, 500.0)),
        ];
        for _ in 0..600 {
            ordinary.step(&mut straddling, &[]);
            ordinary.step(&mut in_open_water, &[]);
        }
        assert!(
            (gap(&straddling, width) - gap(&in_open_water, width)).abs() < 1e-3,
            "a pair two units apart across the join ended up {} apart, and the same pair \
             two units apart in the middle of the world ended up {} apart. The join is a \
             place with different physics, which is exactly what the wrap exists to prevent",
            gap(&straddling, width),
            gap(&in_open_water, width)
        );
        assert!(
            straddling[0].pos.x > 1.0 && straddling[1].pos.x < width - 1.0,
            "the pair moved towards each other rather than apart: {} and {}",
            straddling[0].pos.x,
            straddling[1].pos.x
        );

        // And a cell shoved off the left-hand edge arrives on the right.
        let mut evicted = vec![
            Cell::new(CellKind::Photocyte, Vec2::new(0.0, 500.0)),
            Cell::new(CellKind::Photocyte, Vec2::new(3.0, 500.0)),
        ];
        for _ in 0..600 {
            ordinary.step(&mut evicted, &[]);
        }
        assert!(
            evicted[0].pos.x > width - 5.0,
            "a cell pushed off the left-hand edge ended up at {} rather than coming back \
             on the right",
            evicted[0].pos.x
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        /// The spatial hash finds exactly the pairs that comparing everything with
        /// everything finds - no more, and crucially no fewer.
        ///
        /// The hash is an optimisation, and the only way to trust an optimisation is to
        /// check it against the slow version that is obviously right. Comparing every cell
        /// with every other is unarguable and unusably slow; this is the fast one, and
        /// this test is the whole of the argument that the two are the same thing.
        ///
        /// **What it is really testing is the seam.** A neighbour search that treats the
        /// left and right edges of the world as walls passes every test anybody writes by
        /// hand, because cells in the middle of the world are the ones you think to check.
        /// It fails only for cells within one bucket of the join - who quietly stop
        /// noticing each other - and what that looks like from the outside is a body that
        /// tears in half when it drifts across an invisible line, months later, in a run
        /// nobody was watching. So every crowd here contains cells hard against both edges
        /// and cells resting exactly on the floor, rather than trusting random positions to
        /// find them.
        ///
        /// Five claims.
        ///
        /// The distance between two cells is the distance *through* the join when that is
        /// shorter, which is checked against the same measurement written out the slow and
        /// obvious way from SPEC - all three places the far cell could be, keeping the
        /// nearest. This comes first because it is separate from anything about buckets: a
        /// search that offers the right candidates and then measures them with a plain
        /// subtraction is a search that finds every pair across the join and decides none
        /// of them is touching.
        ///
        /// Every overlapping pair is reported, and every reported pair is genuinely
        /// overlapping. A hash that hands back too much is merely slow; one that hands back
        /// too little lets cells pass through one another.
        ///
        /// Each pair is reported exactly twice - once from each end. The collision pass
        /// leans on that: it walks every cell's neighbours and acts only on the half of
        /// them with a larger index, which is only safe if both ends see each other.
        ///
        /// Two separately built hashes give identical answers in identical order, and a
        /// hash rebuilt over a different crowd and then this one gives the same answer as
        /// a fresh one. The first is what a map-based implementation would fail, since the
        /// standard library deliberately varies a map's iteration order between instances;
        /// the second is what a rebuild that forgot to clear its counts would fail.
        ///
        /// And it does less work than the slow version, which is the only reason it exists.
        #[test]
        fn the_spatial_hash_finds_the_same_neighbours_as_checking_everything(
            ((width, height), cells) in shaped_crowd()
        ) {
            let world = config(|raw| {
                raw.world.width = f64::from(width);
                raw.world.height = f64::from(height);
                raw.limits.max_organisms = 4;
                raw.limits.max_cells_per_organism = 64;
            });

            // Before anything about buckets: the module's own idea of how far apart two
            // cells are has to agree with the slow one written from SPEC.
            for near in 0..cells.len() {
                for far in 0..cells.len() {
                    let measured = wrapped_offset(cells[near].pos, cells[far].pos, width);
                    let honest = spec_separation(cells[near].pos, cells[far].pos, width);

                    prop_assert!(
                        (measured.length() - honest.length()).abs() <= width * 1e-6,
                        "cells {} at ({}, {}) and {} at ({}, {}) are {} apart the slow way \
                         round a world {} wide, and this module makes it {}",
                        near, cells[near].pos.x, cells[near].pos.y,
                        far, cells[far].pos.x, cells[far].pos.y,
                        honest.length(), width, measured.length()
                    );
                    prop_assert!(
                        measured.x.abs() <= width * 0.5 + width * 1e-6,
                        "the offset from cell {} to cell {} is {} across in a world only \
                         {} wide, so it is going the long way round rather than through \
                         the join",
                        near, far, measured.x, width
                    );
                }
            }

            let mut hash = SpatialHash::new(&world);
            hash.rebuild(&cells);

            let mut reported: Vec<(usize, usize)> = Vec::new();
            let mut visits = 0usize;
            for index in 0..cells.len() {
                hash.for_each_neighbour(index, |other| {
                    visits += 1;
                    assert_ne!(
                        other, index,
                        "the search handed a cell back to itself, so every cell is about \
                         to be pushed away from where it already is"
                    );
                    if touching(&cells[index], &cells[other], width) {
                        reported.push((index.min(other), index.max(other)));
                    }
                });
            }
            reported.sort_unstable();

            // Runs of equal pairs, so that "reported twice" can be checked without
            // comparing every pair against every other.
            let mut counted: Vec<((usize, usize), usize)> = Vec::new();
            for pair in &reported {
                match counted.last_mut() {
                    Some((last, times)) if last == pair => *times += 1,
                    _ => counted.push((*pair, 1)),
                }
            }
            for &((near, far), times) in &counted {
                prop_assert_eq!(
                    times, 2,
                    "cells {} and {} are touching and the search reported the pair {} \
                     time(s) rather than once from each end",
                    near, far, times
                );
            }

            let found: Vec<(usize, usize)> = counted.iter().map(|&(pair, _)| pair).collect();
            let expected = every_touching_pair(&cells, width);

            if found != expected {
                let missed: Vec<String> = expected
                    .iter()
                    .filter(|pair| !found.contains(pair))
                    .take(4)
                    .map(|&(near, far)| {
                        let gap = spec_separation(cells[near].pos, cells[far].pos, width);
                        format!(
                            "{near} at ({}, {}) and {far} at ({}, {}), {} apart in a world \
                             {width} wide",
                            cells[near].pos.x,
                            cells[near].pos.y,
                            cells[far].pos.x,
                            cells[far].pos.y,
                            gap.length()
                        )
                    })
                    .collect();
                let invented: Vec<(usize, usize)> = found
                    .iter()
                    .filter(|pair| !expected.contains(pair))
                    .take(4)
                    .copied()
                    .collect();

                prop_assert!(
                    false,
                    "the search found {} touching pairs and there are {}. Missed: {:?}. \
                     Reported but not touching: {:?}",
                    found.len(),
                    expected.len(),
                    missed,
                    invented
                );
            }

            // The same cells hashed again, by a hash that has seen something else first.
            let mut reused = SpatialHash::new(&world);
            reused.rebuild(&cells[..cells.len() / 2]);
            reused.rebuild(&cells);

            for index in 0..cells.len() {
                let mut once = Vec::new();
                hash.for_each_neighbour(index, |other| once.push(other));
                let mut again = Vec::new();
                reused.for_each_neighbour(index, |other| again.push(other));

                prop_assert_eq!(
                    &once, &again,
                    "cell {}'s neighbours came back in a different order from a second \
                     hash, so the same seed does not give the same run",
                    index
                );
            }

            // And it is actually an optimisation. Only asked of worlds with enough buckets
            // for the question to mean anything.
            if hash.cols * hash.rows >= 64 && cells.len() >= 64 {
                prop_assert!(
                    visits < cells.len() * (cells.len() - 1),
                    "the search looked at {} pairs and comparing everything with \
                     everything would have looked at {}, so it is not saving anything",
                    visits,
                    cells.len() * (cells.len() - 1)
                );
            }
        }
    }

    /// A crowd of heavily overlapping cells settles down instead of exploding.
    ///
    /// This is the classic way a spring-and-collision simulation fails, and it fails
    /// loudly rather than subtly, which is exactly why it is worth a test rather than a
    /// hope. Sixty-four cells are stacked so tightly that each overlaps its neighbours by
    /// three quarters of itself; every one of them is being pushed hard in several
    /// directions at once. If the repulsion moves a cell further in one tick than the
    /// overlap it was correcting, the next tick's overlap is larger, and the crowd shakes
    /// itself apart faster every tick until the numbers stop meaning anything.
    ///
    /// The claims are concrete rather than "it looks all right": after two thousand ticks
    /// every position is a real number, every cell is inside the world, no pair is still
    /// overlapping, and the motion in the crowd has fallen to a tiny fraction of its peak.
    /// Peak rather than start, because the crowd begins at a standstill - the interesting
    /// comparison is between how violently it reacted and how quiet it ended up.
    ///
    /// # How much margin the shipped stiffness has
    ///
    /// The second half is the measurement, and it is here because "it is stable" is not
    /// useful without "and how close to the edge is it". The same pile is dropped into
    /// worlds of increasing `collision_stiffness` until one of them fails to settle.
    ///
    /// **The measured edge is between 3,240 and 3,280, and SPEC section 3 ships 40.0.** So
    /// the shipped value sits about eighty times below the point at which this pile stops
    /// settling, which is a long way from a cliff - a slider would have to be dragged
    /// through two orders of magnitude before anything went wrong, and the failure when it
    /// did would be obvious rather than quiet.
    ///
    /// Two things are worth knowing alongside that number.
    ///
    /// It is a figure for a *crowd*. A single pair of cells has no ceiling at all, because
    /// once they stop overlapping there is nothing left to push them; what stiffness buys
    /// there is distance, and a pair overlapping by four units is flung ten apart at 40 and
    /// two hundred apart at 8,000. The crowd is the harder case because a cell in the
    /// middle of the pile is being pushed by eight neighbours at once, and eight forces of
    /// stiffness `k` behave much like one force of `8k`.
    ///
    /// And instability here is not infinity. Two cells cannot overlap by more than their
    /// combined width, so the repulsion between any one pair is bounded however stiff it
    /// is; what an unstable world does instead is jitter for ever, cells shoved through one
    /// another and shoved back, never coming to rest - at 6,000 the pile is still carrying
    /// a tenth of its peak motion after two thousand ticks, with pairs sitting almost
    /// exactly on top of each other. So the test for stability is whether the crowd goes
    /// quiet, and a check written as "are the numbers still finite" would pass on every
    /// value there is.
    #[test]
    fn physics_is_stable_under_a_pile_up() {
        let (cells, peak, latest) = watch_a_pile(40.0, 2_000);

        assert!(peak > 0.0, "the pile never moved at all");

        for (index, cell) in cells.iter().enumerate() {
            assert!(
                cell.pos.x.is_finite() && cell.pos.y.is_finite(),
                "cell {index} ended up at ({}, {}), which is not a place",
                cell.pos.x,
                cell.pos.y
            );
            assert!(
                (0.0..200.0).contains(&cell.pos.x) && (0.0..=120.0).contains(&cell.pos.y),
                "cell {index} ended up at ({}, {}), outside a world 200 by 120",
                cell.pos.x,
                cell.pos.y
            );
        }

        assert!(
            latest < peak * 1e-3,
            "the pile is still moving: it peaked at {peak} and finished at {latest}, so it \
             is simmering rather than settling"
        );

        // Nothing is still overlapping, which is the pile actually having resolved rather
        // than merely having stopped.
        let mut worst = 0.0f32;
        for near in 0..cells.len() {
            for far in near + 1..cells.len() {
                let apart = spec_separation(cells[near].pos, cells[far].pos, 200.0).length();
                worst = worst.max(cells[near].radius + cells[far].radius - apart);
            }
        }
        assert!(
            worst < 0.01,
            "two cells in the settled pile are still overlapping by {worst}"
        );

        // How stiff the repulsion can be before the pile stops settling.
        let ladder = [40.0, 200.0, 1_000.0, 3_000.0, 6_000.0, 12_000.0];
        let mut stiffest_that_settles = 0.0;
        let mut gentlest_that_does_not = f64::INFINITY;

        for stiffness in ladder {
            let (_, peak, latest) = watch_a_pile(stiffness, 2_000);
            let settled = latest.is_finite() && peak > 0.0 && latest < peak * 1e-3;

            if settled {
                stiffest_that_settles = stiffness;
            } else if stiffness < gentlest_that_does_not {
                gentlest_that_does_not = stiffness;
            }
        }

        assert!(
            gentlest_that_does_not.is_finite(),
            "every stiffness up to {} settled, so this test cannot tell a stable world \
             from an unstable one and the margin it reports is meaningless",
            ladder[ladder.len() - 1]
        );
        assert!(
            stiffest_that_settles >= 40.0 * 25.0,
            "the stiffest collision the pile survives is {stiffest_that_settles} and SPEC \
             section 3 ships 40.0, which is less than twenty-five times the margin. That is \
             too close to the edge for a number exposed as a slider, and the measured edge \
             was eighty times the default when this was written"
        );
    }
}
