//! The bench: one command, one config change, a standard battery of answers, across seeds.
//!
//! # Why this exists
//!
//! Thirteen rounds of this project have each been a hand-written test. That is a good way to ask
//! one careful question and a terrible way to ask twenty, and the cost has been paid in a currency
//! nobody was counting: **seven methodological errors in one night**, every one of them a
//! statistic written afresh and written wrong. A mean of ratios with a vanishing denominator. A
//! ledger account that included the weather. A cost added back at an amplitude nothing ran at. An
//! assertion whose threshold sat inside its own noise floor. A conclusion drawn from one seed,
//! twice. And a placement closure that was never called after founding, which quietly turned
//! every ceiling this project has quoted into an answer to a different question.
//!
//! Each was caught, eventually, by a control or a replication. None would have been written at all
//! if the statistic had been written **once**.
//!
//! So this is a bench rather than another test. The measurements live here, correctly, and a
//! question is a command line rather than a hundred lines of Rust:
//!
//! ```text
//! coacervate --bench "light.bloom=1.0,light.patch_drift=0.05" --seeds 3
//! ```
//!
//! # What it guarantees that a hand-written test does not
//!
//! - **Seeds, always.** Every figure comes with the spread across seeds beside it. A reading whose
//!   spread exceeds its own effect is printed as `noise` rather than as a number, because that is
//!   what it is, and this project has believed six such numbers.
//! - **A control, always.** Every run is a *pair*: the world asked about and the shipped world at
//!   the same seed. What is reported is the difference, so a change that made the whole world
//!   richer cannot be read as a change that made one cell kind better.
//! - **In parallel.** Worlds are independent by construction — each has its own `world.seed` and
//!   shares nothing — so the seeds run across every core. On a sixteen-core machine that is the
//!   difference between a question costing twenty minutes and costing two.

use coacervate_sim::cell::CellKind;
use coacervate_sim::config::{Config, RawConfig, spec_defaults};
use coacervate_sim::organism::Organism;
use coacervate_sim::world::World;
use rayon::prelude::*;

/// How long a bench world runs before it is measured. Long enough for the population to settle:
/// `run.rs`'s own measurements use the same figure, and the shipped world is at equilibrium well
/// before it.
const SETTLE: u64 = 60_000;

/// Everything the bench measures about one world.
///
/// One struct, filled in one place, so that a figure means the same thing in every row of every
/// sweep this project ever runs. That is the whole point of the file.
#[derive(Clone, Copy, Default)]
pub struct Reading {
    /// How many organisms are alive at the end.
    pub alive: f64,

    /// How many living cells there are between them — the world's total standing tissue, which is
    /// the quantity Kleiber moved and the one that says whether a world grew or merely
    /// redistributed.
    pub cells: f64,

    /// Mean cells per body, and the spread across bodies **within** the world.
    ///
    /// ⚠️ The spread is the interesting half and it is easy to lose. Kleiber's headline was that
    /// the mean barely moved while the spread quadrupled, which is several body plans coexisting
    /// rather than one — and a table of means alone would have shown nothing at all.
    pub body: f64,
    pub body_spread: f64,

    /// Mean genes per genome: how long a growth program this world can afford to carry.
    pub genome: f64,

    /// The census, as shares of all living cells. Named for what a person watching wants to know
    /// rather than for the cell kinds, because *are there mouths yet* is the question and
    /// `devorocytes ÷ cells` is the answer.
    pub motors: f64,
    pub mouths: f64,
    pub eyes: f64,
    pub armour: f64,

    /// What share of the world's income arrived second-hand: out of living tissue, and out of the
    /// dead. Both measured at three parts and one part in ten thousand in the shipped world.
    pub predated: f64,
    pub scavenged: f64,

    /// What the field is holding, meaned over tiles, and the spread across them. A world whose
    /// tiles all hold the same thing has no spatial structure for anything to exploit.
    pub tile: f64,
    pub tile_spread: f64,
}

