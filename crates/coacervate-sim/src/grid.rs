//! The resource field.
//!
//! See `SPEC.md` section 4 for the rules this implements.
//!
//! This is the world's food, and for now the whole of its contents: a coarse grid of tiles
//! laid over the water, each holding a quantity of energy that the light puts there and
//! that organisms will later take away. It is the only place energy ever enters the
//! simulation, so everything that happens in a run happened to a tile of this first.
//!
//! # Three things decide what a tile does
//!
//! **How deep it is.** The light falls from above and weakens with depth, so a tile near
//! the surface is offered more energy per tick and can hold more of it. SPEC section 4 is
//! emphatic about why this exists rather than a flat world being simpler: *the gradient is
//! what gives movement a reason to exist.* Without it, every position is as good as every
//! other, phototaxis has nothing to discover, and swimming is a cost with no payoff.
//!
//! **Where it is.** Laid over the depth gradient is a fixed blotchiness - some parts of the
//! world are richer than others at the same depth. SPEC calls for it and does not say what
//! it should be made of; this module answers that (Phase 2's open question Q7) with a
//! coarse lattice of random heights, interpolated smoothly, worked out once from the world's
//! seed. Blotches, not speckle: the difference is the whole of
//! `the_light_noise_is_patchy_rather_than_speckled`.
//!
//! **What its neighbours are holding.** Energy spreads sideways a little each tick. That
//! stops an organism from digging a permanent hole under itself and sitting in it, and it
//! is the only part of this module where the arithmetic is not exact - see below.
//!
//! # A tile cannot hold more than its ceiling, and the excess is a real loss
//!
//! Diffusion knows nothing about ceilings, so it will happily leave a tile holding several
//! times what it can. SPEC section 4 requires that a tile genuinely cannot, so the ceiling
//! is enforced once at the end of every tick and what will not fit **leaves the world**,
//! moving `field → dissipated` through [`Ledger::overflow`].
//!
//! That is a decision rather than a detail, and it is the reason the field has a shape at
//! all. Ceilings fall with depth, so energy is permanently being carried downwards into
//! tiles with less and less room for it; let it accumulate and the field fills from the
//! bottom upwards until it is **level**, every tile in the world at the deepest ceiling in
//! it. `the_field_reaches_a_ceiling` recorded exactly that before the rule existed. A level
//! field is one in which being near the surface is worth nothing, and the whole case for
//! having a depth gradient is that it is worth something.
//!
//! Losing it is also the ecologically familiar answer: energy that sinks below the light is
//! energy the ocean loses, which is what the biological pump does. Total influx per tick is
//! unchanged, so SPEC section 4's carrying-capacity argument is untouched.
//!
//! # The field is measured, never stored
//!
//! [`Grid::total_energy`] adds the tiles up on demand, and that sum is SPEC section 5's
//! `field` account. There is no running total kept anywhere. Keeping one would mean two
//! records of a single quantity and the standing job of keeping them in step, which is
//! exactly the bookkeeping that goes wrong; see `ledger.rs`, which is built around not
//! doing it.
//!
//! What this module *does* owe the ledger is a truthful account of the two things that
//! change how much energy the world contains: what the light put in, and what the tiles
//! could not hold. [`Grid::tick`] takes the ledger by reference rather than handing those
//! numbers back, so that neither can be forgotten. Both are what actually happened to the
//! tiles rather than what the formulae asked for, and the difference between those two is
//! the phase's central failure mode - `regrowth_credits_influx_with_the_realised_change`.
//!
//! # Everything is allocated once
//!
//! Four arrays, all built when the world is: the tiles, their ceilings, the movement
//! between them, and one figure per row for how much light that row is offered. Nothing
//! here ever grows, and there is no code in this file that could grow it. CLAUDE.md: a
//! simulation that cannot allocate cannot leak. Twelve bytes per tile plus a rounding, so
//! SPEC's default world of 256 × 144 tiles costs about 430 kilobytes - small enough that
//! the interesting limits in this project are elsewhere.
//!
//! # Where the arithmetic gives
//!
//! Tiles are 32-bit numbers, because SPEC section 2 requires state to be in the form a
//! graphics card works in. That is about seven digits, and the sums this module does are
//! not always representable in seven digits, so it has to be deliberate about where the
//! rounding goes.
//!
//! Regrowth is exact, and so is the ceiling: what a tile gained or gave up is measured
//! after the fact by widening both readings and subtracting, so the influx and dissipated
//! accounts agree with the grid rather than nearly agreeing.
//!
//! Diffusion cannot be exact, because moving energy between tiles means writing it into a
//! tile. What it can be, and is, is *conservative*: energy is only ever moved between pairs
//! of neighbours, one number applied to both ends at once, and the part of a movement too
//! small to write into a tile is kept rather than dropped. What that leaves is a small
//! quantity permanently in transit - at most half a rounding step per tile, about two
//! hundredths of a unit for the default world - which does not grow however long a run
//! goes on. Measured over the default world with nothing alive in it, 100,000 ticks leave
//! the books out by under two parts in ten billion, and never by worse than eight parts in
//! a billion along the way, against a tolerance of one part in a thousand. Enforcing the
//! ceiling did not cost any of that: the same run without it came out at three parts in ten
//! billion, with an identical worst moment.
//!
//! # Taking energy out of a tile
//!
//! There is exactly one way, [`Grid::harvest`], and it takes the ledger as an argument. That
//! is the whole design: the tiles are handed out only as a shared slice, so no code outside
//! this module can write one, and the only routine that lowers a tile cannot be called
//! without the books being present to be told. Energy therefore cannot leave the field
//! without an account gaining it - not by mistake, and not on purpose either.
//!
//! Phase 3 seeds organisms through it, because SPEC section 5 says a seeded organism's
//! starting energy comes out of the field exactly as harvesting would; Phase 4's photocytes
//! will use the same door.
//!
//! # What is not here
//!
//! A world starts empty and fills under the light, which takes a few hundred ticks. Phase 3
//! seeds organisms out of the field, so an organism seeded on tick zero can be given nothing
//! at all - there is nothing there yet - and one seeded shortly afterwards spends its first
//! hundreds of ticks in a famine that is an artefact of the world having just been switched
//! on rather than anything about the organism. Whoever sets a run going has to decide whether
//! to let the field fill first.

use crate::cell::Vec2;
use crate::config::Config;
use crate::ledger::Ledger;

/// Turn a quantity worked out at full precision into one a tile can hold.
///
/// The one cast in this file, and it is here for the same reason `config.rs` has exactly
/// one: the standard library offers no `TryFrom` between the two sizes of number, so a
/// project that forbids lossy casts has to write that conversion itself, once, with the
/// reasoning attached.
///
/// Nothing here can overflow. Every value that goes through it is built out of `light.cap`
/// and `light.influx`, both of which were checked into range on the way in, multiplied by a
/// profile between nought and one and a patchiness factor between nought and two. What the
/// conversion gives up is digits, and giving up digits is the point: SPEC section 2 keeps
/// the grid at 32 bits because that is what a graphics card works in, and doing the
/// arithmetic wider and rounding once at the end is strictly closer to the truth than doing
/// it narrow all the way through.
fn narrowed(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the values are small quantities of energy derived from configuration \
                  numbers already checked into range, so there is nothing here to \
                  overflow; the lost digits are what SPEC section 2 asks for"
    )]
    let held = value as f32;
    held
}

/// SPEC section 4's `light_profile(y) = 1 - gradient × (y / height)`, for one row of tiles.
///
/// The row is measured at the middle of the band of depth it covers, because a tile is not
/// a point - see `light_falls_off_with_depth` for what the alternative costs. The world's
/// height in world units does not appear: the rows divide the world's depth evenly, so
/// SPEC's `y / height` is exactly `(row + ½) / rows` and the height cancels.
///
/// Worked out at full precision and narrowed by the caller once it has been multiplied by
/// whatever it is scaling.
fn light_profile(row: u32, rows: u32, gradient: f32) -> f64 {
    1.0 - f64::from(gradient) * ((f64::from(row) + 0.5) / f64::from(rows))
}

// ---------------------------------------------------------------------------------------
// The blotchiness of the light
//
// SPEC section 4 writes `noise(x, y)` and does not say which noise. This is the answer -
// Phase 2's open question Q7 - and the reasoning behind it is in
// `patchiness_is_stable_across_runs` and `the_light_noise_is_patchy_rather_than_speckled`.
//
// It is value noise: a coarse lattice of random heights laid over the grid, with every tile
// taking a smoothly interpolated reading between the four lattice points around it. That
// gives blotches at the scale of the lattice rather than a different number per tile, and
// it costs one hash per lattice point rather than any state at all.
// ---------------------------------------------------------------------------------------

/// How many tiles across one blotch of light is.
///
/// The number is a compromise between two failures, and the world it has to suit is SPEC
/// section 3's default: 256 x 144 tiles over 2048 x 1152 world units, so a tile is 8 units
/// across and a cell of SPEC section 6 is about 6 units across.
///
/// Too fine and there is nothing to swim towards: a blotch the size of an organism is a
/// blotch an organism is already standing in, and moving costs energy for no return. Too
/// coarse and the world is two places rather than many, so there is one right answer to
/// where to live and every lineage finds it.
///
/// Sixteen tiles makes a blotch 128 world units across - roughly twenty cell-widths, so
/// crossing one is a real journey - and puts 16 x 9 = 144 of them in the default world.
/// It also divides both of the default grid's dimensions exactly, which matters for the
/// horizontal wrap: the lattice fits a whole number of times across the world, so its
/// rightmost points are its leftmost ones and there is no seam.
const NOISE_LATTICE_SPACING: u32 = 16;

/// Stir a number until its bits have nothing to do with the bits that went in.
///
/// This is the finalising step of the well-known `splitmix64`, and it is here rather than a
/// draw from the world's generator because it needs no state: the same coordinates give the
/// same number for ever, whatever else the simulation has done. Every operation is
/// explicitly a wrapping one, because CLAUDE.md turns on integer overflow checking in
/// release builds and a mixer is nothing but deliberate overflow.
fn scramble(value: u64) -> u64 {
    let stirred = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let stirred = (stirred ^ (stirred >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    stirred ^ (stirred >> 31)
}

/// Ease a fraction so that it leaves nought and arrives at one without a corner.
///
/// The classic smoothstep. Interpolating between lattice points in a straight line is
/// cheaper and leaves a visible crease along every lattice line, because the slope changes
/// abruptly where two straight sections meet. This has zero slope at both ends, so the
/// blotches blend into one another and the lattice underneath them cannot be seen.
fn ease(fraction: f64) -> f64 {
    fraction * fraction * (3.0 - 2.0 * fraction)
}

/// A weighted average of two numbers.
fn between(here: f64, there: f64, fraction: f64) -> f64 {
    here + (there - here) * fraction
}

/// The lattice of random heights the light's blotchiness is read off.
///
/// Holds no numbers of its own beyond the shape of the lattice: every height is worked out
/// from the seed and the lattice point's coordinates on demand, so two worlds with the same
/// seed have the same blotches without anything having to be stored or copied.
struct PatchNoise {
    seed: u64,
    cols: u32,
    rows: u32,
    /// How many lattice cells fit across the world. The lattice wraps at this many, which
    /// is what makes the noise continuous where the world joins up.
    lattice_cols: u32,
    /// How many lattice cells fit down the world. This one does not wrap: the world is
    /// closed top and bottom, so the lattice simply has one more row of points than it has
    /// cells.
    lattice_rows: u32,
}

impl PatchNoise {
    /// Lay a lattice over a grid of this shape.
    ///
    /// The lattice is stretched to fit a whole number of cells across the world rather than
    /// laid down every sixteen tiles and cut off wherever it happens to end. That is what
    /// makes the wrap seamless for a world whose width is not a multiple of the spacing;
    /// the price is that its cells are then between sixteen and thirty-two tiles across
    /// instead of exactly sixteen, which is a difference nobody can see.
    ///
    /// A world too small to hold a single cell gets a lattice one cell across, which is a
    /// world with one blotch in it. There is nothing else it could sensibly be.
    fn new(seed: u64, cols: u32, rows: u32) -> Self {
        Self {
            seed,
            cols,
            rows,
            lattice_cols: (cols / NOISE_LATTICE_SPACING).max(1),
            lattice_rows: (rows / NOISE_LATTICE_SPACING).max(1),
        }
    }

    /// Which lattice cell a tile falls in, and how far across that cell it sits.
    ///
    /// Done in whole numbers and divided once at the end, so the answer is a fraction of a
    /// cell exactly rather than the accumulation of a step added up a tile at a time.
    fn place(coordinate: u32, tiles: u32, cells: u32) -> (u32, f64) {
        let scaled = u64::from(coordinate) * u64::from(cells);
        let cell = u32::try_from(scaled / u64::from(tiles))
            .expect("a lattice cell index is smaller than the number of cells");
        let along = u32::try_from(scaled % u64::from(tiles))
            .expect("a remainder is smaller than what it was divided by");
        (cell, f64::from(along) / f64::from(tiles))
    }

    /// The height of one lattice point, between -1 and 1.
    ///
    /// The two coordinates are packed into one number rather than combined arithmetically,
    /// so that no two lattice points can share a height by having coordinates that add or
    /// exclusive-or to the same thing. Sixteen bits of the result are used, which is 65,536
    /// distinct heights - far more than a blotch can be told apart at.
    ///
    /// The range is exact at both ends and its middle is exactly nought: zero maps to -1,
    /// the largest value maps to 1, and the average of all of them is 0. That last part is
    /// what `config.rs` leans on when it bounds `patchiness` at one - noise centred
    /// anywhere else would make the world systematically richer or poorer than `cap` says.
    fn corner(&self, col: u32, row: u32) -> f64 {
        let coordinates = (u64::from(col) << 32) | u64::from(row);
        let hashed = scramble(self.seed ^ scramble(coordinates));
        let sixteen_bits = u16::try_from(hashed >> 48)
            .expect("a 64-bit number shifted down by 48 places fits in 16 bits");
        f64::from(sixteen_bits) / 32_767.5 - 1.0
    }

    /// Slide a position within the lattice along by `shift` whole-and-fractional cells,
    /// wrapping.
    ///
    /// ⭐ **This is the whole of SPEC section 4's drifting field**, and what makes it a drift
    /// rather than a new world is that it moves the *coordinate* the noise is read at. The
    /// lattice heights are still a pure function of the seed, so a blotch keeps its shape,
    /// its size and its neighbours and merely arrives somewhere else. Redrawing the heights
    /// instead would rearrange the whole field under a living population in one tick.
    ///
    /// Nought is exactly nought: `across` is already inside a cell, so adding zero leaves the
    /// floor at zero and the cell index alone, and every number that comes out is bit for bit
    /// the number that came out before this existed. That is what keeps `patch_drift = 0` the
    /// control experiment rather than an approximation to one.
    fn slide(cell: u32, across: f64, shift: f64, cells: u32) -> (u32, f64) {
        let span = f64::from(cells);
        let along = across + shift.rem_euclid(span);
        let whole = along.floor();

        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "`across` is a fraction of a cell and the shift has been wrapped into \
                      `0..cells`, so the floor is a whole number between nought and `cells` \
                      inclusive and the conversion is exact and in range"
        )]
        let carried = whole as u32;

        ((cell + carried) % cells, along - whole)
    }

    /// The blotchiness at one tile, between -1 and 1, with the field slid `shift` lattice
    /// cells to the **west**.
    ///
    /// The shift is added to the coordinate being read, so a growing shift reads what used to
    /// be further east - which is the pattern travelling west. Which way it goes is arbitrary
    /// in a world that wraps; that it goes one way and keeps going is not.
    ///
    /// Sideways only. The lattice wraps sideways because the world does, so a horizontal
    /// drift has nowhere to come from and nowhere to go and the field is seamless at every
    /// offset. Downwards it does not wrap - the world has a surface and a floor - so a
    /// vertical drift would push blotches through the floor and invent replacements at the
    /// surface, which is the reseed this is written to avoid, applied at the two edges.
    fn at(&self, col: u32, row: u32, shift: f64) -> f64 {
        let (west, across) = Self::place(col, self.cols, self.lattice_cols);
        let (west, across) = Self::slide(west, across, shift, self.lattice_cols);
        let (north, down) = Self::place(row, self.rows, self.lattice_rows);

        // Sideways the lattice wraps, because the world does. Downwards it does not: the
        // world has a surface and a floor, so the lattice has a last row of points rather
        // than joining back onto its first.
        let east = (west + 1) % self.lattice_cols;
        let south = north + 1;

        let across = ease(across);
        let above = between(self.corner(west, north), self.corner(east, north), across);
        let below = between(self.corner(west, south), self.corner(east, south), across);

        // A weighted average of four numbers each within one of nought is within one of
        // nought, and the clamp is there for the last bit of the arithmetic rather than for
        // the argument. `config.rs` bounds `patchiness` on the strength of this range being
        // exactly what it says, and a tile's ceiling a hair below zero is a tile that
        // drains into an account that does not exist.
        between(above, below, ease(down)).clamp(-1.0, 1.0)
    }
}

