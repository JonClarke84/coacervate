//! Getting a world to the point where there is something in it to run.
//!
//! Two things have to happen before a run can begin, and both of them are decisions rather
//! than mechanism, which is why they are here rather than in `run.rs`. **The light has to
//! fall**, because `world.rs` is explicit that a world starts dark and an organism seeded on
//! tick zero can be given nothing at all - there is nothing there to give it. And **something
//! has to be put in the water**, because `World::seed` is the only door into the simulation
//! from outside and nothing opens it on its own.
//!
//! `run.rs` is handed the result. That division is deliberate: the runner's whole subject is
//! when a run *ends*, and a runner that also decided what a world starts with would be two
//! unrelated arguments in one file.
//!
//! # The founder is the plainest body that can do both things a life has to do
//!
//! One photocyte and one gonocyte, sprung together. The photocyte earns, which SPEC section 6
//! makes the only source of energy a lineage has that does not involve eating somebody; the
//! gonocyte is what SPEC section 6 requires before an organism may reproduce at all. Take
//! either away and the run has no second generation - a lone photocyte accumulates until it
//! dies of old age, and a lone gonocyte starves.
//!
//! It is deliberately not a good body. It has no muscle, so it cannot move; no sensor, so it
//! cannot tell where anything is; no armour, so anything that grows a mouth can eat it. Every
//! one of those is reachable by mutation from here, and which of them a run actually finds is
//! the question the project exists to ask. Seeding something already competent would be
//! answering it in advance.
//!
//! # Where they go, and why spread out
//!
//! On an even grid over the whole world, laid out so the space each founder has to itself is
//! as near square as the world's shape allows.
//!
//! Spread rather than clustered because of what harvesting does to a tile. A population
//! descended from one body stays where that body was for a long time - a newborn is laid down
//! touching its parent - so a single founder produces one expanding patch of strip-mined water
//! with an untouched world around it, and the ecology that results is a story about the edge of
//! a patch rather than about a world. Founders spread over the field put the population in
//! contact with all of it from the first generation, and the whole world is drawn down together
//! rather than a hole being dug in one corner of it.
//!
//! It measurably does *not* change where the population ends up, which is the more interesting
//! half. One founder and eight founders reach the same level in the shipped world - see
//! `docs/PHASE4.md` for the pair of runs - so the equilibrium is a property of the light and
//! the costs rather than of how the run was started. What spreading changes is how long it
//! takes to get there.
//!
//! # And they are laid over the whole depth rather than at one level
//!
//! SPEC section 4's light falls off with depth, so a row of founders at any single depth is a
//! population that starts by agreeing about the one question the world's geometry poses. A grid
//! puts some of them in the bright water and some in the dim, which is the neutral thing to do:
//! whether depth turns out to matter is for the run to answer.

use coacervate_sim::cell::{CellKind, Vec2};
use coacervate_sim::config::LimitsConfig;
use coacervate_sim::genome::{Action, Gene, Genome, SensorTarget, State};
use coacervate_sim::world::World;

/// What a founder is seeded holding.
///
/// Enough to be alive and not enough to matter. A photocyte on full water earns about 0.05 a
/// tick against the 0.009 its body costs, so a founder is solvent from its first tick and the
/// two units below are gone from the arithmetic within a few hundred. What the number actually
/// has to satisfy is the other end: it comes out of the tiles the body is standing on, so it
/// has to be less than those tiles hold, which is what [`dawn`] is for.
const FOUNDER_ENERGY: f64 = 2.0;

/// How much the field has to stop gaining before the light is called done.
///
/// A thousandth of what it holds, per block of [`DAWN_BLOCK`] ticks. The field approaches its
/// ceiling rather than arriving at it - every tile fills at a rate proportional to how far
/// below its own ceiling it is - so there is no tick on which it is *full*, and a dawn defined
/// by fullness would never end.
const DAWN_SETTLED: f64 = 0.001;

/// How many ticks the light is left to fall between two readings of the field.
const DAWN_BLOCK: u64 = 500;

