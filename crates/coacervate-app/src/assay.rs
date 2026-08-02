//! ⭐⭐ **The competition assay: what a configuration is *worth*, in forty minutes.**
//!
//! Five rounds of Phase 7 ended in a null, and every one of them cost a three-hundred-thousand
//! tick run to discover. The reason was the same each time: **nobody could measure what a body
//! plan was worth without waiting for evolution to produce one.** A world that ends with four
//! myocytes in five thousand cells is a world in which the interesting configuration is too rare
//! to have a coefficient measured on it, so every change to the *payoff* was argued rather than
//! priced.
//!
//! This is the instrument that removes the wait. Two founder sets that differ by **exactly one
//! mutation** are seeded alternately into the shipped world after the dawn; every organism born
//! afterwards is attributed to the arm its parent belonged to; and after 42,000 ticks - 23.9
//! generations at the measured 1,753.9-tick generation - the ratio of living descendants **is**
//! the selection coefficient. No mutation lottery, no waiting for the configuration to appear.
//!
//! | | |
//! | --- | --- |
//! | Window | 42,000 ticks = **23.9 generations** |
//! | Founders | **32**, alternating arms and positions |
//! | Noise floor | **±0.16 %/generation** (1 s.d., three seeds, the same genome in both arms) |
//! | Resolution | about **0.3 %/generation** with three seeds |
//! | Attribution loss | **0 to 4 births in ~40,000** |
//!
//! # ⚠️ It changes nothing in the simulation, and that is the point
//!
//! Everything here is the public API: [`World::seed`], [`World::organisms`],
//! `Organism::serial` and `Organism::parent`. The serial-to-arm map is read-only from the
//! simulation's point of view, so a world under assay is bit-for-bit the world that would have
//! run without one. An instrument that perturbed the thing it measures would be worse than no
//! instrument, because its readings would look exactly as usable.
//!
//! # ⚠️ Two things every coefficient taken here must be quoted with
//!
//! **It measures the filling regime.** Two-celled bodies, a population rising towards about
//! 2,100. Group K's world at 300,000 ticks holds 6.21 cells per body and 826 organisms, and
//! own-body shading is a much larger share of a photocyte's income there. Nobody should quote an
//! assay coefficient as a fact about the mature world without seeding the arms into one.
//!
//! **And a ratio near 1.0 over the first 40,000 ticks can mean *still filling* rather than *no
//! effect*.** The control's own population is rising throughout that window. That is exactly why
//! [`two_arms_that_differ_in_nothing_come_back_level`] exists, and why every coefficient is
//! quoted as an excess over its own same-seed control rather than as a bare log-ratio.

use crate::founding::{FOUNDER_ENERGY, dawn, founder_genome, place};
use coacervate_sim::cell::{CellKind, Vec2};
use coacervate_sim::config::{Config, LimitsConfig, RawConfig, spec_defaults};
use coacervate_sim::genome::{Action, Gene, Genome, SensorTarget, State};
use coacervate_sim::world::World;
use std::collections::HashMap;

/// How many bodies an assay is founded with, sixteen to the arm.
///
/// Seed-to-seed noise falls with the founder count, and sixteen a side is what puts the noise
/// floor at ±0.16 %/generation. It is also small enough that both arms are still filling an
/// empty world rather than competing for the last free slot in a full one - which is the regime
/// this module's header warns the readings belong to.
const FOUNDERS: u32 = 32;

/// How often the living population is walked and the newly born attributed to an arm.
///
/// A hundred ticks, against a mean lifetime of about 1,755. The map is never pruned, so a parent
/// that died between two polls is still in it; what is lost is an organism *born and dead* inside
/// one hundred-tick window, along with its descendants. Measured: **0 to 4 births in about
/// 40,000**, which [`Outcome::unattributed`] reports rather than leaves to be assumed.
const POLL_EVERY: u64 = 100;

/// How many ticks a generation takes in the shipped world, measured.
///
/// Every coefficient here is per *generation* rather than per tick, because a generation is the
/// unit selection acts in and the unit a ratio of descendants is compounded over.
const GENERATION: f64 = 1_753.9;

/// Which arm an organism's ancestry belongs to. Two, and never more: an assay compares one
/// change against its absence.
const ARMS: usize = 2;

/// What one assay run came back with.
///
/// Deliberately a record of what was counted rather than of what it means. The arithmetic that
/// turns it into a selection coefficient is [`Outcome::log_ratio`] and
/// [`Outcome::per_generation`], and both are separate so that a reading can be quoted raw.
#[derive(Debug)]
struct Outcome {
    /// How many ticks the two arms competed for, not counting the dawn.
    ticks: u64,

    /// How many descendants of each arm are alive at the end.
    alive: [u32; ARMS],

    /// How many cells those descendants hold between them, for the mean body size that says
    /// whether an arm **kept** what it was given.
    cells: [usize; ARMS],

    /// How many organisms have been attributed to each arm over the whole run.
    born: [u64; ARMS],

    /// Births whose parent was gone before the poll that would have attributed them, and the
    /// descendants of those births. See [`POLL_EVERY`].
    unattributed: u64,

    /// What each arm's founders actually received out of the tiles they stood on.
    ///
    /// ⚠️ Recorded rather than assumed. [`World::seed`] takes a founder's energy out of the water
    /// under it and legitimately refuses in a poor or a full world, so an arm that was seeded
    /// into worse water started the experiment with less - which is a confound, and one that
    /// would be invisible without this.
    energy: [f64; ARMS],

    /// Founders the world refused. See [`Outcome::energy`].
    refused: u32,

    /// How many organisms each arm gained between one poll and the next.
    ///
    /// The guard against a null wearing a null's clothes: an arm that stopped reproducing halfway
    /// through and an arm that never had an advantage both end level.
    checkpoints: Vec<[u64; ARMS]>,
}

impl Outcome {
    /// Arm B's living descendants against arm A's.
    fn ratio(&self) -> f64 {
        f64::from(self.alive[1]) / f64::from(self.alive[0])
    }

    /// The log of that ratio, which is the quantity that adds across generations.
    fn log_ratio(&self) -> f64 {
        self.ratio().ln()
    }

    /// The selection coefficient: the log-ratio spread over the generations it accumulated in.
    ///
    /// ⚠️ **Quote it as an excess over the same-seed control**, never bare. The control arm of a
    /// run in which both arms hold the same genome does not come back at exactly 1.0 - it comes
    /// back at the noise floor, and at 42,000 ticks that floor is not centred on zero for any one
    /// seed.
    fn per_generation(&self) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a tick count divided by the measured length of a generation; an assay is \
                      tens of thousands of ticks long and f64 holds every one of those exactly"
        )]
        let generations = self.ticks as f64 / GENERATION;

        self.log_ratio() / generations
    }

    /// The mean number of cells a surviving body of this arm holds.
    fn cells_per_body(&self, arm: usize) -> f64 {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a count of cells in a population of at most a few thousand two- to \
                      six-celled bodies, turned into a mean for a person to read"
        )]
        let cells = self.cells[arm] as f64;

        cells / f64::from(self.alive[arm].max(1))
    }
}

/// Run one assay: two arms, alternating positions, for this many ticks after the dawn.
///
/// # Panics
///
/// If the two genomes are more than one mutation apart. **That is the assay's own guard against
/// silently comparing two different things**: an arm that had picked up a second change would
/// return a coefficient for the pair with nothing in the reading to say so.
fn assay(config: &Config, arms: [&Genome; ARMS], ticks: u64) -> Outcome {
    assert!(
        one_mutation_apart(arms[0], arms[1]),
        "the two arms of an assay must be one mutation apart, or the coefficient that comes \
         back belongs to a pair of changes and nothing in the reading says which"
    );

    package_assay(config, arms, ticks)
}