/// Which of a row (or column) of tiles a world coordinate falls in.
///
/// `per_unit` is how many tiles there are per world unit, worked out once when the grid is
/// built, and `last` is the index of the final tile. Multiplying by a precomputed scale
/// rather than dividing by a tile's width is what `physics.rs` does to place a cell in its
/// bucket, and for the same reason: the tiles then divide the world exactly, so the join
/// where the world wraps falls precisely on a tile boundary rather than a hair either side of
/// one.
///
/// The clamp is not defensive tidying. A cell resting exactly on the floor has `y` equal to
/// the world's height, which multiplied out lands one past the last row; without the clamp
/// that is an index off the end of the array. It also makes the conversion below total, which
/// is what allows it to be a conversion at all.
fn tile_along(coordinate: f32, per_unit: f64, last: f64) -> usize {
    let exact = f64::from(coordinate) * per_unit;

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value has been rounded down to a whole number and then clamped \
                  between nought and the index of the last tile, so the conversion is \
                  exact and in range for any coordinate whatever"
    )]
    let tile = exact.floor().clamp(0.0, last) as usize;

    tile
}

/// A grid of energy laid over the world, and everything needed to grow it back.
pub struct Grid {
    cols: usize,
    rows: usize,

    /// Tiles per world unit, across and down, and the index of the last tile in a row and in
    /// a column. Precomputed so that finding the tile under a cell is a multiplication rather
    /// than a division.
    across_per_unit: f64,
    down_per_unit: f64,
    last_col: f64,
    last_row: f64,

    /// How much energy each tile is holding now.
    tiles: Vec<f32>,

    /// The most each tile can hold, which never changes.
    targets: Vec<f32>,

    /// How much energy the light offers each tile of a row per tick, which never changes.
    regrowth: Vec<f32>,

    /// What each tile has gained or lost to its neighbours and not yet managed to hold.
    ///
    /// Never cleared. Whatever is too small to write into a tile stays here and joins next
    /// tick's movements, so nothing is ever rounded away to nothing - which means a little
    /// of the world's energy is always in transit here rather than in `tiles`. See
    /// [`Grid::diffuse`].
    flux: Vec<f32>,

    /// How readily energy spreads between neighbouring tiles.
    diffusion: f32,

    /// The lattice of blotches, worked out from the world's seed and its shape - both of
    /// which SPEC section 3 locks at run start, so this is built once and never rebuilt.
    ///
    /// ⚠️ It is deliberately **not** rebuilt by [`Grid::relight`]. The pattern of the
    /// blotches is a fact about the seed; what a live change to `[light]` may move is how
    /// deep they are and how fast they travel, and nothing else. A retune that re-drew the
    /// pattern would rearrange the whole field under a running population.
    noise: PatchNoise,

    /// What the light alone would let each *row* hold: `cap × light_profile(row)`, before the
    /// blotches are laid over it.
    ///
    /// Kept at full width rather than narrowed, because it is a step in an arithmetic rather
    /// than a quantity anything stores: rounding here and again at the end would round twice.
    /// One number per row, so it is a few hundred bytes.
    ceilings: Vec<f64>,

    /// How deep the blotches are, `light.patchiness`, at the width the arithmetic runs at.
    patchiness: f64,

    /// SPEC.s `light.bloom`: how much of the light.s SUPPLY follows the drifting noise, rather
    /// than only its ceiling. See [`Grid::regrow`].
    bloom: f64,

    /// How far the blotches move each tick, in *lattice cells*.
    ///
    /// `light.patch_drift` is in world units per tick, which is what a person can compare
    /// against a swimming speed. This is that number converted once into the coordinates the
    /// noise is actually read in, because the conversion is four multiplications and the
    /// alternative is doing them 36,864 times per retarget.
    drift_per_tick: f64,

    /// How far the blotches have moved so far, in lattice cells, wrapped.
    ///
    /// ⚠️ **Accumulated rather than worked out from a tick count**, and the difference shows
    /// up exactly once: `[light]` is live, so somebody can turn `patch_drift` up mid-run, and
    /// a `drift × ticks` offset would jump the whole field sideways the instant they did.
    /// Adding a step per tick means a change of drift changes the *speed* and nothing else,
    /// which is what a person turning a dial called "how fast the bright water moves" means.
    drifted: f64,

    /// How many ticks since the ceilings were last worked out. See [`RETARGET_EVERY`].
    since_retarget: u64,

    /// How deep the season runs, `light.season_amplitude`, at the width the arithmetic runs at.
    season_amplitude: f64,

    /// How far through its season one tick takes the world: `1 / light.season_period`.
    ///
    /// Kept as the step rather than as the period for the reason [`Self::drifted`] is
    /// accumulated rather than derived - see [`Self::season_phase`].
    season_step: f64,

    /// Where in its season the world is, between nought and one.
    ///
    /// ⚠️ **Accumulated rather than worked out from a tick count**, exactly as [`Self::drifted`]
    /// is and for exactly the same reason: `[light]` is live, so somebody can turn
    /// `season_period` up mid-run, and a `ticks / period` phase would teleport a living world
    /// into a different part of its year the instant they did. Adding a step per tick means a
    /// change of period changes the *speed* and nothing else.
    season_phase: f64,

    /// Whether the season has started at all.
    ///
    /// ⚠️ **It is held at nought until the first organism is ever seeded**, and that is not
    /// tidiness. `founding.rs` ends its dawn on `gained / after < DAWN_SETTLED` - a
    /// **light-dependent** test - so a season running through the dawn changes the dawn's
    /// *length*, and a seasoned run and a flat run then start their clocks at different ticks
    /// against different fields. It would also land every founder in this project's history at
    /// 0.83x and falling towards the trough 1.6 generations later, when every survival figure
    /// ever measured here was taken on level light.
    seasoned: bool,
}

/// How many ticks pass between two recomputations of the tiles' ceilings.
///
/// The drift is continuous and the field is a grid, so the ceilings have to be re-read off
/// the moved lattice every so often, and this is how often. At the shipped `patch_drift` of
/// 0.0006 world units a tick, a hundred ticks is **six hundredths of a world unit** - a
/// hundred and thirtieth of a tile's width - so what a tile's ceiling does over a run is a
/// smooth slide with steps far below the resolution of the field it is a ceiling for.
///
/// ⭐ **It does not change what the drift costs**, which is the thing worth knowing about it.
/// The energy a drifting ceiling sheds per retarget is proportional to how far the field moved
/// since the last one, and the retargets happen in inverse proportion to the same interval, so
/// the loss per tick is the same at every setting of this number. What it buys is smoothness,
/// and what it costs is the recomputation: 36,864 tiles of four hashed lattice corners each,
/// which amortised over a hundred ticks is about fifteen hundred hashes a tick against a
/// quarter of a million cells of physics.
const RETARGET_EVERY: u64 = 100;

impl Grid {
    /// Build the field a configuration describes.
    ///
    /// Everything here is allocated at exactly the size asked for and never resized
    /// afterwards. CLAUDE.md: a simulation that cannot allocate cannot leak.
    ///
    /// The world starts empty and fills under the light, which takes `cap / influx` ticks -
    /// a little under seven hundred at SPEC's defaults, and the same number at every depth,
    /// because a tile's ceiling and the light it receives are scaled by the same profile.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let cols_across = config.world.grid_cols.get();
        let rows_down = config.world.grid_rows.get();
        let cols = usize::try_from(cols_across).expect("a grid dimension fits in a machine word");
        let rows = usize::try_from(rows_down).expect("a grid dimension fits in a machine word");
        let tile_count = cols * rows;

        let mut grid = Self {
            cols,
            rows,
            across_per_unit: f64::from(cols_across) / f64::from(config.world.width),
            down_per_unit: f64::from(rows_down) / f64::from(config.world.height),
            last_col: f64::from(cols_across - 1),
            last_row: f64::from(rows_down - 1),
            tiles: vec![0.0; tile_count],
            targets: vec![0.0; tile_count],
            regrowth: vec![0.0; rows],
            flux: vec![0.0; tile_count],
            diffusion: 0.0,
            noise: PatchNoise::new(config.world.seed, cols_across, rows_down),
            ceilings: vec![0.0; rows],
            patchiness: 0.0,
            bloom: 0.0,
            drift_per_tick: 0.0,
            drifted: 0.0,
            since_retarget: 0,
            season_amplitude: 0.0,
            season_step: 0.0,
            season_phase: 0.0,
            seasoned: false,
        };
        grid.relight(config);

