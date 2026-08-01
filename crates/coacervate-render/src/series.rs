//! ⭐ **C1 and C3.** What the world has been doing, kept as it happens.
//!
//! [`census`] is a snapshot: it walks the world and says what is there *now*. A chart is the
//! other thing - what was there, all the way back - and nothing in this project held one before
//! this module. SPEC section 13 says what shape it has to be:
//!
//! > `stats.bin` records are small and fixed-size - population, per-kind biomass, mean genome
//! > length, mean cell count, species counts, the five ledger accounts - **so the whole run's
//! > time-series always fits in memory for charting**.
//!
//! That sentence is a design, not a description. It says the *file* and the *chart* read the same
//! records, so Phase 8 writes [`Sample`] to disk unchanged rather than inventing a second shape
//! for the same numbers. This module is the in-memory half, built now because Group C needs it;
//! the writing is Phase 8's and nothing here does any I/O.
//!
//! [`census`]: crate::census
//!
//! # ⚠️ It lives in this crate for `census.rs`'s reason, and not because it is rendering
//!
//! A sample is *made of* a [`Census`] and the ledger, and the panel that draws it is in this
//! crate. `coacervate-app` depends on `coacervate-render` rather than the other way about, so a
//! series kept in the binary could not be reached from a panel - and computing the same eleven
//! numbers twice is the arrangement `census.rs`'s opening paragraph argues against in as many
//! words. It is the same move that module made in Group A, for the same reason.
//!
//! **`coacervate-sim` still knows nothing about any of this.** Nothing here is called from a tick,
//! and [`Series::observe`] takes a `&World` and gives it back unchanged.
//!
//! # ⭐⭐ `C3`: the bound, and the arithmetic that chose it
//!
//! An overnight run is tens of millions of ticks. A record every hundred of them, kept for ever,
//! is not a chart - it is a leak, and it is the exact leak CLAUDE.md's *"allocate once, never
//! grow"* rule exists to prevent:
//!
//! > Every arena - organisms, cells, springs, resource grid - is allocated at startup at fixed
//! > capacity derived from the config, and never resized. **A simulation that cannot allocate
//! > cannot leak.**
//!
//! So [`Series`] is an arena like the others: [`CAPACITY`] records, allocated once in
//! [`Series::new`], never resized. The numbers:
//!
//! | | |
//! | --- | --- |
//! | One record | **72 bytes** - asserted below, not hoped for |
//! | [`CAPACITY`] | 4,096 records |
//! | The whole series | **288 KiB**, for ever, whatever the run does |
//! | Against CLAUDE.md's 2 GB resident target | 0.014% |
//!
//! And what it would have been without a bound. A headless run manages about 1,300 ticks a second
//! on this machine (Phase 5 Group C measured a *watched* run at about 650, and Q23 records that
//! watching costs about half the speed), so:
//!
//! | Run | Ticks | Records unbounded | Bytes unbounded |
//! | --- | --- | --- | --- |
//! | 1 hour | 4.7 M | 47,000 | 3 MB |
//! | 12 hours - CLAUDE.md's default wall-clock bound | 56 M | 562,000 | **36 MB** |
//! | A week - which SPEC section 13 explicitly plans for | 786 M | 7.9 M | **503 MB** |
//!
//! A quarter of a gigabyte of chart is a quarter of a gigabyte that is not a simulation.
//!
//! # ⭐⭐ Why thinning and not a ring buffer
//!
//! Both bound the memory; they lose completely different things, and only one of them survives
//! contact with this program's premise.
//!
//! A **ring buffer** of 4,096 records at one per hundred ticks holds the last **409,600 ticks** -
//! about five minutes of an overnight run. CLAUDE.md: *"You come back hours later and read what
//! happened."* A chart that can only show the last five minutes is a chart that has thrown away
//! everything the person came back to read.
//!
//! **Thinning** halves the resolution each time the arena fills: the stride doubles and every
//! sample not on the new grid is dropped, so the series always spans **the whole run from its
//! first sample to its latest**, at a resolution that falls as the run gets longer. That is
//! exactly the right trade for deep time - nobody wants tick-resolution detail from eight hours
//! ago, and everybody wants to know whether the population that is there now is the same one that
//! was there then.
//!
//! What it costs, stated plainly rather than buried: **after eight halvings a sample is one tick
//! in 25,600, and anything that happened and was over inside that window is not in the series at
//! all.** SPEC section 11's mass extinction - *"population falls by >50% within 5,000 ticks"* -
//! would show on a 12-hour run's chart as a step between two adjacent samples rather than as a
//! cliff with a shape. The chart says *that* it happened and roughly when; the event log (Phase 7)
//! is what says the tick. A chart is for shape.
//!
//! ⚠️ **A dropped sample is dropped, not averaged into its neighbour.** Averaging two readings
//! makes a third that no tick of the world ever produced - a population of 1,712.5, a ledger that
//! does not balance - and a chart drawn from those is a picture of a world that did not happen.
//! Every point on every chart in this program is a reading the world actually gave.
//!
//! # SPEC's `species counts` field, added by the phase that could fill it
//!
//! Phase 6 left [`Sample::species`] out, and wrote down why: SPEC section 13 lists it, but a
//! species did not exist yet, so the field could only have been a `u32` that is nought on every
//! record of every run - a column of zeroes on a chart, and a number in a file that reads as
//! *"this world has no species"*, which is false and is worse than an absent field because an
//! absent field cannot be believed.
//!
//! Phase 7 is what makes a species exist, and it lands before Phase 8 writes anything to disk, so
//! the field arrives with something in it and nothing has been recorded in the meantime for it to
//! be incompatible with. The count comes from `coacervate_sim::species::Taxonomy`, which is the
//! *other* periodic observer of a run - see [`Series::observe`], which is handed one.