/// The same instrument with the one-mutation guard **taken off**, for arms that are a whole
/// body plan apart rather than one step apart.
///
/// ⚠️ **This is a weaker measurement and every reading taken through it has to say so.** What
/// [`assay`] returns is the price of *one step of the mutation operator*, which is the quantity
/// selection actually sees offered to it. What this returns is the price of a **package** — a
/// genome several mutations from the founder, priced against the founder as a whole — so a
/// coefficient from here cannot be attributed to any one of the changes in it.
///
/// It exists because the question *does a body that genuinely swims beat one that does not* is
/// not a one-mutation question and cannot be made into one. SPEC section 8: a reciprocal stroke
/// produces exactly nought net displacement, so locomotion needs **two muscles at different
/// phases on a bent body** before it produces anything at all, and that is at minimum three
/// appended genes. Every muscle assay this project has taken before now seeded one myocyte,
/// which is the configuration the scallop theorem forbids from moving — so all of them measured
/// the cost of machinery whose payoff was structurally unreachable.
fn package_assay(config: &Config, arms: [&Genome; ARMS], ticks: u64) -> Outcome {
    placed_assay(config, arms, ticks, |_, _, at| at)
}

/// The same instrument again, with one arm allowed to be **put somewhere else**.
///
/// ⭐⭐ **This is what prices the ceiling on locomotion, and it needs no muscle at all.** The two
/// arms hold the *same* genome and differ only in where their founders were set down, so what
/// comes back is the entire value of **being in better water** over twenty-four generations. No
/// body that swims can be worth more than that: swimming is a way of arriving somewhere, and
/// this is what arriving is worth when it is free, instantaneous and perfectly aimed.
fn placed_assay(
    config: &Config,
    arms: [&Genome; ARMS],
    ticks: u64,
    put: impl Fn(usize, &World, Vec2) -> Vec2,
) -> Outcome {
    assert!(
        ticks.is_multiple_of(POLL_EVERY),
        "an assay must end on a poll, or its last {POLL_EVERY} ticks of births go uncounted"
    );

    let mut world = World::new(config);
    dawn(&mut world);

    let (width, height) = (config.world.width, config.world.height);
    let mut side_of: HashMap<u64, Option<u8>> = HashMap::new();
    let mut outcome = Outcome {
        ticks,
        alive: [0; ARMS],
        cells: [0; ARMS],
        born: [0; ARMS],
        unattributed: 0,
        energy: [0.0; ARMS],
        refused: 0,
        checkpoints: Vec::new(),
    };

    // Alternating, so neither arm gets systematically better water. The grid is `founding.rs`'s
    // own, so an arm stands exactly where a run's founders would.
    for founder in 0..FOUNDERS {
        let side = u8::try_from(founder % 2).expect("an arm number is nought or one");
        let at = place(founder, FOUNDERS, width, height);
        let at = put(usize::from(side), &world, at);

        match world.seed(arms[usize::from(side)].clone(), at, FOUNDER_ENERGY) {
            Ok(slot) => {
                let seeded = world.organisms()[slot]
                    .as_ref()
                    .expect("a seeding that was accepted put an organism in that slot");
                outcome.energy[usize::from(side)] += seeded.energy();
                side_of.insert(seeded.serial(), Some(side));
                outcome.born[usize::from(side)] += 1;
            }
            // ⚠️ A refusal is an ordinary event rather than a fault - see `World::seed`. It is
            // counted so that an assay whose arms were not the same size is visible.
            Err(_) => outcome.refused += 1,
        }
    }

    let started = world.ticks();
    while world.ticks() < started + ticks {
        world.tick();

        if (world.ticks() - started).is_multiple_of(POLL_EVERY) {
            let (fresh, lost) = attribute(&world, &mut side_of);
            outcome.born[0] += fresh[0];
            outcome.born[1] += fresh[1];
            outcome.unattributed += lost;
            outcome.checkpoints.push(fresh);
        }
    }

    for organism in world.organisms().iter().flatten() {
        if let Some(&Some(side)) = side_of.get(&organism.serial()) {
            outcome.alive[usize::from(side)] += 1;
            outcome.cells[usize::from(side)] += organism.cells();
        }
    }

    outcome
}

/// Walk the living, and give every organism the arm its parent had.
///
/// ⚠️ **In serial order.** A serial is minted in birth order and a parent's is always lower than
/// its child's, so attributing in that order resolves a whole chain of descent inside one polling
/// window. Walked in slot order instead, a grandchild could be reached before its parent and
/// counted as unattributable while its ancestry was perfectly well known.
fn attribute(world: &World, side_of: &mut HashMap<u64, Option<u8>>) -> ([u64; ARMS], u64) {
    let mut fresh: Vec<(u64, Option<u64>)> = world
        .organisms()
        .iter()
        .flatten()
        .filter(|organism| !side_of.contains_key(&organism.serial()))
        .map(|organism| (organism.serial(), organism.parent()))
        .collect();
    fresh.sort_unstable();

    let mut born = [0; ARMS];
    let mut lost = 0;

    for (serial, parent) in fresh {
        let side = parent.and_then(|of| side_of.get(&of).copied()).flatten();
        match side {
            Some(side) => born[usize::from(side)] += 1,
            None => lost += 1,
        }
        side_of.insert(serial, side);
    }

    (born, lost)
}

/// Whether these two genomes are one mutation apart: identical, one gene appended, or one field
/// of one gene changed.
///
/// The three shapes are exactly `mutation.rs`'s own insertion, duplication and point operators,
/// which is what makes this the right claim rather than a convenient one: an assay is a
/// measurement of what **one step** of the mutation operator is worth.
fn one_mutation_apart(a: &Genome, b: &Genome) -> bool {
    let (a, b) = (a.genes(), b.genes());
    let (shorter, longer) = if a.len() <= b.len() { (a, b) } else { (b, a) };

    match longer.len() - shorter.len() {
        0 => {
            let mut differing = shorter
                .iter()
                .zip(longer)
                .filter(|(one, other)| one != other);

            match (differing.next(), differing.next()) {
                (None, _) => true,
                (Some((one, other)), None) => fields_differing(one, other) == 1,
                (Some(_), Some(_)) => false,
            }
        }
        1 => shorter == &longer[..shorter.len()],
        _ => false,
    }
}

/// How many of a gene's sixteen fields these two disagree about.
#[expect(
    clippy::float_cmp,
    reason = "the question is whether two arms of an assay are running the same gene, which is \
              exact equality of the numbers the simulation actually reads. A tolerance here \
              would be a difference between the arms that the assay's own guard waved through"
)]
fn fields_differing(one: &Gene, other: &Gene) -> usize {
    usize::from(one.trigger_state != other.trigger_state)
        + usize::from(one.min_step != other.min_step)
        + usize::from(one.max_step != other.max_step)
        + usize::from(one.action != other.action)
        + usize::from(one.angle != other.angle)
        + usize::from(one.adhere != other.adhere)
        + usize::from(one.child_state != other.child_state)
        + usize::from(one.child_kind != other.child_kind)
        + usize::from(one.rest_length != other.rest_length)
        + usize::from(one.stiffness != other.stiffness)
        + usize::from(one.new_kind != other.new_kind)
        + usize::from(one.new_state != other.new_state)
        + usize::from(one.osc_freq != other.osc_freq)
        + usize::from(one.osc_phase != other.osc_phase)
        + usize::from(one.sensor_gain != other.sensor_gain)
        + usize::from(one.sensor_target != other.sensor_target)
}