        grid
    }

    /// Work out what the light does to this field, from a configuration's `[light]` table.
    ///
    /// ⭐ **Called by [`Grid::new`] and by nothing else until Phase 6, and that is the point of
    /// it being a method rather than four lines inside the constructor.** SPEC section 3:
    /// *"`[world]`, `[limits]` and `seed` lock at run start; the rest can be changed live, which
    /// is how environmental events work."* `[light]` is not locked, so a run can be dimmed while
    /// it is going - and what a tile's ceiling is, and what the light offers a row, are both
    /// worked out *once* from `cap`, `gradient`, `patchiness` and `influx` rather than on every
    /// tick. A live change therefore has to reach these two tables, and the only way to be sure
    /// it computes them the same way the constructor did is for there to be one computation.
    ///
    /// ⚠️ **Nothing here is allocated and nothing here is resized.** Every vector is already the
    /// size the grid's own dimensions imply, and those come from `[world]`, which is locked. So
    /// this is a live change that cannot violate CLAUDE.md's *"allocate once, never grow"*.
    ///
    /// ⚠️ **The tiles are left holding whatever they were holding.** A ceiling lowered under a
    /// full field leaves every tile above its target, which is a state SPEC section 4 already
    /// has an answer for: [`Grid::spill`] moves the excess `field → dissipated` at the end of the
    /// tick it is noticed on. So the energy that no longer fits is *accounted for* rather than
    /// deleted, and the ledger goes on balancing across the change. Clamping the tiles here
    /// instead would destroy that energy with no account to put it in, which is the exact
    /// failure SPEC section 4's opening paragraphs are about.
    pub fn relight(&mut self, config: &Config) {
        let light = &config.light;
        let rows_down = config.world.grid_rows.get();

        for (row, offered) in self.regrowth.iter_mut().enumerate() {
            let row = u32::try_from(row).expect("a grid row fits in the type it was counted in");
            *offered =
                narrowed(f64::from(light.influx) * light_profile(row, rows_down, light.gradient));
        }

        for (row, ceiling) in self.ceilings.iter_mut().enumerate() {
            let row = u32::try_from(row).expect("a grid row fits in the type it was counted in");
            *ceiling = f64::from(light.cap) * light_profile(row, rows_down, light.gradient);
        }

        self.patchiness = f64::from(light.patchiness);
        self.bloom = f64::from(light.bloom);

        // `light.patch_drift` is world units per tick, because that is what a person can hold
        // beside a swimming speed. The noise is read in lattice cells, so the conversion
        // happens once here rather than 36,864 times per retarget: world units to tiles, then
        // tiles to lattice cells.
        self.drift_per_tick = f64::from(light.patch_drift)
            * self.across_per_unit
            * f64::from(self.noise.lattice_cols)
            / f64::from(self.noise.cols);

        self.diffusion = light.diffusion;

        self.season_amplitude = f64::from(light.season_amplitude);
        // A period is a count of ticks and the gate refuses anything below `SEASON_PERIOD_FLOOR`,
        // so this division cannot be by nought.
        #[expect(
            clippy::cast_precision_loss,
            reason = "a season's period is a count of ticks bounded below at eight thousand and \
                      a run is not nine quadrillion ticks long; every period a person can write \
                      is exact as a 64-bit float"
        )]
        let period = light.season_period as f64;
        self.season_step = 1.0 / period;

        // ⚠️ `season_phase` is deliberately left where it is, for the reason `drifted` below is:
        // turning a dial changes how fast the seasons run and not where in one the world is.
        // See `retuning_the_season_changes_its_speed_and_not_where_it_is`.

        // ⚠️ `drifted` is deliberately left where it is. A live change to `[light]` moves how
        // fast the blotches travel and how deep they are; it does not move them, and a retune
        // that reset the offset would jump the whole field sideways under a living population
        // - which is the reseed this design exists to avoid, arrived at through the settings
        // panel.
        self.retarget();
    }

    /// Read every tile's ceiling off the lattice as it now stands.
    ///
    /// ⭐ **The one place a target is ever computed**, which is the point of it being a
    /// method. SPEC section 3 makes `[light]` live, so a person can dim the world mid-run;
    /// SPEC section 4 makes the blotches move, so the world dims itself a little every
    /// hundred ticks. Those are two callers for one piece of arithmetic, and the only way to
    /// be sure the drift computes a ceiling the same way a retune does is for there to be one
    /// computation. A second copy would be a world whose ceilings changed shape the first time
    /// somebody touched a slider.
    ///
    /// ⚠️ **The tiles are left holding whatever they are holding.** A ceiling that has just
    /// slid down under a full tile leaves that tile above its target, which is a state SPEC
    /// section 4 already has an answer for: [`Grid::spill`] moves the excess `field →
    /// dissipated` at the end of the same tick. So the energy a drifting ceiling takes out of
    /// the world is *accounted for* rather than deleted, and the ledger goes on balancing
    /// across every step of the drift. That accounting is also the whole cost of the feature -
    /// see `a_drifting_field_sheds_what_its_ceiling_slides_off`.
    fn retarget(&mut self) {
        let (cols_across, rows_down) = (self.noise.cols, self.noise.rows);
        let (patchiness, drifted) = (self.patchiness, self.drifted);

        for row in 0..rows_down {
            let down = usize::try_from(row).expect("a grid row fits in a machine word");
            let ceiling = self.ceilings[down];

            for col in 0..cols_across {
                let across = usize::try_from(col).expect("a grid column fits in a machine word");
                self.targets[down * self.cols + across] =
                    narrowed(ceiling * (1.0 + patchiness * self.noise.at(col, row, drifted)));
            }
        }

        self.since_retarget = 0;
    }

    /// Which tile of the field a point in the world is standing on.
    ///
    /// The grid is coarser than the world it covers - SPEC section 3's default is a tile
    /// eight world units across, which is a little over one cell-width - so this is the
    /// question "which patch of water is this cell in", and it is asked once per cell of
    /// every organism being seeded, and from Phase 4 once per photocyte per tick.
    #[must_use]
    pub fn tile_at(&self, at: Vec2) -> usize {
        let col = tile_along(at.x, self.across_per_unit, self.last_col);
        let row = tile_along(at.y, self.down_per_unit, self.last_row);

        row * self.cols + col
    }

    /// Take up to `wanted` out of one tile, tell the ledger where it went, and hand back what
    /// the tile actually gave up.
    ///
    /// **The only way energy leaves the field**, and the ledger is an argument rather than
    /// something the caller is trusted to remember afterwards. That is the same decision
    /// [`Grid::tick`] makes about the light and it is the more important of the two: light
    /// credited to nobody would show up immediately as a world inventing energy, while a
    /// harvest debited to nobody looks exactly like a world quietly running out.
    ///
    /// A tile gives up as much as is asked of it or as much as it has, whichever is less. It
    /// is never taken below nothing, so a caller cannot dig a hole in the water and owe it
    /// back later - which matters because the field is the account nobody keeps a running
    /// total of, and a negative tile would be a debt that only appears when the tiles are
    /// next added up.
    ///
    /// # What the ledger is told, and why it is not what was asked for
    ///
    /// The realised change, measured by widening both readings of the tile and subtracting -
    /// the same arithmetic, in the same order, as [`Grid::regrow`] and [`Grid::spill`], and
    /// for the same reason. A tile is a 32-bit number and the amount wanted is not, so
    /// subtracting a quantity from a tile does not lower it by exactly that quantity. Telling
    /// the ledger the intention rather than the outcome would leave the accounts a hair away
    /// from the grid on every harvest for ever, and SPEC section 5's tolerance would absorb
    /// the difference and hide it.
    ///
    /// The caller should treat what comes back as the truth about how much it got. Phase 2's
    /// second design decision in `docs/PHASE2.md` is this same rule stated once for the whole
    /// project: record what actually happened, not what was intended.
    ///
    /// # Panics
    ///
    /// If there is no such tile in this grid, which means a caller has worked out a tile
    /// index some way other than [`Grid::tile_at`].
    pub fn harvest(&mut self, ledger: &mut Ledger, tile: usize, wanted: f64) -> f64 {
        let before = self.tiles[tile];
        let taken = narrowed(wanted.clamp(0.0, f64::from(before)));

        // The floor is against the narrowing rather than against the caller: rounding a
        // quantity into a tile-sized number can round it *up*, and a tile that gave up a
        // fraction more than it held would be a tile holding a negative amount of energy.
        let after = (before - taken).max(0.0);
        self.tiles[tile] = after;

        let realised = f64::from(before) - f64::from(after);
        ledger.harvest(realised);

        realised
    }

    /// Put `amount` into one tile, tell the ledger where it came from, and hand back what
    /// the tile actually took.
    ///
    /// [`Grid::harvest`] backwards, and it exists for the one thing that puts energy back
    /// into the water rather than taking it out: a grain of detritus rotting into the tile it
    /// is lying in. SPEC section 5 lists that movement as `detritus → field` and this is the
    /// only door it goes through, so a grain cannot lose energy without the books being told
    /// where it went.
    ///
    /// # Why what comes back can be less than what was offered
    ///
    /// Because a tile is a 32-bit number. Adding a thousandth to a tile already holding eight
    /// moves it by whatever the nearest number it can hold happens to be, and if the tile were
    /// large enough - a configuration is free to set a ceiling of a million - the addition
    /// would round away to nothing at all. The caller is handed the *realised* change and is
    /// expected to treat it as the truth: a grain keeps whatever the water would not take, and
    /// tries again next tick. The alternative is a grain that believes it has unloaded energy
    /// no tile ever received, which is a leak of exactly the kind SPEC section 5 says the
    /// invariant cannot see.
    ///
    /// # Nothing here defends the tile's ceiling, deliberately
    ///
    /// A grain will happily rot into a tile that is already full, and the tile is left holding
    /// more than it can. SPEC section 4 anticipates exactly this - *"from Phase 4 detritus
    /// will decay into already-full tiles"* - and gives the answer: the ceiling is enforced
    /// once, by [`Grid::spill`], after everything that moves energy has finished moving it.
    /// The excess leaves the world on the next tick, through an account, which is the
    /// biological pump doing what it does.
    ///
    /// # Panics
    ///
    /// If there is no such tile in this grid, which means a caller has worked out a tile
    /// index some way other than [`Grid::tile_at`].
    pub fn deposit(&mut self, ledger: &mut Ledger, tile: usize, amount: f64) -> f64 {
        let before = self.tiles[tile];
        let after = before + narrowed(amount);
        self.tiles[tile] = after;

        // Both readings widened and *then* subtracted, as in `regrow`, `spill` and `harvest`,
        // and for the same reason: the difference between two 32-bit numbers is not always a
        // 32-bit number, and this is the quantity that has to make the ledger agree with the
        // grid rather than nearly agree.
        let realised = f64::from(after) - f64::from(before);
        ledger.decay(realised);

        realised
    }

    /// How many tiles across the world is.
    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// How many tiles deep the world is.
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Every tile, in row order, the surface first.
    ///
    /// Handed out whole rather than a tile at a time because everything that will ever want
    /// this wants all of it: SPEC section 2's determinism test compares two runs of the same
    /// seed tile by tile, and Phase 5's renderer hands the lot to the graphics card in one
    /// go. A shared slice, so nothing outside this module can write a tile - the only ways
    /// energy enters or leaves the field are the ones that tell the ledger what happened.
    #[must_use]
    pub fn tiles(&self) -> &[f32] {
        &self.tiles
    }

    /// Everything the grid is holding, added up.
    ///
    /// This is SPEC section 5's `field` account, and it is deliberately measured rather
    /// than stored. A `field` number kept alongside the grid would be a second copy of
    /// something the grid already knows, and keeping two copies of one quantity in step is
    /// exactly the bookkeeping that goes wrong; a measured field cannot disagree with the
    /// grid, because it is the grid. See `ledger.rs`.
    ///
    /// Added up at 64 bits, which SPEC section 5 requires by name. Thirty-six thousand
    /// tiles of eight units each is a total large enough that a 32-bit accumulator would
    /// stop being able to see the tiles at the far end of it.
    #[must_use]
    pub fn total_energy(&self) -> f64 {
        self.tiles.iter().copied().map(f64::from).sum()
    }

    /// Move the field on by one tick.
    ///
    /// The light falls and then the energy spreads, in that order, which is the order SPEC
    /// section 4 gives them.
    ///
    /// The ledger is passed in rather than the light's contribution being handed back,
    /// because handing it back would make crediting it something a caller could forget to
    /// do. Forgetting would not be caught by anything until the invariant failed, at which
    /// point the world would appear to be inventing energy from nowhere; and the one thing
    /// that must be impossible in this project is energy appearing from nowhere.
    ///
    /// Diffusion tells the ledger nothing, and that is correct rather than an omission: it
    /// moves energy between tiles, and both of those tiles are the field. What it costs the
    /// world is the rounding, which is the drift SPEC section 5's tolerance is for.
    ///
    /// What diffusion *does* need is somebody to clear up after it, which is the third
    /// line. It has no idea what a tile's ceiling is and will happily leave a tile holding
    /// several times what it can, so the ceiling is enforced once, after the energy has
    /// finished moving, rather than being defended by every operation that touches a tile.
    pub fn tick(&mut self, ledger: &mut Ledger) {
        self.turn();
        self.drift();
        ledger.light(self.regrow());
        self.diffuse();
        ledger.overflow(self.spill());
    }

    /// Start the seasons.
    ///
    /// ⭐ **Called by [`crate::world::World::seed`] and by nothing else.** SPEC section 4's
    /// season is a fact about a world with something living in it: the dawn that fills the field
    /// ends on a light-dependent test, so a season running through it would change how long the
    /// dawn takes and a seasoned run and a flat run would begin at different ticks against
    /// different fields. Idempotent, because a world is seeded many times and there is one
    /// moment the clock starts.
    pub(crate) fn begin_season(&mut self) {
        self.seasoned = true;
    }

    /// Where in its season the world is, between nought and one: nought and a half are the mean,
    /// a quarter is the brightest and three quarters the dimmest.
    ///
    /// ⭐ **The phase, and deliberately not the multiplier.** The multiplier is two-to-one over a
    /// cycle - 1.0 happens twice, once on the way up and once on the way down - so it cannot say
    /// whether the light is rising or falling, and a *lag* between the light and what lives under
    /// it is the whole of what the season was built to make testable.
    #[must_use]
    pub const fn season_phase(&self) -> f64 {
        self.season_phase
    }

    /// Move the world on through its season by one tick.
    ///
    /// Held at nought until something is alive to feel it - see [`Self::seasoned`] - and
    /// wrapped every tick rather than allowed to grow, exactly as [`Self::drift`] wraps its own
    /// offset, so the arithmetic keeps every digit it started with however long a run goes on.
    fn turn(&mut self) {
        if self.seasoned {
            self.season_phase = (self.season_phase + self.season_step).rem_euclid(1.0);
        }
    }

    /// What this tick multiplies [`crate::config::LightConfig::influx`] by.
    ///
    /// ⭐⭐ **Computed unconditionally, and there is no branch here for a reason.**
    /// `1 + amplitude × triangle(phase)` is **exactly** 1.0 when the amplitude is nought, and
    /// `f64::from(x) * 1.0` narrows back to `x` for every `x` there is - so a world with no
    /// season is bit-for-bit the world that was there before one existed, without anything
    /// having to remember to skip the arithmetic. An early return here is the most dangerous
    /// thing this feature could contain: it would leave the multiplier frozen wherever the
    /// season was when the amplitude dial reached nought, and the world would run permanently at
    /// up to 1.25x its stated influx with `season_amplitude = 0` written in the config file, in
    /// the panel and in the replay log, while SPEC section 5's ledger balanced to the last digit
    /// and said nothing.
    ///
    /// # Why a triangle rather than a sine
    ///
    /// One scalar multiplies all 36,864 tiles every tick, which is **36,864 times** the
    /// golden-vector exposure that `behaviour.rs`'s one-spring `sin` carries. A triangle is
    /// exact in 64-bit arithmetic at every phase, needs no library function, is mean-preserving
    /// over a period by exact antisymmetry rather than by cancellation, is exactly 1.0 at an
    /// amplitude of nought, and reads identically to a sine at plus or minus a quarter.
    fn season(&self) -> f64 {
        // The triangle: nought at the start, +1 a quarter of the way through, nought at the
        // half, −1 three quarters through. Every coefficient is a small power of two, so the
        // whole of this is exact.
        let wave = if self.season_phase < 0.25 {
            4.0 * self.season_phase
        } else if self.season_phase < 0.75 {
            2.0 - 4.0 * self.season_phase
        } else {
            4.0 * self.season_phase - 4.0
        };

        self.season_amplitude.mul_add(wave, 1.0)
    }

    /// Move the blotches on by one tick, and re-read the ceilings if they have moved far
    /// enough to be worth re-reading.
    ///
    /// SPEC section 4's drifting field. The offset advances every tick and the *targets* are
    /// recomputed every [`RETARGET_EVERY`] of them, which is the same slide sampled coarsely
    /// enough to be cheap and far more finely than a tile is wide.
    ///
    /// It runs first in the tick, before the light falls, so that a ceiling which has just
    /// come down under a full tile is the ceiling that tick's [`Grid::spill`] measures
    /// against, rather than the excess sitting in the field for a hundred ticks with the
    /// books disagreeing with the grid about whether it is there.
    ///
    /// A world told not to drift does nothing here at all: no offset moves, no target is
    /// recomputed, and the ceilings stay bit for bit the numbers [`Grid::relight`] left. That
    /// is what makes `light.patch_drift = 0` the control experiment for every claim about a
    /// drifting field rather than an approximation to one.
    fn drift(&mut self) {
        if self.drift_per_tick <= 0.0 {
            return;
        }

        // Wrapped every tick rather than allowed to grow, so the offset stays a small number
        // however long a run goes on and the arithmetic keeps every digit it started with. The
        // lattice wraps sideways, so this loses nothing whatever.
        let span = f64::from(self.noise.lattice_cols);
        self.drifted = (self.drifted + self.drift_per_tick).rem_euclid(span);
        self.since_retarget += 1;

        if self.since_retarget >= RETARGET_EVERY {
            self.retarget();
        }
    }

    /// Let the light fall for one tick, and report what it actually put into the world.
    ///
    /// SPEC section 4: each tile is offered `influx × light_profile(y)` and takes as much
    /// of it as fits under its ceiling.
    ///
    /// What comes back is the *realised* change - what the tiles gained, not what they were
    /// offered - and it is the number the ledger has to be credited with. See
    /// `regrowth_credits_influx_with_the_realised_change` for why the difference between
    /// those two is the phase's central failure mode.
    ///
    /// Note the order of the arithmetic in the loop's last line. The two tile readings are
    /// widened to 64 bits and *then* subtracted, rather than subtracted and then widened.
    /// The difference between two 32-bit numbers is not always a 32-bit number, so doing it
    /// the other way round rounds the answer - and this is the one quantity in the whole
    /// simulation that has to be exact, since it is what makes the influx account agree
    /// with the grid rather than merely nearly agree.
    ///
    /// # The light cannot take energy away
    ///
    /// A tile already over its ceiling is left where it is. SPEC section 4's formula, read
    /// literally, drags it back down - and the energy that removes is not moved anywhere,
    /// so the only way to keep the books balanced is to tell the ledger the light
    /// contributed a negative amount, which is a way of destroying energy that no invariant
    /// can see. `light_never_runs_backwards` is where that argument is written out in full.
    fn regrow(&mut self) -> f64 {
        let mut realised = 0.0;
        // ⭐ **SPEC section 4's season, and it is this one multiplication.** It is here rather
        // than in `relight` because `relight` is where a *ceiling* is worked out, and the whole
        // energy argument for a season on `influx` is that `influx` enters no ceiling: no
        // retarget, no target moving down under a full tile, no spill shed to `dissipated`. See
        // [`Grid::season`] and `a_season_moves_no_ceiling`.
        let season = self.season();
        let (bloom, drifted) = (self.bloom, self.drifted);

        for row in 0..self.rows {
            let offered = f64::from(self.regrowth[row]) * season;
            let down = u32::try_from(row).expect("a grid row fits in the type it was counted in");

            for col in 0..self.noise.cols {
                let across = usize::try_from(col).expect("a grid column fits in a machine word");
                let tile = row * self.cols + across;

                // ⭐⭐⭐ **Where the light ARRIVES, not just how full a tile may get.**
                //
                // ⚠️ Until this existed, `regrowth` was indexed by ROW ALONE: every tile at a
                // given depth was offered exactly the same light, and all of `light.patchiness`
                // went into the CEILING. Since the field runs about 65% drawn down, almost
                // nothing is ever near its ceiling, so the patchiness was very nearly invisible
                // in what the water actually held. **The supply had no horizontal structure at
                // all**, which is why twelve rounds of spatial levers -- shadow spread, currents,
                // diffusion, uptake, dispersal, density -- all came back null: there was nothing
                // spatial there to exploit.
                //
                // `light.bloom` moves that patchiness onto the supply. The noise is the same
                // field the ceilings use and it already drifts with `light.patch_drift`, so the
                // good water both varies and MOVES -- which is the one thing an ideal free
                // distribution cannot flatten, because a population can never finish settling
                // onto it.
                //
                // ⚠️ It is centred, not scaled: `noise.at` is mean-zero across the field, so the
                // row.s total offer is conserved and this moves light about rather than making
                // any. Clamped at nought because a tile cannot be offered negative light.
                let lit = if bloom > 0.0 {
                    (offered * (1.0 + bloom * self.noise.at(col, down, drifted))).max(0.0)
                } else {
                    offered
                };
                let lit = narrowed(lit);

                let before = self.tiles[tile];
                let after = (before + lit).min(self.targets[tile]).max(before);
                self.tiles[tile] = after;
                realised += f64::from(after) - f64::from(before);
            }
        }

        realised
    }

    /// Let energy spread sideways for one tick.
    ///
    /// SPEC section 4's five-point stencil, written as a list of *pairs of neighbours*
    /// rather than as a sum over each tile's surroundings. The difference is the whole
    /// reason `diffusion_does_not_leak_at_the_edges` passes.
    ///
    /// Written the usual way - add up my four neighbours, subtract four times me - the
    /// edges of the world become a special case that has to be remembered, and the way it
    /// is usually forgotten is to reach past the edge, find nothing, and treat the nothing
    /// as a tile holding zero. Every tile on the floor then spends every tick giving a
    /// share of itself to something that does not exist, and the world's energy drains out
    /// through its own floor. It looks like an ecology that is a little too hungry.
    ///
    /// Here there is nothing to remember. A pair of neighbours is a pair or it is not: what
    /// leaves one tile is the same number that arrives at the other, applied with opposite
    /// signs in the same breath, exactly as `ledger.rs` moves energy between accounts. A
    /// tile at the world's edge does not need to know it is at the edge - it simply appears
    /// in fewer pairs. There is no route out of the world to write down, and therefore none
    /// to write down wrongly.
    ///
    /// The movements are collected in `flux` and applied at the end rather than written
    /// straight into the tiles. That serves two purposes. Every pair sees the field as it
    /// was at the start of the tick, so the result does not depend on the order the pairs
    /// are visited in; and each tile's gains and losses are added up at their own small
    /// size and only then added to the tile, which rounds once rather than four times, and
    /// rounds a great deal less.
    ///
    /// # What will not fit in a tile is kept rather than dropped
    ///
    /// A tile holds about seven digits. Once the field is anywhere near full, a tile is
    /// holding around eight units and the movements between neighbours are millionths, so
    /// writing a movement into a tile rounds it - and quite often rounds it away
    /// altogether, because it is smaller than the smallest change that tile can express.
    ///
    /// Dropped, those roundings do not cancel out. They are systematically a loss, because
    /// of the shape of the field rather than any bias in the arithmetic: a handful of tiles
    /// lose a large amount each, which gets written down faithfully, while the many tiles
    /// that each gain a little have their gains rounded to nothing. Measured over the
    /// default world, that alone cost 67 units by tick 100,000 - within SPEC section 5's
    /// tolerance, but growing, and an overnight run is tens of millions of ticks. The run
    /// would have stopped with a conservation failure and there would have been no bug to
    /// find.
    ///
    /// So the part of a movement that will not fit in a tile is not discarded: it is left
    /// in `flux`, and next tick's movements are added on top of it. A millionth that cannot
    /// be written this tick is written on the tick when enough of them have piled up
    /// together. Energy is then never lost, only ever *in transit* - and what is in transit
    /// is at most half a rounding step per tile, which for the default world is around two
    /// hundredths of a unit in total, and does not grow however long the run goes on.
    ///
    /// # This is stable up to a rate of a quarter, and no further
    ///
    /// A tile that gives away a share of its difference to each of four neighbours
    /// overshoots once that share passes a quarter: it hands over more than the difference,
    /// so it is sent past the neighbours it was levelling with and has to be dragged back
    /// further next tick. Above a quarter that overshoot grows every tick, and neighbouring
    /// tiles swap ever larger positive and negative amounts until the numbers stop being
    /// finite.
    ///
    /// Energy is conserved perfectly the whole way down, because every pair still moves one
    /// number both ways - so the ledger reports a healthy world right up until the field is
    /// nonsense, and there is no second line of defence anywhere below this one. That is
    /// why `config.rs` refuses a `diffusion` above `0.25` outright rather than warning
    /// about it, and why the bound there is a stability limit rather than a reading of what
    /// SPEC section 3 calls a "lateral spread per tick". SPEC's default is `0.04`,
    /// comfortably inside.
    fn diffuse(&mut self) {
        let rate = self.diffusion;

        // Note what is *not* here: the flux is not cleared. Whatever was too small to write
        // into a tile last tick is still sitting in it, waiting for company.

        // Every tile is paired with the one to its east, and the last column's eastern
        // neighbour is the first column, because the world wraps.
        for row in 0..self.rows {
            let start = row * self.cols;
            for col in 0..self.cols {
                self.share(start + col, start + (col + 1) % self.cols, rate);
            }
        }

        // Every tile is paired with the one below it, except those on the floor. The loop
        // stopping one row short *is* the closed bottom of the world: there is no pair
        // between the last row and anything, so there is nowhere for its energy to go.
        // `rows` is at least one, so this cannot run off the end.
        for row in 0..self.rows - 1 {
            let start = row * self.cols;
            for col in 0..self.cols {
                self.share(start + col, start + col + self.cols, rate);
            }
        }

        for (tile, moved) in self.tiles.iter_mut().zip(self.flux.iter_mut()) {
            let before = *tile;
            *tile = before + *moved;
            *moved -= *tile - before;
        }
    }

    /// Cut every tile back to its ceiling, and report how much energy that took out of the
    /// world.
    ///
    /// SPEC section 4's second half: a tile genuinely cannot hold more than its target, and
    /// what will not fit is **dissipated** rather than deleted. The caller hands the number
    /// straight to [`Ledger::overflow`], so the excess leaves the field and arrives in an
    /// account, and the invariant can see the whole journey.
    ///
    /// # Why this has to exist at all, given that the light already stops at the ceiling
    ///
    /// Because the light is not the only thing that puts energy into a tile. Diffusion
    /// moves it from wherever there is more to wherever there is less and knows nothing
    /// about ceilings; ceilings fall with depth; so energy is permanently being carried
    /// downwards into tiles with less and less room for it. From Phase 4 detritus rots into
    /// whatever tile is beneath it, full or not.
    ///
    /// Left to accumulate, that has one destination, and it is not obviously wrong until
    /// you look: the field fills from the bottom upwards until it is **level**, every tile
    /// in the world holding the largest ceiling in it. `the_field_reaches_a_ceiling`
    /// recorded exactly that before this existed. A level field is one where being near the
    /// surface is worth nothing, and SPEC section 4 rests the case for having a depth
    /// gradient at all on its being worth something.
    ///
    /// # Where the energy goes, and why that is the honest answer
    ///
    /// Away. Not back up the water column, and not into an adjustment of the ceiling: the
    /// world simply no longer has it. That is the ecologically familiar case rather than a
    /// convenience - energy carried below the depth the light can support is energy the
    /// ocean loses, which is what the biological pump does. Total influx still bounds total
    /// living biomass, so SPEC section 4's carrying-capacity argument is untouched.
    ///
    /// # The arithmetic
    ///
    /// A cut-back tile is set to its ceiling exactly, and what it gave up is measured by
    /// widening both readings and subtracting - the same order as [`Grid::regrow`], and for
    /// the same reason. The difference between two 32-bit numbers is not always a 32-bit
    /// number, so subtracting first and widening afterwards would round the one quantity
    /// that has to be exact if the ledger is to agree with the grid rather than nearly
    /// agree.
    ///
    /// A tile at or below its ceiling is not touched at all. Writing `tile.min(ceiling)`
    /// unconditionally would give the same tiles and read more neatly; it is avoided
    /// because it makes every tile in the world a write on every tick, and because the
    /// version of that mistake which pulls a tile *towards* its ceiling instead of capping
    /// it would drain a world that is merely filling up.
    fn spill(&mut self) -> f64 {
        let mut shed = 0.0;

        for (tile, ceiling) in self.tiles.iter_mut().zip(&self.targets) {
            if *tile > *ceiling {
                shed += f64::from(*tile) - f64::from(*ceiling);
                *tile = *ceiling;
            }
        }

        shed
    }

    /// Move a share of the difference between two neighbouring tiles from the fuller to the
    /// emptier.
    ///
    /// One number, applied to both ends at once, so the amount that leaves one tile cannot
    /// disagree with the amount that reaches the other - not by mistake, and not by
    /// arithmetic either, because negating a number is the one floating-point operation
    /// that is always exact. It is `ledger.rs`'s transfer, one level down.
    fn share(&mut self, here: usize, there: usize, rate: f32) {
        let flow = rate * (self.tiles[here] - self.tiles[there]);
        self.flux[here] -= flow;
        self.flux[there] += flow;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, RawConfig, spec_defaults};
    use crate::ledger::Ledger;
    use proptest::prelude::*;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    ///
    /// Every test here starts from the shipped defaults and alters only what it is about,
    /// so a test that talks about depth is visibly not also talking about patchiness.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// SPEC section 4's `light_profile(y) = 1 - gradient × (y / height)`, written out here
    /// a second time from the specification rather than borrowed from the module above.
    ///
    /// A test that calls the code it is testing is a test that agrees with whatever that
    /// code does. This is the formula as SPEC states it, with `y` the depth of the middle
    /// of the row's band and `height` the depth of the whole world - so what it returns can
    /// disagree with what the module computes, which is the entire point of it.
    fn spec_light_profile(row: u32, rows: u32, gradient: f64) -> f64 {
        1.0 - gradient * ((f64::from(row) + 0.5) / f64::from(rows))
    }

    /// One tile's index into the flat arrays, for a test that wants to reach past the
    /// public surface and look at a target or poke a tile.
    fn at(grid: &Grid, col: usize, row: usize) -> usize {
        row * grid.cols() + col
    }

    /// The grid is the size the configuration asked for, and it is that size once and for
    /// ever.
    ///
    /// CLAUDE.md: "A simulation that cannot allocate cannot leak." Every arena in this
    /// project is built at startup from the configuration and never resized, and the
    /// resource field is the largest of them. So this checks two separate things.
    ///
    /// The first is that the dimensions come from the *configuration* rather than from a
    /// constant somebody wrote down while testing against the defaults - which is why the
    /// second grid here is seven by three, a shape nothing else in the project uses.
    ///
    /// The second is that each array holds exactly as many tiles as there are tiles, with
    /// no spare capacity. A vector with room to grow is a vector that will grow, quietly,
    /// the first time something pushes onto it.
    #[test]
    fn the_grid_is_allocated_once_at_the_size_the_config_asks_for() {
        let default_grid = Grid::new(&config(|_| {}));

        assert_eq!(default_grid.cols(), 256, "SPEC section 3's grid_cols");
        assert_eq!(default_grid.rows(), 144, "SPEC section 3's grid_rows");
        assert_eq!(
            default_grid.tiles.len(),
            36_864,
            "the default world is 256 x 144 tiles"
        );

        let odd_shape = Grid::new(&config(|raw| {
            raw.world.grid_cols = 7;
            raw.world.grid_rows = 3;
        }));

        assert_eq!(odd_shape.cols(), 7, "the grid ignored the configured width");
        assert_eq!(
            odd_shape.rows(),
            3,
            "the grid ignored the configured height"
        );

        for (name, held) in [
            ("tiles", &odd_shape.tiles),
            ("targets", &odd_shape.targets),
            ("flux", &odd_shape.flux),
        ] {
            assert_eq!(held.len(), 21, "the {name} array is not one entry per tile");
            assert_eq!(
                held.capacity(),
                21,
                "the {name} array was allocated with room to grow, and an arena with room \
                 to grow is one that will"
            );
        }

        assert_eq!(
            odd_shape.regrowth.len(),
            3,
            "the regrowth array is not one entry per row of tiles"
        );

        // And it is that size once. A thousand ticks of a running world must not move a
        // single one of the four arrays, which is what would happen the moment anything in
        // here grew one of them: the tiles would be copied to a new address, the old space
        // handed back, and the world would be doing that for ever afterwards on a machine
        // that has other things to be getting on with.
        let mut running = Grid::new(&config(|raw| {
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
        }));
        let mut ledger = Ledger::new(running.total_energy());
        let addresses = [
            running.tiles.as_ptr(),
            running.targets.as_ptr(),
            running.regrowth.as_ptr(),
            running.flux.as_ptr(),
        ];
        let sizes = [
            running.tiles.capacity(),
            running.targets.capacity(),
            running.regrowth.capacity(),
            running.flux.capacity(),
        ];

        for _ in 0..1_000 {
            running.tick(&mut ledger);
        }

        assert_eq!(
            addresses,
            [
                running.tiles.as_ptr(),
                running.targets.as_ptr(),
                running.regrowth.as_ptr(),
                running.flux.as_ptr(),
            ],
            "a thousand ticks moved the world somewhere else in memory, so something in \
             here is allocating as it goes"
        );
        assert_eq!(
            sizes,
            [
                running.tiles.capacity(),
                running.targets.capacity(),
                running.regrowth.capacity(),
                running.flux.capacity(),
            ],
            "a thousand ticks changed how much room the world takes up"
        );
    }

    /// Light is strongest at the surface and weakest at the floor.
    ///
    /// SPEC section 4 is emphatic about why this matters: **the gradient is what gives
    /// movement a reason to exist.** A world lit evenly is a world where every position is
    /// as good as every other, so there is nothing for phototaxis to discover and swimming
    /// costs energy for no return. This one number is what makes the world have a shape.
    ///
    /// Patchiness is turned off throughout, so what is left is depth and nothing else.
    ///
    /// Three claims. That every row is dimmer than the one above it; that the numbers are
    /// SPEC's formula and not merely some falling curve; and that a gradient of nought
    /// gives a world with no depth to it at all, which is what SPEC section 3's "0 =
    /// uniform" promises.
    ///
    /// # Where a row is measured from
    ///
    /// A tile is not a point. It covers a band of depth, and this module samples the light
    /// at the middle of that band, so the top row of the default world comes out at 0.9974
    /// of full brightness rather than exactly 1. The alternative - sampling each row at its
    /// upper edge - makes the surface row exactly as bright as the surface and the floor
    /// row brighter than the floor, which is the whole grid sitting half a tile too
    /// shallow. Measuring from the middle is also the only choice under which the average
    /// over the rows equals the average over the world's actual depth.
    ///
    /// Note what does *not* appear anywhere below: the world's height in world units. SPEC
    /// writes the profile as `y / height` in those units, but the grid's rows divide the
    /// world's depth evenly, so that ratio is exactly `(row + ½) / rows` and the height
    /// cancels out. A resource field of a given shape is lit identically whether the world
    /// it covers is a micrometre deep or a mile.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a gradient of zero must give every row exactly the same light, not \
                  nearly the same; an approximate comparison would pass a world with a \
                  faint gradient in it that nobody asked for"
    )]
    fn light_falls_off_with_depth() {
        let lit = Grid::new(&config(|raw| raw.light.patchiness = 0.0));

        for row in 1..lit.rows() {
            assert!(
                lit.regrowth[row] < lit.regrowth[row - 1],
                "row {row} is lit at least as brightly as the row above it, so the world \
                 has no reason for anything to swim upwards"
            );
            assert!(
                lit.targets[at(&lit, 0, row)] < lit.targets[at(&lit, 0, row - 1)],
                "row {row} can hold at least as much energy as the row above it"
            );
        }

        // SPEC's formula, row by row, for both of the things the profile scales: how much
        // light a tile is offered each tick, and the most it can ever hold.
        for row in 0..144 {
            let profile = spec_light_profile(row, 144, 0.75);
            let row = usize::try_from(row).expect("a row index fits in a machine word");

            let offered = f64::from(lit.regrowth[row]);
            let expected = 0.001 * profile;
            assert!(
                (offered - expected).abs() < expected * 1e-6,
                "row {row} is offered {offered} of light per tick, and SPEC section 4's \
                 influx x light_profile(y) makes it {expected}"
            );

            let ceiling = f64::from(lit.targets[at(&lit, 0, row)]);
            let expected = 8.0 * profile;
            assert!(
                (ceiling - expected).abs() < expected * 1e-6,
                "row {row} can hold {ceiling}, and SPEC section 4's cap x light_profile(y) \
                 makes it {expected}"
            );
        }

        // The surface is nearly at full strength and the floor is at a quarter of it,
        // because the default gradient is 0.75. Written out as the numbers a person can
        // check by eye against SPEC section 3.
        assert!(
            f64::from(lit.regrowth[0]) > 0.001 * 0.997,
            "the surface row is not being lit at nearly full strength"
        );
        assert!(
            f64::from(lit.regrowth[143]) < 0.001 * 0.253,
            "the floor row is being lit more brightly than a gradient of 0.75 allows"
        );

        // A gradient of one is SPEC section 3's "fully top-weighted": the floor is as dark
        // as the world can make it. Dark, and not *unlit* - a negative influx would be
        // tiles draining into no account at all, which is the energy ledger ceasing to
        // balance.
        let steep = Grid::new(&config(|raw| {
            raw.light.gradient = 1.0;
            raw.light.patchiness = 0.0;
        }));
        assert!(
            steep.regrowth[143] > 0.0,
            "the floor of a fully top-weighted world is being lit backwards"
        );
        assert!(
            f64::from(steep.regrowth[143]) < 0.001 * 0.004,
            "a fully top-weighted world's floor is not dark"
        );

        // And a gradient of nought is SPEC section 3's "uniform": no depth at all.
        let flat = Grid::new(&config(|raw| {
            raw.light.gradient = 0.0;
            raw.light.patchiness = 0.0;
        }));
        for row in 0..flat.rows() {
            assert_eq!(
                flat.regrowth[row], 0.001,
                "a world with no gradient is lit unevenly at row {row}"
            );
            assert_eq!(
                flat.targets[at(&flat, 0, row)],
                8.0,
                "a world with no gradient holds different amounts at different depths, at \
                 row {row}"
            );
        }
    }

    /// The blotchiness of the light is a fixed feature of a run, not something that
    /// happens to it.
    ///
    /// SPEC section 4 multiplies each tile's ceiling by `1 + patchiness × noise(x, y)` and
    /// says nothing whatever about which noise. This is the answer to that question -
    /// Phase 2's open question Q7 - and the answer has to satisfy one hard constraint
    /// before anything about how it looks: **it is a pure function of the world's seed and
    /// the tile's coordinates, worked out once when the world is built.**
    ///
    /// The alternative, drawing the numbers from the world's own generator, is the obvious
    /// thing to do and is wrong in a way that would take a long time to find. Those draws
    /// come off a shared sequence, so the shape of the light would depend on how many
    /// numbers had been taken before the field was built - which makes it depend on the
    /// order things were evaluated in, on how many organisms were seeded first, and on
    /// every later change to any code that draws a number. The world's blotches would move
    /// because something unrelated changed, and there would be nothing to see except a run
    /// that no longer matched its recording.
    ///
    /// Four claims here, and the last is the one that makes the others mean something.
    ///
    /// Same seed, same blotches - twice over, from two separately-built worlds. A
    /// different seed puts them somewhere else, so the seed genuinely reaches this.
    /// Neither the brightness of the light nor its gradient moves them, so the noise is a
    /// function of position and the seed and of nothing else. And the world's ceilings
    /// really are built out of it, so all of the above is a statement about the world
    /// rather than about an unused function.
    #[test]
    fn patchiness_is_stable_across_runs() {
        let shape = |raw: &mut RawConfig| {
            raw.world.grid_cols = 96;
            raw.world.grid_rows = 64;
        };

        let once = PatchNoise::new(42, 96, 64);
        let again = PatchNoise::new(42, 96, 64);
        let elsewhere = PatchNoise::new(43, 96, 64);

        let mut moved_with_the_seed = 0;
        for row in 0..64 {
            for col in 0..96 {
                assert!(
                    (once.at(col, row, 0.0) - again.at(col, row, 0.0)).abs() < f64::EPSILON,
                    "the same seed put the light's blotches in two different places at \
                     ({col}, {row}), so no run can ever be replayed"
                );
                if (once.at(col, row, 0.0) - elsewhere.at(col, row, 0.0)).abs() > 1e-6 {
                    moved_with_the_seed += 1;
                }
            }
        }
        assert!(
            moved_with_the_seed > 96 * 64 / 2,
            "changing the seed moved the light's blotches on only {moved_with_the_seed} of \
             the 6144 tiles, so the seed barely reaches the shape of the world"
        );

        // Now the same claim about the world that gets built out of it. Two worlds from one
        // configuration hold identical ceilings, to the last bit - a tolerance here would
        // let a run drift away from its own recording by a hair a tick.
        let world = Grid::new(&config(shape));
        let same_world = Grid::new(&config(shape));
        assert!(
            world
                .targets
                .iter()
                .zip(&same_world.targets)
                .all(|(here, there)| here.to_bits() == there.to_bits()),
            "two worlds built from one configuration do not hold the same energy"
        );

        // The ceilings are SPEC section 4's formula with this noise in it, and not merely
        // some other blotchy thing.
        for row in 0..64u32 {
            let profile = spec_light_profile(row, 64, 0.75);
            for col in 0..96u32 {
                let expected = 8.0 * profile * (1.0 + 0.5 * once.at(col, row, 0.0));
                let held = f64::from(
                    world.targets[at(
                        &world,
                        usize::try_from(col).expect("a column index fits in a machine word"),
                        usize::try_from(row).expect("a row index fits in a machine word"),
                    )],
                );
                assert!(
                    (held - expected).abs() < expected.abs() * 1e-6,
                    "the tile at ({col}, {row}) can hold {held}, and SPEC section 4's \
                     cap x light_profile(y) x (1 + patchiness x noise) makes it {expected}"
                );
            }
        }

        // And nothing about the light except the seed and the shape decides where the
        // blotches are. A world lit twice as brightly has the same blotches, twice as
        // bright.
        let brighter = Grid::new(&config(|raw| {
            shape(raw);
            raw.light.cap = 16.0;
        }));
        for (dim, bright) in world.targets.iter().zip(&brighter.targets) {
            assert!(
                (f64::from(*bright) - 2.0 * f64::from(*dim)).abs() < 1e-5,
                "turning up the light moved the blotches rather than brightening them"
            );
        }
    }

    /// ⭐⭐ The blotches of light **slide sideways** as the run goes on, and the field that
    /// arrives is the one that left rather than a new one.
    ///
    /// This is the whole of SPEC section 4's drifting field, and both halves of the sentence
    /// are load-bearing.
    ///
    /// # Why it moves at all
    ///
    /// Phase 7 made swimming possible and a 310,000-tick run still ended with one myocyte,
    /// because a body that can swim has nowhere better to go: the blotches were worked out
    /// once from the seed and never moved again, so the best tile in the world was the best
    /// tile in the world for ever and sitting still was the whole optimal strategy. A field
    /// that moves is a reason to be able to follow it.
    ///
    /// # ⚠️ Why it must be an offset and not a reseed
    ///
    /// Re-drawing the noise from a moving seed would also produce a field that changes, and
    /// it would be a completely different world: every blotch in it would vanish and be
    /// replaced somewhere else in one tick, under a living population, so what a lineage had
    /// found would stop existing rather than move. There would be nothing to follow and no
    /// advantage to being able to. **The check that separates the two is the rotation**: the
    /// field 8,000 ticks later is the field at the start, moved one tile west, tile for tile
    /// across the whole world. A reseed passes no part of that.
    ///
    /// # The arithmetic of the numbers chosen
    ///
    /// A tile is `2048 / 256 = 8` world units across, and the drift is a thousandth of a unit
    /// per tick, so 8,000 ticks is one tile exactly. The lattice fits a whole number of times
    /// across the world, so one tile of drift is exactly one tile of lattice coordinate and
    /// the comparison is a rotation rather than an approximation - the tolerance below is for
    /// `light.patch_drift` being narrowed to 32 bits on its way through the gate, which is
    /// worth about four parts in ten million of a tile.
    ///
    /// # And nought really is still
    ///
    /// A world told not to drift holds the same ceilings, to the last bit, for as long as it
    /// is run. That is the control experiment for everything above, and a drift that was
    /// quietly always on would take it away.
    #[test]
    fn the_light_blotches_drift_sideways_without_being_redrawn() {
        // A tile is 8 world units across, so a thousandth of a unit per tick moves the field
        // one whole tile in 8,000 ticks.
        let mut world = Grid::new(&config(|raw| raw.light.patch_drift = 0.001));
        let mut ledger = Ledger::new(world.total_energy());
        let before = world.targets.clone();

        for _ in 0..8_000 {
            world.tick(&mut ledger);
        }

        let cols = world.cols();
        let mut moved = 0;
        for row in 0..world.rows() {
            for col in 0..cols {
                let here = f64::from(world.targets[at(&world, col, row)]);
                let came_from = f64::from(before[at(&world, (col + 1) % cols, row)]);
                assert!(
                    (here - came_from).abs() < 1e-4,
                    "the tile at ({col}, {row}) can hold {here}, and one tile east of it \
                     could hold {came_from} eight thousand ticks ago - so the field has been \
                     redrawn rather than moved, and a lineage that had found somewhere good \
                     has had it deleted rather than towed away"
                );
                if (here - f64::from(before[at(&world, col, row)])).abs() > 1e-4 {
                    moved += 1;
                }
            }
        }

        // ...and it is genuinely somewhere else, rather than a field so smooth that any
        // rotation of it matches. A patch is sixteen tiles across, so a shift of one moves
        // most tiles by a real amount.
        assert!(
            moved > cols * world.rows() / 2,
            "only {moved} of the {} tiles hold a different amount from eight thousand ticks \
             ago, so the drift is not reaching the field",
            cols * world.rows()
        );

        // ⚠️ And the ceiling rule of SPEC section 4 still holds against a ceiling that is
        // moving. `the_field_reaches_a_ceiling` states it on a still field, because a
        // drifting world never comes to rest and cannot be asked to; this is the same rule
        // under the conditions the world actually ships in.
        for (tile, (standing, ceiling)) in world.tiles.iter().zip(&world.targets).enumerate() {
            assert!(
                standing <= ceiling,
                "tile {tile} holds {standing} against a ceiling of {ceiling}, so a ceiling \
                 that slid down under a full tile left the excess in the water"
            );
        }

        // The control: nought is nought, bit for bit, over the same distance.
        let mut still = Grid::new(&config(|raw| raw.light.patch_drift = 0.0));
        let mut books = Ledger::new(still.total_energy());
        let fixed = still.targets.clone();
        for _ in 0..8_000 {
            still.tick(&mut books);
        }
        assert!(
            still
                .targets
                .iter()
                .zip(&fixed)
                .all(|(now, then)| now.to_bits() == then.to_bits()),
            "a world with `light.patch_drift = 0` moved its blotches anyway, so the control \
             experiment for every claim about a drifting field no longer exists"
        );
    }

    /// ⭐ What the drift costs, which is a real quantity of energy and is measured here
    /// rather than reasoned about.
    ///
    /// SPEC section 4: a tile cannot hold more than its ceiling, and what will not fit is
    /// **dissipated**. A drifting patch field is a field of ceilings sliding sideways, so
    /// every tick, every full tile whose ceiling has moved down under it sheds the difference
    /// out of the world. **The drift therefore destroys energy continuously**, and the loss
    /// scales with `patchiness × patch_drift`, which is what `PATCH_DRIFT_CEILING` is derived
    /// from.
    ///
    /// The world here is deliberately the worst case: full to its ceiling everywhere and with
    /// nothing living in it to have eaten a tile down below the level where a falling ceiling
    /// can reach it. What a real run pays is less, in proportion to how much of the field a
    /// population has already drawn down.
    ///
    /// Two claims, and the second is the one that matters:
    ///
    /// - the drift costs something, so a run that shows no extra dissipation is a run where
    ///   the drift is not reaching the tiles;
    /// - and it costs **less than the light delivers**, at the shipped settings, by a wide
    ///   enough margin that the world is not being run down. That is the claim
    ///   `PATCH_DRIFT_CEILING` exists to keep true, and it is checked here at the shipped
    ///   value rather than only argued at the bound.
    #[test]
    fn a_drifting_field_sheds_what_its_ceiling_slides_off() {
        let spent = |drift: f64| {
            let mut grid = Grid::new(&config(|raw| raw.light.patch_drift = drift));
            let mut ledger = Ledger::new(grid.total_energy());

            // Fill the world to its ceilings first, so that every tile is one a falling
            // ceiling can take something from. `cap / influx` ticks at the shipped light.
            for _ in 0..10_000 {
                grid.tick(&mut ledger);
            }
            let (shed, lit) = (ledger.dissipated(), ledger.influx_total());

            for _ in 0..10_000 {
                grid.tick(&mut ledger);
            }
            (
                (ledger.dissipated() - shed) / 10_000.0,
                (ledger.influx_total() - lit) / 10_000.0,
            )
        };

        let (still, _) = spent(0.0);
        let (drifting, _) = spent(0.0006);
        let extra = drifting - still;

        // What the world is *offered* per tick, which is the figure the cost has to be
        // compared against. The number the ledger reports as influx at a full field is much
        // smaller than this and is not the right comparison: at rest the light only replaces
        // what has spilled, so comparing the two would be comparing the drift's loss with the
        // loss it is part of.
        //
        // `grid_cols × grid_rows × influx × mean light_profile`, and the mean profile of
        // `1 - gradient × (y / height)` over the whole depth is `1 - gradient / 2`.
        let offered = 256.0 * 144.0 * 0.001 * (1.0 - 0.75 / 2.0);

        assert!(
            extra > 0.005,
            "a full field drifting at the shipped 0.0006 loses {drifting} a tick against a \
             still one's {still}, a difference of {extra} - which is nothing at all, so the \
             ceilings are not moving"
        );
        assert!(
            extra < offered * 0.1,
            "the drift destroys {extra} a tick on top of what a still field loses, against \
             the {offered} the light offers this world - so more than a tenth of the world's \
             income is being dragged out of it by the weather, and the carrying capacity of \
             SPEC section 4 is set by `patch_drift` rather than by `influx`"
        );

        // Measured 1 August 2026, Windows 11 x86-64, release build, on a field held full with
        // nothing living in it: **0.0179 a tick** against the 23.04 the light offers, which is
        // **0.08%**. A living world pays less again, because a population has already eaten
        // most of the field down below the level a falling ceiling can reach. See
        // `docs/PHASE7.md` for the figure taken off the 300,000-tick run.
    }

    /// The noise makes *patches*, which is what SPEC section 4's word "patchiness" means.
    ///
    /// This is the test that rules out the obvious implementation. Hashing each tile's own
    /// coordinates gives a perfectly good random number per tile, is a pure function of the
    /// seed and the position, and satisfies every claim in
    /// `patchiness_is_stable_across_runs`. It is also useless: neighbouring tiles would be
    /// unrelated, so the field would be television static rather than a world with rich
    /// places and poor places in it. Nothing could swim towards anything, because at the
    /// scale an organism can sense there would be no *towards*.
    ///
    /// So the claim is about scale rather than randomness. Tiles that are neighbours are
    /// much more alike than tiles a patch apart. A fifth is a generous bar, chosen so this
    /// fails on speckle rather than on a change of mind about the exact lattice spacing;
    /// the measured figures at the default world are 0.032 between neighbours against 0.391
    /// a patch apart, so neighbouring tiles are twelve times as alike as tiles a patch
    /// away, and a per-tile hash would put both numbers at about 0.67.
    ///
    /// The last claim is about the seam. The world wraps sideways, so the noise has to wrap
    /// with it: the lattice is laid out to fit a whole number of times across the grid and
    /// its rightmost points *are* its leftmost ones. Without that there would be a visible
    /// wall of discontinuity down the join, which is a thing organisms would evolve to
    /// exploit and nobody would ever work out why. Measured, tiles either side of the join
    /// differ by 0.005 - less than an ordinary pair of neighbours, so the join is not a
    /// feature of the world at all.
    #[test]
    fn the_light_noise_is_patchy_rather_than_speckled() {
        let noise = PatchNoise::new(42, 256, 144);

        let mut between_neighbours = 0.0;
        let mut across_a_patch = 0.0;
        let mut across_the_seam = 0.0;
        for row in 0..144 {
            for col in 0..256 {
                between_neighbours +=
                    (noise.at(col, row, 0.0) - noise.at((col + 1) % 256, row, 0.0)).abs();
                across_a_patch += (noise.at(col, row, 0.0)
                    - noise.at((col + NOISE_LATTICE_SPACING) % 256, row, 0.0))
                .abs();
            }
            across_the_seam += (noise.at(255, row, 0.0) - noise.at(0, row, 0.0)).abs();
        }

        let tiles = f64::from(256 * 144);
        between_neighbours /= tiles;
        across_a_patch /= tiles;
        across_the_seam /= 144.0;

        assert!(
            across_a_patch > 0.1,
            "tiles a whole patch apart differ by only {across_a_patch} on average, so the \
             light has no spatial structure for anything to move towards"
        );
        assert!(
            between_neighbours * 5.0 < across_a_patch,
            "neighbouring tiles differ by {between_neighbours} against {across_a_patch} a \
             patch apart, which is speckle rather than patchiness"
        );
        assert!(
            between_neighbours > 0.0,
            "the noise is the same everywhere, which is not noise"
        );
        assert!(
            across_the_seam < between_neighbours * 2.0,
            "the join where the world wraps is a wall: tiles either side of it differ by \
             {across_the_seam} against {between_neighbours} for any other pair of \
             neighbours"
        );
    }

    /// A tile fills up under the light and then holds steady.
    ///
    /// SPEC section 4's regrowth rule in three parts: a tile gains `influx ×
    /// light_profile(y)` a tick, it never gains more than would take it past its ceiling,
    /// and once it is at that ceiling it stays there. A field that kept growing would have
    /// no carrying capacity, and a carrying capacity is the pressure the whole simulation
    /// is built on.
    ///
    /// Patchiness is off, which buys one extra claim worth having. A tile's ceiling and the
    /// light it gets are both scaled by the same depth profile, so the profile cancels: the
    /// floor fills at a quarter of the rate into a quarter of the space and arrives at the
    /// same moment as the surface. Depth decides how much a tile *holds*, not how long it
    /// takes to get there, and the whole world saturates on the same tick.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a tile that stops at its ceiling must stop at exactly its ceiling; \
                  approximately would leave a tile drifting a fraction past it every tick \
                  for the rest of the run"
    )]
    fn a_tile_regrows_towards_its_target_and_stops_there() {
        // A light of this test's own, and blotchiness turned off. The rule under test is
        // "fill towards the ceiling and stop", and how many ticks that takes is
        // `cap / influx` - a ratio Group D moved by a factor of twelve when it tuned the
        // shipped ecology, and which decides nothing about whether the rule is right. Fixed
        // here so the arithmetic below stays legible: eight over 0.012 is 666 and two thirds.
        let mut world = Grid::new(&config(|raw| {
            raw.light.influx = 0.012;
            raw.light.patchiness = 0.0;
        }));
        let surface = at(&world, 0, 0);
        let floor = at(&world, 0, 143);

        assert_eq!(
            world.tiles[surface], 0.0,
            "a world at the moment the light comes on holds nothing"
        );

        world.regrow();
        assert_eq!(
            world.tiles[surface], world.regrowth[0],
            "one tick of light did not put one tick of light into the surface"
        );
        assert_eq!(
            world.tiles[floor], world.regrowth[143],
            "one tick of light did not put one tick of light into the floor"
        );
        assert!(
            world.tiles[surface] > world.tiles[floor] * 3.0,
            "the floor is filling nearly as fast as the surface, so the light has no depth \
             to it"
        );

        // This test's cap divided by this test's influx is 666 and two thirds, so the world
        // is still filling on the 666th tick and full on the 667th - all of it at once.
        for _ in 1..666 {
            world.regrow();
        }
        assert!(
            world.tiles[surface] < world.targets[surface],
            "the surface reached its ceiling early, so the light is stronger than SPEC \
             section 3 asks for"
        );

        world.regrow();
        for (tile, (held, ceiling)) in world.tiles.iter().zip(&world.targets).enumerate() {
            assert_eq!(
                held, ceiling,
                "tile {tile} is holding {held} rather than the {ceiling} it should have \
                 filled to"
            );
        }

        // And it stops. A thousand more ticks of light onto a full world change nothing at
        // all.
        for _ in 0..1_000 {
            world.regrow();
        }
        for (tile, (held, ceiling)) in world.tiles.iter().zip(&world.targets).enumerate() {
            assert_eq!(
                held, ceiling,
                "tile {tile} went past its ceiling: it holds {held} against a ceiling of \
                 {ceiling}, so the world has no carrying capacity"
            );
        }
    }

    /// What the ledger is told the light did is what the light actually did.
    ///
    /// This is the phase's central failure mode, and it is entirely invisible if you write
    /// it the obvious way. Tiles are 32-bit numbers. Adding `0.012` to one does not
    /// increase it by `0.012`; it increases it by whatever the nearest number that tile can
    /// hold happens to be. And a tile at its ceiling does not increase at all, however much
    /// light falls on it.
    ///
    /// So there are two quantities: what SPEC section 4's formula *asked* for, and what the
    /// grid actually gained. The ledger has to be told the second one. Told the first, its
    /// influx account would be a running total of intentions, permanently and increasingly
    /// adrift from the world it claims to describe.
    ///
    /// Three claims here, in increasing order of importance.
    ///
    /// Every tick, what the light reports is what the grid's own total moved by, to a
    /// hair - and the hair is a millionth of a unit, against a ledger tolerance that at
    /// this scale is around a hundred. Which is to say: not "within tolerance" but exact,
    /// as far as adding up thirty-six thousand numbers twice can be.
    ///
    /// After a thousand ticks the books balance and, because nothing is alive yet, the
    /// world holds precisely what the light has put into it. Nothing else has a way in.
    ///
    /// And the last one is the counterfactual, without which the other two prove nothing.
    /// A thousand ticks is long enough for the world to fill up and the light to start
    /// falling on tiles that cannot take it. By then the intended total has run away from
    /// the truth by more than half of itself - hundreds of times the ledger's whole
    /// tolerance - so the naive version of this is not a subtle inaccuracy that the
    /// tolerance absorbs. It is a run that stops with a conservation failure and no
    /// apparent cause.
    #[test]
    fn regrowth_credits_influx_with_the_realised_change() {
        // A light of this test's own. The claim is that the ledger is credited with what the
        // tiles *took* rather than what they were *offered*, and the two only diverge once the
        // world is full - so what this test needs from the light is that a thousand ticks of
        // it is comfortably more than enough to fill the field. At the shipped `influx` that
        // takes twelve thousand ticks and this test would be measuring a world still filling.
        let mut world = Grid::new(&config(|raw| raw.light.influx = 0.012));
        let mut ledger = Ledger::new(world.total_energy());

        // What SPEC section 4's formula asks for each tick, before any tile refuses it:
        // the light offered to every tile of every row, added up.
        let asked_for_each_tick: f64 = world
            .regrowth
            .iter()
            .map(|offered| f64::from(*offered) * 36_864.0 / 144.0)
            .sum();
        let mut asked_for = 0.0;

        for tick in 0..1_000 {
            let before = world.total_energy();
            let realised = world.regrow();
            let after = world.total_energy();

            assert!(
                (realised - (after - before)).abs() < 1e-6,
                "on tick {tick} the light says it added {realised} and the grid says it \
                 gained {}",
                after - before
            );

            ledger.light(realised);
            ledger.check(after);
            asked_for += asked_for_each_tick;
        }

        let held = world.total_energy();
        assert!(
            (ledger.influx_total() - held).abs() < 1e-6,
            "after a thousand ticks the influx account holds {} and the world holds {held}, \
             and with nothing alive in it those are the same quantity",
            ledger.influx_total()
        );

        // The world filled up somewhere around the 667th tick, so by now the formula has
        // been asking for far more than the grid could take.
        assert!(
            asked_for > held * 1.5,
            "a thousand ticks of light asked for {asked_for} against a world holding \
             {held}, which is not enough of a gap for this test to be about anything"
        );
        assert!(
            (asked_for - held) > held * 1e-3,
            "crediting the light with what it asked for rather than what it delivered \
             would still be inside the ledger's tolerance, so this test is not describing \
             a failure that can happen"
        );
    }

    /// Energy spreads to the tiles around it.
    ///
    /// SPEC section 4 asks for "a simple 5-point stencil at rate `diffusion`" after
    /// regrowth, and says what it is for: it smooths harvest shadows, so that an organism
    /// parked on one tile cannot strip-mine it and sit in a permanent hole of its own
    /// making. Without it the field would be thousands of independent little wells and
    /// there would be no reason ever to move.
    ///
    /// One unit of energy is dropped into an otherwise empty world, well away from any
    /// edge, and one tick of diffusion is run. Four things must be true afterwards.
    ///
    /// Each of the four neighbours holds exactly the configured rate, which is what "at
    /// rate `diffusion`" means and is the assertion that would catch a stencil running at
    /// half or twice the rate it was asked for. The four diagonal tiles hold nothing at
    /// all, which is what makes it a five-point stencil rather than a nine-point one. What
    /// is left in the middle is what the four neighbours took away. And the total is
    /// unchanged, because diffusion moves energy and does not make or destroy it.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a neighbour must receive exactly the configured rate; near enough would \
                  pass a stencil running at a rate nobody configured"
    )]
    fn diffusion_moves_energy_sideways() {
        let mut world = Grid::new(&config(|raw| {
            raw.world.grid_cols = 9;
            raw.world.grid_rows = 7;
        }));

        let middle = at(&world, 4, 3);
        world.tiles[middle] = 1.0;
        let before = world.total_energy();

        world.diffuse();

        for (name, neighbour) in [
            ("west", at(&world, 3, 3)),
            ("east", at(&world, 5, 3)),
            ("above", at(&world, 4, 2)),
            ("below", at(&world, 4, 4)),
        ] {
            assert_eq!(
                world.tiles[neighbour], 0.04,
                "the tile {name} of the spike holds {} rather than the configured \
                 diffusion rate",
                world.tiles[neighbour]
            );
        }

        for (name, corner) in [
            ("north-west", at(&world, 3, 2)),
            ("north-east", at(&world, 5, 2)),
            ("south-west", at(&world, 3, 4)),
            ("south-east", at(&world, 5, 4)),
        ] {
            assert_eq!(
                world.tiles[corner], 0.0,
                "the tile to the {name} of the spike was given energy, so this is not the \
                 five-point stencil SPEC section 4 asks for"
            );
        }

        assert!(
            (f64::from(world.tiles[middle]) - 0.84).abs() < 1e-7,
            "the spike kept {} of its unit of energy, and having given a rate of 0.04 to \
             each of four neighbours it should be holding 0.84",
            world.tiles[middle]
        );
        // A tenth of a millionth, which is a little under two rounding steps at this size.
        // Writing a smaller number into a tile than that tile can distinguish is the drift
        // SPEC section 5's tolerance exists to allow, and it is the only drift there is
        // here: what one tile gives up is exactly what its neighbour is handed, and it is
        // only the writing down that rounds.
        assert!(
            (world.total_energy() - before).abs() < 1e-7,
            "diffusion changed how much energy there is by more than rounding explains: \
             {before} became {}",
            world.total_energy()
        );

        // And a world configured not to spread its energy about does not.
        let mut still = Grid::new(&config(|raw| {
            raw.world.grid_cols = 9;
            raw.world.grid_rows = 7;
            raw.light.diffusion = 0.0;
        }));
        still.tiles[middle] = 1.0;
        still.diffuse();
        assert_eq!(
            still.tiles[middle], 1.0,
            "a world with diffusion turned off moved energy anyway"
        );
    }

    /// Nothing falls off the edges of the world.
    ///
    /// PHASE2.md calls this the most likely conservation bug in the phase, and it is:
    /// **a closed vertical axis that quietly loses energy at the floor.** The world wraps
    /// sideways, so the tile at column zero has the last column for a western neighbour.
    /// It does not wrap vertically - there is a surface and there is a floor - so the top
    /// row has nothing above it and the bottom row has nothing below.
    ///
    /// The way this goes wrong is not exotic. Write the stencil as "add up my four
    /// neighbours, subtract four times me", reach past the edge, find nothing there, and
    /// use nought - and every tile in the top and bottom rows now spends every tick giving
    /// a share of itself to a tile that does not exist. The world's energy drains out
    /// through its floor at a rate proportional to how much is lying on it. It looks
    /// exactly like an ecology that is slightly too hungry.
    ///
    /// The first claim below is the one that catches it, and it is worth writing out
    /// because it is so much stronger than it looks: **a world holding the same amount
    /// everywhere must not change at all.** There is no gradient anywhere in such a world,
    /// so there is nothing for diffusion to move - including at the edges, where the
    /// missing-neighbour version has an enormous imaginary gradient between the floor and
    /// the nothing below it. A hundred ticks of this and every tile must still hold exactly
    /// what it started with.
    ///
    /// After that: the wrap actually wraps, the floor and surface actually do not, and a
    /// world with energy piled along every edge and in every corner still holds all of it
    /// after a thousand ticks.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a world with nothing to spread must be left exactly alone; a tolerance \
                  here is a slow leak with permission"
    )]
    fn diffusion_does_not_leak_at_the_edges() {
        let small = |raw: &mut RawConfig| {
            raw.world.grid_cols = 9;
            raw.world.grid_rows = 7;
        };

        // A world with the same energy everywhere has nothing to spread about.
        let mut even = Grid::new(&config(small));
        even.tiles.fill(5.0);
        let filled = even.total_energy();
        for _ in 0..100 {
            even.diffuse();
        }
        assert_eq!(
            even.total_energy(),
            filled,
            "an evenly-filled world does not hold what it did"
        );
        for (tile, held) in even.tiles.iter().enumerate() {
            assert_eq!(
                *held, 5.0,
                "tile {tile} of an evenly-filled world holds {held} after a hundred ticks \
                 of diffusion, so energy is being moved to or from somewhere that is not \
                 in the world"
            );
        }

        // The world wraps sideways: the first column and the last are neighbours.
        let mut wrapping = Grid::new(&config(small));
        let west_edge = at(&wrapping, 0, 3);
        wrapping.tiles[west_edge] = 1.0;
        wrapping.diffuse();
        assert_eq!(
            wrapping.tiles[at(&wrapping, 8, 3)],
            0.04,
            "energy at the western edge did not reach the eastern one, so the world does \
             not wrap"
        );
        assert_eq!(
            wrapping.tiles[at(&wrapping, 1, 3)],
            0.04,
            "energy at the western edge did not reach the tile beside it either"
        );

        let mut back = Grid::new(&config(small));
        let east_edge = at(&back, 8, 3);
        back.tiles[east_edge] = 1.0;
        back.diffuse();
        assert_eq!(
            back.tiles[at(&back, 0, 3)],
            0.04,
            "the world wraps one way and not the other"
        );

        // And it does not wrap top to bottom. A tile on the floor has three neighbours,
        // not four, so it keeps three quarters of what a tile in open water would give
        // away - and the quarter it does not give away stays in it rather than vanishing.
        for (name, row, phantom) in [("floor", 6, 5), ("surface", 0, 1)] {
            let mut closed = Grid::new(&config(small));
            let edge = at(&closed, 4, row);
            closed.tiles[edge] = 1.0;
            let before = closed.total_energy();
            closed.diffuse();

            assert!(
                (f64::from(closed.tiles[edge]) - 0.88).abs() < 1e-7,
                "a tile on the {name} kept {} of its energy; with three neighbours and a \
                 rate of 0.04 it should be holding 0.88, and holding 0.84 instead means it \
                 is giving a share to the nothing beyond the edge",
                closed.tiles[edge]
            );
            assert_eq!(
                closed.tiles[at(&closed, 4, phantom)],
                0.04,
                "the tile beside the {name} did not receive its share"
            );
            assert!(
                (closed.total_energy() - before).abs() < 1e-7,
                "a spike on the {name} lost energy: {before} became {}",
                closed.total_energy()
            );
        }

        // Now everything at once: energy piled along all four edges and into all four
        // corners, left to spread for a thousand ticks.
        let mut edged = Grid::new(&config(small));
        for col in 0..9 {
            let (surface, floor) = (at(&edged, col, 0), at(&edged, col, 6));
            edged.tiles[surface] = 3.0;
            edged.tiles[floor] = 7.0;
        }
        for row in 0..7 {
            let (west, east) = (at(&edged, 0, row), at(&edged, 8, row));
            edged.tiles[west] = 5.0;
            edged.tiles[east] = 2.0;
        }
        let piled = edged.total_energy();
        for _ in 0..1_000 {
            edged.diffuse();
        }
        assert!(
            (edged.total_energy() - piled).abs() < 1e-4,
            "a thousand ticks of diffusion over a world with energy along every edge \
             turned {piled} into {}",
            edged.total_energy()
        );

        // A thousand ticks is more than enough to even that world out, which is the check
        // that the energy really did spread rather than merely stay put.
        let evened = piled / 63.0;
        for (tile, held) in edged.tiles.iter().enumerate() {
            assert!(
                (f64::from(*held) - evened).abs() < 1e-3,
                "tile {tile} holds {held} rather than the {evened} an evened-out world \
                 would have, so the energy did not actually move"
            );
        }
    }

    /// The light adds energy to the world. It never takes any away.
    ///
    /// SPEC section 4 now says so outright, and the sentence it says it in is a correction
    /// to an earlier draft that this test is what caught. That draft wrote the rule as
    ///
    /// ```text
    /// tile += min(regrowth, target - tile)      // never exceeds target
    /// ```
    ///
    /// and read literally that is a subtraction whenever a tile is holding more than its
    /// ceiling: `target - tile` is then negative, and the smaller of it and the regrowth is
    /// that negative number, so the tile is dragged back down to its ceiling. The comment
    /// beside it says what was meant - light must not push a tile past its ceiling - and
    /// the arithmetic quietly does something else as well.
    ///
    /// A tile over its ceiling is not a hypothetical. Ceilings fall with depth, so
    /// diffusion is permanently carrying energy down from brighter rows into dimmer ones,
    /// and from Phase 4 detritus will rot back into tiles that may already be full.
    ///
    /// What the literal reading costs is not a rounding error. It is energy leaving the
    /// world through a door that no account is watching: the world holds less, and nothing
    /// holds more. The books can only be made to balance by telling the ledger the light
    /// contributed a *negative* amount - which `Ledger::light` refuses outright, and
    /// rightly, because a light that can shine backwards is a way to destroy energy with
    /// the invariant still passing. Whether the run survives then depends on whether the
    /// tiles being drained happen to outweigh the tiles being filled on that particular
    /// tick, which is not a foundation to build an ecology on.
    ///
    /// So the light here is a source and only a source, and a tile above its ceiling is
    /// left where it is. Which raises the obvious question of what *does* bring it back
    /// down, since a tile above its ceiling is not a state the world should be allowed to
    /// stay in - and the answer is [`Grid::spill`], running after diffusion, where the
    /// excess is moved to `dissipated` rather than removed. Both halves are needed and
    /// neither is sufficient: without the clamp here the light destroys energy on its way
    /// past; without the spill the ceilings do not hold and the field goes level. The two
    /// are tested apart because they fail apart.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "an overfull tile must be left exactly alone; approximately alone is a \
                  slow drain with a tolerance in front of it"
    )]
    fn light_never_runs_backwards() {
        let mut world = Grid::new(&config(|_| {}));

        let overfull = at(&world, 5, 100);
        let brimming = at(&world, 6, 100);
        let ceiling = world.targets[overfull];
        world.tiles[overfull] = ceiling * 2.0;
        world.tiles[brimming] = ceiling;

        let realised = world.regrow();

        assert_eq!(
            world.tiles[overfull],
            ceiling * 2.0,
            "the light drained a tile that was holding more than its ceiling, and the \
             energy it took has gone nowhere"
        );
        assert_eq!(
            world.tiles[brimming], ceiling,
            "a tile exactly at its ceiling was moved"
        );
        assert!(
            realised > 0.0,
            "a tick in which the light shone on a world with one overfull tile in it \
             reported a contribution of {realised}, and a light that contributes a \
             negative amount is a way of destroying energy that the ledger cannot see"
        );

        // The other half of the same claim, and the one that makes it a bound rather than
        // an accident of which tiles happened to be full: whatever the world looks like,
        // the light's contribution is never negative. Here every tile in it is over its
        // ceiling, so there is nothing at all for the light to add.
        let mut drowned = Grid::new(&config(|_| {}));
        for (tile, ceiling) in drowned.tiles.iter_mut().zip(&drowned.targets) {
            *tile = ceiling * 1.5;
        }
        let held = drowned.total_energy();
        let realised = drowned.regrow();

        assert_eq!(
            realised, 0.0,
            "the light took {realised} out of a world where every tile was already over \
             its ceiling"
        );
        assert_eq!(
            drowned.total_energy(),
            held,
            "an overfull world lost energy to the light"
        );
    }

    /// A tile cannot hold more than its ceiling, whatever arrives in it.
    ///
    /// The light is a source and only a source, so it can never push a tile past its
    /// ceiling - that is `light_never_runs_backwards`. Diffusion has no such manners. It
    /// moves energy from wherever there is more to wherever there is less, and it knows
    /// nothing about ceilings, so a tile beside a full one is handed energy it has no room
    /// for. SPEC section 4's answer, and this is it, is that the tile is cut back to its
    /// ceiling and the excess **leaves the world**.
    ///
    /// A great deal of energy is dropped into one tile, and one tick is run. What matters is
    /// the tile *below* it: nobody touched that one by hand, and diffusion has just handed
    /// it several times what it can hold. Afterwards it is sitting at exactly its ceiling.
    ///
    /// Then the same claim about the whole world at once - not one tile anywhere is above
    /// its ceiling - and the two claims that make this a *transfer* rather than a clamp. The
    /// books still balance, which they could not if the excess had simply been deleted; and
    /// what the world lost is what `dissipated` gained, to within the small quantity
    /// diffusion always leaves in transit.
    ///
    /// The last part is the other half of the rule, and it is the one a careless
    /// implementation gets wrong: a tile that is *not* over its ceiling must be left
    /// completely alone. A version that wrote every tile back at `min(tile, ceiling)` would
    /// pass everything above and would still be correct - but a version that pulled every
    /// tile *towards* its ceiling would drain a world that was merely filling up, and the
    /// only place that shows is a tile a long way from the spike.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a tile that is cut back must land exactly on its ceiling, and a tile that \
                  is left alone must be exactly untouched; a tolerance on either would let \
                  a slow drain through"
    )]
    fn a_tile_pushed_past_its_ceiling_is_cut_back_to_it() {
        let mut world = Grid::new(&config(|raw| {
            raw.world.grid_cols = 9;
            raw.world.grid_rows = 7;
        }));

        let spike = at(&world, 4, 3);
        let below = at(&world, 4, 4);
        let far_away = at(&world, 0, 0);

        // Four hundred units in one tile of a world whose richest tile can hold about
        // eight. A fortieth of that reaches each neighbour on the first tick, which is
        // still more than any tile in the world has room for.
        world.tiles[spike] = 400.0;

        let mut ledger = Ledger::new(world.total_energy());
        let opening = world.total_energy();

        world.tick(&mut ledger);

        assert_eq!(
            world.tiles[below], world.targets[below],
            "diffusion handed the tile below the spike far more than it can hold and it is \
             still holding {}, against a ceiling of {}",
            world.tiles[below], world.targets[below]
        );
        for (tile, (held, ceiling)) in world.tiles.iter().zip(&world.targets).enumerate() {
            assert!(
                held <= ceiling,
                "tile {tile} is holding {held} against a ceiling of {ceiling}, so a tile in \
                 this world can hold more than its target after all"
            );
        }

        let held = world.total_energy();
        ledger.check(held);
        assert!(
            ledger.dissipated() > 300.0,
            "the world went from {opening} to {held} and only {} of that is accounted for \
             as having left the world",
            ledger.dissipated()
        );
        // The bound is what diffusion leaves in transit - a fraction of a rounding step per
        // tile, which at this size is nothing like a unit of energy.
        let lost = opening + ledger.influx_total() - held;
        assert!(
            (lost - ledger.dissipated()).abs() < 1e-3,
            "the world lost {lost} and {} of it was written down as dissipated",
            ledger.dissipated()
        );

        // And a tile nowhere near the spike is still holding about the one tick of light
        // that fell on it. The band is a generous five per cent, because a little of it
        // trickles into the dimmer row below on the same tick; what it rules out is not
        // subtle, being a tile hauled up to a ceiling six hundred times where it is.
        let untouched = f64::from(world.tiles[far_away]);
        let one_tick = f64::from(world.regrowth[0]);
        assert!(
            (untouched - one_tick).abs() < one_tick * 0.05,
            "a tile a long way from the spike holds {untouched} after a tick of light worth \
             {one_tick}, so something has moved a tile that was nowhere near its ceiling"
        );
    }

    /// The world fills up, stops, and still has a surface and a floor.
    ///
    /// **The headline is the depth gradient, and it is the whole reason SPEC section 4's
    /// overflow rule exists.** A world at rest must still hold materially more energy near
    /// the surface than at the floor. If it does not, swimming upwards pays nothing, there
    /// is no spatial structure for phototaxis to find, and section 4's claim that "the
    /// gradient is what gives movement a reason to exist" has stopped being true - silently,
    /// with every test in this file still green.
    ///
    /// That is not hypothetical, and it is what this test used to record. With the light
    /// clamped to a source but no ceiling that actually holds, a tile's ceiling falling with
    /// depth means energy trickles downwards for ever: the light refills the top, the floor
    /// has nowhere to pass anything on to, and the world fills from the bottom up until it
    /// is **level** - every tile in it at the largest ceiling in the world, no depth left at
    /// all. For the world used here that arrived on tick 89,235 at 288 x 8.492838. A run
    /// reaches that state; it is slow, but an overnight run is tens of millions of ticks.
    ///
    /// With the rule in section 4 it comes to rest somewhere quite different, and far
    /// sooner: **tick 1,724, holding 1,477.45**, with the top row holding **3.26 times** what
    /// the bottom row holds. The floor sits exactly on its own ceiling and sheds everything
    /// that reaches it; the rows above sit below theirs, because each is losing more
    /// downwards than the light hands it. The world is a third of the size it used to settle
    /// at, and it has a shape.
    ///
    /// # What "at rest" means here, and why it is no longer "not one bit"
    ///
    /// It used to be possible to demand that a tick move not one tile by one bit, because
    /// the level world really did freeze: no gradient anywhere means no energy moving
    /// anywhere, and nothing left for the rounding to do.
    ///
    /// A world with a gradient in it never freezes in that sense, and it should not. Light
    /// is falling on every tile every tick and the same amount is leaving through tiles that
    /// cannot hold what reaches them, so energy is permanently in motion even though the
    /// picture is not changing. Diffusion carries the part of a movement too small to write
    /// into a tile over to the next tick - see [`Grid::diffuse`] - so a resting world goes on
    /// nudging its tiles by their last bit for ever.
    ///
    /// So the bar is: a tick no longer moves any tile by more than **a millionth of what it
    /// holds**. A 32-bit tile can only distinguish about a ten-millionth of itself, so that
    /// is a handful of rounding steps - as still as a tile of this size can be. It is not a
    /// slack tolerance quietly hiding a world that is still filling: the tick before it is
    /// met, some tile is still moving seven times further than that, and the world stays
    /// under the bar for every one of the next twenty thousand ticks.
    ///
    /// # A world at rest is not a world where nothing is happening
    ///
    /// That is what the last group of claims is for. Over twenty thousand further ticks the
    /// field's total barely moves, while `influx_total` and `dissipated` each climb by
    /// twenty thousand ticks' worth of light. Practically every unit that arrives leaves
    /// again the same tick, from the tiles at the bottom of the world that have no room for
    /// it. This is the state the ecology will actually be run in.
    ///
    /// The tick count is a statement about the *shape* of the world rather than this
    /// module's speed. Filling a world is slower the deeper it is, so SPEC's default world,
    /// sixteen times deeper than this one, takes considerably longer to come to rest.
    #[test]
    fn the_field_reaches_a_ceiling() {
        // Small enough, and lit brightly enough, to run to a complete standstill inside a
        // test suite. Neither decides the behaviour: a smaller or dimmer world comes to the
        // same rest, later. The light is this test's own rather than the shipped one for that
        // reason - Group D moved `light.influx` by a factor of twelve when it tuned the
        // ecology, which multiplies the ticks below by twelve and changes nothing they say.
        let mut world = Grid::new(&config(|raw| {
            raw.light.influx = 0.012;
            raw.world.grid_cols = 32;
            raw.world.grid_rows = 9;
            // ⚠️ **A still field, and the claim below is about a still field.** SPEC section
            // 4's drifting blotches move a tile's ceiling every hundred ticks for ever, so a
            // world with `patch_drift` on never comes to rest and never can - that is the
            // whole purpose of the setting. What this test is about is the *ceiling rule*:
            // that a tile cannot hold more than its target and that the field therefore keeps
            // a depth structure instead of filling until it is level. Asserting stillness
            // against a moving ceiling would be asserting something that is not true of the
            // shipped world rather than something the model promises.
            //
            // The rule itself is not left untested under drift.
            // `the_light_blotches_drift_sideways_without_being_redrawn` checks that no tile
            // finishes above its ceiling after eight thousand ticks of a moving one, and
            // `energy_is_conserved_over_a_long_run` runs the shipped configuration.
            raw.light.patch_drift = 0.0;
        }));
        let ceilings = world.targets.clone();

        let mut ledger = Ledger::new(world.total_energy());
        let mut previous = world.tiles.clone();
        let mut settled_at = None;

        // A tick that moves no tile by more than a millionth of what it holds. See the note
        // above for why this is the bar rather than bitwise stillness.
        let still = |tiles: &[f32], before: &[f32]| {
            tiles
                .iter()
                .zip(before)
                .all(|(now, then)| (*now - *then).abs() <= *now * 1e-6)
        };

        // ⚠️ **A hundred still ticks in a row, not one.** Diffusion carries the part of a
        // movement too small to write into a tile over to the next tick, so a tile can sit
        // unchanged for a while and then jump when enough of them have piled up together -
        // which means a single still tick can happen on the way to somewhere and does. At the
        // shipped `patchiness` of 0.5 the first still tick of this world is **2,310** and the
        // last moving one is **2,320**; taking the first would declare the world settled ten
        // ticks before it is, and the twenty thousand ticks below would then, correctly,
        // catch it moving. Asking for a stretch is stricter than asking for an instant.
        let mut unbroken = 0u32;
        for tick in 1..=200_000u32 {
            world.tick(&mut ledger);
            ledger.check(world.total_energy());

            if still(&world.tiles, &previous) {
                unbroken += 1;
                if unbroken == 100 {
                    settled_at = Some(tick - 99);
                    break;
                }
            } else {
                unbroken = 0;
            }
            previous.copy_from_slice(&world.tiles);
        }

        let settled = settled_at.expect(
            "the world was still changing after two hundred thousand ticks, so nothing in \
             this test has established that it has a ceiling at all",
        );
        let held = world.total_energy();

        // Every tile is at or under its own ceiling. This is the rule itself, stated over
        // the whole world after a hundred thousand chances to break it.
        for (tile, (standing, ceiling)) in world.tiles.iter().zip(&ceilings).enumerate() {
            assert!(
                standing <= ceiling,
                "tile {tile} came to rest holding {standing} against a ceiling of \
                 {ceiling}, so a tile in this world can hold more than its target after all"
            );
        }

        // The headline. The top row against the bottom row, which is the difference an
        // organism would be swimming for.
        let row_holds = |row: usize| -> f64 {
            world.tiles[row * world.cols()..(row + 1) * world.cols()]
                .iter()
                .copied()
                .map(f64::from)
                .sum()
        };
        let surface = row_holds(0);
        let floor = row_holds(world.rows() - 1);

        assert!(
            surface > floor * 1.5,
            "the world came to rest on tick {settled} with {surface} in its top row against \
             {floor} in its bottom row, which is close enough to level that there is nothing \
             left for anything to swim towards"
        );

        // And a monotone gradient the whole way down, rather than merely two ends that
        // differ. A field that were bright at both ends and dim in the middle would satisfy
        // the claim above and would be a strange world to build an ecology on.
        for row in 1..world.rows() {
            assert!(
                row_holds(row) < row_holds(row - 1),
                "row {row} came to rest holding {} against {} in the row above it, so the \
                 world's standing energy does not fall with depth",
                row_holds(row),
                row_holds(row - 1)
            );
        }

        // And it stays there. Twenty thousand more ticks, every one of which has to leave
        // the world as still as the tick that ended the loop above - so what was measured
        // is a resting state rather than a pause on the way somewhere.
        let (light_so_far, lost_so_far) = (ledger.influx_total(), ledger.dissipated());
        for tick in 0..20_000 {
            previous.copy_from_slice(&world.tiles);
            world.tick(&mut ledger);
            ledger.check(world.total_energy());
            assert!(
                still(&world.tiles, &previous),
                "the world started moving again {tick} ticks after it had apparently come \
                 to rest"
            );
        }

        let resting = world.total_energy();
        assert!(
            (resting - held).abs() < held * 1e-4,
            "twenty thousand ticks after coming to rest the world holds {resting} rather \
             than the {held} it settled at"
        );

        // A world at rest is not a world in which nothing is happening. The light goes on
        // falling on every tile of it, and practically all of that energy leaves again the
        // same tick, out of the tiles near the floor that have no room for it. A tenth of a
        // per cent is the slack, and what is inside it is the last of the filling: the
        // world is still creeping upwards by a fraction of a unit over these twenty
        // thousand ticks.
        let arrived = ledger.influx_total() - light_so_far;
        let left = ledger.dissipated() - lost_so_far;
        assert!(
            arrived > 20_000.0,
            "only {arrived} of light fell on a resting world over twenty thousand ticks, so \
             what was measured is a dark world rather than a full one"
        );
        assert!(
            (arrived - left).abs() < arrived * 1e-3,
            "over twenty thousand ticks of a world that has stopped changing, {arrived} of \
             light arrived and {left} left again, and a field that holds steady is one \
             where those are the same number"
        );

        // The ceilings themselves never moved. A hundred thousand ticks did not recompute
        // the noise, which is what `patchiness_is_stable_across_runs` claims and this is
        // where a version that recomputed it per tick would be caught.
        assert!(
            world
                .targets
                .iter()
                .zip(&ceilings)
                .all(|(now, then)| now.to_bits() == then.to_bits()),
            "the world's ceilings changed while it was running"
        );

        // The three numbers this world actually settles into, written down. Every claim
        // above is a statement about the *kind* of world it comes to rest in, and would
        // stay green through a change that moved all three; these are what make a change of
        // behaviour something that has to be argued for rather than absorbed.
        //
        // Recorded 31 July 2026, Windows 11 x86-64.
        // ⚠️ **Re-recorded in Phase 7, Group G, and the old figures are kept beside the new
        // ones** so that a reading taken before today can still be identified. `patchiness`
        // moved from 0.15 to 0.5, which is the depth of the blotches this world's ceilings
        // are made of, so all three of these had to move and a set that survived would have
        // meant the change had not landed. What it was, at 0.15:
        //
        // ```text
        // came to rest on tick 1,724, holding 1,477.4486, top row 3.260245 x bottom row
        // ```
        //
        // The world it describes is the same *kind* of world - it fills, it stops, and it
        // keeps a depth structure - which is what every claim above this line is about. It
        // now takes about a third longer to settle and comes to rest holding 7% more, because
        // deeper blotches mean more tiles with ceilings above the depth average and the light
        // has further to go before the world stops filling; and the surface-to-floor ratio is
        // steeper for the same reason.
        assert_eq!(
            settled, 2_321,
            "the world came to rest on tick {settled} rather than the 2321 recorded here"
        );
        assert!(
            (held - 1_583.581_2).abs() < 1e-3,
            "the world came to rest holding {held} rather than the 1583.5812 recorded here"
        );
        assert!(
            (surface / floor - 3.601_976).abs() < 1e-4,
            "the top row holds {} times what the bottom row does, against the 3.601976 \
             recorded here",
            surface / floor
        );
    }

    /// The blotches seed 42 actually produces, written down.
    ///
    /// Every other test about the noise asks whether it behaves *consistently* - the same
    /// seed twice, patches rather than speckle, nothing outside `-1..=1`. All of them would
    /// stay green if the mixer were replaced tomorrow with a different one that also
    /// behaved consistently. This is the only test that asks whether it still produces the
    /// same world it produced on the day it was written.
    ///
    /// It matters for the same reason `rng.rs`'s golden vectors matter. The moment a run is
    /// archived, the route from a seed to a world stops being an implementation detail and
    /// becomes a file format: a recording of an overnight run is a recording of *that* run
    /// only for as long as seed 42 still puts the rich patches in the same places.
    ///
    /// ⚠️ **If this test fails, investigate. Do not paste in the new numbers.** A failure
    /// means either that something changed the shape of the world a seed produces - in
    /// which case every archived run is now unplayable and that is the thing to deal with -
    /// or that this machine's arithmetic differs from the machine these were recorded on,
    /// which is a much larger finding than a stale test. Updating the literals makes the
    /// message go away and destroys the only evidence.
    ///
    /// The last two assertions are about the lattice rather than the mixer, and they are
    /// what makes the spacing visible rather than buried in a constant: at the default grid
    /// the lattice is exactly sixteen tiles wide, so the tile at column 16 sits precisely
    /// on the next lattice point and reads its height exactly, and the tile at column 0
    /// sits on the first.
    ///
    /// Recorded 31 July 2026, Windows 11 x86-64.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a golden value is only golden if it matches exactly; a tolerant \
                  comparison here would wave through a changed mixer and with it every \
                  archived run"
    )]
    fn the_noise_lattice_is_pinned() {
        let noise = PatchNoise::new(42, 256, 144);

        // Five lattice points, including three that sit next to each other, so a mixer
        // that had lost its ability to tell neighbouring coordinates apart would show up
        // here as three numbers that are suspiciously alike.
        assert_eq!(noise.corner(0, 0), 0.307_423_514_152_742_8);
        assert_eq!(noise.corner(1, 0), 0.718_410_009_918_364_1);
        assert_eq!(noise.corner(0, 1), 0.520_714_122_224_765_3);
        assert_eq!(noise.corner(15, 8), 0.818_478_675_516_899_3);
        assert_eq!(noise.corner(16, 9), -0.773_708_705_271_992_1);

        // And five readings off it: one on a lattice point, one in the middle of a cell,
        // one a whole cell along, and two well inside the world.
        assert_eq!(noise.at(0, 0, 0.0), 0.307_423_514_152_742_8);
        assert_eq!(noise.at(8, 8, 0.0), 0.220_538_643_472_953_32);
        assert_eq!(noise.at(16, 0, 0.0), 0.718_410_009_918_364_1);
        assert_eq!(noise.at(100, 50, 0.0), -0.300_683_068_476_052_9);
        assert_eq!(noise.at(255, 143, 0.0), -0.448_059_758_283_621_43);

        assert_eq!(
            noise.at(0, 0, 0.0),
            noise.corner(0, 0),
            "the first tile is no longer sitting on the first lattice point"
        );
        assert_eq!(
            noise.at(NOISE_LATTICE_SPACING, 0, 0.0),
            noise.corner(1, 0),
            "the lattice is no longer exactly {NOISE_LATTICE_SPACING} tiles wide on the \
             default grid, so the blotches have changed size"
        );
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The tests above use one seed and one or two grid shapes, chosen by hand. These ask
    // the same questions of seeds and shapes the machine invents, hundreds of times over,
    // including the awkward ones - a world one tile wide, a world one tile deep, a
    // patchiness of exactly one. When one fails it shrinks the failure to the smallest
    // example that still breaks and writes it to a file beside this one, so that example
    // is re-run for ever afterwards.
    // ---------------------------------------------------------------------------------

    proptest! {
        // Stated outright rather than left to the library's changing default, so the
        // strength of the check is a decision written on the page.
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// No tile can ever be asked to hold less than nothing, whatever the world is
        /// like.
        ///
        /// This is the load-bearing property of the noise, and `config.rs` already leans
        /// on it: `patchiness` is checked into `0..=1` there with a comment explaining
        /// that anything above one would drive some tiles' ceilings negative, and that a
        /// tile draining below zero is energy leaving the world through an account that
        /// does not exist. That argument only holds if the noise itself stays within one
        /// of nought, which is a promise this side has to keep and this is where it is
        /// kept.
        ///
        /// Both halves are checked: the noise stays inside `-1..=1` at every tile, and
        /// the ceilings that get built out of it are at or above zero even when
        /// patchiness is turned all the way up.
        #[test]
        fn no_world_asks_a_tile_to_hold_less_than_nothing(
            seed: u64,
            cols in 1u32..48,
            rows in 1u32..48,
            patchiness in 0.0f64..=1.0,
        ) {
            let noise = PatchNoise::new(seed, cols, rows);
            for row in 0..rows {
                for col in 0..cols {
                    let value = noise.at(col, row, 0.0);
                    prop_assert!(
                        (-1.0..=1.0).contains(&value),
                        "the noise at ({col}, {row}) is {value}, which is outside the \
                         range config.rs's bound on patchiness depends on"
                    );
                }
            }

            let world = Grid::new(&config(|raw| {
                raw.world.seed = seed;
                raw.world.grid_cols = cols;
                raw.world.grid_rows = rows;
                raw.light.patchiness = patchiness;
            }));
            for (tile, ceiling) in world.targets.iter().enumerate() {
                prop_assert!(
                    *ceiling >= 0.0,
                    "tile {tile} of this world is asked to hold {ceiling}"
                );
            }
        }

        /// Diffusion moves energy about and never changes how much of it there is,
        /// whatever shape the world is and however fast it spreads.
        ///
        /// `diffusion_does_not_leak_at_the_edges` makes this claim about worlds a person
        /// chose: nine by seven, energy piled along the edges. This asks it of worlds the
        /// machine invents, which is where the shapes a person would not think to try
        /// live - a world one tile wide, whose every tile is its own eastern neighbour; a
        /// world one tile deep, which is all surface and all floor at once; a world two
        /// tiles wide, where wrapping makes each tile its neighbour's neighbour twice
        /// over.
        ///
        /// The bound is a millionth, relative, which is a thousand times tighter than the
        /// tolerance SPEC section 5 allows the whole ledger. That is the point of stating
        /// it: the tolerance exists for this rounding, and this says how much of it
        /// diffusion actually needs.
        ///
        /// The rate is drawn up to a quarter and no further, which is now the whole of
        /// what `config.rs` will accept - see the note on `Grid::diffuse`. That is not this
        /// test conveniently avoiding the hard cases: past a quarter the field oscillates
        /// and grows without limit, and while the energy in it stays conserved exactly as
        /// well as it is here, the test would be measuring the difference between two
        /// enormous nearly-equal numbers and reporting on floating-point cancellation
        /// rather than on anything this module does. The reason that band is untested here
        /// is the reason it is unreachable there.
        #[test]
        fn diffusion_moves_energy_without_changing_how_much_there_is(
            cols in 1u32..24,
            rows in 1u32..24,
            rate in 0.0f64..=0.25,
            heights in prop::collection::vec(0.0f32..8.0, 1..576),
        ) {
            let mut world = Grid::new(&config(|raw| {
                raw.world.grid_cols = cols;
                raw.world.grid_rows = rows;
                raw.light.diffusion = rate;
            }));

            // However many heights the machine handed over, laid across the world from the
            // top left and repeated until it is full.
            for (tile, height) in world.tiles.iter_mut().enumerate() {
                *height = heights[tile % heights.len()];
            }
            let opening = world.total_energy();

            for tick in 0..16 {
                world.diffuse();
                let held = world.total_energy();
                prop_assert!(
                    (held - opening).abs() <= opening * 1e-6,
                    "after {tick} ticks of diffusion a {cols} x {rows} world spreading at \
                     {rate} holds {held} rather than the {opening} it started with"
                );
            }
        }
    }

    // -------------------------------------------------------------------------------------
    // Phase 7's Group L — the season
    // -------------------------------------------------------------------------------------

    /// A field with no population on it, ticked for a while, so a claim about the *light* is
    /// not also a claim about who was eating.
    fn ticked(config: &Config, ticks: u64) -> (Grid, Ledger) {
        let mut grid = Grid::new(config);
        let mut ledger = Ledger::new(grid.total_energy());

        grid.begin_season();
        for _ in 0..ticks {
            grid.tick(&mut ledger);
        }

        (grid, ledger)
    }

    /// ⭐⭐ **A world with no season is the world that was there before one existed** — to the
    /// bit, and not only on the tick it was built.
    ///
    /// The multiplier is `1 + amplitude × triangle(phase)`, computed **unconditionally**. At an
    /// amplitude of nought that expression is exactly 1.0, `f64::from(x) * 1.0` narrows back to
    /// `x` for every `x` there is, and the identity holds to the last bit however long the run
    /// goes on and whatever the period is set to.
    ///
    /// ⚠️⚠️ **The second half is the one that matters, and every reviewer of this design found
    /// the same defect.** An implementation that takes an early return when the amplitude is
    /// nought — the obvious optimisation, and the obvious way to write "no season means no
    /// work" — leaves the light multiplier frozen wherever the season happened to be when the
    /// dial reached zero. The world then runs permanently at up to **1.25× its stated influx**,
    /// with `season_amplitude = 0` showing in the config file, in the panel and in the replay
    /// log, while SPEC section 5's ledger balances to the last digit and says nothing. That is
    /// section 4's named failure — *"a world that was poorer than its influx said, with a
    /// carrying capacity nobody could explain"* — reached through the settings panel and in the
    /// other direction.
    #[test]
    fn a_world_with_no_season_is_the_world_that_was_there_before() {
        // Two grids differing only in the season's *period*, at an amplitude of nought.
        let (slow, slow_books) = ticked(&config(|raw| raw.light.season_period = 8_000), 5_000);
        let (quick, quick_books) =
            ticked(&config(|raw| raw.light.season_period = 1_000_000), 5_000);

        assert_eq!(
            slow.tiles
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            quick
                .tiles
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            "two worlds with no season in them, differing only in how long that absent season \
             would take, are holding different energy after five thousand ticks"
        );
        assert_eq!(
            slow_books.influx_total().to_bits(),
            quick_books.influx_total().to_bits(),
            "the same two worlds have taken in different amounts of light"
        );

        // ⭐ And the retune. Two thousand ticks of a real season, then the dial to nought: the
        // very next tick has to offer exactly what a world that never had one offers.
        let seasoned = config(|raw| raw.light.season_amplitude = 0.25);
        let flat = config(|_| {});

        let (mut retuned, mut retuned_books) = ticked(&seasoned, 2_000);
        retuned.relight(&flat);

        let (mut always_flat, mut flat_books) = ticked(&flat, 2_000);
        assert!(
            retuned.season_phase() > 0.0,
            "the season never advanced, so this measures nothing"
        );

        let before = retuned_books.influx_total();
        let flat_before = flat_books.influx_total();
        retuned.tick(&mut retuned_books);
        always_flat.tick(&mut flat_books);

        assert_eq!(
            (retuned_books.influx_total() - before).to_bits(),
            (flat_books.influx_total() - flat_before).to_bits(),
            "a world retuned to `season_amplitude = 0` took in {} of light on its next tick \
             where a world that never had a season took {}. The multiplier is frozen where the \
             season left it, so this world is permanently lit at something other than its stated \
             influx and nothing anywhere says so",
            retuned_books.influx_total() - before,
            flat_books.influx_total() - flat_before
        );
    }

    /// ⭐⭐ **The anti-Group-H test: the light a row is offered really does rise and fall.**
    ///
    /// Group H shipped a sevenfold change to every muscle in the world and only afterwards
    /// discovered that the code path it acted on was essentially never taken. The lesson is that
    /// *the mechanism landing* and *the mechanism doing something* are two claims, and only the
    /// second is worth anything. This is the second one for the season: over a whole period the
    /// light offered peaks at 1.25× the flat constant a quarter of the way in, troughs at 0.75×
    /// three quarters in, and the total over the period is preserved.
    ///
    /// ⚠️ **The fixture is the hard part of this test and it is easy to get wrong.** The obvious
    /// setup — turn the drift and the diffusion off and pick a period — reads *clipped* light
    /// rather than *offered* light: at the shipped ceiling the tiles are 99.997% full by the end
    /// of one period, so what a tile takes stops following what it is offered and the sum ratio
    /// comes back at 1.000027 against an asserted 1e-9, four orders of magnitude out. So
    /// `patchiness` is nought, the ceiling is raised until the field stays near one per cent of
    /// it throughout, and the assertions are against the season **scalar** rather than against
    /// the argmax of realised light.
    ///
    /// The tolerance is **1e-5**, which is what narrowing 144 rows to 32 bits over eight thousand
    /// ticks actually delivers. ⚠️ It is a tolerance on the *fixture* rather than on the season:
    /// a tile is a 32-bit number and adding a thousandth to one that already holds a great deal
    /// rounds the addition by a fraction of what was added, so at the shipped ceiling — where a
    /// tile is full — a single tick's realised light could not be read anything like this
    /// closely. Raising `cap` is what buys the digits, and it is the same choice as turning the
    /// patchiness off: this test is about the offer and not about the clip.
    #[test]
    fn the_light_a_row_is_offered_rises_and_falls_with_the_season() {
        const PERIOD: u64 = 8_000;
        const AMPLITUDE: f64 = 0.25;

        let settings = config(|raw| {
            raw.light.season_period = PERIOD;
            raw.light.season_amplitude = AMPLITUDE;
            raw.light.patchiness = 0.0;
            raw.light.patch_drift = 0.0;
            raw.light.diffusion = 0.0;
            // A ceiling a whole period of light cannot come near, so every tile takes all of
            // what it is offered and this measures the offer rather than the clip.
            raw.light.cap = 1_000.0;
        });

        let mut grid = Grid::new(&settings);
        let mut ledger = Ledger::new(grid.total_energy());
        grid.begin_season();

        // What one tick of the same world with no season in it puts into the field.
        let flat: f64 = grid
            .regrowth
            .iter()
            .map(|row| f64::from(*row) * f64::from(u32::try_from(grid.cols()).expect("small")))
            .sum();

        let mut offered = Vec::with_capacity(usize::try_from(PERIOD).expect("a period is small"));
        for _ in 0..PERIOD {
            let before = ledger.influx_total();
            grid.tick(&mut ledger);
            offered.push((ledger.influx_total() - before) / flat);
        }

        // The phase advances before the light falls, so the `n`th tick of the run is at phase
        // `n / period`: the peak is the two-thousandth tick and the trough the six-thousandth.
        let at = |ticks: u64| offered[usize::try_from(ticks).expect("small") - 1];
        let (peak, trough) = (at(PERIOD / 4), at(PERIOD * 3 / 4));

        assert!(
            (peak - (1.0 + AMPLITUDE)).abs() < 1e-5,
            "a quarter of the way through a season a row was offered {peak} times what a flat \
             world offers it, against the {} an amplitude of {AMPLITUDE} asks for",
            1.0 + AMPLITUDE
        );
        assert!(
            (trough - (1.0 - AMPLITUDE)).abs() < 1e-5,
            "three quarters of the way through a season a row was offered {trough} times what a \
             flat world offers it, against the {} an amplitude of {AMPLITUDE} asks for",
            1.0 - AMPLITUDE
        );

        // And the total over a whole period is the flat world's total. A triangle is exactly
        // antisymmetric under a half-period shift, so this is not an approximation that happens
        // to hold - it is why the wave is a triangle.
        let total: f64 = offered.iter().sum();
        let ticks = f64::from(u32::try_from(PERIOD).expect("a period fits"));
        assert!(
            (total / ticks - 1.0).abs() < 1e-5,
            "a whole period of a season delivered {} times what a flat world delivers over the \
             same stretch, so the wave is not mean-preserving and a season is a change in this \
             world's income rather than in its timing",
            total / ticks
        );
    }

    /// ⭐⭐ **A season moves no ceiling**, and this is the whole energy argument for putting it on
    /// `influx` rather than on `cap`, written as a test instead of as a paragraph.
    ///
    /// `Grid::relight` builds `regrowth` from `influx` and `targets` from
    /// `cap × light_profile × (1 + patchiness × noise)`. **`influx` enters no ceiling.** So a
    /// season needs no retarget, moves no target down under a full tile, and sheds no spill
    /// whatever — and the same amplitude applied to `cap` instead would add 0.656/tick of
    /// ceiling spill, 4.3% of throughput, and swing the standing field by 90.7% while SPEC
    /// section 5's ledger balanced perfectly throughout.
    ///
    /// This is the claim that would go silently false the day somebody "simplified" the season
    /// by folding it into the relight table.
    ///
    /// ⚠️ **It does not also assert that `dissipated` is unchanged, and that omission is
    /// measured rather than an oversight.** Over three periods on an empty world, dissipated per
    /// tick runs **15.184 flat, 14.992 at ±25%, 14.480 at ±50%** — a 1.3% and a 4.6% difference,
    /// because diffusion knocks tiles off their patchy ceilings every tick whatever the light is
    /// doing and a dimmer half-cycle leaves fewer of them there to be knocked off. Asserting
    /// equality there would be asserting something false and then weakening it until it passed.
    #[test]
    fn a_season_moves_no_ceiling() {
        let settings = config(|raw| {
            raw.light.season_period = 8_000;
            raw.light.season_amplitude = 0.25;
            // The drift off, so the only thing that could move a ceiling is the season.
            raw.light.patch_drift = 0.0;
        });

        let untouched = Grid::new(&settings);
        let (seasoned, _) = ticked(&settings, 8_000);

        assert_eq!(
            seasoned
                .targets
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            untouched
                .targets
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            "a whole seasoned period moved a ceiling. The season is on `light.influx`, which \
             enters no target, and the moment it reaches one it starts shedding energy to \
             `dissipated` on every tick with the ledger balancing perfectly throughout"
        );
        assert_eq!(
            seasoned
                .regrowth
                .iter()
                .map(|row| row.to_bits())
                .collect::<Vec<u32>>(),
            untouched
                .regrowth
                .iter()
                .map(|row| row.to_bits())
                .collect::<Vec<u32>>(),
            "the season has been folded into the relight table, so a retune and a season are \
             now two ways of computing one number and neither of them can be turned off"
        );
    }

    /// ⭐⭐ **Retuning the season changes its speed and not where it is.**
    ///
    /// Group G's decision about `drifted`, applied to the second clock. `[light]` is live — SPEC
    /// section 3 — so somebody can turn the period up mid-run, and a phase worked out as
    /// `ticks / period` would teleport a living world into a different part of its year the
    /// instant they did. The phase is therefore **accumulated**, one step of `1 / period` per
    /// tick, exactly as the drift's offset is.
    ///
    /// It is also what makes it impossible to move the season into the relight table: doing so
    /// would make a retune and a season two ways of computing the same number.
    #[test]
    fn retuning_the_season_changes_its_speed_and_not_where_it_is() {
        let quick = config(|raw| {
            raw.light.season_period = 8_000;
            raw.light.season_amplitude = 0.25;
        });
        let slow = config(|raw| {
            raw.light.season_period = 16_000;
            raw.light.season_amplitude = 0.25;
        });

        let (mut grid, mut ledger) = ticked(&quick, 2_000);
        let where_it_was = grid.season_phase();
        assert!(
            (where_it_was - 0.25).abs() < 1e-9,
            "two thousand ticks of an eight-thousand-tick season should be a quarter of the way \
             through it, and this world is {where_it_was} of the way"
        );

        grid.relight(&slow);
        assert!(
            (grid.season_phase() - where_it_was).abs() < f64::EPSILON,
            "turning the dial moved the world from {where_it_was} of the way through its season \
             to {}, which is a living population being teleported into a different part of the \
             year by a settings change",
            grid.season_phase()
        );

        for _ in 0..2_000 {
            grid.tick(&mut ledger);
        }
        assert!(
            (grid.season_phase() - 0.375).abs() < 1e-9,
            "after two thousand more ticks at half the speed the world should be three eighths \
             of the way through its season, and it is {} of the way",
            grid.season_phase()
        );
    }
}