use crate::census::Census;
use coacervate_sim::cell::CellKind;
use coacervate_sim::species::Taxonomy;
use coacervate_sim::world::World;

/// How many ticks pass between two records, at full resolution.
///
/// SPEC section 13's own number. It is the *starting* stride: see [`Series::stride`], which
/// doubles each time the arena fills.
pub const EVERY: u64 = 100;

/// How many kinds of cell a sample carries a biomass figure for.
pub const KINDS: usize = CellKind::ALL.len();

/// How many records the series holds, for the whole life of a run.
///
/// 4,096 at [`Sample`]'s 64 bytes is **256 KiB**, allocated once in [`Series::new`] and never
/// resized. See this module's documentation for the arithmetic and for what a viewer loses.
///
/// It is also comfortably more than any chart can draw: the panel is about 150 points wide, so
/// even at a point a pixel there are twenty-seven samples behind every column. The extra
/// resolution is not for the screen - it is what makes the *next* halving cheap, because a
/// thinned series is still finer than the chart that draws it.
pub const CAPACITY: usize = 4_096;

/// One reading of the whole world, at one tick.
///
/// ⚠️ **Fixed-size, and the size is asserted below.** SPEC section 13 asks for records Phase 8 can
/// write and seek through as a flat array, which is what makes an eight-hour run's chart cost one
/// read rather than a parse. A field of variable length here would be a file format decision made
/// by accident.
///
/// # ⚠️ Why the energies are 32-bit and the tick is not
///
/// The ledger keeps `f64` and the panel prints `f64`, and this narrows them. The reason is that
/// **nothing reads a `Sample` as a figure** - the panel's own numbers come from the world through
/// [`crate::panel::readings`], and these are read as a *chart*, which is a box about twenty points
/// tall. A 32-bit float carries seven significant digits, so the greatest error this can introduce
/// on an account of a billion is about a hundred, which is a hundred-thousandth of a pixel.
///
/// The tick is not narrowed because it is not a magnitude, it is an *identity*: it says which
/// moment this reading is of, [`Series::thin`] filters on it, and 16.7 million ticks - four hours
/// in - is exactly where a 32-bit float stops being able to tell one tick from the next.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sample {
    /// Which tick of the world this is a reading of.
    pub tick: u64,

    /// How many organisms were alive. [`Census::population`].
    pub population: u32,

    /// ⭐ **Phase 7.** How many species there were: `Taxonomy::species_count`.
    ///
    /// SPEC section 13's *species counts*, added by the phase that made a species exist. It is
    /// the number of clusters that had persisted through SPEC section 11's twenty consecutive
    /// samples at the moment this record was taken, so it is **nought for the first ten thousand
    /// ticks of every run** - not because nothing is being counted but because nothing has been
    /// there long enough to count.
    ///
    /// ⚠️ It moves on the clustering's five-hundred-tick grid rather than on this record's
    /// hundred-tick one, so five consecutive records carry the same figure. That is a reading of
    /// the last sample and not an interpolation: every point on every chart in this program is a
    /// number the world actually gave.
    pub species: u32,

    /// The mean number of cells in a body. [`Census::mean_cells`].
    pub mean_cells: f32,

    /// The mean number of genes in a genome. [`Census::mean_genes`].
    pub mean_genes: f32,

    /// SPEC section 13's *per-kind biomass*: how much of the living energy was held in cells of
    /// each kind, in [`CellKind::ALL`]'s order.
    ///
    /// ⚠️ **Apportioned by cell count, because the model does not localise energy.** An organism
    /// holds one pool of energy and its cells hold none, so the only division of it that invents
    /// nothing is an equal share per cell. The consequence is a property worth having and worth
    /// testing: **these six sum to [`Sample::biomass`]**, so the per-kind figures are a
    /// decomposition of the ledger rather than a second opinion about it.
    pub biomass_of: [f32; KINDS],

    /// Energy in the resource field. `Grid::total_energy`.
    pub field: f32,

    /// Energy in living organisms. `Ledger::biomass`.
    pub biomass: f32,

    /// Energy in dead bodies not yet returned to the field. `Ledger::detritus`.
    pub detritus: f32,

    /// Energy spent for good. `Ledger::dissipated`.
    pub dissipated: f32,

    /// Everything light has added since the run began. `Ledger::influx_total`.
    pub influx: f32,
}

