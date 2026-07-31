//! What a population looks like from outside: how many, how big, how long a program.
//!
//! The simulation deliberately keeps no summary of itself. `world.rs` holds slots, cells and
//! books, and every number here is worked out by walking them - which is the right way round,
//! because a summary kept inside the world would be a second copy of something the world
//! already knows, and keeping two copies of one quantity in step is the bookkeeping that goes
//! wrong. This is the reader, and it lives outside the simulation because reading is not
//! simulating.
//!
//! There are three callers and they want the same six numbers for different reasons. The
//! progress line a person watches wants them so a run says something while it is happening.
//! `run.rs`'s ecology test wants them because they are the whole of what "a living,
//! non-degenerate population" means: not extinct, not pressed against the arena, still
//! turning over, and not one clone repeated. And Phase 6's panel wants them because they are
//! what the world is doing, written where somebody watching can read it.
//!
//! # ⚠️ This module was `coacervate-app`'s until Phase 6, and the move is the whole reason
//!
//! The third caller is `panel.rs`, which is in *this* crate, and `coacervate-app` depends on
//! `coacervate-render` rather than the other way about - so a census left where it was could
//! not be reached from a panel. The two ways out of that were to move the six numbers or to
//! compute them twice, and the second is the arrangement this module's own opening paragraph
//! argues against: **keeping two copies of one quantity in step is the bookkeeping that goes
//! wrong.** A panel that said 1,712 while the progress line said 1,713 would be a bug nobody
//! could see and everybody would half-believe.
//!
//! Nothing about the numbers changed in the move. `coacervate-app` imports them from here and
//! `report` prints exactly what it printed.
//!
//! # Why the *spread* is here and not only the mean
//!
//! Because a mean cannot tell a population apart from a photocopier. Four thousand identical
//! two-celled bodies and a population ranging from one cell to twelve have the same mean body
//! size, and only one of them is something evolution is happening in. SPEC section 15 asks for
//! a population that is not "a single clone filling the world", and a standard deviation is
//! the plainest number that can say so.

use coacervate_sim::world::World;

/// A reading of the living population at one moment.
pub struct Census {
    /// How many organisms are alive.
    pub population: usize,

    /// How many organisms have ever lived in this world, the dead included.
    ///
    /// Differencing this across a stretch of ticks gives the births over that stretch; with
    /// the population it also gives the deaths. See [`Census::deaths`].
    pub born: u64,

    /// The mean number of cells in a body.
    pub mean_cells: f64,

    /// How far body sizes vary about that mean, as a standard deviation.
    pub cell_spread: f64,

    /// The mean number of genes in a genome.
    pub mean_genes: f64,

    /// How far genome lengths vary about that mean, as a standard deviation.
    pub gene_spread: f64,
}

impl Census {
    /// Walk the world and count what is in it.
    #[must_use]
    pub fn of(world: &World) -> Self {
        let mut population = 0usize;
        let mut cells = Tally::new();
        let mut genes = Tally::new();

        for organism in world.organisms().iter().flatten() {
            population += 1;
            cells.add(organism.cells());
            genes.add(organism.genome().genes().len());
        }

        Self {
            population,
            born: world.born(),
            mean_cells: cells.mean(),
            cell_spread: cells.spread(),
            mean_genes: genes.mean(),
            gene_spread: genes.spread(),
        }
    }

    /// How many organisms have died in this world.
    ///
    /// Every organism that has ever existed is either alive or dead, and no serial number is
    /// ever reused, so this is a subtraction rather than a count kept somewhere.
    #[must_use]
    pub fn deaths(&self) -> u64 {
        self.born - u64::try_from(self.population).expect("a population fits in a machine word")
    }
}

/// How far this world has got, in millions of years.
///
/// ⭐ **CLAUDE.md's deep time, in one place.** *"Tick counts are displayed as millions of
/// years, so a long run reads as Earth's history rather than a number going up."* Three things
/// show a person how far a run has got - `main.rs`'s progress line, `window.rs`'s title bar and
/// Phase 6's panel - and until this existed the first two each did the arithmetic themselves.
/// A third copy of it is how the panel and the title bar end up disagreeing about what year it
/// is.
///
/// It is presentation and it never enters the physics: `world.rs` keeps the tick count and
/// `config.world.years_per_tick` is a number nothing in a tick reads.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "a tick count is turned into a span of geological time for a person to read; the \
              digits lost are far below the resolution of a figure printed to one decimal place"
)]
pub fn millions_of_years(world: &World) -> f64 {
    world.ticks() as f64 * f64::from(world.config().world.years_per_tick) / 1e6
}

/// A running mean and standard deviation over a set of counts.
///
/// The sum of the squares is accumulated alongside the sum, so both come out of one walk of
/// the population rather than two. That matters at the scale this runs at - four thousand
/// organisms surveyed on every progress line of an overnight run - and it costs nothing in
/// accuracy here, because the quantities are small integers well inside what a 64-bit number
/// holds exactly.
struct Tally {
    count: u32,
    total: f64,
    squares: f64,
}

impl Tally {
    const fn new() -> Self {
        Self {
            count: 0,
            total: 0.0,
            squares: 0.0,
        }
    }

    fn add(&mut self, value: usize) {
        let value =
            f64::from(u32::try_from(value).expect("a body is not four billion of anything"));

        self.count += 1;
        self.total += value;
        self.squares += value * value;
    }

    /// The mean, or nothing at all if there is nobody to take a mean of.
    ///
    /// An empty world answers zero rather than refusing. A dead world has a mean body size,
    /// and it is nought - which is what the progress line should print and what the ecology
    /// test should see if everything has died.
    fn mean(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        self.total / f64::from(self.count)
    }

    /// The standard deviation about that mean.
    ///
    /// Clamped at nought before the square root, because the shortcut this uses -
    /// `mean of squares - square of mean` - can land a hair below zero when every value in the
    /// population is identical, which is exactly the case a clone world produces and exactly
    /// the case this number exists to report.
    fn spread(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        let mean = self.mean();

        (self.squares / f64::from(self.count) - mean * mean)
            .max(0.0)
            .sqrt()
    }
}
