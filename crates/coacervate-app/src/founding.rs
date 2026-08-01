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
/// ⭐ Visible to the crate since Phase 7's Group L, because `assay.rs` seeds its own founders
/// and a competition assay whose founders started life holding a different amount from a run's
/// would be measuring a different world from the one every figure in `docs/PHASE7.md` was taken
/// on.
pub(crate) const FOUNDER_ENERGY: f64 = 2.0;

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

    for founder in 0..founders {
        world
            .seed(
                founder_genome(&limits),
                place(founder, founders, width, height),
                FOUNDER_ENERGY,
            )
            .expect("a lit world has room and water for the founders it was asked for");
    }

    taken
}

/// Where founder `founder` of `founders` goes, on the even grid this module's header describes.
///
/// The middle of that founder's share of the world, so the gap between two founders is the same
/// as the gap between the outermost and the edge - which across the width is also the gap over
/// the seam, since SPEC section 8 wraps there.
///
/// ⭐ A function since Phase 7's Group L rather than four lines inside [`genesis`], because the
/// competition assay in `assay.rs` puts **two** founder sets on this same grid at alternating
/// positions. Where a founder goes has to be one computation: an assay whose arms stood
/// somewhere other than a run's founders would be measuring a different experiment from the one
/// it is meant to predict, and neither of the two copies would look wrong.
pub(crate) fn place(founder: u32, founders: u32, width: f32, height: f32) -> Vec2 {
    let columns = columns_of(founders, width, height);
    let rows = founders.div_ceil(columns);

    Vec2::new(
        width * middle(founder % columns, columns),
        height * middle(founder / columns, rows),
    )
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
///
/// ⭐ Visible to the crate since Phase 7's Group L: `assay.rs` seeds its arms into the shipped
/// world *after the dawn*, and a dawn worked out a second time there would be a second stopping
/// rule for the one moment a run begins at.
pub(crate) fn dawn(world: &mut World) -> u64 {
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
///
/// ⚠️ **That last clause was long thought to be the whole of why every body in this world is two
/// cells for the first hundred thousand ticks of a run**, and it is a fact about the founder only
/// by accident: development stops at *any* cell whose state no gene names, and only **2.2%** of
/// grown cells sat in a state their own genome named when that was first measured.
///
/// ⭐⭐ **Phase 7's Group J biased where a re-drawn state lands and took that fraction to 17.7%,
/// and mean body size did not move.** So the founder being two cells is still true of the founder
/// and is no longer an explanation of the population: what holds bodies at two cells while the
/// world is filling appears to be the energy budget rather than the addressing. See SPEC sections
/// 7 and 15, and `docs/PHASE7.md`'s Group J and Q32.
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

#[cfg(test)]
mod tests {
    use super::genesis;
    use coacervate_sim::config::{Config, RawConfig, spec_defaults};
    use coacervate_sim::world::World;

    /// A world a sixteenth of the shipped one in area, at the shipped **light**.
    ///
    /// The light is deliberately not raised the way `run.rs`'s small world raises it. Both tests
    /// here are about the *season*, whose floor is `cap / influx` and whose whole subject is what
    /// the light does over time, so a brighter world would be a different experiment. What is cut
    /// is the number of tiles and the arena, and the arena is cut by less than the water is, so
    /// the energy budget still binds before the arena does — carrying capacity here is about 140
    /// bodies against a cap of 250.
    fn a_sixteenth_of_the_world(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        raw.world.width = 512.0;
        raw.world.height = 288.0;
        raw.world.grid_cols = 64;
        raw.world.grid_rows = 36;
        raw.limits.max_organisms = 250;
        change(&mut raw);

        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// ⭐⭐ **The season does not begin until something is alive to feel it.**
    ///
    /// [`dawn`] stops on `gained / after < DAWN_SETTLED`, which is a **light-dependent** test. So
    /// a season running through the dawn changes the dawn's *length* — and a seasoned run and a
    /// flat run would then start their clocks at different ticks, against different fields, with
    /// nothing in either recording saying so. Every comparison this project has ever made
    /// between two profiles assumes the two worlds began at the same moment.
    ///
    /// ⚠️ **And there is a second reason, which is about the founders rather than the clock.**
    /// A season whose phase were the world's tick count would land the founders of a shipped run
    /// at 0.83× and falling towards the trough 1.6 generations later. Every survival figure in
    /// `docs/PHASE7.md` was taken on level light at the founding; a season that moved the
    /// founding would quietly invalidate all of them rather than adding to them.
    #[test]
    fn the_season_does_not_begin_until_something_is_alive_to_feel_it() {
        let flat = a_sixteenth_of_the_world(|_| {});
        let seasoned = a_sixteenth_of_the_world(|raw| raw.light.season_amplitude = 0.25);

        let mut plain = World::new(&flat);
        let mut weathered = World::new(&seasoned);

        let plain_dawn = genesis(&mut plain, 8);
        let weathered_dawn = genesis(&mut weathered, 8);

        assert_eq!(
            plain_dawn, weathered_dawn,
            "a seasoned world's dawn took {weathered_dawn} ticks against a flat world's \
             {plain_dawn}, so the two runs start their clocks at different moments and no \
             figure taken on one is comparable with a figure taken on the other"
        );
        assert_eq!(
            weathered
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            plain
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            "the two worlds' founders were seeded into different water"
        );
        assert!(
            weathered.grid().season_phase().abs() < f64::EPSILON,
            "the first founder was placed with the season already {} of the way through itself",
            weathered.grid().season_phase()
        );

        // And once there is something alive, it starts.
        weathered.tick();
        assert!(
            weathered.grid().season_phase() > 0.0,
            "the season never starts at all, so `light.season_amplitude` is a setting with no \
             effect and the config file describes a world that does not exist"
        );
    }

    /// ⭐⭐ **Energy is conserved across three whole seasons of a living world**, checked on every
    /// single tick.
    ///
    /// SPEC section 5's invariant is already asserted by `World::tick` — every tick in a debug
    /// build and periodically in a release one. What this adds is **where** it is asserted: three
    /// whole periods of a seasoned world, so the books are checked across the **trough**, which
    /// is where biomass falls fastest and organisms are likeliest to die owing. Death is the one
    /// movement in section 5's ledger that runs backwards, and a season is the only thing in this
    /// world that makes it happen to a great many bodies at once on a known schedule.
    ///
    /// Sixty-three thousand ticks at the shipped period, in a world a sixteenth of the shipped
    /// area at the shipped light — see [`a_sixteenth_of_the_world`]. What is cut is the number of
    /// tiles, so the season, the ecology and the trough are all the shipped ones and only the
    /// cost of a tick is not.
    #[test]
    #[ignore = "63,000 ticks; check.ps1 runs it via --include-ignored in the release pass"]
    fn energy_is_conserved_across_three_whole_seasons() {
        let settings = a_sixteenth_of_the_world(|raw| raw.light.season_amplitude = 0.25);
        let period = settings.light.season_period;

        let mut world = World::new(&settings);
        genesis(&mut world, 8);

        let (mut fewest, mut most) = (usize::MAX, 0);
        for tick in 1..=period * 3 {
            world.tick();

            // Every tick, and in the release profile too, which is the whole point: `World::tick`
            // checks the books every hundredth tick there, and the trough is where a tick that
            // does not balance would be.
            world.ledger().check(world.grid().total_energy());

            // The swing is read over the **last two periods** only. A world founded with eight
            // bodies runs from eight to a few hundred whatever the weather is doing, so a
            // minimum taken over the whole run would say a season had happened when only a
            // founding had.
            if tick > period {
                let alive = world.organisms().iter().flatten().count();
                fewest = fewest.min(alive);
                most = most.max(alive);
            }
        }

        assert!(
            fewest > 0,
            "the world went extinct, so most of these sixty-three thousand ticks checked the \
             books of an empty world"
        );
        // ⚠️ A **half**, not a doubling, and the figure is measured rather than aimed at: this
        // world runs between **90 and 173** over its last two seasons, which is a 1.92-fold
        // swing. The 330,000-tick runs of the full world read 4.5-fold at the same amplitude,
        // and most of that is the secular fall of a population that is still finding its level
        // rather than the season. What this bound is for is only that there **was** a trough for
        // the invariant to be asserted across.
        assert!(
            2 * most > 3 * fewest,
            "the population ran between {fewest} and {most} over the last two of three whole \
             seasons, which is less than a half again — so this ran without a trough in it and \
             the invariant was never asserted across the one movement in SPEC section 5's ledger \
             that runs backwards"
        );
    }
}