/// ⚠️ The record is seventy-two bytes, checked at compile time.
///
/// This is the whole of what *"small and fixed-size"* means for [`CAPACITY`]'s arithmetic, and it
/// is the sort of thing a field added in the wrong place breaks silently - a `vec3`-shaped
/// mistake, and `camera.rs` carries the same note about the same class of fault.
///
/// It was sixty-four until Phase 7 added [`Sample::species`]: eight for the tick and fifteen
/// four-byte scalars is sixty-eight, which the eight-byte alignment the tick needs rounds up to
/// seventy-two. ⚠️ **The last four bytes are therefore padding**, which matters to exactly one
/// thing: Phase 8 writes these records to disk as a flat array and must write those four bytes as
/// zeroes rather than as whatever the stack held, or two runs that did the same thing would
/// produce files that differ.
const _: () = assert!(
    size_of::<Sample>() == 72,
    "a stats record is 72 bytes; see series.rs's CAPACITY arithmetic"
);

impl Sample {
    /// Read the whole world at this moment.
    ///
    /// ⭐ **Every figure here is `Census::of`'s or the ledger's own**, for the reason `census.rs`
    /// was moved into this crate in the first place: a chart that walked the population itself
    /// would be a second implementation of six numbers, and the day it disagreed with the panel
    /// above it is the day somebody spends an hour believing the wrong one.
    #[must_use]
    pub fn of(world: &World, taxonomy: &Taxonomy) -> Self {
        let census = Census::of(world);
        let ledger = world.ledger();

        Self {
            tick: world.ticks(),
            population: u32::try_from(census.population)
                .expect("a population is bounded by the organism arena"),
            species: taxonomy.species_count(),
            mean_cells: narrowed(census.mean_cells),
            mean_genes: narrowed(census.mean_genes),
            biomass_of: biomass_by_kind(world),
            field: narrowed(world.grid().total_energy()),
            biomass: narrowed(ledger.biomass()),
            detritus: narrowed(ledger.detritus()),
            dissipated: narrowed(ledger.dissipated()),
            influx: narrowed(ledger.influx_total()),
        }
    }

    /// Everything the world was holding at this moment: SPEC section 5's four accounts.
    ///
    /// ⚠️ **Not five.** `influx` is where the energy *came from* and is already inside these four -
    /// adding it would count every joule of light twice. It is the growth of this total over the
    /// run, which is what makes the ledger chart's whole height mean something.
    #[must_use]
    pub fn total(&self) -> f32 {
        self.field + self.biomass + self.detritus + self.dissipated
    }
}

/// The whole run's time-series, bounded.
///
/// See this module's documentation for the bound, the arithmetic behind it, and why the older
/// half of a full series is thinned rather than dropped.
#[derive(Debug)]
pub struct Series {
    /// The readings, oldest first. **Allocated once at [`CAPACITY`] and never resized** -
    /// [`Series::thin`] uses `Vec::retain`, which does not touch the allocation.
    samples: Vec<Sample>,