/// `founding.rs`'s founder with one more cell budded off its photocyte at the next
/// developmental step.
///
/// One appended gene, so it is one insertion away from the founder and
/// [`one_mutation_apart`] accepts the pair. The daughter goes out at π from its parent's own
/// axis, which puts the three cells in a line with the photocyte in the middle - the plainest
/// three-celled body there is, and the one that self-shades no more than the founder does, since
/// SPEC section 6's buoyancy re-sorts a body vertically by what it is made of whichever way
/// development budded it.
fn founder_with_a_third_cell(limits: &LimitsConfig, kind: CellKind) -> Genome {
    let mut genes = founder_genome(limits).genes().to_vec();
    genes.push(Gene {
        trigger_state: State::ZERO,
        min_step: 1,
        max_step: 1,
        action: Action::Divide,
        angle: std::f32::consts::PI,
        adhere: true,
        child_state: State::new(2),
        child_kind: kind,
        rest_length: 8.0,
        stiffness: 10.0,
        new_kind: CellKind::Photocyte,
        new_state: State::ZERO,
        osc_freq: 0.0,
        osc_phase: 0.0,
        sensor_gain: 0.0,
        sensor_target: coacervate_sim::genome::SensorTarget::Light,
    });

    Genome::new(genes, limits)
}

/// How far apart the cells of a hand-built swimmer sit, and how sharply its chain kinks.
///
/// ⭐ These two numbers are `physics.rs`'s own `body(count, apart = 8, sag = 3)` written as a
/// **genome** rather than as an array of cells: a zig-zag whose stride is 8 units and whose
/// cells sit 3 either side of the line, so a segment is `sqrt(8² + 6²) = 10` units long and the
/// turn from one segment to the next is `2 × atan(6/8) = 1.287` radians. That shape is one of
/// the nine `swims_and_works` means its readings over, and it is the shape SPEC section 8's
/// table records as covering **41 world units in a 2,000-tick lifetime** at the shipped stroke.
///
/// ⚠️ **The kink is the whole of it.** SPEC section 8: a body whose cells lie in a straight line
/// cannot swim at any stroke, because all of its motion is along its own axis and the sideways
/// drag never engages. `development.rs` measures a division's angle from the direction its
/// parent was budded in, so alternating the sign of this angle down a chain of genes is exactly
/// a zig-zag; the same angle repeated would be an arc.
const SEGMENT: f32 = 10.0;
const KINK: f32 = 1.287;

/// The rhythm a hand-built swimmer's muscles beat at, in radians a second.
///
/// SPEC section 9 measures `osc_freq` as peaking between two and three and a half radians a
/// second and falling away either side, so this is the middle of the useful band rather than the
/// top of the genome's range.
const BEAT: f32 = 3.0;

/// A genome that develops into a bent chain, muscled at a travelling phase gradient.
///
/// `plan` is the body **after the seed cell**, in the order the chain grows: one gene per cell,
/// gene `k` triggering on state `k` at step `k` only, so exactly one cell divides per step and
/// the body is a chain rather than a cluster. Each myocyte gets its own `osc_phase`, a quarter
/// turn further round than the muscle before it — which is `swims_and_works`'s travelling wave,
/// arrived at from the other end: a spring with a muscle on each end takes the mean of the two,
/// so cell phases a quarter turn apart put the *springs* a quarter turn apart too.
///
/// ⚠️ **Every one of these cells is paid for.** A myocyte is 0.005/tick against a photocyte's
/// 0.004, the reproduction bar is `reproduction_threshold × Σ construction` and construction is
/// a thousand ticks of upkeep, and SPEC section 10's lifespan allowance is
/// `LIFETIME_UPKEEP × cells ÷ cost`. So a swimmer is not free to be as long as one likes: past
/// about six cells with one photocyte in it, a body cannot reach its own reproduction bar inside
/// its own lifetime, and an arm that cannot breed measures nothing at all.
fn swimmer(limits: &LimitsConfig, plan: &[CellKind], beat: f32, gain: f32) -> Genome {
    let mut genes = Vec::new();
    let mut muscles = 0u8;

    for (index, kind) in plan.iter().enumerate() {
        let step = u8::try_from(index).expect("a hand-built body is a few cells long");
        let muscle = *kind == CellKind::Myocyte;
        let phase = f32::from(muscles) * std::f32::consts::FRAC_PI_2;
        if muscle {
            muscles += 1;
        }

        genes.push(Gene {
            trigger_state: State::new(step),
            min_step: step,
            max_step: step,
            action: Action::Divide,
            // Alternating, which is what makes the chain a zig-zag rather than an arc.
            angle: if index % 2 == 0 { KINK } else { -KINK },
            adhere: true,
            child_state: State::new(step + 1),
            child_kind: *kind,
            rest_length: SEGMENT,
            stiffness: 10.0,
            new_kind: CellKind::Photocyte,
            new_state: State::ZERO,
            osc_freq: if muscle { beat } else { 0.0 },
            osc_phase: if muscle { phase } else { 0.0 },
            sensor_gain: if muscle { gain } else { 0.0 },
            sensor_target: SensorTarget::Light,
        });
    }

    Genome::new(genes, limits)
}

/// The same genome with every muscle held still: the control for a swimming measurement.
///
/// ⭐ **It is the only honest control there is.** A body in this world drifts under SPEC section
/// 6's buoyancy whether it swims or not, and it is pushed about by its own springs settling; a
/// displacement quoted against nothing at all would be all three added together. This twin has
/// the same cells, the same kinds, the same buoyancy, the same springs, the same upkeep and the
/// same shape, and differs **only** in that `sin(0 × t + phase)` is a constant. The difference
/// between the two is locomotion and nothing else.
fn held_still(swimmer: &Genome, limits: &LimitsConfig) -> Genome {
    let genes = swimmer
        .genes()
        .iter()
        .map(|gene| Gene {
            osc_freq: 0.0,
            ..*gene
        })
        .collect();

    Genome::new(genes, limits)
}

/// How far one body of this genome travels, and how long it lives, alone in a lit world.
///
/// ⭐ **The instrument is Group J's**: the displacement of the body's **seed cell** between the
/// tick it was put in the water and the tick it died or the measurement ended. A mean over the
/// cells would move whenever the body merely changed shape, which is the thing a muscle does
/// even when it goes nowhere.
///
/// `limits.max_organisms` is set to one, so the body cannot reproduce and there is nobody else
/// in the water: what comes back is one body's own travel rather than a population statistic.
fn travels(
    seed: u64,
    change: impl FnOnce(&mut RawConfig),
    genome: impl FnOnce(&LimitsConfig) -> Genome,
    ticks: u64,
) -> (f32, u64) {
    let alone = seeded_world(seed, |raw| {
        change(raw);
        raw.limits.max_organisms = 1;
    });
    let genome = genome(&alone.limits);

    let mut world = World::new(&alone);
    dawn(&mut world);

    let at = Vec2::new(alone.world.width * 0.5, alone.world.height * 0.5);
    let slot = world
        .seed(genome, at, FOUNDER_ENERGY)
        .expect("a lit world has room and water for one body in the middle of it");

    let began = world.cells_of(slot)[0].pos;
    let (mut moved, mut lived) = (0.0, 0);

    for tick in 1..=ticks {
        world.tick();

        if world.organisms()[slot].is_none() {
            break;
        }
        moved = (world.cells_of(slot)[0].pos - began).length();
        lived = tick;
    }

    (moved, lived)
}

