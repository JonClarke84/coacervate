//! What a population looks like from outside: how many, how big, how long a program.
//!
//! The simulation deliberately keeps no summary of itself. `world.rs` holds slots, cells and
//! books, and every number here is worked out by walking them - which is the right way round,
//! because a summary kept inside the world would be a second copy of something the world
//! already knows, and keeping two copies of one quantity in step is the bookkeeping that goes
//! wrong. This is the reader, and it lives outside the simulation because reading is not
//! simulating.
//!
//! There are two callers and they want the same six numbers for different reasons. The
//! progress line a person watches wants them so a run says something while it is happening.
//! `run.rs`'s ecology test wants them because they are the whole of what "a living,
//! non-degenerate population" means: not extinct, not pressed against the arena, still
//! turning over, and not one clone repeated.
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