    /// How many ticks apart the readings currently are. [`EVERY`], doubling on every thinning.
    stride: u64,
}

impl Default for Series {
    fn default() -> Self {
        Self::new()
    }
}

impl Series {
    /// An empty series, with the whole of its memory already taken.
    #[must_use]
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(CAPACITY),
            stride: EVERY,
        }
    }

    /// ⭐ **C1.** Take a reading if one is due at this tick.
    ///
    /// Called after every tick by whatever is doing the ticking - `Run::step`, which is the one
    /// place in the program a tick happens. Most calls do nothing at all: the world is read only
    /// when its own tick count lands on the grid.
    ///
    /// ⚠️ **The grid is the world's tick count and not a count of calls**, which is what makes a
    /// series reproducible. Two runs of one seed record the same ticks, a run watched through a
    /// window records the same ticks as the same run headless, and a run resumed from a snapshot
    /// (Phase 8) lands on the same grid as the run it was taken from. A counter that started at
    /// nought whenever this was constructed would have none of those properties.
    pub fn observe(&mut self, world: &World, taxonomy: &Taxonomy) {
        if !world.ticks().is_multiple_of(self.stride) {
            return;
        }

        // A world drawn twice, or a tick the caller offered twice, is one reading. Two records of
        // one tick would put two points on top of each other on every chart and would make the
        // spacing `thin` relies on uneven.
        if self
            .samples
            .last()
            .is_some_and(|last| last.tick == world.ticks())
        {
            return;
        }

        self.record(Sample::of(world, taxonomy));
    }

    /// Put a reading in, thinning the series first if it is full.
    ///
    /// The primitive [`Series::observe`] is the policy over. It is public because Phase 8 reads
    /// `stats.bin` back into one of these to chart a finished run, and that reader has records
    /// rather than a world.
    ///
    /// ⚠️ Ticks are expected to arrive in increasing order and on the grid [`Series::stride`]
    /// describes, which is what `observe` produces. [`Series::thin`] holds the bound even when
    /// they do not.
    pub fn record(&mut self, sample: Sample) {
        if self.samples.len() >= CAPACITY {
            self.thin();
        }

        self.samples.push(sample);
    }

    /// The readings, oldest first.
    #[must_use]
    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    /// How many ticks apart the readings currently are.
    ///
    /// [`EVERY`] until the arena first fills, and double the previous value after every thinning.
    #[must_use]
    pub const fn stride(&self) -> u64 {
        self.stride
    }

    /// How many records this series can hold. [`CAPACITY`], for the whole life of the run.
    #[must_use]
    pub fn room(&self) -> usize {
        self.samples.capacity()
    }

    /// ⭐ **C3.** Halve the resolution, keeping the whole run.
    ///
    /// The stride doubles and every reading not on the new grid goes. Two things about that are
    /// worth writing down:
    ///
    /// ⭐ **The filter is on the tick and not on the position in the list**, and that is what
    /// keeps the spacing exactly even for ever. Dropping every other *record* would leave the
    /// survivors evenly spaced only until the next reading arrived, because `observe` records on
    /// multiples of the stride and the survivors would be on multiples of it offset by wherever
    /// the run happened to start - so every thinning would leave a short interval at the seam.
    /// Filtered on the tick, the survivors are *exactly* the readings `observe` would have taken
    /// at this stride all along, and the run continues on the same grid. It is idempotent, which
    /// is the plainest statement of the same thing.
    ///
    /// ⚠️ **The second half is for a caller that is not `observe`.** [`Series::record`] is public,
    /// because Phase 8 reads a file back through it, and a caller handing over ticks that are not
    /// on the grid could have every one of them survive the filter - which would push the vector
    /// past its capacity and reallocate. That is the one thing this type promises cannot happen,
    /// so it is held by a second pass rather than by the contract being kept.
    fn thin(&mut self) {
        self.stride = self.stride.saturating_mul(2);

        let stride = self.stride;
        self.samples
            .retain(|sample| sample.tick.is_multiple_of(stride));

        if self.samples.len() >= CAPACITY {
            let mut seen = 0_usize;
            self.samples.retain(|_| {
                seen += 1;
                seen.is_multiple_of(2)
            });
        }
    }
}