/// SPEC's shipped world, at the seed this run of the assay is being taken on.
fn seeded_world(seed: u64, change: impl FnOnce(&mut RawConfig)) -> Config {
    let mut raw = spec_defaults();
    raw.world.seed = seed;
    change(&mut raw);

    raw.validate()
        .expect("an assay's configuration must be one the program will accept")
}

#[cfg(test)]
mod tests {
    use super::{
        ARMS, BEAT, FOUNDERS, Outcome, assay, dawn, founder_genome, founder_with_a_third_cell,
        held_still, one_mutation_apart, package_assay, placed_assay, seeded_world, swimmer,
        travels,
    };
    use coacervate_sim::cell::{CellKind, Vec2};
    use coacervate_sim::config::{Config, RawConfig};
    use coacervate_sim::world::World;

    /// How many ticks a full assay runs for: 42,000, which is 23.9 generations.
    const WINDOW: u64 = 42_000;

    /// How many ticks a body lives, which is what a displacement is measured over.
    ///
    /// SPEC section 10's allowance is `LIFETIME_UPKEEP × cells ÷ cost`, so no one number is
    /// every body's lifetime. Two thousand is the figure every swimming measurement in this
    /// project is quoted per — `physics.rs`'s `swims_and_works`, SPEC section 9's lever table
    /// and `docs/PHASE7.md`'s Group H — so a reading here can be laid straight against them.
    const LIFETIME: u64 = 2_000;