/// The most ticks a dawn may take, however dark the configuration is.
///
/// A configuration can ask for a light so faint that the field takes millions of ticks to
/// approach its ceiling, and a dawn is not the interesting part of any run. This is what stops
/// a badly-chosen `light.influx` turning into a program that appears to have hung. At SPEC
/// section 3's shipped numbers the dawn ends after about nine thousand ticks, well inside it.
const DAWN_LIMIT: u64 = 100_000;

/// Let the light fall until the field stops filling, then put the first bodies in it.
///
/// Returns how many ticks the dawn took, which is worth knowing because it is the part of a
/// run's tick count that has nothing alive in it.
///
/// # Panics
///
/// If the world cannot afford the founders it was asked for. That is a configuration that
/// cannot start a run - a world of a thousand tiles asked for ten thousand founders, or a
/// light so faint that a tile never reaches [`FOUNDER_ENERGY`] - and it is better to say so at
/// the start than to run an experiment with a population that is not the one that was asked
/// for.
pub fn genesis(world: &mut World, founders: u32) -> u64 {
    let taken = dawn(world);

    let limits = world.config().limits.clone();
    let (width, height) = (world.config().world.width, world.config().world.height);
    let columns = columns_of(founders, width, height);
    let rows = founders.div_ceil(columns);

    for founder in 0..founders {
        // The middle of this founder's share of the world, so the gap between two founders is
        // the same as the gap between the outermost and the edge - which across the width is
        // also the gap over the seam, since SPEC section 8 wraps there.
        let at = Vec2::new(
            width * middle(founder % columns, columns),
            height * middle(founder / columns, rows),
        );

        world
            .seed(founder_genome(&limits), at, FOUNDER_ENERGY)
            .expect("a lit world has room and water for the founders it was asked for");
    }

    taken
}

/// How many columns a grid of `founders` should have over a world of this shape.
///
/// The one that comes nearest to giving each founder a square of the world to itself: the
/// world is `width / height` times as wide as it is deep, so a grid with that same ratio of
/// columns to rows divides it into squares. Rounded up, so the grid always has room for
/// everybody, and never wider than the number of founders, so that one founder is one column
/// in the middle of the world rather than the left-hand half of an empty pair.
fn columns_of(founders: u32, width: f32, height: f32) -> u32 {
    let square = (f64::from(founders) * f64::from(width) / f64::from(height))
        .sqrt()
        .ceil();

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped between one and the number of founders, both of which are whole \
                  numbers already inside the range of the type it is becoming"
    )]
    let columns = square.clamp(1.0, f64::from(founders.max(1))) as u32;

    columns
}

/// Where the middle of share `index` of `count` equal shares falls, as a fraction of the whole.
fn middle(index: u32, count: u32) -> f32 {
    // A grid is never more than a few thousand across in either direction, so both of these
    // are exact as 32-bit numbers and the division is a single rounding.
    let index = f32::from(u16::try_from(index).expect("a founding grid is not 65,536 across"));
    let count = f32::from(u16::try_from(count).expect("a founding grid is not 65,536 across"));

    (index + 0.5) / count
}

/// Tick the world until the light has very nearly finished filling the field.
fn dawn(world: &mut World) -> u64 {
    let mut before = world.grid().total_energy();

    while world.ticks() < DAWN_LIMIT {
        for _ in 0..DAWN_BLOCK {
            world.tick();
        }

        let after = world.grid().total_energy();
        let gained = after - before;
        before = after;

        if after > 0.0 && gained / after < DAWN_SETTLED {
            break;
        }
    }

    world.ticks()
}

/// One photocyte with one gonocyte sprung to it: the founder.
///
/// A single gene. SPEC section 7 starts every body as one photocyte, so the only cell that has
/// to be asked for is the second one, and the gene that asks for it fires on step nought only
/// and hands its daughter a state no gene answers to - which is what makes the body two cells
/// rather than a chain of them.
#[must_use]
pub fn founder_genome(limits: &LimitsConfig) -> Genome {
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