/// How much of the living energy is held in cells of each kind.
///
/// See [`Sample::biomass_of`] for why this is apportioned by cell count rather than read off the
/// cells: an organism holds one pool of energy and its cells hold none.
fn biomass_by_kind(world: &World) -> [f32; KINDS] {
    let mut totals = [0.0_f64; KINDS];

    for (slot, organism) in world.organisms().iter().enumerate() {
        let Some(organism) = organism else {
            continue;
        };

        let cells = world.cells_of(slot);
        if cells.is_empty() {
            continue;
        }

        let share = organism.energy()
            / f64::from(u32::try_from(cells.len()).expect("a body is not four billion cells"));
        for cell in cells {
            totals[cell.kind as usize] += share;
        }
    }

    totals.map(narrowed)
}

/// One of the world's `f64` quantities, as a record holds it.
///
/// See [`Sample`]'s note on why a chart's numbers are 32-bit. `panel.rs` narrows for the same
/// reason with the same shape.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an energy or a mean, on its way into a fixed-size record that exists to be drawn as \
              a box twenty points tall. Seven significant digits is five more than the chart can \
              show, and the figures a person actually reads come from the world through \
              panel::readings rather than from here"
)]
fn narrowed(value: f64) -> f32 {
    value as f32
}

/// A world with something alive in it, and the series taken as it ran.
///
/// ⚠️ **Here rather than in each test module, because `coacervate-app`'s `founding.rs` cannot be
/// reached from this crate** - it is the binary, and it depends on this. `World::seed` is the only
/// door into a world from outside and every test that wants a *population* rather than empty water
/// has to open it, so the two lines that do are written once.
///
/// It is a much smaller genesis than a run's: a few hundred ticks of light rather than nine
/// thousand, and a handful of bodies rather than eight spread over the world. What it has to
/// produce is a census that is not all zeroes, which is what makes `Census::of`-matches-the-record
/// a claim about anything.
#[cfg(test)]
pub(crate) mod testing {
    use super::Series;
    use coacervate_sim::cell::{CellKind, Vec2};
    use coacervate_sim::chronicle::Chronicle;
    use coacervate_sim::config::spec_defaults;
    use coacervate_sim::genome::{Action, Gene, Genome, SensorTarget, State};
    use coacervate_sim::species::Taxonomy;
    use coacervate_sim::world::World;

    /// How long the light is left to fall before anything is put in the water.
    ///
    /// `World::seed` takes a founder's energy out of the tiles it stands on and refuses if they
    /// have not got it, so a world seeded on tick nought has nothing to seed with - and a body
    /// seeded into water that is still nearly empty starves inside a few hundred ticks, which is
    /// a test that quietly measures an extinct world.
    const DAWN: u64 = 2_000;

    /// What each body is seeded holding. `founding.rs`'s own figure.
    const HELD: f64 = 2.0;

    /// A world with light in it and four bodies standing in the light, ready to be ticked.
    ///
    /// ⚠️ **A sixteenth of the shipped world in each direction, and brighter**, which is
    /// `run.rs`'s `a_small_living_world` and is here for its reason: the dawn at the shipped light
    /// is ten thousand ticks of watching water, and no test in this crate is about the ecology
    /// inside. It is also the size `panel.rs`'s own scene and camera are already built at.
    pub(crate) fn seeded() -> World {
        let mut raw = spec_defaults();
        raw.world.width = 512.0;
        raw.world.height = 288.0;
        raw.world.grid_cols = 64;
        raw.world.grid_rows = 36;
        raw.limits.max_organisms = 250;
        raw.light.influx = 0.012;

        let config = raw
            .validate()
            .expect("a smaller, brighter world is a world");
        let mut world = World::new(&config);

        for _ in 0..DAWN {
            world.tick();
        }

        for founder in 0..4_u8 {
            let at = Vec2::new(
                config.world.width * (f32::from(founder) + 0.5) / 4.0,
                config.world.height * 0.5,
            );
            world
                .seed(founder_genome(&config.limits), at, HELD)
                .expect("a world lit for 300 ticks has water for four small bodies");
        }

        world
    }

    /// The same, ticked on, with the series and the event log recorded as it ran.
    ///
    /// All three observers are driven exactly as `run.rs` drives them and in the same order, so
    /// what these tests are handed is what a real run produces rather than one record taken
    /// without the others that fill part of it.
    pub(crate) fn living(ticks: u64) -> (World, Series, Chronicle) {
        let mut world = seeded();
        let mut series = Series::new();
        let mut taxonomy = Taxonomy::new(world.config());
        let mut log = Chronicle::new(world.config());

        for _ in 0..ticks {
            world.tick();
            taxonomy.observe(&world);
            series.observe(&world, &taxonomy);
            log.observe(&world, &taxonomy);
        }

        (world, series, log)
    }