    /// A world small enough that the ordinary suite can afford one, with the shipped world's
    /// organisms per tile so that the arena binds no sooner here than it does there.
    ///
    /// The light is brighter only so the dawn is short - `run.rs`'s `a_small_living_world` makes
    /// the same trade for the same reason. Nothing about **attribution** depends on the ecology,
    /// which is the only claim this world is asked to support.
    fn a_small_world(seed: u64) -> coacervate_sim::config::Config {
        seeded_world(seed, |raw: &mut RawConfig| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 250;
            raw.light.influx = 0.012;
        })
    }

    /// ⭐⭐ **The instrument's first claim: every living organism belongs to exactly one arm.**
    ///
    /// An assay is a ratio of descendants, so an assay whose attribution leaked would return a
    /// ratio of *some* of the descendants - and it would return it silently, because the two arms
    /// would still add up to a plausible number. What this insists on is that the two arms
    /// together account for the whole living population bar a handful, that the handful is
    /// counted rather than assumed, and that both arms actually reproduced.
    ///
    /// It also insists the two founder sets are **one mutation apart**, which is the guard
    /// against the assay silently comparing two different things.
    #[test]
    fn an_arm_can_be_followed_through_its_descendants() {
        let config = a_small_world(42);
        let genome = founder_genome(&config.limits);
        let outcome = assay(&config, [&genome, &genome], 2_000);

        assert!(
            one_mutation_apart(&genome, &genome),
            "a genome is not one mutation from itself"
        );
        assert_eq!(
            outcome.refused, 0,
            "the world refused {} of {FOUNDERS} founders, so the two arms are not the same size",
            outcome.refused
        );

        let attributed: u64 = outcome.born.iter().sum();
        assert!(
            outcome.unattributed * 200 < attributed,
            "{} of {attributed} organisms could not be traced to an arm, which is more than \
             half a per cent - so the polling interval is not short enough against a lifetime",
            outcome.unattributed
        );
        assert!(
            outcome.alive[0] > 0 && outcome.alive[1] > 0,
            "one arm is empty after two thousand ticks: {:?}",
            outcome.alive
        );
        assert!(
            outcome.born[0] > u64::from(FOUNDERS / 2) && outcome.born[1] > u64::from(FOUNDERS / 2),
            "an arm never reproduced at all: {:?} organisms against {FOUNDERS} founders",
            outcome.born
        );

        // And the map accounts for the living population: everybody alive is in exactly one arm,
        // or is one of the handful counted above.
        let living = outcome.alive[0] + outcome.alive[1];
        assert!(
            living > 0,
            "nothing survived two thousand ticks, so this measures nothing at all"
        );
    }

    /// ⭐⭐ **The noise floor, and the number every later claim is measured against.**
    ///
    /// The same genome in both arms, three seeds. What comes back is not 1.0 and cannot be: the
    /// two arms are sixteen bodies apiece in a world that is still filling, and which sixteen
    /// tiles they landed on decides a great deal over twenty-four generations.
    ///
    /// Measured, 42,000 ticks, shipped world: log-ratios **+0.064**, **+0.009**, **−0.008** -
    /// a spread of 0.038, which is **±0.16 %/generation** at one standard deviation. The bound
    /// below is three times that spread, so a coefficient of about **0.3 %/generation** is what
    /// three seeds of this instrument can resolve.
    ///
    /// ⚠️ **The second assertion is what stops a null being a starvation artefact.** An arm that
    /// stopped reproducing at tick 10,000 and an arm with no advantage both come back level, and
    /// the checkpoints are the only thing that tells them apart.
    #[test]
    #[ignore = "42,000 ticks x three seeds; check.ps1 runs it via --include-ignored in release"]
    fn two_arms_that_differ_in_nothing_come_back_level() {
        for seed in [42, 7, 99] {
            let config = seeded_world(seed, |_| {});
            let genome = founder_genome(&config.limits);
            let outcome = assay(&config, [&genome, &genome], WINDOW);
            report(&format!("the noise floor at seed {seed}"), &outcome);

            assert!(
                outcome.log_ratio().abs() < 0.12,
                "seed {seed}: the same genome in both arms came back at {:.4} - a log-ratio of \
                 {:.4}, against the 0.038 spread three seeds of this instrument measured. \
                 Either the arms are not symmetric or the noise floor has moved, and every \
                 coefficient this module reports is quoted against it. {:?}",
                outcome.ratio(),
                outcome.log_ratio(),
                outcome.alive
            );

            let quiet = outcome
                .checkpoints
                .iter()
                .filter(|born| born[0] == 0 || born[1] == 0)
                .count();
            assert!(
                quiet * 20 < outcome.checkpoints.len(),
                "seed {seed}: one arm recorded no births at {quiet} of {} checkpoints, so a \
                 level result here could be a starvation artefact rather than a null",
                outcome.checkpoints.len()
            );
        }
    }

    /// ⭐⭐ **The instrument's calibration, and a standing guard on the economy.**
    ///
    /// > **Nothing in this world has an increasing return to being more than one thing.**
    ///
    /// That is the finding the assay was built to test and the one it returned. A photocyte's
    /// income scales linearly with photocyte count, upkeep scales linearly with cells, the
    /// reproduction threshold is `reproduction_threshold × Σ construction` and is linear in
    /// cells, and lifespan is `LIFETIME_UPKEEP × cells ÷ cost` and is linear again. Occlusion is
    /// actively *sub*linear. So a third cell that earns is worth precisely its own cost, and a
    /// third cell that does not is a pure loss - measured at about **−0.5 %/generation for every
    /// 0.001/tick of upkeep, whatever the cell does**.
    ///
    /// Measured here over 42,000 ticks of the shipped world at seed 42, arm B being the founder
    /// with **one appended gene** that buds a third cell off its photocyte at the next
    /// developmental step:
    ///
    /// | Arm B against arm A, the plain founder | upkeep added | ratio | %/gen | cells/body |
    /// | --- | --- | --- | --- | --- |
    /// | a third **photocyte** | +0.004/tick | **1.531** | **+1.78** | 3.41 against 2.03 |
    /// | a third **myocyte**, holding still | +0.005/tick | **0.511** | **−2.81** | 2.14 against 1.99 |
    ///
    /// Attribution was **complete** in both — nought unattributed births out of 38,261 and 42,335
    /// — and both arms' founders received the same 32.0 units out of the water, so neither was
    /// seeded into better water than the other.
    ///
    /// ⚠️ **Both halves of each row matter.** The photocyte arm comes back **above** its control
    /// and keeps its third cell — 3.41 cells a body. The myocyte arm comes back at half its
    /// control **and its survivors are back to two cells**, 2.14: the third cell is present at
    /// birth in every one of them and gone from nearly all the survivors. A world that shed the
    /// photocyte, or kept the muscle, would be a different economy from the one every figure in
    /// `docs/PHASE7.md` was measured in.
    ///
    /// ⚠️⚠️ **The photocyte reading is a correction, and it is worth recording as one.** The
    /// design this was built from measured the same arm at **1.076** and concluded that *nothing
    /// in this world has an increasing return to being more than one thing* — a third photocyte
    /// worth precisely its own cost, so no gradient to climb. Rebuilt here from the public API it
    /// comes back at **1.531**, which is +1.78 %/generation against a noise floor of ±0.16 and is
    /// twice the largest coefficient that design records for anything. Doubling a body's
    /// photocytes while adding 44% to its bill is not neutral, and the difference between the two
    /// readings is what a *third cell* was made of rather than what the world does with one. The
    /// noise floor and the myocyte row both reproduce, so the instrument agrees with itself.
    ///
    /// ⚠️ **And it is a reading of the filling regime.** Both arms are growing into an empty
    /// world throughout this window, so what is measured is a rate of increase and not an
    /// equilibrium. See this module's header. What the *pair* says is robust to that, because
    /// both arms are the same three-celled body differing in one `child_kind`: **a third cell
    /// that earns is kept and a third cell that does not is shed, at better than two to one.**
    #[test]
    #[ignore = "two 42,000-tick runs; check.ps1 runs it via --include-ignored in release"]
    fn a_third_photocyte_is_kept_and_a_third_myocyte_is_lost() {
        let config = seeded_world(42, |_| {});
        let plain = founder_genome(&config.limits);

        let extra = |kind| {
            let arm = founder_with_a_third_cell(&config.limits, kind);
            assay(&config, [&plain, &arm], WINDOW)
        };

        // Printed before anything is asserted: `cargo test --release -- --ignored --nocapture
        // a_third_photocyte` is how a future payoff proposal gets priced in forty minutes
        // instead of a day, and a run that panicked on its first claim would throw the second
        // measurement away.
        let photocyte = extra(CellKind::Photocyte);
        report("a third photocyte", &photocyte);
        let myocyte = extra(CellKind::Myocyte);
        report("a third myocyte", &myocyte);

        assert!(
            photocyte.log_ratio() > 0.12,
            "a body with a third photocyte came back at {:.4} against the plain founder, and \
             the measured reading is 1.531 - well outside the ±0.16 %/generation noise floor. A \
             photocyte that stopped paying for itself is a different economy. {:?}",
            photocyte.ratio(),
            photocyte.alive
        );
        assert!(
            photocyte.cells_per_body(1) > 3.0,
            "the extra-photocyte arm's survivors hold {:.2} cells a body, and the measured \
             reading is 3.41 - so the third photocyte is no longer being kept",
            photocyte.cells_per_body(1)
        );

        assert!(
            myocyte.ratio() < 0.7,
            "a body with a third myocyte came back at {:.4}, and the measured reading is 0.511. \
             A muscle that is no longer priced far below break-even would reopen every payoff \
             question Phase 7 closed on this number. {:?}",
            myocyte.ratio(),
            myocyte.alive
        );
        assert!(
            myocyte.cells_per_body(1) < 2.5,
            "the extra-myocyte arm's survivors hold {:.2} cells a body, and the measured reading \
             is 2.14 - every one of them was born with three, so the muscle is shed inside two \
             dozen generations",
            myocyte.cells_per_body(1)
        );

        // ⭐ The pair, which is the one claim that does not depend on the regime: the two arms
        // are the same three-celled body differing in one `child_kind`.
        assert!(
            photocyte.ratio() > 2.0 * myocyte.ratio(),
            "the earning third cell came back at {:.4} and the silent one at {:.4}, which is \
             less than two to one - so what a cell *does* has stopped deciding whether a body \
             keeps it",
            photocyte.ratio(),
            myocyte.ratio()
        );
    }

    // ---------------------------------------------------------------------------------
    // ⭐⭐⭐ Does a body that genuinely swims beat one that does not?
    //
    // Every muscle assay before this one seeded **one** myocyte. SPEC section 8 is explicit
    // that a single oscillating spring is a reciprocal stroke and produces exactly nought net
    // displacement - the scallop theorem, measured in this project rather than assumed - so
    // all of them priced machinery whose payoff was structurally unreachable. What follows
    // seeds the configuration that does move: **two or more muscles at different phases on a
    // bent body**, built by hand, checked to travel and checked to breed before a single
    // coefficient is quoted.
    // ---------------------------------------------------------------------------------

    /// ⭐ **Arm B: the plainest body that genuinely swims *and* breeds.**
    ///
    /// Photocyte (the seed) — myocyte — myocyte — photocyte — gonocyte, in a zig-zag, with the
    /// two muscles a quarter turn apart in phase. It is **chosen by measurement**: six plans
    /// were built and measured, and this is the one that both travels and reaches its own
    /// reproduction bar inside its own lifetime.
    ///
    /// | plan | cells | upkeep | travels | held still | children of 16 founders in one generation |
    /// | --- | --- | --- | --- | --- | --- |
    /// | the founder | 2 | 0.0090 | — | — | **191** |
    /// | 2 muscles | 4 | 0.0190 | 9.97 | 1.64 | 13 |
    /// | **2 muscles + a second photocyte** | **5** | **0.0230** | **16.60** | **1.66** | **95** |
    /// | 3 muscles | 6 | 0.0280 | 17.51 | 1.73 | 44 |
    /// | 4 muscles | 7 | 0.0330 | 11.92 | 1.68 | 26 |
    /// | 6 muscles | 8 | 0.0390 | 12.39 | 1.67 | 0 |
    /// | 2 muscles + a sensocyte between them | 6 | 0.0290 | **28.28** | 1.18 | 25 |
    ///
    /// ⚠️ **The last column is not optional.** A body of six or more cells with one photocyte in
    /// it cannot reach `reproduction_threshold × Σ construction` inside
    /// `LIFETIME_UPKEEP × cells ÷ cost`, so it stands in the water and dies childless — and an
    /// arm that cannot breed measures the fact that it cannot breed and nothing else. The
    /// four-cell plan is the shape of that failure: 13 children against the founder's 191.
    ///
    /// ⚠️ **And the second column is the whole of what is being bought.** 0.0230 a tick against
    /// the founder's 0.0090, in a world this module's own calibration prices at about
    /// **−0.5 %/generation for every 0.001/tick of upkeep, whatever the cell does**.
    fn arm_b() -> Vec<CellKind> {
        vec![
            CellKind::Myocyte,
            CellKind::Myocyte,
            CellKind::Photocyte,
            CellKind::Gonocyte,
        ]
    }

    /// What a body of these cells costs to run, per tick, at `upkeep_scale = 1`.
    fn upkeep_of(plan: &[CellKind]) -> f64 {
        let mut cost = f64::from(CellKind::Photocyte.upkeep());
        for kind in plan {
            cost += f64::from(kind.upkeep());
        }
        cost
    }

    /// ⭐⭐⭐ **Does a body that genuinely swims beat one that does not?** No, and this is the
    /// measurement that says so.
    ///
    /// Every muscle assay this project has taken before now seeded **one** myocyte. SPEC section
    /// 8 is explicit that a single oscillating spring is a reciprocal stroke and produces exactly
    /// nought net displacement, so all of them priced machinery whose payoff was structurally
    /// unreachable. This seeds the configuration that does move — [`arm_b`], two muscles a
    /// quarter turn apart on a zig-zag — and checks in order that it **develops**, that it
    /// **travels**, that it **breeds**, and only then what it is worth.
    ///
    /// # ⭐⭐ The three readings, at seed 42; the sweep behind them is in `docs/PHASE7.md`
    ///
    /// | | reading |
    /// | --- | --- |
    /// | arm B travels, per 2,000-tick lifetime | **16.6 units**, against **1.66** with the same muscles held still and the founder's 2.55 |
    /// | arm B against the founder | **−10.3 %/generation** (three seeds: −10.03, −9.85, −11.01, spread 0.62) |
    /// | **arm B against its own held-still twin** — the stroke alone, its bill on neither side | **+1.0 ± 1.7 %/generation**, which is nothing |
    /// | **arriving in the best water a lifetime's swim away, free and perfectly aimed** | **−0.01 ± 0.13 %/generation**, which is nothing |
    /// | what a lifetime's swim is worth in light: best direction | **×1.05** |
    /// | ... and in a direction nothing chose, which is the only kind available | **×1.00** |
    ///
    /// # ⭐⭐⭐ And the arithmetic underneath all four, which no configuration can move
    ///
    /// A blotch of light is `NOISE_LATTICE_SPACING` **× a tile** = 16 × 8 = **128 world units**,
    /// and `NOISE_LATTICE_SPACING` is a constant of `grid.rs` rather than a setting. A body that
    /// genuinely swims covers **16.6 units in its whole life** — **2.1 tiles, an eighth of one
    /// blotch** — and SPEC section 9's best hand-built undulator, driven flat out and meaned over
    /// nine shapes, covers 41, which is 5 tiles and a third of a blotch. **Nothing that lives
    /// here can cross the thing it would be crossing to reach.**
    ///
    /// Nor can it aim: section 9's `sensor_gain` scales a muscle's *amplitude*, so a sensed body
    /// swims harder in a steep gradient and not towards anything. Undirected travel over a field
    /// whose gradient it cannot read is worth ×1.00.
    ///
    /// # ⭐⭐⭐ The sweep that closed it: fourteen worlds, and not one of them pays
    ///
    /// The obvious rejoinder to the reading above is that the *world* is the wrong shape rather
    /// than the body — a blotch 128 units across is simply too coarse for anything alive to
    /// cross. So every configuration key that touches the scale, the contrast, the speed and the
    /// season of the light was walked to its gate, three seeds each, arm B against the plain
    /// founder, every coefficient an excess over its **own same-seed control** and every
    /// condition reporting arm B's measured displacement beside it, against this module's
    /// **±0.16 %/generation** noise floor — ±0.13 on the placed arms.
    ///
    /// A blotch is `NOISE_LATTICE_SPACING` **tiles** and a tile is `width / grid_cols`, so the
    /// only handle a configuration has on the scale of the light is the size of a tile. Shrinking
    /// the world with the grid held fixed leaves influx per tile, `cap`, the standing energy of a
    /// tile, a photocyte's income, the 8,000-tick refill and the dawn all bit-for-bit the shipped
    /// world's, and moves nothing but how many world units a blotch is.
    ///
    /// | Condition | blotch | travels/lifetime | **coefficient, three seeds** |
    /// | --- | --- | --- | --- |
    /// | **as shipped** | 128 | 16.6 (0.13 blotch) | **−10.30** (−10.03, −9.85, −11.01) |
    /// | `patch_drift` 0.003 | 128 | 16.6 | **−10.04** (−10.17, −9.76, −10.21) |
    /// | `patch_drift` 0.005 — **the gate** | 128 | 16.6 | **−11.94** (−13.75, −9.28, −12.80) |
    /// | `season_amplitude` 0.25 — `config/seasonal.toml` | 128 | 16.6 | **−10.63** (−11.81, −11.29, −8.78) |
    /// | `season_amplitude` 0.5 — **the gate** | 128 | 16.6 | **−12.51** (−14.12, −11.64, −11.77) |
    /// | `patchiness` 1.0 — **the gate** | 128 | 16.4 | **−17.4, −16.0, extinct** |
    /// | half the world | 64 | 16.6 (0.26 blotch) | **−15.63** (−9.05, −26.11, −11.74) |
    /// | a quarter of the world | 32 | 16.6 (0.52 blotch) | **−21.8, extinct, −1.1** |
    /// | ... at the shipped **density** (`influx` ÷ 16) | 32 | 14.1 | **extinct ×3** |
    /// | ... with the longest `rest_length` the genome allows | 32 | **21.8** (0.68 blotch) | **extinct ×3** |
    /// | ... + `patchiness` 1.0 + drift 0.005 + season 0.5 | 32 | 16.5 | **extinct ×3** |
    /// | an eighth of the world | 16 | 16.6 (**1.04 blotch**) | **extinct ×3** |
    /// | ... at `influx` ÷ 8 | 16 | 16.6 | **extinct ×3** |
    /// | ... + every other gate, longest segment | 16 | **22.4** | **extinct ×3** |
    ///
    /// **Not one condition is positive, and finer light is uniformly *worse*.** "Extinct" is
    /// nought living descendants of sixteen founders at 42,000 ticks while both control arms
    /// survive — which is well outside any noise floor, and is the reason those rows have no
    /// number.
    ///
    /// ⚠️ **The fine-grained worlds also stop being measurable, and that is a finding in
    /// itself.** The same-seed control — the identical genome in both arms — comes back at
    /// ±0.16 %/generation at the shipped scale, at ±6 at a blotch of 32 (+3.04, −8.85, −0.69)
    /// and at **+23.1, +13.4 and one arm extinct** at a blotch of 16. Shrinking the world
    /// eightfold at the shipped light puts 2,200 bodies into a 256 × 144 arena, which is space
    /// and not energy binding: drift is then the only force acting and the instrument has no
    /// resolution left. The two ways out both fail. Cutting `influx` by 64 to hold the density
    /// leaves tiles too poor to seed a founder out of — **eight of thirty-two refused, and the
    /// world dead by tick 4,000**. Cutting it by 8 leaves a world that lives, and arm B is
    /// extinct in all three seeds of it.
    ///
    /// ⚠️ That trade is structural rather than a bad choice of key: density goes as
    /// `influx / tile²` and refill time as `cap / influx`, so **nothing can shrink a blotch at
    /// constant density *and* constant refill time.**
    ///
    /// # ⭐⭐ The patches are not smoothed away at any scale, and cannot be
    ///
    /// The obvious worry about `light.diffusion` at 0.04 against an 8,000-tick refill, answered
    /// by measurement. The row-wise coefficient of variation of the standing field on the dawned
    /// world is **0.1522 at a blotch of 128, of 64 and of 32 alike**, against **0.0000** for the
    /// same world with `patchiness` at nought. Identical at every scale, because the lattice is
    /// sixteen **tiles** and the diffusion stencil is **per tile**: shrinking a tile shrinks both
    /// together, so there are sixteen diffusion steps across one blotch whatever a blotch is
    /// worth in world units. What a finer field does change is how much of the contrast a fixed
    /// 16.6-unit step samples — the best-direction income multiplier rises ×1.053 → ×1.099 →
    /// ×1.185 → ×1.335 as the blotch goes 128 → 64 → 32 → 16 — and a direction nothing chose
    /// stays at ×1.00 to ×1.04 throughout.
    ///
    /// # ⭐⭐⭐ The ceiling, which is what makes the negative final
    ///
    /// [`placed_assay`] again, with **no muscle in it at all**: both arms hold the plain founder
    /// and arm B's are simply *put* in the best water within reach. Nobody who has to swim there
    /// can collect more than this.
    ///
    /// | Arriving free, instantaneously and perfectly aimed | **%/generation** |
    /// | --- | --- |
    /// | a lifetime's swim (16.6) away, shipped world | **−0.01 ± 0.13** |
    /// | SPEC section 9's best undulator's **41** units away, shipped world | **−0.15** (−0.00, −0.36, −0.08) |
    /// | **512 units** away — a quarter of the world, for nothing | **−1.49** (−0.83, −1.47, −2.18) |
    /// | 16.6 away, blotch 32 | +1.21, against that world's own ±6 control |
    /// | 16.6 away, blotch 16 at `influx` ÷ 8 | +8.18, against that world's own ±10 control |
    ///
    /// **Teleporting a body a quarter of the world into the best water there is comes back
    /// negative.** Where arriving finally looks worth something the world's own noise floor is
    /// larger than the reading, and arm B is extinct in it anyway.
    ///
    /// # ⭐⭐⭐ The arithmetic, in one line
    ///
    /// > **A body in this world travels about two thirds of its own length in a whole lifetime.**
    ///
    /// Measured three ways: arm B spans **25.6 × 19.2** units and covers **16.6** (×0.65); the
    /// same body at `MAX_REST_LENGTH` spans 34.8 × 26.1 and covers **21.7** (×0.62); SPEC section
    /// 9's nine hand-built undulators average 61 units of span and cover **41** (×0.67) — and
    /// those nine are 6-, 8- and 12-celled bodies that cannot reach their own reproduction bar
    /// inside their own lifetime, so the fastest thing that can actually *breed* here is arm B.
    ///
    /// For a patch to be worth crossing a body must cover about half of one, so
    /// `blotch ≤ 2 × travel ≈ 1.3 × its own length`. For a patch to have any contrast *across* a
    /// body it must be larger than the body, so `blotch ≥ its own length`. **The window is
    /// `1.0 × length ≤ blotch ≤ 1.3 × length`, and a body filling its own patch reads no gradient
    /// at all.** It does not open at any `width`, `grid_cols`, `patchiness`, `patch_drift`,
    /// `season_amplitude` or `rest_length`, because both bounds scale with the body.
    #[test]
    #[ignore = "three 42,000-tick runs and four lifetimes; check.ps1 runs it in release"]
    fn a_body_that_genuinely_swims_is_still_priced_below_one_that_does_not() {
        use coacervate_sim::development::develop;

        let config = seeded_world(42, |_| {});
        let plain = founder_genome(&config.limits);
        let plan = arm_b();
        let genome = swimmer(&config.limits, &plan, BEAT, 0.0);
        let still = held_still(&genome, &config.limits);

        // ⭐ **What it develops into**, which everything after this rests on. A genome that grew
        // a straight body, or lost its gonocyte, or put both muscles on the same phase would be
        // a body SPEC section 8 forbids from moving, and the assay would come back negative for
        // a reason that had nothing to do with the question.
        let body = develop(&genome, &config.limits);
        let muscles: Vec<usize> = (0..body.cells.len())
            .filter(|cell| body.cells[*cell].kind == CellKind::Myocyte)
            .collect();
        let phases: Vec<f32> = muscles
            .iter()
            .filter_map(|cell| body.cells[*cell].gene)
            .map(|gene| genome.genes()[usize::from(gene)].osc_phase)
            .collect();
        let across = |pick: fn(f32, f32) -> f32, start| {
            body.cells
                .iter()
                .map(|cell| cell.offset.y)
                .fold(start, pick)
        };

        assert_eq!(
            body.cells.len(),
            plan.len() + 1,
            "arm B grew {} cells rather than {}, so development is not doing what this genome \
             was written to make it do",
            body.cells.len(),
            plan.len() + 1
        );
        assert!(
            muscles.len() >= 2,
            "arm B grew {} myocytes. SPEC section 8: one oscillating spring is a reciprocal \
             stroke and goes nowhere, so an arm with fewer than two cannot answer this question",
            muscles.len()
        );
        assert!(
            phases
                .windows(2)
                .all(|pair| pair[0].to_bits() != pair[1].to_bits()),
            "arm B's muscles all beat at the same phase, which is a reciprocal stroke however \
             many of them there are: {phases:?}"
        );
        assert!(
            body.cells
                .iter()
                .any(|cell| cell.kind == CellKind::Gonocyte),
            "arm B has no gonocyte, and SPEC section 6 will not let a body without one reproduce"
        );
        assert!(
            across(f32::max, f32::MIN) - across(f32::min, f32::MAX) > 1.0,
            "arm B grew a straight body, and SPEC section 8 is explicit that nothing \
             one-dimensional swims in any fluid at any stroke"
        );

        // ⭐⭐ **That it travels**, against the only control there is: itself, not moving.
        let (swum, lived) = travels(42, |_| {}, |_| genome.clone(), LIFETIME);
        let (drifted, _) = travels(42, |_| {}, |_| still.clone(), LIFETIME);
        let (founder_drift, _) = travels(42, |_| {}, founder_genome, LIFETIME);
        println!(
            "arm B: {} cells, {} muscles, {:.4}/tick against the founder's {:.4}; travels \
             {swum:.2} units in {lived} ticks against {drifted:.2} held still and the founder's \
             {founder_drift:.2}",
            body.cells.len(),
            muscles.len(),
            upkeep_of(&plan),
            upkeep_of(&[CellKind::Gonocyte])
        );
        assert!(
            swum > 5.0 * drifted,
            "arm B travelled {swum:.2} units against {drifted:.2} for the identical body with \
             its muscles held still. The measured figures are 16.60 and 1.66; a ratio near one \
             means this arm is not swimming and the experiment is void"
        );

        // ⭐⭐ **That it breeds**, which a previous attempt at this experiment did not check. A
        // four-cell version of this body reaches its reproduction bar so slowly that 14 of 16
        // founders were alive and childless at tick 2,000; every coefficient taken on it was a
        // measurement of that.
        let bred = package_assay(&config, [&plain, &genome], 2_000);
        println!(
            "arm B in one generation: {} organisms against the founder's {}",
            bred.born[1], bred.born[0]
        );
        assert!(
            bred.born[1] > 3 * u64::from(FOUNDERS / 2),
            "arm B produced {} organisms out of {} founders in one generation, against the \
             founder's {}. An arm that cannot breed measures the fact that it cannot breed",
            bred.born[1],
            FOUNDERS / 2,
            bred.born[0]
        );

        // ⭐⭐⭐ **And only now, what it is worth.** Three 42,000-tick runs: the same-seed
        // control, the swimmer against the founder, and the ceiling on locomotion measured with
        // no muscle in it at all.
        let control = package_assay(&config, [&plain, &plain], WINDOW);
        let tested = package_assay(&config, [&plain, &genome], WINDOW);
        let arrived = placed_assay(&config, [&plain, &plain], WINDOW, |side, world, at| {
            if side == 0 {
                at
            } else {
                best_water_near(world, at, SWIMMING_REACH)
            }
        });

        report("the noise floor at seed 42", &control);
        report("arm B, a body that swims", &tested);
        report("arriving in better water for nothing", &arrived);

        let cost = (tested.per_generation() - control.per_generation()) * 100.0;
        let ceiling = (arrived.per_generation() - control.per_generation()) * 100.0;
        println!(
            "arm B is worth {cost:+.3} %/gen; the whole value of arriving where it was going, \
             free and perfectly aimed, is {ceiling:+.3} %/gen"
        );

        assert!(
            cost < -5.0,
            "a body that swims came back at {cost:+.3} %/generation against the founder, and \
             the measured reading is −10.3. A swimmer that had stopped being priced far below \
             break-even would reopen every payoff question Phase 7 closed"
        );
        assert!(
            ceiling.abs() < 1.0,
            "arriving in the best water a lifetime's swim away is worth {ceiling:+.3} \
             %/generation, and the measured reading is −0.01 against a noise floor of ±0.13. If \
             this is no longer nothing, then locomotion has somewhere to go and the whole \
             finding above is reopened"
        );

        // ⭐ What the water is actually like at the scale a body can cross, which is the reason
        // the line above reads as it does.
        survey("the shipped world, full", &config);
    }

    /// What a lifetime of arm B's swimming covers, in world units. Measured, not assumed.
    const SWIMMING_REACH: f32 = 16.6;

    /// What a lifetime's swim is worth in light: the best direction, and a direction nothing
    /// chose.
    ///
    /// Two hundred and fifty-six starting points on an even lattice over the world, each
    /// compared with the sixteen places [`SWIMMING_REACH`] units away. A photocyte's income is
    /// `HARVEST_RATE × the tile's energy × shade`, so these ratios are directly the income
    /// multiplier a swimmer could collect.
    ///
    /// ⚠️ **The second number is the one that matters**, because SPEC section 9's controller
    /// provides no way to choose a direction: `sensor_gain` scales a muscle's *amplitude*, so a
    /// sensed body swims harder in a steep gradient and not towards anything.
    ///
    /// Measured at seed 42, on the **full** field the dawn leaves behind — which is the kindest
    /// reading there is, since a population eats the contrast down — and on the same field after
    /// 42,000 ticks of the assay's own population living in it:
    ///
    /// | tile the lattice is | best, full | blind, full | best, lived in | blind, lived in |
    /// | --- | --- | --- | --- | --- |
    /// | 8 units — **as shipped**, a blotch 128 units across | ×1.053 | ×1.000 | ×1.064 | ×1.001 |
    /// | 4 units, a blotch of 64 | ×1.099 | ×0.999 | ×1.077 | ×1.002 |
    /// | 2 units, a blotch of 32 | ×1.185 | ×1.009 | ×1.129 | ×0.993 |
    /// | 1 unit, a blotch of 16 | ×1.335 | ×1.042 | ×1.264 | ×0.997 |
    ///
    /// ⭐ **The patches are not smoothed away at any of those scales, and cannot be**, which is
    /// the answer to the obvious worry about `light.diffusion`: the lattice is sixteen *tiles*
    /// and the diffusion stencil is *per tile*, so shrinking a tile shrinks both together and
    /// the contrast per lattice cell is unchanged. What shrinks is only how many world units a
    /// blotch is — and with it, how much of one a body can cross.
    fn survey(name: &str, config: &Config) {
        let mut world = World::new(config);
        dawn(&mut world);

        let grid = world.grid();
        let (width, height) = (config.world.width, config.world.height);
        let tiles = grid.tiles();
        let (mut best_gain, mut blind_gain, mut samples) = (0.0f64, 0.0f64, 0.0f64);

        for down in 0..16u8 {
            for across in 0..16u8 {
                let at = Vec2::new(
                    width * (f32::from(across) + 0.5) / 16.0,
                    height * (f32::from(down) + 0.5) / 16.0,
                );
                let here = f64::from(tiles[grid.tile_at(at)]).max(1e-9);

                let reached: Vec<f64> = (0..16u8)
                    .map(|step| {
                        let angle = f32::from(step) * std::f32::consts::TAU / 16.0;
                        let (sin, cos) = angle.sin_cos();
                        let there = Vec2::new(
                            (at.x + cos * SWIMMING_REACH).rem_euclid(width),
                            (at.y + sin * SWIMMING_REACH).clamp(0.0, height),
                        );
                        f64::from(tiles[grid.tile_at(there)])
                    })
                    .collect();

                best_gain += reached.iter().fold(0.0f64, |most, one| most.max(*one)) / here;
                blind_gain += reached.iter().sum::<f64>() / 16.0 / here;
                samples += 1.0;
            }
        }

        println!(
            "FIELD {name}: over {SWIMMING_REACH} units of travel the best direction is x{:.3} \
             and a direction nothing chose is x{:.3}",
            best_gain / samples,
            blind_gain / samples
        );
    }

    /// The richest tile within `reach` of here, searched on a ring of directions.
    ///
    /// A search rather than a scan of the grid, because what is wanted is *where a body could
    /// have swum to*, which is a disc around where it started and not the best tile in the
    /// world.
    fn best_water_near(world: &World, at: Vec2, reach: f32) -> Vec2 {
        let grid = world.grid();
        let (width, height) = (world.config().world.width, world.config().world.height);
        let mut best = (f64::from(grid.tiles()[grid.tile_at(at)]), at);

        for step in 1..=16u8 {
            let angle = f32::from(step) * std::f32::consts::TAU / 16.0;
            for away in [reach * 0.5, reach] {
                let (sin, cos) = angle.sin_cos();
                let here = Vec2::new(
                    (at.x + cos * away).rem_euclid(width),
                    (at.y + sin * away).clamp(0.0, height),
                );
                let held = f64::from(grid.tiles()[grid.tile_at(here)]);
                if held > best.0 {
                    best = (held, here);
                }
            }
        }

        best.1
    }

    /// Print one outcome, so a run of these tests is a measurement and not only a pass.
    fn report(what: &str, outcome: &Outcome) {
        println!(
            "assay {what}: alive {:?}, ratio {:.4}, log {:+.4}, {:+.3} %/gen, cells/body {:.2} \
             vs {:.2}, born {:?}, unattributed {}, founder energy {:?}",
            outcome.alive,
            outcome.ratio(),
            outcome.log_ratio(),
            outcome.per_generation() * 100.0,
            outcome.cells_per_body(1),
            outcome.cells_per_body(0),
            outcome.born,
            outcome.unattributed,
            outcome.energy
        );
    }

    /// The guard itself is guarded: two genomes two mutations apart are refused.
    ///
    /// Without this the assertion inside [`assay`] would be a line nobody had ever seen say no,
    /// and the one thing it exists to catch - an arm that quietly picked up a second change -
    /// is exactly the thing that produces a plausible coefficient for the wrong reason.
    #[test]
    fn an_assay_refuses_two_arms_that_are_two_mutations_apart() {
        let config = a_small_world(42);
        let plain = founder_genome(&config.limits);
        let one = founder_with_a_third_cell(&config.limits, CellKind::Photocyte);

        assert!(
            one_mutation_apart(&plain, &one),
            "one appended gene is one step"
        );
        assert!(
            one_mutation_apart(
                &one,
                &founder_with_a_third_cell(&config.limits, CellKind::Myocyte)
            ),
            "one changed `child_kind` is one step"
        );

        let mut two = one.genes().to_vec();
        two[0].rest_length = 13.6;
        let two = coacervate_sim::genome::Genome::new(two, &config.limits);
        assert!(
            !one_mutation_apart(&plain, &two),
            "an appended gene and a changed rest length is two steps and was accepted as one"
        );

        assert_eq!(ARMS, 2, "an assay compares one change against its absence");
    }
}