impl Reading {
    /// The measurements, in the order a person reads them.
    ///
    /// Returned as a list rather than reached through field names so that the printer, the
    /// difference and the spread are one loop each and cannot disagree about which column is
    /// which — which is exactly the kind of bookkeeping this file exists to stop doing by hand.
    #[must_use]
    pub fn columns(&self) -> [(&'static str, f64); 14] {
        [
            ("alive", self.alive),
            ("cells", self.cells),
            ("body", self.body),
            ("body±", self.body_spread),
            ("genome", self.genome),
            ("motors", self.motors),
            ("mouths", self.mouths),
            ("eyes", self.eyes),
            ("armour", self.armour),
            ("predated", self.predated),
            ("scavenged", self.scavenged),
            ("tile", self.tile),
            ("tile±", self.tile_spread),
            ("_", 0.0),
        ]
    }
}

/// Run one world to equilibrium and measure it.
fn measure(config: &Config, seeded_with_motors: bool) -> Reading {
    let mut world = World::new(config);
    if seeded_with_motors {
        genesis_motorised(&mut world);
    } else {
        crate::founding::genesis(&mut world, 8);
    }
    for _ in 0..SETTLE {
        world.tick();
    }

    let bodies: Vec<usize> = world
        .organisms()
        .iter()
        .flatten()
        .map(Organism::cells)
        .collect();
    let genes: Vec<usize> = world
        .organisms()
        .iter()
        .flatten()
        .map(|o| o.genome().genes().len())
        .collect();

    let cells = world.living_cells();
    let kinds = |kind: CellKind| {
        let n = cells.iter().filter(|c| c.kind == kind).count();
        ratio(n, cells.len())
    };

    let tiles = world.grid().tiles();
    let (tile, tile_spread) = spread(tiles.iter().map(|t| f64::from(*t)));
    let (body, body_spread) = spread(bodies.iter().map(|n| whole(*n)));
    let (genome, _) = spread(genes.iter().map(|n| whole(*n)));

    let ledger = world.ledger();
    let light = ledger.influx_total().max(1.0);

    Reading {
        alive: whole(bodies.len()),
        cells: whole(cells.len()),
        body,
        body_spread,
        genome,
        motors: kinds(CellKind::Flagellocyte),
        mouths: kinds(CellKind::Devorocyte),
        eyes: kinds(CellKind::Sensocyte),
        armour: kinds(CellKind::Sclerocyte),
        predated: ledger.predation_total() / light,
        scavenged: ledger.scavenge_total() / light,
        tile,
        tile_spread,
    }
}

/// A count as a number, without a lossy cast anybody has to justify at every call site.
fn whole(n: usize) -> f64 {
    // A population, a cell count and a gene count are all far inside what an f64 holds exactly.
    u32::try_from(n).map_or(f64::from(u32::MAX), f64::from)
}

/// One count over another, as a share, with nought over nought reading as nought.
fn ratio(part: usize, whole_of: usize) -> f64 {
    if whole_of == 0 {
        0.0
    } else {
        whole(part) / whole(whole_of)
    }
}

/// The mean and the standard deviation of a sequence, in one pass over a collected list.
fn spread(of: impl Iterator<Item = f64>) -> (f64, f64) {
    let values: Vec<f64> = of.collect();
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let n = whole(values.len());
    let mean = values.iter().sum::<f64>() / n;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;

    (mean, variance.sqrt())
}

/// ⭐ Run the bench: the world described by `change`, against the shipped world, at every seed.
///
/// Returns, for each measurement, `(the world asked about, its spread across seeds, the shipped
/// world, its spread)`. The caller prints; this only measures.
///
/// # Panics
///
/// If `change` produces a configuration the program would refuse. A bench is a thing a person
/// drives from a command line, so a bad setting has to stop with the validator's own sentence
/// rather than be quietly clamped into something that measures a world nobody asked for.
pub fn run(seeds: &[u64], change: &(dyn Fn(&mut RawConfig) + Sync), motors: bool) -> Vec<[f64; 4]> {
    let build = |seed: u64, tuned: bool| {
        let mut raw = spec_defaults();
        raw.world.seed = seed;
        if tuned {
            change(&mut raw);
        }
        raw.validate()
            .expect("a bench configuration must be one the program will accept")
    };

    // ⭐ Every world in the sweep at once. They share nothing — each carries its own
    // `world.seed`, and SPEC section 2's per-organism streams are what make that true rather than
    // merely likely — so the answer is the same whatever order they finish in.
    let pairs: Vec<(Reading, Reading)> = seeds
        .par_iter()
        .map(|seed| {
            (
                measure(&build(*seed, true), motors),
                measure(&build(*seed, false), motors),
            )
        })
        .collect();

    let width = Reading::default().columns().len();
    (0..width)
        .map(|column| {
            let (tuned, tuned_spread) = spread(pairs.iter().map(|(t, _)| t.columns()[column].1));
            let (shipped, shipped_spread) =
                spread(pairs.iter().map(|(_, s)| s.columns()[column].1));

            [tuned, tuned_spread, shipped, shipped_spread]
        })
        .collect()
}

/// Print a bench result the way a person reads one: what changed, by how much, and whether the
/// change is bigger than the disagreement between seeds.
///
/// ⚠️ **The last column is the point of the whole file.** A figure whose between-seed spread
/// covers its own effect is printed as `noise` and not as a number. Six readings this project
/// believed were exactly that, and each one cost a round.
pub fn report(what: &str, seeds: &[u64], rows: &[[f64; 4]]) {
    let names = Reading::default().columns();

    println!("\n{what}   ({} seeds)", seeds.len());
    println!(
        "{:<10} | {:>12} | {:>12} | {:>10} | verdict",
        "", "this world", "shipped", "change"
    );

    for (column, [tuned, tuned_spread, shipped, shipped_spread]) in rows.iter().enumerate() {
        let name = names[column].0;
        if name == "_" {
            continue;
        }

        let change = tuned - shipped;
        let noise = tuned_spread.max(*shipped_spread);
        let verdict = if change.abs() > noise * 2.0 {
            if change > 0.0 { "UP" } else { "DOWN" }
        } else {
            "noise"
        };

        println!(
            "{name:<10} | {tuned:>12.5} | {shipped:>12.5} | {change:>+10.5} | {verdict} (±{noise:.4})"
        );
    }
}

/// Turn `light.bloom=1.0,light.patch_drift=0.05` into something that changes a configuration.
///
/// ⚠️ **Every field is listed by hand and an unknown one is refused.** A bench that silently
/// ignored a misspelt field would report that a change did nothing — which is the single most
/// expensive failure this project has had. `physics.thrust` was measured across three whole
/// experiments while the arm it was meant to move could not feel it, and `light.uptake` sat as a
/// compiled-in constant for eleven rounds because nothing could reach it. A typo here must stop
/// the run, loudly, rather than produce a beautifully formatted null.
pub fn overrides(spec: &str) -> Result<impl Fn(&mut RawConfig) + Sync + use<>, String> {
    let mut set: Vec<(String, f64)> = Vec::new();

    for clause in spec.split(',').map(str::trim).filter(|c| !c.is_empty()) {
        let (field, value) = clause
            .split_once('=')
            .ok_or_else(|| format!("`{clause}` is not `field=value`"))?;
        let parsed: f64 = value
            .trim()
            .parse()
            .map_err(|_| format!("`{value}` in `{clause}` is not a number"))?;

        set.push((field.trim().to_owned(), parsed));
    }

    Ok(move |raw: &mut RawConfig| {
        for (field, value) in &set {
            let hit = match field.as_str() {
                "light.influx" => take(&mut raw.light.influx, *value),
                "light.uptake" => take(&mut raw.light.uptake, *value),
                "light.bloom" => take(&mut raw.light.bloom, *value),
                "light.cap" => take(&mut raw.light.cap, *value),
                "light.gradient" => take(&mut raw.light.gradient, *value),
                "light.patchiness" => take(&mut raw.light.patchiness, *value),
                "light.patch_drift" => take(&mut raw.light.patch_drift, *value),
                "light.diffusion" => take(&mut raw.light.diffusion, *value),
                "light.season_amplitude" => take(&mut raw.light.season_amplitude, *value),
                "light.shadow_depth" => take(&mut raw.light.shadow_depth, *value),
                "light.shadow_spread" => take(&mut raw.light.shadow_spread, *value),
                "physics.drag" => take(&mut raw.physics.drag, *value),
                "physics.drag_anisotropy" => take(&mut raw.physics.drag_anisotropy, *value),
                "physics.current" => take(&mut raw.physics.current, *value),
                "physics.thrust" => take(&mut raw.physics.thrust, *value),
                "behaviour.resting_amplitude" => take(&mut raw.behaviour.resting_amplitude, *value),
                "behaviour.stroke" => take(&mut raw.behaviour.stroke, *value),
                "metabolism.upkeep_scale" => take(&mut raw.metabolism.upkeep_scale, *value),
                "metabolism.tissue_share" => take(&mut raw.metabolism.tissue_share, *value),
                "metabolism.motor_upkeep" => take(&mut raw.metabolism.motor_upkeep, *value),
                "metabolism.scaling_exponent" => take(&mut raw.metabolism.scaling_exponent, *value),
                "metabolism.movement_cost" => take(&mut raw.metabolism.movement_cost, *value),
                "metabolism.reproduction_threshold" => {
                    take(&mut raw.metabolism.reproduction_threshold, *value)
                }
                "metabolism.offspring_share" => take(&mut raw.metabolism.offspring_share, *value),
                "mutation.point_rate" => take(&mut raw.mutation.point_rate, *value),

                // ⚠️⚠️ **The arena caps, and a bench needs them because a world can hit one.**
                // A run that reported `alive = 4000.00000` exactly was not measuring an ecology;
                // it was measuring `limits.max_organisms`. A censored population inflates the
                // between-seed spread and makes every other row read as noise, which is how a
                // wall disguises itself as variance.
                "limits.max_organisms" => whole_field(&mut raw.limits.max_organisms, *value),
                "limits.max_cells_per_organism" => {
                    whole_field(&mut raw.limits.max_cells_per_organism, *value)
                }
                "limits.max_genes" => whole_field(&mut raw.limits.max_genes, *value),
                "mutation.duplication_rate" => take(&mut raw.mutation.duplication_rate, *value),
                _ => false,
            };

            assert!(hit, "`{field}` is not a field the bench knows how to set");
        }
    })
}

/// Write `value` into `field` and say that it happened, so the caller can refuse a name it does
/// not recognise rather than accept it silently.
fn take(field: &mut f64, value: f64) -> bool {
    *field = value;
    true
}

/// ⭐⭐⭐ The founder that already has a motor, for asking whether a world **keeps** one.
///
/// # Why retention rather than invention
///
/// The bench's `motors` column reads 0.0004 in the shipped world — one cell in two and a half
/// thousand — because mutation has to *find* a flagellocyte before anything can select on it, and
/// in sixty thousand ticks it barely does. A share that small cannot clear its own between-seed
/// spread, so every world reads `noise` whatever it does, and the instrument answers nothing.
///
/// Seeding every founder with a motor turns an invention problem into a **retention** problem, and
/// retention resolves. A world that punishes motility sheds them and the share falls to nothing; a
/// world that rewards it keeps them and the share stays up. The same sixty thousand ticks, the
/// same cost, and an observable that starts at a hundred per cent instead of at nought.
///
/// ⚠️ It answers a narrower question than *would evolution find this*, and the difference matters:
/// a world can keep a motor it was given and still never invent one. What it does answer is the
/// one that has blocked this project for thirteen rounds — **is there anything here worth moving
/// for** — and it answers it in three minutes instead of a night.
fn motorised(limits: &coacervate_sim::config::LimitsConfig) -> coacervate_sim::genome::Genome {
    use coacervate_sim::genome::{Action, Gene, Genome, SensorTarget, State};

    let gene = |step: u8, kind: CellKind, driven: bool| Gene {
        trigger_state: State::new(step),
        min_step: step,
        max_step: step,
        action: Action::Divide,
        angle: 0.0,
        adhere: true,
        child_state: State::new(step + 1),
        child_kind: kind,
        rest_length: 8.0,
        stiffness: 10.0,
        new_kind: CellKind::Photocyte,
        new_state: State::ZERO,
        // ⚠️ A motor whose gene carries an `osc_freq` of nought produces no force and is charged
        // nothing — it is a sclerocyte with a dearer upkeep. That mistake cost this project three
        // whole experiments, so the frequency is set here and the reason is written here.
        osc_freq: if driven { 3.0 } else { 0.0 },
        osc_phase: 0.0,
        sensor_gain: 0.0,
        sensor_target: SensorTarget::Light,
    };

    Genome::new(
        vec![
            gene(0, CellKind::Gonocyte, false),
            gene(1, CellKind::Flagellocyte, true),
        ],
        limits,
    )
}

/// Found a world whose every founder already carries a motor. See [`motorised`].
fn genesis_motorised(world: &mut World) {
    crate::founding::dawn(world);

    let limits = world.config().limits.clone();
    let (width, height) = (world.config().world.width, world.config().world.height);

    for founder in 0..8 {
        let _ = world.seed(
            motorised(&limits),
            crate::founding::place(founder, 8, width, height),
            crate::founding::FOUNDER_ENERGY,
        );
    }
}

/// Write a whole number into a limit, from the decimal a command line hands over.
///
/// A limit is a count and the parser reads decimals, so the value is rounded rather than
/// truncated: `--bench limits.max_organisms=16000` should mean sixteen thousand and not fifteen
/// thousand nine hundred and ninety-nine because of how a decimal literal landed.
fn whole_field(field: &mut u32, value: f64) -> bool {
    // Walked up rather than cast. A cast from a float to an integer is the one operation
    // CLAUDE.md bans outright, and there is no need for one here: a limit is a small count, so
    // finding it by comparison costs nothing anybody can measure and cannot truncate or wrap.
    let wanted = value.round();
    *field = if wanted.is_finite() && wanted >= 1.0 {
        // ⚠️ Bounded at a million, which is far past any arena this machine could allocate and
        // far short of a walk that anybody would notice. An unbounded search here would hang on
        // a typo.
        (1u32..=1_000_000)
            .take_while(|n| f64::from(*n) <= wanted)
            .last()
            .unwrap_or(1)
    } else {
        0
    };

    true
}