    /// One photocyte with one gonocyte sprung to it, which is `founding.rs`'s founder.
    ///
    /// Two kinds rather than one on purpose: SPEC section 13's *per-kind biomass* is a claim about
    /// a decomposition, and a population of one kind cannot tell a decomposition from a total.
    fn founder_genome(limits: &coacervate_sim::config::LimitsConfig) -> Genome {
        Genome::new(
            vec![Gene {
                trigger_state: State::ZERO,
                min_step: 0,
                max_step: 0,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(1),
                child_kind: CellKind::Gonocyte,
                rest_length: 8.0,
                stiffness: 10.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            }],
            limits,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, EVERY, KINDS, Sample, Series};
    use crate::census::Census;
    use coacervate_sim::species::{self, Taxonomy};

    /// A reading of nothing in particular, at this tick, whose population says which one it is.
    fn marked(tick: u64, population: u32) -> Sample {
        Sample {
            tick,
            population,
            ..Sample::default()
        }
    }

    /// ⭐ **C1.** A record is taken every hundred ticks, and it is `Census::of`'s reading of that
    /// tick rather than a second opinion about it.
    ///
    /// Both halves matter and they are different claims. *Every hundred ticks* is what makes the
    /// series a grid Phase 8 can write as a flat array and a chart can draw without an x axis.
    /// *`Census::of`'s own numbers* is `census.rs`'s argument applied to a third caller: a chart
    /// that walked the population itself would disagree with the panel above it one day, and
    /// nobody would be able to see which was wrong.
    #[test]
    fn a_time_series_is_recorded_as_the_run_goes() {
        let mut world = super::testing::seeded();
        let mut series = Series::new();
        let mut taxonomy = Taxonomy::new(world.config());
        let mut expected = Vec::new();

        for _ in 0..250 {
            world.tick();
            taxonomy.observe(&world);
            series.observe(&world, &taxonomy);

            if world.ticks().is_multiple_of(EVERY) {
                expected.push((world.ticks(), Census::of(&world), world.ledger().biomass()));
            }
        }

        assert_eq!(
            series.samples().len(),
            expected.len(),
            "250 ticks at one record every {EVERY} should be {} records and the series has {}",
            expected.len(),
            series.samples().len()
        );
        assert_eq!(
            series.samples().iter().map(|s| s.tick).collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(tick, _, _)| *tick)
                .collect::<Vec<_>>(),
            "the records are not on the {EVERY}-tick grid SPEC section 13 asks for"
        );

        for (sample, (tick, census, biomass)) in series.samples().iter().zip(&expected) {
            assert_eq!(
                u64::from(sample.population),
                u64::try_from(census.population).expect("a population is a count"),
                "the record at tick {tick} says {} alive and Census::of says {}",
                sample.population,
                census.population
            );
            assert!(
                (f64::from(sample.mean_cells) - census.mean_cells).abs() < 1e-3,
                "the record at tick {tick} says a mean body of {} cells and Census::of says {}",
                sample.mean_cells,
                census.mean_cells
            );
            assert!(
                (f64::from(sample.mean_genes) - census.mean_genes).abs() < 1e-3,
                "the record at tick {tick} says a mean genome of {} genes and Census::of says {}",
                sample.mean_genes,
                census.mean_genes
            );

            // ⭐ SPEC section 13's per-kind biomass is a decomposition of the ledger's own
            // account and not a second opinion about it. See `Sample::biomass_of`.
            let apportioned = f64::from(sample.biomass_of.iter().sum::<f32>());
            assert!(
                (apportioned - biomass).abs() < biomass.abs() * 1e-4 + 1e-6,
                "the per-kind biomass at tick {tick} sums to {apportioned} and the ledger's \
                 biomass account is {biomass}"
            );
        }

        // ⚠️ And none of the above is vacuous. Every one of those claims is satisfied perfectly by
        // a world with nothing alive in it, where the census is zeroes and the biomass is nought,
        // so what makes this a test is that the world it was taken on has a population and more
        // than one kind of cell in it.
        let last = series.samples().last().expect("there are records");
        assert!(
            last.population > 0,
            "the world these records were taken on has nothing alive in it, so every figure \
             compared above is nought against nought"
        );
        assert!(
            last.biomass_of.iter().filter(|held| **held > 0.0).count() >= 2,
            "the population is one kind of cell, so SPEC section 13's per-kind biomass cannot be \
             told from its total: {:?} over {KINDS} kinds",
            last.biomass_of
        );
    }

    /// ⭐ **Phase 7, A4's other end.** A record carries how many species there were, and the
    /// figure is the taxonomy's own.
    ///
    /// SPEC section 13 lists *species counts* among the things `stats.bin` records, and Phase 6
    /// deliberately left the field out because nothing could fill it: a species did not exist yet,
    /// and a column of zeroes in a file reads as *"this world has no species"*, which is false and
    /// is worse than an absent field because an absent field cannot be believed. This is the phase
    /// that has something to put in it.
    ///
    /// # ⚠️ Why this one is ignored by the debug suite
    ///
    /// SPEC section 11 promotes a cluster after **twenty consecutive samples** five hundred ticks
    /// apart, so the earliest a species can exist is ten thousand ticks into a run. There is no
    /// way to assert that the field carries a real count without running a world that far, and a
    /// run that far in a debug build is most of a minute. `scripts/check.ps1`'s release pass runs
    /// it with `--include-ignored`, so the claim is still proved on every check.
    ///
    /// A test that only asserted the field *existed* would pass against a `species: u32` that was
    /// nought on every record of every run, which is precisely the thing Phase 6 refused to add.
    #[test]
    #[ignore = "ten thousand ticks: a species cannot exist before twenty samples have passed"]
    fn the_series_records_how_many_species_there_are() {
        let mut world = super::testing::seeded();
        let mut series = Series::new();
        let mut taxonomy = Taxonomy::new(world.config());

        // Two samples past the earliest a species could be promoted.
        let until = world.ticks() + species::EVERY * (u64::from(species::PERSISTENCE) + 2);
        while world.ticks() < until {
            world.tick();
            taxonomy.observe(&world);
            series.observe(&world, &taxonomy);
        }

        assert!(
            taxonomy.species_count() > 0,
            "a world ticked past {} samples has no species in it, so this test is comparing \
             nought with nought",
            species::PERSISTENCE
        );

        let last = series.samples().last().expect("there are records");
        assert_eq!(
            last.species,
            taxonomy.species_count(),
            "the last record says {} species and the taxonomy says {}, so the chart and the \
             species list are two opinions about one number",
            last.species,
            taxonomy.species_count()
        );

        // ⚠️ And the run really did start with none and gain some, rather than the field being
        // filled with a constant. A species that existed from the first record would mean the
        // twenty-sample rule was not being applied at all.
        let first = series.samples().first().expect("there are records");
        assert_eq!(
            first.species,
            0,
            "the first record of the run already had {} species in it, and the earliest one can \
             exist is {} samples in",
            first.species,
            species::PERSISTENCE
        );
    }

    /// ⭐⭐ **C3.** The series is bounded, and the memory it holds does not grow.
    ///
    /// Driven far past the arena - a hundred times its own capacity, which at the shipped stride
    /// is forty million ticks and about eight hours of a headless run. The claim is stated the way
    /// CLAUDE.md states it about every other arena in the program: *"allocated at startup at fixed
    /// capacity… and never resized. A simulation that cannot allocate cannot leak."*
    #[test]
    fn the_series_is_bounded() {
        let mut series = Series::new();
        let room = series.room();

        let mut tick = 0_u64;
        for step in 0..CAPACITY * 100 {
            tick += EVERY;
            if tick.is_multiple_of(series.stride()) {
                series.record(marked(tick, u32::try_from(step % 1000).expect("small")));
            }

            assert!(
                series.samples().len() <= CAPACITY,
                "the series holds {} records against a capacity of {CAPACITY} at tick {tick}",
                series.samples().len()
            );
            assert_eq!(
                series.room(),
                room,
                "the series reallocated at tick {tick}: it was built with room for {room} records \
                 and now has room for {}, so an overnight run grows without bound",
                series.room()
            );
        }

        // And it still spans the run rather than the last five minutes of it, which is the whole
        // reason the older half is thinned rather than dropped.
        let first = series.samples().first().expect("a full series has records");
        let last = series.samples().last().expect("a full series has records");
        // ⚠️ The earliest sample is the first tick that is *on the grid*, which after seven
        // halvings is one stride in - a thousandth of a per cent of this run. What matters is
        // that it is a stride and not a window: a ring buffer would have thrown away everything
        // but the last 409,600 ticks of forty million.
        assert!(
            first.tick <= series.stride(),
            "the series begins at tick {} of a run that began at tick {EVERY} and is now recorded \
             every {}, so the early part of the run has been thrown away rather than thinned",
            first.tick,
            series.stride()
        );
        assert_eq!(
            last.tick, tick,
            "the series does not reach the tick the run has got to"
        );
    }

    /// ⭐ **C3, and it is the claim that decides between thinning and a ring buffer.** Thinning
    /// keeps the shape of the run.
    ///
    /// A hump is driven through the series - a run whose population rises, peaks and falls, which
    /// is the shape this program exists to produce - for long enough to thin the arena several
    /// times over. Three things are then true of what is left, and together they are what *"the
    /// whole run stays visible at decreasing detail"* means:
    ///
    /// - it still spans the whole run, first sample to last;
    /// - the samples are **evenly spaced**, so the chart needs no x axis and no gaps appear
    ///   where the thinning happened;
    /// - the peak is still the peak, to within the resolution that is left.
    #[test]
    fn thinning_keeps_the_shape_of_the_run() {
        /// How long the run is, in ticks. Enough to fill the arena several times over.
        const RUN: u64 = EVERY * CAPACITY as u64 * 9;

        /// The population at a tick: up, over and down, peaking at the two-thirds mark.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            reason = "a made-up population between nought and four thousand at a made-up tick, in \
                      a test whose subject is which readings survive rather than what they say"
        )]
        fn hump(tick: u64) -> u32 {
            let place = tick as f64 / RUN as f64;

            (4000.0
                * (place * std::f64::consts::PI / (2.0 / 3.0) / 2.0)
                    .sin()
                    .abs()) as u32
        }

        let mut series = Series::new();
        let mut tick = 0_u64;
        while tick < RUN {
            tick += EVERY;
            if tick.is_multiple_of(series.stride()) {
                series.record(marked(tick, hump(tick)));
            }
        }

        let samples = series.samples();
        assert!(
            series.stride() > EVERY,
            "a run of {RUN} ticks put {} records into an arena of {CAPACITY} without the \
             resolution ever falling, so nothing was thinned and this test proves nothing",
            samples.len()
        );
        assert!(
            samples.len() > CAPACITY / 2 && samples.len() <= CAPACITY,
            "a thinned series holds {} records of {CAPACITY}, so the arena is being emptied \
             rather than halved",
            samples.len()
        );

        // Evenly spaced, every one of them. A chart with a gap in it is a chart that lies about
        // when things happened.
        let spacing = samples[1].tick - samples[0].tick;
        for pair in samples.windows(2) {
            assert_eq!(
                pair[1].tick - pair[0].tick,
                spacing,
                "the samples at ticks {} and {} are {} apart and the rest are {spacing} apart, so \
                 the thinning left a seam",
                pair[0].tick,
                pair[1].tick,
                pair[1].tick - pair[0].tick
            );
        }
        assert_eq!(
            spacing,
            series.stride(),
            "the series says its records are {} ticks apart and they are {spacing}",
            series.stride()
        );

        // It spans the run.
        assert!(
            samples[0].tick <= spacing,
            "the series begins at tick {} rather than at the beginning of the run",
            samples[0].tick
        );
        assert_eq!(
            samples[samples.len() - 1].tick,
            tick,
            "the series does not reach the end of the run"
        );

        // And the peak is still the peak. The hump is smooth, so a sample within one stride of the
        // true peak is within a fraction of a per cent of its height - which is what "the shape is
        // kept" means when it is written as a number rather than as a word.
        let kept = samples
            .iter()
            .map(|sample| sample.population)
            .max()
            .expect("a series with records has a greatest population");
        let truth = (0..=RUN / EVERY)
            .map(|step| hump(step * EVERY))
            .max()
            .expect("a hump has a peak");
        assert!(
            f64::from(kept) > f64::from(truth) * 0.99,
            "the run peaked at {truth} and the thinned series' greatest reading is {kept}, so the \
             shape of the run did not survive being thinned"
        );
    }
}
