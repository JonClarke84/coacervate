//! The run's configuration.
//!
//! See `SPEC.md` section 3 for the document this mirrors.
//!
//! Everything a run does is decided by two things: the seed, and the numbers in this
//! module. There is nothing else - no hidden constant, no environment variable, no value
//! read off the clock. Give the program the same document and the same seed and you get
//! the same run back.
//!
//! # Why there are two of everything
//!
//! Each table in the configuration document appears here twice: once as a `Raw...` type
//! and once as a checked one. The pair looks like duplication and is not.
//!
//! The `Raw...` types are a transcript. They say what the document *said*, one field per
//! key, in the order SPEC section 3 writes them, and they will hold any number a person
//! can type: a negative light influx, a gradient of nine, a genome cap of four thousand.
//! Reading a document can therefore fail for one reason only, which is that the document
//! was not a configuration document at all.
//!
//! The checked types say what a *run* can be given. Getting from one to the other goes
//! through [`RawConfig::validate`], the single place where a number is looked at and
//! either accepted or refused in a sentence naming the field it came from. Past that
//! point every value has been through the gate, so nothing downstream needs to ask again,
//! and nothing downstream *can* forget to, because the checked types are the only ones
//! the simulation is ever handed.
//!
//! # The two sizes of number
//!
//! A configuration document writes numbers to sixteen or so digits of precision; the
//! simulation runs on numbers of about seven, because that is what a graphics card works
//! in and SPEC section 2 requires the two to agree. Something has to give up those
//! digits, and this module is where it happens: once, on the way in, with a check. The
//! test `narrowing_is_rejected_or_faithful` is where the rule for that is written down.

use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

// ---------------------------------------------------------------------------------------
// The document as written
//
// One `Raw...` struct per table in SPEC section 3, fields in SPEC's order. Two things
// about these declarations are load-bearing, and both are about what happens when a
// document is *wrong*.
//
// `deny_unknown_fields` appears on all eight of them, and the repetition is not
// carelessness: serde does not pass the setting down into nested tables. Put it only on
// `RawConfig` and every table inside it still accepts whatever it is given, so
// `influks = 0.012` is read as a key nobody asked about and thrown away. See
// `typos_and_omissions_are_rejected_not_ignored` for what that costs.
//
// And there is no `serde(default)` anywhere, so every key is required. A configuration
// that leaves something out is a configuration whose author did not decide that value,
// and guessing on their behalf is the same failure wearing a different hat.
// ---------------------------------------------------------------------------------------

/// The `[world]` table as written: the seed, the size of the world, and the resolution of
/// the resource grid laid over it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWorld {
    pub seed: u64,
    pub width: f64,
    pub height: f64,
    pub grid_cols: u32,
    pub grid_rows: u32,
    pub years_per_tick: f64,
}

/// The `[light]` table as written: where the energy in the world comes from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLight {
    pub influx: f64,
    pub cap: f64,
    pub gradient: f64,
    pub patchiness: f64,
    pub patch_drift: f64,
    pub diffusion: f64,
    pub season_period: u64,
    pub season_amplitude: f64,
}

/// The `[physics]` table as written: how the soup pushes back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPhysics {
    pub drag: f64,
    pub drag_anisotropy: f64,
    pub collision_stiffness: f64,
    pub spring_damping: f64,
}

/// The `[behaviour]` table as written: how hard SPEC section 9's controller drives a muscle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBehaviour {
    pub resting_amplitude: f64,
    pub stroke: f64,
}

/// The `[metabolism]` table as written: what living costs, and what reproducing costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMetabolism {
    pub upkeep_scale: f64,
    pub gene_cost: f64,
    pub movement_cost: f64,
    pub reproduction_threshold: f64,
    pub offspring_share: f64,
}

/// The `[mutation]` table as written: how often a genome is copied imperfectly, and how
/// far off each kind of mistake takes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMutation {
    pub point_rate: f64,
    pub point_sigma: f64,
    pub duplication_rate: f64,
    pub deletion_rate: f64,
    pub insertion_rate: f64,
    pub reorder_rate: f64,
    pub genome_duplication_rate: f64,
}

/// The `[limits]` table as written: the sizes every arena in the simulation is built to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawLimits {
    pub max_organisms: u32,
    pub max_cells_per_organism: u32,
    pub max_genes: u32,
    pub max_dev_steps: u32,
}

/// The `[run]` table as written: when a run is to stop, and what to do if everything
/// dies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRun {
    pub max_wall_clock_hours: f64,
    pub max_ticks: u64,
    pub max_ticks_per_second: u32,
    pub reseed_on_extinction: bool,
}

/// A configuration document as it was written, before anything has been checked.
///
/// Field order is SPEC section 3's table order, which is also the order the tables come
/// back out in when a configuration is written to a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    pub world: RawWorld,
    pub light: RawLight,
    pub physics: RawPhysics,
    pub behaviour: RawBehaviour,
    pub metabolism: RawMetabolism,
    pub mutation: RawMutation,
    pub limits: RawLimits,
    pub run: RawRun,
}

/// SPEC section 3's default configuration, written out in Rust.
///
/// The simulation crate has no way to read a TOML document - that is the binary's job,
/// and keeping it that way is what stops file handling leaking into the simulation. So
/// the tests in this module, which are all about what happens to the *numbers*, need a
/// configuration to work from that did not come out of a file. This is it.
///
/// Writing SPEC's defaults down twice - here and in `config/default.toml` - is a real
/// risk, because the two could be edited apart and nothing would notice. The binary's
/// `spec_defaults_fixture_matches_the_shipped_file` closes it: it reads the shipped
/// document and insists it comes out equal to this.
#[must_use]
pub fn spec_defaults() -> RawConfig {
    RawConfig {
        world: RawWorld {
            seed: 42,
            width: 2048.0,
            height: 1152.0,
            grid_cols: 256,
            grid_rows: 144,
            years_per_tick: 1000.0,
        },
        light: RawLight {
            influx: 0.001,
            cap: 8.0,
            gradient: 0.75,
            patchiness: 0.5,
            patch_drift: 0.0006,
            diffusion: 0.04,
            season_period: 21_000,
            season_amplitude: 0.0,
        },
        physics: RawPhysics {
            drag: 0.92,
            drag_anisotropy: 2.0,
            collision_stiffness: 40.0,
            spring_damping: 0.35,
        },
        behaviour: RawBehaviour {
            resting_amplitude: 0.8,
            stroke: 1.0,
        },
        metabolism: RawMetabolism {
            upkeep_scale: 1.0,
            gene_cost: 0.0001,
            movement_cost: 0.0001,
            reproduction_threshold: 2.2,
            offspring_share: 0.45,
        },
        mutation: RawMutation {
            point_rate: 0.06,
            point_sigma: 0.12,
            duplication_rate: 0.02,
            deletion_rate: 0.02,
            insertion_rate: 0.01,
            reorder_rate: 0.02,
            genome_duplication_rate: 0.0008,
        },
        limits: RawLimits {
            max_organisms: 4000,
            max_cells_per_organism: 64,
            max_genes: 128,
            max_dev_steps: 16,
        },
        run: RawRun {
            max_wall_clock_hours: 12.0,
            max_ticks: 0,
            max_ticks_per_second: 0,
            reseed_on_extinction: false,
        },
    }
}

// ---------------------------------------------------------------------------------------
// Everything that can be wrong with a configuration
// ---------------------------------------------------------------------------------------

/// A reason a configuration was refused, in a sentence naming the setting at fault.
///
/// A bad configuration is somebody having typed something, which makes it an ordinary
/// thing that happens rather than a sign the program has gone wrong - so this is returned
/// and reported, not panicked on. What the person gets back is the sentence, which is why
/// the exact wording of each one is pinned by `errors_name_the_field_in_plain_english`
/// rather than left to drift.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// A number the simulation's arithmetic cannot hold at anything like the size it was
    /// written at.
    NotRepresentable { field: &'static str, value: f64 },

    /// A setting whose meaning makes it a fraction of something, given a value that is
    /// not one.
    NotAFraction { field: &'static str, value: f32 },

    /// A setting whose upper end is set by whether the arithmetic stays stable rather than
    /// by what the setting means, given a value past it.
    ///
    /// The only one of these is `light.diffusion`, and it is the only bound in the file
    /// that nothing else would catch. See [`DIFFUSION_STABILITY_LIMIT`].
    Unstable {
        field: &'static str,
        value: f32,
        limit: f32,
    },

    /// A setting whose upper end is set by what the light can put back, given a value past
    /// it.
    ///
    /// The only one of these is `light.patch_drift`, and it is a separate kind of refusal
    /// from [`Self::Unstable`] because nothing here stops computing: the arithmetic is
    /// perfectly well behaved and the *world* is the thing that fails. See
    /// [`PATCH_DRIFT_CEILING`].
    OutrunsTheLight {
        field: &'static str,
        value: f32,
        limit: f32,
    },

    /// A setting bounded at both ends, given a value outside them.
    ///
    /// The only one of these is `physics.drag_anisotropy`, and it is a separate kind of
    /// refusal from [`Self::Unstable`] because its *lower* end means something too: one is
    /// isotropic water. See [`DRAG_ANISOTROPY_CEILING`].
    OutsideRange {
        field: &'static str,
        value: f32,
        least: f32,
        most: f32,
    },

    /// A setting whose upper end is where the **evidence** stops rather than where anything
    /// breaks, given a value past it.
    ///
    /// The only one of these is `light.season_amplitude`, and it is a fifth kind of refusal
    /// rather than a use of [`Self::OutsideRange`] because that variant's sentence is
    /// `physics.drag_anisotropy`'s own — the water resisting a cell equally in every direction
    /// — and the whole value of a refusal in this file is the sentence it writes. See
    /// [`SEASON_AMPLITUDE_CEILING`].
    Unmeasured {
        field: &'static str,
        value: f32,
        least: f32,
        most: f32,
    },

    /// A clock told to run faster than the thing it is a clock for, given a period below what
    /// the world can follow.
    ///
    /// The only one of these is `light.season_period`, and it is a sixth kind of refusal for the
    /// reason [`Self::OutrunsTheLight`] is a fourth: the arithmetic is perfectly well behaved
    /// and it is the *world* that stops being able to feel the setting. See
    /// [`SEASON_PERIOD_FLOOR`].
    FasterThanTheWater {
        field: &'static str,
        value: u64,
        least: u64,
    },

    /// A setting that may be nothing but cannot be less than nothing, given a value below
    /// zero.
    Negative { field: &'static str, value: f32 },

    /// A setting that has to exist for the simulation to mean anything, given zero or
    /// less.
    NotPositive { field: &'static str, value: f32 },

    /// A count of something the simulation builds room for, given zero.
    ///
    /// It carries no value, because there is only one value it could ever be reporting.
    Zero { field: &'static str },

    /// A limit set higher than the largest value this program is built to survive.
    AboveCeiling {
        field: &'static str,
        value: u32,
        ceiling: u32,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            // `{value:e}` - scientific notation, asked for deliberately. Rust never
            // chooses it on its own, so the number that provoked this message, which is
            // by definition a very small one, would otherwise arrive as forty characters
            // of zeroes in the one sentence whose whole job is to explain that its size
            // is the problem.
            Self::NotRepresentable { field, value } => write!(
                out,
                "{field}: {value:e} cannot be represented as a 32-bit float \
                 without losing its magnitude"
            ),
            Self::NotAFraction { field, value } => {
                write!(out, "{field}: {value} is outside 0..=1")
            }
            // Much the longest sentence here, and the only one that says *why*. Every other
            // bound in this file follows from what its setting means, so naming the range
            // says everything there is to say. This one does not: half is an entirely
            // ordinary fraction, it is what this program used to accept, and the reason it
            // is refused cannot be guessed from the word "diffusion". A refusal with no
            // reason attached reads as an arbitrary house rule, and the obvious response to
            // an arbitrary house rule is to go and delete it.
            Self::Unstable {
                field,
                value,
                limit,
            } => write!(
                out,
                "{field}: {value} is outside 0..={limit}. The upper end is a limit of the \
                 arithmetic rather than a preference: past it energy spreads faster than \
                 it settles, the field oscillates and grows without limit, and because \
                 that still conserves energy exactly nothing else in the program will \
                 catch it"
            ),
            // Long for the same reason the one above it is. A drift of a hundredth of a
            // world unit per tick is a sixth of a cell-width a second and reads as nothing at
            // all; what makes it refused is that a ceiling sliding out from under a full tile
            // takes more energy out of the world per tick than the light puts in, and a
            // person who has just been told "0..=0.005" with no reason attached will conclude
            // the bound is squeamishness and go and widen it.
            Self::OutrunsTheLight {
                field,
                value,
                limit,
            } => write!(
                out,
                "{field}: {value} is outside 0..={limit}. The upper end is what the light can \
                 replace rather than a preference: a tile's ceiling moving out from under it \
                 sheds the difference to `dissipated`, so a field dragged sideways faster \
                 than this destroys more energy per tick than `light.influx` delivers, and \
                 the world empties while the energy ledger balances perfectly throughout"
            ),
            // Also long, and for the same reason: neither end of this one can be guessed
            // from the name of the setting. The lower end is where the water stops being
            // anisotropic at all, which is the world in which SPEC section 8's conservation
            // law holds and nothing can swim; the upper end is where the arithmetic stops.
            Self::OutsideRange {
                field,
                value,
                least,
                most,
            } => write!(
                out,
                "{field}: {value} is outside {least}..={most}. At {least} the water resists \
                 a cell equally in every direction, which is the model this project shipped \
                 with and the one in which a free body's total velocity is conserved and \
                 decays to nothing, so no arrangement of muscles can move it. The upper end \
                 is a limit of the arithmetic rather than a preference: measured at 3 with a \
                 collision stiffness of 5,000, a cell's velocity became not-a-number"
            ),
            // The one refusal in this file whose upper end is not a fact about the arithmetic
            // or about the world, but about **what has been run**. It says that and nothing
            // else. In particular it does not say the population falls far enough for drift to
            // outrun selection: with the flat world's own trough measured at 766 organisms and
            // the largest real coefficient in this world at 0.85 %/generation, `N·s` is 3.6 at
            // an amplitude of a half and 1.8 at three quarters - so that sentence would be
            // false everywhere the gate allows, and a refusal somebody can disprove is a
            // refusal somebody deletes.
            Self::Unmeasured {
                field,
                value,
                least,
                most,
            } => write!(
                out,
                "{field}: {value} is outside {least}..={most}. Nothing deeper than a half has \
                 ever been measured in this world, and that is the whole of the reason: the \
                 upper end is where the evidence stops. The lower end is no season at all, \
                 which is the control for every claim about one"
            ),
            // Long for the reason the two above are long. A period is a number of ticks and
            // reads like any other; what makes a short one refused is that the light would be
            // changing faster than the field it is filling can follow, so the world would be
            // running under a season nothing in it could feel - which looks identical in a
            // settings file to one it can.
            Self::FasterThanTheWater {
                field,
                value,
                least,
            } => write!(
                out,
                "{field}: {value} is below {least}. That floor is `light.cap / light.influx` - \
                 the time a tile takes to fill from empty - and below it the light changes and \
                 the water does not: measured, the standing field swings 2.04% at a period of \
                 2,000 against 6.74% at 20,000. There is no upper bound, because a very slow \
                 climate is a legitimate experiment; and nought is refused rather than meaning \
                 no season, because `light.season_amplitude` is the off switch and a second one \
                 is two ways of saying the same thing"
            ),
            // A sentence of its own, rather than a use of the one above with a lower end
            // of zero and no upper end. Written that way it would read
            // "light.influx: -0.1 is outside 0..=340282350000000000000000000000000000000",
            // which is thirty-nine digits of noise around the one word that matters.
            Self::Negative { field, value } => {
                write!(out, "{field}: {value} must not be negative")
            }
            Self::NotPositive { field, value } => {
                write!(out, "{field}: {value} must be greater than zero")
            }
            Self::Zero { field } => write!(out, "{field}: must not be zero"),
            Self::AboveCeiling {
                field,
                value,
                ceiling,
            } => write!(out, "{field}: {value} exceeds the ceiling of {ceiling}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Turn one of the document's numbers into one the simulation can hold, or refuse it.
///
/// This is the only place in the project where a number changes size, and it is a
/// function rather than a cast so that the change can be refused. A plain cast cannot
/// fail: hand it something too big and it returns infinity, something too small and it
/// returns zero, and in both cases the run carries on with a number nobody wrote.
///
/// `field` is the setting's full path, `light.influx` and the like. It is carried here
/// purely so a refusal can name it - a complaint about a number, with no indication which
/// of thirty-six settings it came from, sends you hunting through the file by hand.
///
/// # The rule
///
/// A converted number is accepted when it comes out **normal**, and zero is accepted as
/// well.
///
/// "Normal" is the arithmetic's own word for a number carrying its full complement of
/// digits, and asking for it turns out to close three quite different holes at once:
///
/// - Infinity is not normal, so a number too large to hold is refused instead of becoming
///   infinity.
/// - Zero is not normal, so a number too small to hold is refused instead of silently
///   becoming nothing - which is the more dangerous of the two, because a world with no
///   light does not crash, it just goes dark.
/// - And the *degraded* numbers are not normal either. Below about `1e-38` the arithmetic
///   can still hold a value, but only by throwing away most of its digits: `1e-40` is
///   finite, is not zero, passes every check anybody would think to write, and arrives
///   carrying error some tens of thousands of times larger than the same number written
///   anywhere else. That band is invisible to inspection and it is why the rule is
///   phrased this way rather than as the two obvious checks.
///
/// Zero itself is let through as a deliberate exception, because a person who writes
/// `patchiness = 0.0` means a light field with no blotchiness in it, which is a
/// perfectly good thing to ask for and a likely first experiment. The exception is on
/// what was *written*, not on what came out: a document saying `0.0` is accepted, and one
/// saying `1e-300` - which would also arrive as zero - is not.
///
/// The rule that suggests itself first, and is wrong, is to insist the number converts
/// back to exactly what was written. See `every_spec_default_literal_narrows` for what
/// that costs.
fn narrow(field: &'static str, value: f64) -> Result<f32, ConfigError> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the one narrowing cast in the project. CLAUDE.md's rule is to use \
                  TryFrom and handle the failure; the standard library provides no \
                  TryFrom for this pair, so this function is that TryFrom, and the check \
                  below is the handling"
    )]
    let narrowed = value as f32;

    let written_as_zero = value == 0.0;

    if narrowed.is_normal() || written_as_zero {
        Ok(narrowed)
    } else {
        Err(ConfigError::NotRepresentable { field, value })
    }
}

// ---------------------------------------------------------------------------------------
// The ceilings
//
// The largest each `[limits]` value is allowed to be, all four taken from CLAUDE.md's own
// table of caps rather than invented here. Checking a configuration is the last thing that
// happens before Phase 2 allocates every arena in the simulation, which makes it the only
// gate that runs while the defences still hold "even if the simulation code is wrong".
// ---------------------------------------------------------------------------------------

/// CLAUDE.md's figure for a run on the graphics card. The default of 4,000 is what the
/// processor-side implementation is built for; the ceiling is set at the larger number so
/// that a Phase 9 experiment does not need a code change to run.
const MAX_ORGANISMS_CEILING: u32 = 100_000;

/// Bounds how long growing a body takes and how much physics each one costs.
const MAX_CELLS_PER_ORGANISM_CEILING: u32 = 64;

/// **This number moves only in the same commit that adds a metabolic cost per gene.**
///
/// Gene duplication is the mutation operator that makes complexity possible - it is the
/// reason the genome is designed the way it is - and it is also an exponential bloat
/// machine. A lineage that duplicates faster than selection punishes it grows a genome
/// into the megabytes and takes the process down with it. What stops that is not this cap
/// but the *cost* of carrying a gene, which is the thing selection can act on. Until that
/// cost exists, this cap is the only thing standing in the way, so raising it without
/// adding the cost removes the guard and puts nothing in its place.
const MAX_GENES_CEILING: u32 = 128;

/// Read off SPEC section 7, where a gene's `min_step` and `max_step` are single bytes, so
/// a development step beyond 255 could not be named by any gene in the genome.
const MAX_DEV_STEPS_CEILING: u32 = 255;

/// The fastest energy may be told to spread between neighbouring tiles, and the only bound
/// in this file that comes from arithmetic rather than from meaning.
///
/// SPEC section 3 calls `diffusion` a "lateral spread per tick", which reads like every
/// other fraction here and would suggest a bound of one. SPEC section 4 explains why that
/// is wrong. The resource field spreads energy with an explicit five-point stencil, in
/// which a tile gives away this share of its difference to each of four neighbours; past a
/// quarter it gives away more than the difference, so it is sent beyond the neighbours it
/// was levelling with and has to be dragged back further next tick. The overshoot compounds
/// until the numbers stop being finite.
///
/// **Nothing further down would notice.** Overshoot moves energy rather than inventing it,
/// so every pair of tiles still trades one number both ways and the energy ledger of SPEC
/// section 5 goes on reporting a perfectly balanced world while the field turns into
/// nonsense. That is what makes this a gate rather than a warning: it is the only thing
/// standing between a plausible-looking number in a configuration file and a run whose
/// output means nothing.
///
/// A quarter itself is allowed. A limit that cannot be reached is a limit one step lower
/// with nobody able to tell which.
///
/// ⭐ **Public since Phase 6, and the reason is the one thing a slider must not be able to
/// do.** `panel.rs` puts `light.diffusion` on a slider, and a slider whose far end was
/// written out as `0.25` in a second file would be one edit away from being able to express
/// a value this gate refuses - and, worse, one edit away from being able to express a value
/// this gate *would* refuse if it were asked, in a program where the only reason it is asked
/// is that somebody remembered to ask it. The slider's upper end is this constant, so the
/// two cannot drift apart; and [`RawConfig::validate`] is still run over every change, so
/// the bound is a convenience and the gate is the guarantee. See
/// `coacervate_render::settings`.
pub const DIFFUSION_STABILITY_LIMIT: f32 = 0.25;

/// The fastest the blotches of light may be told to slide sideways, in world units per
/// tick, and the second bound in this file that comes from what the world *does* rather than
/// from what the setting means.
///
/// ⭐ **The failure past it is the same shape as [`DIFFUSION_STABILITY_LIMIT`]'s and arrives
/// by a completely different road.** Nothing here stops computing. What happens is that the
/// world quietly runs out of energy while the ledger of SPEC section 5 balances to the last
/// digit throughout, because the loss is *accounted for* - it goes to `dissipated` - and an
/// account that is being credited correctly is invisible to every check in the project.
///
/// # The arithmetic
///
/// A tile cannot hold more than its ceiling (SPEC section 4), and a drifting patch field is
/// a field of ceilings sliding sideways. Every tick, a full tile whose ceiling has moved down
/// under it sheds the difference to `dissipated`. So the drift costs the world
///
/// ```text
/// loss per tile per tick  =  patch_drift × |d(target)/dx|
/// ```
///
/// and `target = cap × profile(y) × (1 + patchiness × noise)`, so
///
/// ```text
/// |d(target)/dx|  =  cap × profile × patchiness × |d(noise)/dx|
/// ```
///
/// The noise is a smoothstep between lattice points `NOISE_LATTICE_SPACING` tiles apart,
/// which in the shipped world is 128 world units; a smoothstep's steepest slope is 1.5 across
/// its span, and two neighbouring lattice heights can differ by 2. So `|d(noise)/dx|` is at
/// most `2 × 1.5 / 128 = 0.0234` per world unit, and at the shipped `cap` of 8 and a
/// `patchiness` of 1 the steepest ceiling in the world falls by `0.1875 × profile` per unit
/// of drift.
///
/// A tile is offered `influx × profile` per tick. The profile cancels, and the two are equal
/// at `0.001 / 0.1875 = 0.0053`. Past that the drift takes more out of the brightest water
/// than the light puts into it, whatever is living there.
///
/// **Five thousandths, rounded down from that, and it is a bound on the shipped light rather
/// than on any light.** That is a real difference from `DIFFUSION_STABILITY_LIMIT`, which is
/// a fact about a stencil and holds at every configuration; a world with `light.influx`
/// turned up is a world that could follow a faster drift. It is written as one number anyway,
/// because a bound that moved with four other settings is a bound nobody can reason about and
/// because the interesting range is nowhere near it: the shipped **0.0006** is a ninth of it,
/// and SPEC section 4's whole argument is about the window between 0.0003 and 0.001.
///
/// Nought is allowed and is the world as it was before Phase 7's Group G: a fixed field of
/// blotches, worked out once from the seed. It is the control for every claim about drift.
pub const PATCH_DRIFT_CEILING: f32 = 0.005;

/// The shortest season the water can follow, in ticks, and the transfer function that decides
/// it.
///
/// ⭐⭐ **Phase 7's Group L.** `light.season_period` is how long one whole rise and fall of
/// `light.influx` takes. What bounds it below is not the arithmetic and not the ledger: it is
/// that **the field the light is filling has its own time constant**, and below that the light
/// changes and the water does not.
///
/// # The arithmetic
///
/// A tile fills from empty at `influx × profile` a tick against a ceiling of `cap × profile`, so
/// the profile cancels and the filling time is `cap / influx` — **8,000 ticks** at SPEC section
/// 3's shipped 8.0 and 0.001. A season shorter than that is a light the standing field averages
/// away rather than follows.
///
/// **Measured, on an empty world at an amplitude of 0.25**: the standing field swings **2.04%**
/// peak to trough at a period of 2,000, against **6.74%** at 20,000. Three times the season for
/// a third of the effect, and the shape of that curve is the tile's own filling time.
///
/// # What the shipped period is, and why 21,000
///
/// The window runs from this floor up to about 50,500 ticks, the median species lifetime, above
/// which a lineage lives entirely inside one half-cycle and the season is a trend rather than a
/// season. **21,000** sits inside it and is chosen from three readings at once: it is 12.0
/// generations at the measured 1,754-tick generation, so **6.0 generations per half cycle**;
/// 2.4 whole cycles fit inside a median species life; and `gcd(21,000, 25,000) = 1,000`, so the
/// project's 25,000-tick checkpoints walk **21 distinct phases** of the season instead of the
/// four that a period of 20,000 would give them.
///
/// # ⚠️ There is deliberately no ceiling
///
/// A million-tick climate is a legitimate experiment. An invented upper bound is one somebody
/// argues with on the evening an experiment is refused, and there is nothing on the other side
/// of it that fails.
pub const SEASON_PERIOD_FLOOR: u64 = 8_000;

/// The deepest season anything has been measured at, as a fraction of `light.influx`.
///
/// ⭐⭐ **Phase 7's Group L.** The multiplier the light is scaled by is
/// `1 + season_amplitude × triangle(phase)`, so an amplitude of 0.25 is a world running between
/// 0.75× and 1.25× its stated influx over a period. Carrying capacity is proportional to influx
/// to within 10% over a fourfold range — measured biomass 14,936 / 23,320 / 32,276 / 49,356 /
/// 65,985 at 0.5× through 2× — so the amplitude **is** a carrying-capacity swing and can be read
/// as one.
///
/// # A half is where the evidence stops, and that is the whole of the reason
///
/// 0.25 and 0.5 are the only amplitudes ever run in this world. What they do to the population is
/// measured against the flat run's **own** second-half minimum rather than against its mean,
/// which is the correction that halves the apparent depth of the trough — and against whole-cycle
/// integrals rather than end-of-run readings. Fifteen whole periods, world ticks 15,000 to
/// 330,000:
///
/// | | flat | ±25% | ±50% |
/// | --- | --- | --- | --- |
/// | second-half **low** | **517** | 468 | 529 |
/// | second-half **high** | **1,234** | 2,109 | 2,202 |
/// | peak to trough | **2.39×** | 4.51× | 4.16× |
/// | mean alive over the window | **1,099** | 1,369 | 1,454 |
/// | mean cells per body | **5.65** | 4.11 | 3.32 |
///
/// **The flat world already swings by about two and a half on its own**, and a seasoned world
/// does not collapse: it carries *more* organisms on average, because the same energy goes into
/// more and smaller bodies. ⚠️ The trough row is undersampled at 5,000-tick resolution — the ±50%
/// low sits above the ±25% one, which is noise rather than a reversal — and it should be read as
/// *the trough does not deepen much* and no further.
///
/// ⚠️ **It is not a drift bound**, and the refusal must not say it is. With a trough of a few
/// hundred bodies and the largest real selection coefficient in this world at 0.85 %/generation,
/// `N·s` is comfortably above one everywhere the gate allows and for some way past it.
///
/// A bound is still not optional. Above **1.0** the multiplier goes negative, which is light
/// running backwards: tiles draining into no account, and SPEC section 5's invariant failing.
pub const SEASON_AMPLITUDE_CEILING: f32 = 0.5;

/// Isotropic water: the drag a cell feels across its own body axis is the drag it feels
/// along it, which is what this project shipped with until Phase 7.
///
/// It is the *floor* rather than the default, and the two are deliberately different. Below
/// it the water would resist a cell **less** across its axis than along it, which is not a
/// slender body in a fluid at all - it is a body that slips sideways more easily than it
/// slides forwards - and nothing in SPEC describes such a thing.
///
/// At exactly this value SPEC section 8's conservation law holds: every internal force
/// appears as `+f` on one cell and `-f` on another, there is no mass, and one scalar
/// multiplies every cell's velocity, so a free body's **total** velocity is a conserved
/// quantity of the integrator that decays to nothing. Nothing can move, under any
/// parameters, by any arrangement of muscles. See `physics.rs`.
pub const DRAG_ANISOTROPY_FLOOR: f32 = 1.0;

/// The most the drag across a cell's body axis may be sharpened, and the one bound in this
/// file that was found by breaking the arithmetic rather than by reading SPEC.
///
/// The drag across the axis is `drag` raised to this power, so a larger number is thicker
/// water sideways. Three is where a prototype stopped computing: at `collision_stiffness =
/// 5,000` - which is inside the range `physics_is_stable_under_a_pile_up` measures the
/// explicit integrator to survive, and about a hundred and twenty times what the world
/// ships with - a pile of cells produced not-a-number within a few hundred ticks and every
/// cell in it left the world.
///
/// The mechanism is worth writing down, because it is not the ordinary stiff-spring
/// overshoot. Splitting a velocity into two components and damping them by different
/// amounts is a *rotation* of the velocity towards the body axis, and a cell whose axis is
/// itself turning under a stiff collision can be handed a velocity that points somewhere the
/// force never pushed it. The sharper the split, the further that goes; past three the
/// correction and the overshoot stop being able to cancel.
///
/// Three itself is allowed, because a limit that cannot be reached is a limit one step lower
/// with nobody able to tell which - the same reading [`DIFFUSION_STABILITY_LIMIT`] takes.
/// The world ships at **2.0**, which is a third of the way in and is where slender-body
/// theory puts a real one: it makes a cell about **2.1 times** as mobile along its axis as
/// across it, against the factor of two a long thin thing in water actually has.
pub const DRAG_ANISOTROPY_CEILING: f32 = 3.0;

// ---------------------------------------------------------------------------------------
// The three kinds of bound
//
// Each of these narrows a number and then asks one question of it. Which one a setting
// gets is decided entirely by what SPEC says that setting *means*, and where SPEC says
// nothing, nothing is imposed. An invented bound is a bound somebody has to argue with
// later, on an evening when nobody remembers why it was put there - and the argument will
// happen at the worst moment, because the reason to widen a bound is always that an
// experiment has just been refused.
// ---------------------------------------------------------------------------------------

/// A setting SPEC describes as a fraction of something: `0..=1`, ends included.
fn fraction(field: &'static str, value: f64) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if (0.0..=1.0).contains(&narrowed) {
        Ok(narrowed)
    } else {
        Err(ConfigError::NotAFraction {
            field,
            value: narrowed,
        })
    }
}

/// A fraction whose upper end is lower than one because the arithmetic stops working
/// there: `0..=limit`, ends included.
///
/// Deliberately a separate gate from [`fraction`] rather than that function with a
/// parameter. The two look identical and are not the same kind of claim at all. A fraction
/// is bounded because of what the setting *means*, and widening one is a conversation about
/// meaning; this is bounded because of what the arithmetic *does*, and widening it is a
/// conversation about whether the simulation still computes anything. Sharing one function
/// between them would put both conversations behind the same name.
fn stable(field: &'static str, value: f64, limit: f32) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if (0.0..=limit).contains(&narrowed) {
        Ok(narrowed)
    } else {
        Err(ConfigError::Unstable {
            field,
            value: narrowed,
            limit,
        })
    }
}

/// A speed bounded above by what the light can put back: `0..=limit`, ends included.
///
/// A fourth gate rather than [`stable`] with a different constant, for the reason [`stable`]
/// itself is not [`fraction`]: the two are not the same claim. `stable` says the arithmetic
/// stops working; this says the arithmetic works perfectly and the world empties. Sharing a
/// function would share the sentence, and the sentence is the whole value of the refusal.
fn followable(field: &'static str, value: f64, limit: f32) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if (0.0..=limit).contains(&narrowed) {
        Ok(narrowed)
    } else {
        Err(ConfigError::OutrunsTheLight {
            field,
            value: narrowed,
            limit,
        })
    }
}

/// A setting bounded at both ends, where both ends mean something.
///
/// Deliberately a third gate rather than [`stable`] with a lower end, for the reason
/// [`stable`] gives about not being [`fraction`]: the sentence a refusal has to write is
/// different, because here there are two ends to explain instead of one.
fn within(field: &'static str, value: f64, least: f32, most: f32) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if (least..=most).contains(&narrowed) {
        Ok(narrowed)
    } else {
        Err(ConfigError::OutsideRange {
            field,
            value: narrowed,
            least,
            most,
        })
    }
}

/// A setting bounded at both ends, where the upper end is where the **measurements** stop.
///
/// A fifth gate rather than [`within`] with different constants, for the reason [`within`]
/// itself is not [`stable`]: the two are not the same claim, and the sentence a refusal writes
/// is the whole of its value. `within` says the arithmetic or the model stops working; this says
/// the arithmetic works perfectly and **nobody has run it**.
fn measured(field: &'static str, value: f64, least: f32, most: f32) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if (least..=most).contains(&narrowed) {
        Ok(narrowed)
    } else {
        Err(ConfigError::Unmeasured {
            field,
            value: narrowed,
            least,
            most,
        })
    }
}

/// A clock bounded below by how fast the thing it is a clock for can respond.
///
/// A sixth gate, and a whole number rather than a fraction, so nothing narrows: a period is a
/// count of ticks. Nought is refused by the same comparison as every other value below the
/// floor, which is what stops there being a forbidden gap a slider can be dragged into.
fn followable_by_the_water(
    field: &'static str,
    value: u64,
    least: u64,
) -> Result<u64, ConfigError> {
    if value >= least {
        Ok(value)
    } else {
        Err(ConfigError::FasterThanTheWater {
            field,
            value,
            least,
        })
    }
}

/// A setting that has to exist for the simulation to mean anything: a world with no
/// width, a tile that can hold no energy, a tick worth no years.
fn positive(field: &'static str, value: f64) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if narrowed > 0.0 {
        Ok(narrowed)
    } else {
        Err(ConfigError::NotPositive {
            field,
            value: narrowed,
        })
    }
}

/// A count of something the simulation builds room for.
///
/// What comes back is a number that has no way to be zero. That is the whole trick: every
/// count in a checked configuration is one of these, so "a count is never zero" is not a
/// rule anybody has to remember to apply to the next one that gets added. It is a rule the
/// compiler applies, because there is no way to put a zero into the field at all.
fn counted(field: &'static str, value: u32) -> Result<NonZeroU32, ConfigError> {
    NonZeroU32::new(value).ok_or(ConfigError::Zero { field })
}

/// A count that also has a largest value this program is built to survive.
fn capped(field: &'static str, value: u32, ceiling: u32) -> Result<NonZeroU32, ConfigError> {
    let count = counted(field, value)?;

    if count.get() <= ceiling {
        Ok(count)
    } else {
        Err(ConfigError::AboveCeiling {
            field,
            value,
            ceiling,
        })
    }
}

/// A setting that may perfectly well be nothing, but cannot be less than nothing.
///
/// A light influx of zero is a dark world, which is a legitimate and rather bleak
/// experiment. A *negative* influx is tiles that drain into no account, which is the
/// energy ledger of SPEC section 5 ceasing to balance.
fn non_negative(field: &'static str, value: f64) -> Result<f32, ConfigError> {
    let narrowed = narrow(field, value)?;

    if narrowed >= 0.0 {
        Ok(narrowed)
    } else {
        Err(ConfigError::Negative {
            field,
            value: narrowed,
        })
    }
}

// ---------------------------------------------------------------------------------------
// A configuration a run can actually be given
//
// The same seven tables again, holding numbers of the size the simulation runs on. There
// is no way to build one of these except by putting a document through
// `RawConfig::validate`, so possession of a `Config` is itself the evidence that its
// contents were checked.
// ---------------------------------------------------------------------------------------

/// The checked `[world]` table: the seed, the size of the world, and the resolution of
/// the resource grid laid over it.
///
/// Named `WorldConfig` rather than `World` deliberately. Phase 2 needs `World` for the
/// thing this describes - the actual soup, its grid of energy and its living population -
/// and a settings struct sitting on that name would have to be moved out of the way at
/// exactly the moment there is most else to think about.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    pub seed: u64,
    pub width: f32,
    pub height: f32,
    pub grid_cols: NonZeroU32,
    pub grid_rows: NonZeroU32,
    pub years_per_tick: f32,
}

/// The checked `[light]` table: where the energy in the world comes from.
#[derive(Debug, Clone, PartialEq)]
pub struct LightConfig {
    pub influx: f32,
    pub cap: f32,
    pub gradient: f32,
    pub patchiness: f32,

    /// How fast the blotches of light slide sideways through the world, in world units per
    /// tick.
    ///
    /// ⭐ **The setting that makes swimming worth anything**, and it is the counterpart to
    /// `physics.drag_anisotropy`: that one made locomotion *possible*, and a 310,000-tick run
    /// with it in place still ended with one myocyte, because a static field gives a body
    /// nowhere better to go. See SPEC section 4 for the window this number has to sit in.
    ///
    /// The short version is three measured speeds. Budding disperses a lineage at **0.0003**
    /// world units per tick - one roughly six-unit bud every fifteen-hundred-tick generation.
    /// A tile refills from empty in eight thousand ticks, which over a tile's own width is
    /// **0.001**. An anisotropic swimmer manages **0.0005 to 0.0025**. So a field drifting
    /// between the first and the second of those is one a body that can swim can follow and a
    /// body that can only bud cannot.
    pub patch_drift: f32,

    pub diffusion: f32,

    /// How long one whole rise and fall of [`Self::influx`] takes, in ticks.
    ///
    /// ⭐⭐ **Phase 7's Group L, and it is the only clock in this world besides the drift.** SPEC
    /// section 4's field already moves in *space*; this is what makes it move in *time*, so that
    /// being adapted stops being a fixed fact about a lineage. It is on `influx` and on nothing
    /// else, which is the whole energy argument: `influx` enters no ceiling, so a season needs
    /// no retarget, moves no target down under a full tile, and sheds no spill whatever. See
    /// [`SEASON_PERIOD_FLOOR`] for the window it has to sit in.
    pub season_period: u64,

    /// How deep that rise and fall goes, as a fraction of [`Self::influx`].
    ///
    /// ⭐⭐ **The off switch, and the only one.** The light is scaled by
    /// `1 + season_amplitude × triangle(phase)`, computed **unconditionally** — at nought that
    /// expression is exactly 1.0 and the world is bit-for-bit the world that was there before a
    /// season existed. It ships at nought. See [`SEASON_AMPLITUDE_CEILING`] for why a half is
    /// the far end and what the trough actually does.
    pub season_amplitude: f32,
}

/// The checked `[physics]` table: how the soup pushes back.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsConfig {
    pub drag: f32,

    /// How much harder the water resists a cell moving **across** its own body axis than
    /// along it, as the power `drag` is raised to.
    ///
    /// ⭐ **This is the one setting in the file without which nothing in the world can
    /// swim**, and the reason is arithmetic rather than balance. With one drag for every
    /// direction, `physics.rs`'s integrator has a conserved quantity: every internal force
    /// is `+f` on one cell and `-f` on another, there is no mass, and one scalar multiplies
    /// every velocity - so a free body's *total* velocity is only ever multiplied by `drag`
    /// and decays to nothing. That is stronger than the scallop theorem, which a travelling
    /// wave normally escapes. Measured over 2,000 ticks of a twelve-celled undulator, the
    /// body moved 0.00015 world units, which is the noise floor of a 32-bit float.
    ///
    /// What real swimming at this scale works on is **drag anisotropy**: a slender body
    /// resists motion across its axis roughly twice as hard as along it, so a wave passing
    /// down it pushes the water backwards and the body forwards. Two is that factor, and it
    /// is what the world ships with. See [`DRAG_ANISOTROPY_FLOOR`] for what one means and
    /// [`DRAG_ANISOTROPY_CEILING`] for why three is the end.
    pub drag_anisotropy: f32,

    pub collision_stiffness: f32,
    pub spring_damping: f32,
}

/// The checked `[behaviour]` table: how hard SPEC section 9's controller drives a muscle.
#[derive(Debug, Clone, PartialEq)]
pub struct BehaviourConfig {
    /// How hard a myocyte works with nothing telling it otherwise: SPEC section 9's
    /// `amplitude = clamp(resting_amplitude + sensor_gain × signal, 0, 1)`.
    ///
    /// ⭐ **This is the number a lineage's *first* myocyte is worth**, which is the one that
    /// has to pay before any of the others can. A muscle with no sensocyte adhered to it, or
    /// one whose sensors read nothing, still contracts - so locomotion is reachable by mutation
    /// before sensing is, rather than the two having to appear together. SPEC section 9 shipped
    /// this at **0.3** until Phase 7's Group H, and at 0.3 an unsensed eight-celled undulator
    /// covered **0.3 world units in a two-thousand-tick lifetime**: a twentieth of one of its
    /// own cells, which is nothing for selection to see.
    ///
    /// What it costs to raise it is the room a sensor has to work in *upwards*: at 0.8 a
    /// sensocyte can add 0.2 and take away 0.8. That asymmetry is deliberate. `sensor_gain` is
    /// signed, so inhibition was always half of what a sensor is for, and a body with one side
    /// inhibited and the other not is a **turn** - which is the thing a swimmer needs and a
    /// pulse is not.
    pub resting_amplitude: f32,

    /// How much of its rest length a myocyte at full amplitude works through, either way: SPEC
    /// section 9's `rest_len = base_rest × (1 + amplitude × stroke × sin(...))`.
    ///
    /// ⭐⭐ **Phase 7's Group H, and the whole of it.** Group F made swimming possible and
    /// Group G gave a body somewhere to swim *to*; neither made swimming worth anything,
    /// because at the **0.4** SPEC was first written with a perfect undulator driven flat out
    /// covered about four world units in a lifetime while the field it was chasing moved 1.2.
    /// The margin existed and nothing found it in 310,000 ticks.
    ///
    /// The measurement that decided this is in `docs/PHASE7.md`, and its shape is the reason
    /// this is the lever rather than the water: **speed goes as roughly the cube of the
    /// stroke** and only as the *square root* of anything else. Doubling the stroke from 0.4
    /// to 0.8 multiplies the distance a body covers by eleven while multiplying the work it
    /// does by four; sharpening `physics.drag_anisotropy` from 2 to 3 multiplies it by 1.1.
    ///
    /// **One is the end of it, and the reason is arithmetic rather than taste.** The amplitude
    /// above is clamped into `0..=1`, so the shortest a spring ever asks to be is
    /// `base_rest × (1 − stroke)` - and past one that is *negative*, which is a spring that
    /// pulls at every phase of its cycle instead of oscillating about anything. It is not a
    /// slower failure than it sounds: measured, a body at 1.5 travels twenty-four times
    /// further than one at 1.0, by hauling itself through its own cells. So the bound a
    /// fraction gets is the bound this needs, and the two numbers being the same is not a
    /// coincidence.
    pub stroke: f32,
}

/// The checked `[metabolism]` table: what living costs, and what reproducing costs.
#[derive(Debug, Clone, PartialEq)]
pub struct MetabolismConfig {
    pub upkeep_scale: f32,

    /// What one gene costs its organism per tick, on top of what its cells cost.
    ///
    /// ⭐ **This is not here to stop genome bloat, and reading it that way gets the sign of
    /// the argument backwards.** CLAUDE.md's table of caps says *"never remove or raise
    /// [`limits.max_genes`] without also adding a metabolic cost per gene"*, which sounds like
    /// a brake. It is the opposite.
    ///
    /// SPEC section 7's mutation rates have duplication and insertion together at 0.03 against
    /// deletion's 0.02, so genomes drift **upward** and a lineage left alone ends up pressed
    /// against the cap. And SPEC section 7 is explicit that at the cap a lengthening mutation
    /// *fails* rather than truncating - deliberately, because truncating would eat the neutral
    /// tail at the end of the genome, which is exactly the raw material duplication feeds on.
    ///
    /// Put those two together and the consequence is bad in a way that is easy to miss: **gene
    /// duplication, the operator the whole genome design exists for, switches itself off
    /// precisely when a lineage is at its most elaborate.** A saturated lineage can only
    /// duplicate in a generation where a deletion has happened to make room first.
    ///
    /// So the cost is here to hold genomes *away* from the ceiling, so that duplication stays
    /// available. A lineage should be pushed back by selection long before it arrives there,
    /// and never find out that the wall exists.
    pub gene_cost: f32,

    pub movement_cost: f32,
    pub reproduction_threshold: f32,
    pub offspring_share: f32,
}

/// The checked `[mutation]` table: how often a genome is copied imperfectly, and how far
/// off each kind of mistake takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationConfig {
    pub point_rate: f32,
    pub point_sigma: f32,
    pub duplication_rate: f32,
    pub deletion_rate: f32,
    pub insertion_rate: f32,
    pub reorder_rate: f32,
    pub genome_duplication_rate: f32,
}

/// The checked `[limits]` table: the sizes every arena in the simulation is built to.
#[derive(Debug, Clone, PartialEq)]
pub struct LimitsConfig {
    pub max_organisms: NonZeroU32,
    pub max_cells_per_organism: NonZeroU32,
    pub max_genes: NonZeroU32,
    pub max_dev_steps: NonZeroU32,
}

/// The checked `[run]` table: when a run is to stop, and what to do if everything dies.
#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    pub max_wall_clock_hours: f32,

    /// How many ticks the run may last, or nothing at all if it is to run until it is
    /// stopped some other way.
    ///
    /// The document writes "no limit" as zero, which is fine in a text file and would be
    /// a trap here: every later check against it would have to know that one particular
    /// value means the opposite of what it says. The translation happens once, at the
    /// gate, and this type is what is left when the special value has been taken out of
    /// circulation.
    pub max_ticks: Option<u64>,

    /// How many ticks the simulation may compute per second of real time, or nothing at
    /// all if it should go as fast as the machine allows.
    ///
    /// This is the only thing separating a run you watch from a run you *notice*. The
    /// simulation's own clock is fixed - every tick is the same slice of simulated time
    /// however long it took to compute - so slowing this down does not change what
    /// happens, only how fast it arrives. It is what the `slow` profile is made of.
    ///
    /// Zero means uncapped, translated away here for the same reason as `max_ticks`.
    pub max_ticks_per_second: Option<u32>,
    pub reseed_on_extinction: bool,
}

/// A configuration that has been checked, and which a run can therefore be given.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub world: WorldConfig,
    pub light: LightConfig,
    pub physics: PhysicsConfig,
    pub behaviour: BehaviourConfig,
    pub metabolism: MetabolismConfig,
    pub mutation: MutationConfig,
    pub limits: LimitsConfig,
    pub run: RunConfig,
}

impl RawConfig {
    /// Check a document, and turn it into a configuration a run can be given.
    ///
    /// This is the only gate. Everything downstream of it may assume its numbers are
    /// sane, because there is no other way to obtain a [`Config`].
    ///
    /// # Errors
    ///
    /// Returns the first thing wrong with the document, in a sentence naming the setting
    /// at fault. It returns rather than crashing because a bad configuration is somebody
    /// having typed something, which is an ordinary event rather than a sign the program
    /// has gone wrong; what is wanted there is a sentence that can be acted on, not a
    /// stack trace.
    pub fn validate(self) -> Result<Config, ConfigError> {
        Ok(Config {
            world: WorldConfig {
                seed: self.world.seed,
                width: positive("world.width", self.world.width)?,
                height: positive("world.height", self.world.height)?,
                grid_cols: counted("world.grid_cols", self.world.grid_cols)?,
                grid_rows: counted("world.grid_rows", self.world.grid_rows)?,
                // Presentation only - SPEC section 2 keeps it out of the physics
                // entirely - but a tick worth no years makes the deep-time display read
                // zero for ever.
                years_per_tick: positive("world.years_per_tick", self.world.years_per_tick)?,
            },
            light: LightConfig {
                // May be nothing: a world with no light coming in is a legitimate, if
                // short, experiment.
                influx: non_negative("light.influx", self.light.influx)?,
                cap: positive("light.cap", self.light.cap)?,
                // "0 = uniform, 1 = fully top-weighted", says the document itself.
                gradient: fraction("light.gradient", self.light.gradient)?,
                // Bounded above by arithmetic rather than by a comment. SPEC section 4
                // builds each tile's target as
                // `cap x light_profile(y) x (1 + patchiness x noise)`, and the noise runs
                // either side of zero - so any patchiness above one drives some tiles'
                // targets negative, those tiles drain into no account, and the energy
                // ledger of section 5 stops balancing. Nothing in section 3 says so; it
                // follows from the formula.
                patchiness: fraction("light.patchiness", self.light.patchiness)?,
                // How fast those blotches slide sideways, and the one setting here bounded
                // by what the light can afford. A ceiling moving out from under a full tile
                // sheds the difference to `dissipated`, so the drift is a continuous drain
                // proportional to `patchiness × patch_drift`; past `PATCH_DRIFT_CEILING` it
                // takes more out of the brightest water than `light.influx` puts in. See
                // that constant, and SPEC section 4.
                patch_drift: followable(
                    "light.patch_drift",
                    self.light.patch_drift,
                    PATCH_DRIFT_CEILING,
                )?,
                // "lateral spread per tick", and the one setting here bounded by whether
                // the arithmetic survives rather than by what the setting means. SPEC
                // section 4: past a quarter the five-point stencil overshoots and the
                // field grows without limit, while conserving energy perfectly the whole
                // way down, so the ledger never catches it. See
                // `DIFFUSION_STABILITY_LIMIT`.
                diffusion: stable(
                    "light.diffusion",
                    self.light.diffusion,
                    DIFFUSION_STABILITY_LIMIT,
                )?,
                // How long one whole rise and fall of the light takes, and the one setting here
                // bounded below by how fast the *field* can respond. A tile fills from empty in
                // `cap / influx` ticks; below that the light changes and the water does not, so
                // the world runs under a season nothing in it can feel. No upper bound: a very
                // slow climate is a legitimate experiment. See `SEASON_PERIOD_FLOOR`.
                season_period: followable_by_the_water(
                    "light.season_period",
                    self.light.season_period,
                    SEASON_PERIOD_FLOOR,
                )?,
                // How deep that rise and fall goes, and the one setting here bounded by what has
                // been **run** rather than by what anything means. Nought is no season, which is
                // what ships and what every earlier figure was measured under; a half is where
                // the measurements stop. See `SEASON_AMPLITUDE_CEILING`.
                season_amplitude: measured(
                    "light.season_amplitude",
                    self.light.season_amplitude,
                    0.0,
                    SEASON_AMPLITUDE_CEILING,
                )?,
            },
            physics: PhysicsConfig {
                // "velocity retained per tick" - a proportion of what was there before.
                drag: fraction("physics.drag", self.physics.drag)?,
                // Bounded at both ends, and neither end is SPEC's. One is the isotropic
                // water this project shipped with, in which a body's total velocity is
                // conserved and nothing can move; three is where the arithmetic stopped
                // computing when it was pushed. See the two constants.
                drag_anisotropy: within(
                    "physics.drag_anisotropy",
                    self.physics.drag_anisotropy,
                    DRAG_ANISOTROPY_FLOOR,
                    DRAG_ANISOTROPY_CEILING,
                )?,
                collision_stiffness: positive(
                    "physics.collision_stiffness",
                    self.physics.collision_stiffness,
                )?,
                // No upper bound, deliberately. Damping is conventionally written as a
                // ratio from nought to one and it is tempting to bound it that way, but
                // SPEC says nothing of the kind and a stiffly over-damped spring is a
                // real thing to want. Negative damping is refused because it is not
                // damping: it feeds energy into the springs on every tick.
                spring_damping: non_negative(
                    "physics.spring_damping",
                    self.physics.spring_damping,
                )?,
            },
            behaviour: BehaviourConfig {
                // SPEC section 9 clamps the amplitude into `0..=1` on the line after this
                // one is read, so a resting amplitude outside that range is one the clamp
                // silently undoes. Nought is a muscle that does nothing until a sensor tells
                // it to, which is a coherent world and the control for whether the resting
                // stroke is doing any work.
                resting_amplitude: fraction(
                    "behaviour.resting_amplitude",
                    self.behaviour.resting_amplitude,
                )?,
                // ⚠️ The bound here reads like tidiness and is not. A spring's rest length is
                // `base_rest × (1 + amplitude × stroke × sin)` and the amplitude is clamped
                // into `0..=1`, so one is exactly where the shortest the spring ever asks to
                // be reaches nought. Past it the rest length is negative: the spring pulls at
                // every phase of its cycle rather than oscillating, and a body hauls itself
                // through its own cells - measured at twenty-four times the distance, which
                // is how a wrong model looks like a working one. See `BehaviourConfig`.
                stroke: fraction("behaviour.stroke", self.behaviour.stroke)?,
            },
            metabolism: MetabolismConfig {
                // The document calls it a "temperature": scale it to zero and nothing
                // costs anything to be alive.
                upkeep_scale: positive("metabolism.upkeep_scale", self.metabolism.upkeep_scale)?,
                // May be nothing, which is the world every run before Phase 4 Group B was
                // in - genomes cost their organism nothing to carry - and is a legitimate
                // experiment to set up deliberately, since it is the control case for
                // whether the cost is doing anything. Not less than nothing, which would be
                // a lineage paid to grow a genome.
                gene_cost: non_negative("metabolism.gene_cost", self.metabolism.gene_cost)?,
                // May be nothing - free movement is a coherent world to run - but not
                // less, which would be organisms earning energy by swimming.
                movement_cost: non_negative(
                    "metabolism.movement_cost",
                    self.metabolism.movement_cost,
                )?,
                reproduction_threshold: positive(
                    "metabolism.reproduction_threshold",
                    self.metabolism.reproduction_threshold,
                )?,
                // "fraction of parent energy passed to offspring".
                offspring_share: fraction(
                    "metabolism.offspring_share",
                    self.metabolism.offspring_share,
                )?,
            },
            // Every one of these is a probability: how often, per gene or per genome, a
            // copy comes out wrong. There is no such thing as one and a half times out of
            // one.
            mutation: MutationConfig {
                point_rate: fraction("mutation.point_rate", self.mutation.point_rate)?,
                point_sigma: fraction("mutation.point_sigma", self.mutation.point_sigma)?,
                duplication_rate: fraction(
                    "mutation.duplication_rate",
                    self.mutation.duplication_rate,
                )?,
                deletion_rate: fraction("mutation.deletion_rate", self.mutation.deletion_rate)?,
                insertion_rate: fraction("mutation.insertion_rate", self.mutation.insertion_rate)?,
                reorder_rate: fraction("mutation.reorder_rate", self.mutation.reorder_rate)?,
                genome_duplication_rate: fraction(
                    "mutation.genome_duplication_rate",
                    self.mutation.genome_duplication_rate,
                )?,
            },
            limits: LimitsConfig {
                max_organisms: capped(
                    "limits.max_organisms",
                    self.limits.max_organisms,
                    MAX_ORGANISMS_CEILING,
                )?,
                max_cells_per_organism: capped(
                    "limits.max_cells_per_organism",
                    self.limits.max_cells_per_organism,
                    MAX_CELLS_PER_ORGANISM_CEILING,
                )?,
                max_genes: capped("limits.max_genes", self.limits.max_genes, MAX_GENES_CEILING)?,
                max_dev_steps: capped(
                    "limits.max_dev_steps",
                    self.limits.max_dev_steps,
                    MAX_DEV_STEPS_CEILING,
                )?,
            },
            run: RunConfig {
                max_wall_clock_hours: positive(
                    "run.max_wall_clock_hours",
                    self.run.max_wall_clock_hours,
                )?,
                // SPEC's `0 = unbounded` sentinel, spent here so nothing downstream
                // inherits it.
                max_ticks: (self.run.max_ticks > 0).then_some(self.run.max_ticks),
                max_ticks_per_second: (self.run.max_ticks_per_second > 0)
                    .then_some(self.run.max_ticks_per_second),
                reseed_on_extinction: self.run.reseed_on_extinction,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// One deliberate act of sabotage on an otherwise perfectly good configuration.
    ///
    /// Several tests below work the same way: take SPEC's defaults, break exactly one
    /// setting, and insist that the complaint which comes back is about that setting. One
    /// change at a time is what makes the complaint unambiguous - if two things were
    /// wrong, a check that named either of them would look correct.
    type Corruption = fn(&mut RawConfig);

    /// Every floating-point setting in a configuration, paired with its full path.
    ///
    /// Listed by hand rather than derived, so that adding a setting to SPEC and
    /// forgetting to check it shows up as a count that no longer matches.
    fn float_fields(raw: &RawConfig) -> Vec<(&'static str, f64)> {
        vec![
            ("world.width", raw.world.width),
            ("world.height", raw.world.height),
            ("world.years_per_tick", raw.world.years_per_tick),
            ("light.influx", raw.light.influx),
            ("light.cap", raw.light.cap),
            ("light.gradient", raw.light.gradient),
            ("light.patchiness", raw.light.patchiness),
            ("light.patch_drift", raw.light.patch_drift),
            ("light.diffusion", raw.light.diffusion),
            ("light.season_amplitude", raw.light.season_amplitude),
            ("physics.drag", raw.physics.drag),
            ("physics.drag_anisotropy", raw.physics.drag_anisotropy),
            (
                "physics.collision_stiffness",
                raw.physics.collision_stiffness,
            ),
            ("physics.spring_damping", raw.physics.spring_damping),
            (
                "behaviour.resting_amplitude",
                raw.behaviour.resting_amplitude,
            ),
            ("behaviour.stroke", raw.behaviour.stroke),
            ("metabolism.upkeep_scale", raw.metabolism.upkeep_scale),
            ("metabolism.gene_cost", raw.metabolism.gene_cost),
            ("metabolism.movement_cost", raw.metabolism.movement_cost),
            (
                "metabolism.reproduction_threshold",
                raw.metabolism.reproduction_threshold,
            ),
            ("metabolism.offspring_share", raw.metabolism.offspring_share),
            ("mutation.point_rate", raw.mutation.point_rate),
            ("mutation.point_sigma", raw.mutation.point_sigma),
            ("mutation.duplication_rate", raw.mutation.duplication_rate),
            ("mutation.deletion_rate", raw.mutation.deletion_rate),
            ("mutation.insertion_rate", raw.mutation.insertion_rate),
            ("mutation.reorder_rate", raw.mutation.reorder_rate),
            (
                "mutation.genome_duplication_rate",
                raw.mutation.genome_duplication_rate,
            ),
            ("run.max_wall_clock_hours", raw.run.max_wall_clock_hours),
        ]
    }

    /// The configuration the program ships with must actually load.
    ///
    /// Obvious, and it is here because the obvious rule for narrowing a number fails it.
    /// That rule is "accept the number only if converting it back gives exactly what was
    /// written", which sounds like precisely the right standard and rejects fifteen of
    /// the twenty-six numbers in SPEC's own defaults - `influx`, `drag`, `patchiness` and
    /// most of the mutation rates among them. The reason is that a value like `0.012` has
    /// no exact representation in binary at either size, so the two sizes round it
    /// slightly differently and the comparison fails. Under that rule the shipped
    /// configuration could not be loaded at all, and every plainly reasonable number a
    /// person might type would be refused.
    ///
    /// So this test is the guard rail on the narrowing rule: whatever that rule ends up
    /// being, it has to let the specification's own numbers through.
    #[test]
    fn every_spec_default_literal_narrows() {
        let raw = spec_defaults();
        let fields = float_fields(&raw);

        assert_eq!(
            fields.len(),
            29,
            "SPEC section 3 has twenty-nine decimal settings; this list has {}, so one has \
             been added or removed without being checked here",
            fields.len()
        );

        let refused: Vec<String> = fields
            .into_iter()
            .filter_map(|(field, value)| narrow(field, value).err())
            .map(|problem| problem.to_string())
            .collect();

        assert!(
            refused.is_empty(),
            "{} of the 26 numbers in SPEC's own default configuration cannot be loaded:\n  {}",
            refused.len(),
            refused.join("\n  ")
        );
    }

    /// Numbers the simulation's arithmetic cannot hold are refused, and zero is not one
    /// of them.
    ///
    /// Three ways a number can be too much for it, all of which arrive looking like
    /// perfectly ordinary decimals in a text file:
    ///
    /// **Too large.** `1e300` is a real number in the document and simply does not exist
    /// at the simulation's size. Left unchecked it becomes infinity, and infinity is
    /// catching: one tile's energy goes infinite, the total goes infinite, and the ledger
    /// SPEC section 5 leans on has nothing left to compare.
    ///
    /// **Too small.** `1e-300` collapses to nothing at all. That is the more dangerous of
    /// the two, because a light influx of zero does not crash anything: the world simply
    /// goes dark, everything starves, and it looks like a result.
    ///
    /// **Not a number.** Anything compared against it is false, including itself, so a
    /// single one turns every bound in the simulation into a check that quietly never
    /// fires again.
    ///
    /// And zero, which is none of these. `patchiness = 0.0` is a light field with no
    /// blotchiness in it, which is a perfectly good thing to want and an obvious first
    /// experiment. Negative zero is accepted with it, because it is the same quantity and
    /// a person who writes `-0.0` has not made a mistake.
    #[test]
    fn narrowing_rejects_overflow_underflow_and_nan() {
        narrow("light.cap", 1e300).expect_err("a number too large to hold must be refused");
        narrow("light.influx", 1e-300).expect_err("a number too small to hold must be refused");
        narrow("light.influx", f64::NAN).expect_err("not-a-number must be refused");
        narrow("light.cap", f64::INFINITY).expect_err("infinity must be refused");

        assert!(
            narrow("light.patchiness", 0.0).is_ok(),
            "a light field with no blotchiness in it is a valid thing to ask for"
        );
        assert!(
            narrow("light.patchiness", -0.0).is_ok(),
            "negative zero is the same quantity as zero and is not a mistake"
        );
    }

    /// SPEC's own defaults go through the gate and come out the other side unchanged.
    ///
    /// Every one of the thirty-six settings is checked, not a sample, and the reason is
    /// the shape of the code being tested rather than thoroughness for its own sake.
    /// Turning a document into a checked configuration is thirty-six hand-written
    /// assignments in a row, all of the same shape, several of them neighbours with
    /// identical types - `gradient` and `patchiness` sit side by side and are both
    /// fractions between zero and one. Copy the wrong one and every test that looks at a
    /// *sample* still passes, the configuration still loads, and the light field is
    /// blotchy in the wrong proportion for the rest of the project. Listing all of them
    /// is what makes that mistake impossible to miss.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "each value is pinned to the literal it came from; an approximate match \
                  would let a mixed-up pair of neighbouring fields through"
    )]
    fn spec_defaults_convert_into_a_validated_config() {
        let config = spec_defaults()
            .validate()
            .expect("SPEC's own default configuration must be one the program will accept");

        assert_eq!(config.world.seed, 42);
        assert_eq!(config.world.width, 2048.0);
        assert_eq!(config.world.height, 1152.0);
        assert_eq!(config.world.grid_cols.get(), 256);
        assert_eq!(config.world.grid_rows.get(), 144);
        assert_eq!(config.world.years_per_tick, 1000.0);

        assert_eq!(config.light.influx, 0.001);
        assert_eq!(config.light.cap, 8.0);
        assert_eq!(config.light.gradient, 0.75);
        assert_eq!(config.light.patchiness, 0.5);
        assert_eq!(config.light.patch_drift, 0.0006);
        assert_eq!(config.light.diffusion, 0.04);

        assert_eq!(config.physics.drag, 0.92);
        assert_eq!(config.physics.drag_anisotropy, 2.0);
        assert_eq!(config.physics.collision_stiffness, 40.0);
        assert_eq!(config.physics.spring_damping, 0.35);

        assert_eq!(config.metabolism.upkeep_scale, 1.0);
        assert_eq!(config.metabolism.gene_cost, 0.0001);
        assert_eq!(config.metabolism.movement_cost, 0.0001);
        assert_eq!(config.metabolism.reproduction_threshold, 2.2);
        assert_eq!(config.metabolism.offspring_share, 0.45);

        assert_eq!(config.mutation.point_rate, 0.06);
        assert_eq!(config.mutation.point_sigma, 0.12);
        assert_eq!(config.mutation.duplication_rate, 0.02);
        assert_eq!(config.mutation.deletion_rate, 0.02);
        assert_eq!(config.mutation.insertion_rate, 0.01);
        assert_eq!(config.mutation.reorder_rate, 0.02);
        assert_eq!(config.mutation.genome_duplication_rate, 0.0008);

        assert_eq!(config.limits.max_organisms.get(), 4000);
        assert_eq!(config.limits.max_cells_per_organism.get(), 64);
        assert_eq!(config.limits.max_genes.get(), 128);
        assert_eq!(config.limits.max_dev_steps.get(), 16);

        assert_eq!(config.run.max_wall_clock_hours, 12.0);
        assert!(!config.run.reseed_on_extinction);
    }

    /// The seed arrives at the far end of the gate as the number that was written, for
    /// any number that can be written.
    ///
    /// It is the one setting that must not be touched at all. Every other number in the
    /// document is shortened on the way through, and the seed is not a quantity - it is a
    /// name for a run. Shorten it and two different seeds start naming the same run,
    /// which does not fail, does not warn, and quietly makes some fraction of the seed
    /// space unreachable. The values here sit at both ends of the range and either side
    /// of its halfway point, which is where a mistake of that kind shows up first.
    #[test]
    fn seed_survives_the_whole_u64_range() {
        for seed in [0, 1, 42, 1 << 31, 1 << 32, 1 << 63, u64::MAX - 1, u64::MAX] {
            let mut raw = spec_defaults();
            raw.world.seed = seed;

            let config = raw.validate().expect("any seed is a valid seed");
            assert_eq!(
                config.world.seed, seed,
                "seed {seed} did not survive being checked"
            );
        }
    }

    /// Every setting with a meaning that rules some numbers out actually rules them out,
    /// and says which setting it was talking about.
    ///
    /// One corruption per decimal setting, applied to an otherwise perfectly good
    /// configuration, so that when it is refused there is exactly one thing it could be
    /// complaining about. Then the complaint has to name that thing. Both halves matter:
    /// a check that rejects the value but blames the wrong setting sends you to edit a
    /// line that was never wrong.
    ///
    /// Which corruption a setting gets is decided by what SPEC says the setting *means*,
    /// and nothing is invented beyond that. A gradient is described as running from
    /// uniform to fully top-weighted, so it is a fraction and `1.5` is not one. A width
    /// is a size, so zero is not a width. An influx is an amount of energy, which may be
    /// none but cannot be less than none. Where SPEC says nothing - `spring_damping` has
    /// no stated upper end - nothing is imposed, because a bound invented here is a
    /// bound somebody has to argue with later and no one will remember why it was there.
    #[test]
    fn out_of_range_values_are_rejected_and_the_field_is_named() {
        // Each entry: the setting's path, and a value that its meaning excludes. A
        // fraction gets 1.5, a quantity that must exist gets 0.0, and a quantity that may
        // be nothing but not less gets -0.1. `diffusion` is the one exception and gets 0.5,
        // because 0.5 *is* a fraction: what excludes it is the stability of the arithmetic
        // rather than the meaning of the setting, and it is the value somebody would
        // actually write.
        let corruptions: [(&str, Corruption); 26] = [
            ("world.width", |raw| raw.world.width = 0.0),
            ("world.height", |raw| raw.world.height = 0.0),
            ("world.years_per_tick", |raw| raw.world.years_per_tick = 0.0),
            ("light.influx", |raw| raw.light.influx = -0.1),
            ("light.cap", |raw| raw.light.cap = 0.0),
            ("light.gradient", |raw| raw.light.gradient = 1.5),
            ("light.patchiness", |raw| raw.light.patchiness = 1.5),
            // A hundredth of a world unit per tick, which is a sixth of a cell-width a
            // second and reads as a perfectly reasonable drift. What excludes it is what the
            // light can replace rather than the meaning of the setting, exactly as with
            // `diffusion` below it - see `PATCH_DRIFT_CEILING`.
            ("light.patch_drift", |raw| raw.light.patch_drift = 0.01),
            ("light.diffusion", |raw| raw.light.diffusion = 0.5),
            ("physics.drag", |raw| raw.physics.drag = 1.5),
            // Past the ceiling rather than below the floor, because the ceiling is the end
            // that was found by the arithmetic going wrong rather than by argument, and 4
            // is a number somebody experimenting with a livelier world would type.
            ("physics.drag_anisotropy", |raw| {
                raw.physics.drag_anisotropy = 4.0;
            }),
            ("physics.collision_stiffness", |raw| {
                raw.physics.collision_stiffness = 0.0;
            }),
            ("physics.spring_damping", |raw| {
                raw.physics.spring_damping = -0.1;
            }),
            ("metabolism.upkeep_scale", |raw| {
                raw.metabolism.upkeep_scale = 0.0;
            }),
            ("metabolism.gene_cost", |raw| {
                raw.metabolism.gene_cost = -0.1;
            }),
            ("metabolism.movement_cost", |raw| {
                raw.metabolism.movement_cost = -0.1;
            }),
            ("metabolism.reproduction_threshold", |raw| {
                raw.metabolism.reproduction_threshold = 0.0;
            }),
            ("metabolism.offspring_share", |raw| {
                raw.metabolism.offspring_share = 1.5;
            }),
            ("mutation.point_rate", |raw| raw.mutation.point_rate = 1.5),
            ("mutation.point_sigma", |raw| raw.mutation.point_sigma = 1.5),
            ("mutation.duplication_rate", |raw| {
                raw.mutation.duplication_rate = 1.5;
            }),
            ("mutation.deletion_rate", |raw| {
                raw.mutation.deletion_rate = 1.5;
            }),
            ("mutation.insertion_rate", |raw| {
                raw.mutation.insertion_rate = 1.5;
            }),
            ("mutation.reorder_rate", |raw| {
                raw.mutation.reorder_rate = 1.5;
            }),
            ("mutation.genome_duplication_rate", |raw| {
                raw.mutation.genome_duplication_rate = 1.5;
            }),
            ("run.max_wall_clock_hours", |raw| {
                raw.run.max_wall_clock_hours = 0.0;
            }),
        ];

        let base = spec_defaults();

        for (field, corrupt) in corruptions {
            let mut raw = base.clone();
            corrupt(&mut raw);

            let complaint = raw
                .validate()
                .expect_err(&format!("a bad {field} must stop the run"))
                .to_string();

            assert!(
                complaint.starts_with(&format!("{field}: ")),
                "{field} was set to something its meaning excludes, and the complaint was \
                 about something else: {complaint}"
            );
        }
    }

    /// ⭐ Both ends of `physics.drag_anisotropy` are reachable, and a hair outside either is
    /// refused.
    ///
    /// The two ends are bounds of different kinds and the test is here because neither is
    /// SPEC's. **One** is isotropic water - the model this project shipped with, and the one
    /// in which a free body's total velocity is a conserved quantity of the integrator, so
    /// that no arrangement of muscles moves anything. It has to stay reachable, because it is
    /// the control experiment for every claim about swimming. **Three** is where the
    /// arithmetic stopped: see [`DRAG_ANISOTROPY_CEILING`].
    ///
    /// Both ends are accepted rather than only approached, for the reason
    /// `DIFFUSION_STABILITY_LIMIT` gives: a limit that cannot be reached is a limit one step
    /// lower with nobody able to tell which.
    #[test]
    fn the_drag_anisotropy_range_is_closed_at_both_ends() {
        let at = |value: f64| {
            let mut raw = spec_defaults();
            raw.physics.drag_anisotropy = value;
            raw.validate()
        };

        assert!(
            at(f64::from(DRAG_ANISOTROPY_FLOOR)).is_ok(),
            "isotropic water is the control experiment for every claim about swimming and \
             the gate refuses it"
        );
        assert!(
            at(f64::from(DRAG_ANISOTROPY_CEILING)).is_ok(),
            "a limit that cannot be reached is a limit one step lower"
        );

        for outside in [0.99, 3.01] {
            let complaint = at(outside)
                .expect_err("a drag anisotropy outside the range must stop the run")
                .to_string();

            assert!(
                complaint.starts_with("physics.drag_anisotropy: "),
                "{outside} was refused and the complaint was about something else: {complaint}"
            );
        }
    }

    /// ⭐⭐ **Phase 7, Group H.** Both ends of `behaviour.stroke` are reachable, and the upper
    /// one is not the tidy bound it looks like.
    ///
    /// SPEC section 9 builds a myocyte's spring as
    /// `rest_len = base_rest × (1 + amplitude × stroke × sin(...))`, with `amplitude` clamped
    /// into `0..=1` immediately above it. So the smallest a spring's rest length ever gets is
    /// `base_rest × (1 − stroke)`, and **one is exactly the point at which that reaches zero**.
    /// Past it a rest length is *negative*: the spring pulls its two cells together at every
    /// phase of the cycle rather than oscillating about anything, which is no longer a rest
    /// length and no longer a stroke. The arithmetic carries on perfectly - measured, a body at
    /// `stroke = 1.5` travels twenty-four times further than one at 1.0 - and what it is doing
    /// is hauling itself through its own cells, which is why the number this is bounded at
    /// happens to be the same one a fraction is bounded at. That is not a coincidence and it is
    /// not tidiness.
    ///
    /// `behaviour.resting_amplitude` is bounded for the plainer reason: SPEC section 9 clamps
    /// the amplitude into `0..=1`, so a resting amplitude outside that is one the clamp on the
    /// next line silently undoes.
    #[test]
    fn the_stroke_cannot_take_a_rest_length_below_nothing() {
        for (field, set) in [
            (
                "behaviour.stroke",
                (|raw: &mut RawConfig, value: f64| {
                    raw.behaviour.stroke = value;
                }) as fn(&mut RawConfig, f64),
            ),
            (
                "behaviour.resting_amplitude",
                |raw: &mut RawConfig, value: f64| {
                    raw.behaviour.resting_amplitude = value;
                },
            ),
        ] {
            for reachable in [0.0, 0.5, 1.0] {
                let mut raw = spec_defaults();
                set(&mut raw, reachable);
                assert!(
                    raw.validate().is_ok(),
                    "{field} = {reachable} is inside the range SPEC section 9 describes and \
                     the gate refused it"
                );
            }

            for outside in [-0.01, 1.01] {
                let mut raw = spec_defaults();
                set(&mut raw, outside);
                let complaint = raw
                    .validate()
                    .expect_err("a stroke outside 0..=1 must stop the run")
                    .to_string();

                assert!(
                    complaint.starts_with(&format!("{field}: ")),
                    "{field} = {outside} was refused and the complaint was about something \
                     else: {complaint}"
                );
            }
        }
    }

    /// Nothing that gets counted is allowed to be zero.
    ///
    /// Six settings are counts of things the simulation builds room for: the two
    /// dimensions of the resource grid, and the four sizes in `[limits]`. A zero in any
    /// of them is a world with no tiles in it, or room for no organisms, or a genome that
    /// may hold no genes. None of those is a small world. Each is a world in which the
    /// thing being simulated does not exist, and none of them announces itself as an
    /// error - the run simply starts and nothing ever happens.
    ///
    /// The second half of this test is the more important one, and it is the reason the
    /// checked settings hold a different kind of number from the ones in the document.
    /// Rejecting zero with an `if` works right up until somebody adds a seventh count and
    /// does not think to write the `if`. So the checked configuration holds a kind of
    /// number that has *no way to be zero*: the compiler will not build one, and the only
    /// way to get one is to go through the check. A count that nobody remembered to
    /// verify is not a bug that can be introduced here; it is a program that does not
    /// compile.
    ///
    /// The `.get()` calls below are what that costs, and they are the point: every place
    /// in the project that reads one of these has to unwrap it from a type that says, in
    /// its name, that it is not zero.
    #[test]
    fn zero_limits_and_grid_dimensions_are_rejected() {
        let counts: [(&str, Corruption); 6] = [
            ("world.grid_cols", |raw| raw.world.grid_cols = 0),
            ("world.grid_rows", |raw| raw.world.grid_rows = 0),
            ("limits.max_organisms", |raw| raw.limits.max_organisms = 0),
            ("limits.max_cells_per_organism", |raw| {
                raw.limits.max_cells_per_organism = 0;
            }),
            ("limits.max_genes", |raw| raw.limits.max_genes = 0),
            ("limits.max_dev_steps", |raw| raw.limits.max_dev_steps = 0),
        ];

        let base = spec_defaults();

        for (field, corrupt) in counts {
            let mut raw = base.clone();
            corrupt(&mut raw);

            let complaint = raw
                .validate()
                .expect_err(&format!("{field} = 0 must stop the run"))
                .to_string();

            assert_eq!(
                complaint,
                format!("{field}: must not be zero"),
                "{field} was set to zero and the complaint was not about {field}"
            );
        }

        let config = base.validate().expect("SPEC's defaults are valid");
        assert_eq!(config.world.grid_cols.get(), 256);
        assert_eq!(config.world.grid_rows.get(), 144);
        assert_eq!(config.limits.max_organisms.get(), 4000);
        assert_eq!(config.limits.max_cells_per_organism.get(), 64);
        assert_eq!(config.limits.max_genes.get(), 128);
        assert_eq!(config.limits.max_dev_steps.get(), 16);
    }

    /// The four sizes in `[limits]` are ceilings, not suggestions, and the two grid
    /// dimensions are not.
    ///
    /// Checking a configuration is the last thing that happens before Phase 2 allocates
    /// every arena in the simulation, and CLAUDE.md asks that the defences against a
    /// runaway run "hold even if the simulation code is wrong". A defence that only holds
    /// while the code is right is not one. So the ceilings are enforced here, at the one
    /// point that runs before any memory is committed and regardless of what the rest of
    /// the program does afterwards.
    ///
    /// Each ceiling is read off CLAUDE.md's own table of caps rather than chosen here.
    /// The genome cap is the one it marks critical: gene duplication is the mutation
    /// operator that makes complexity possible at all, and it is also exponential, so a
    /// lineage that duplicates faster than selection punishes it will grow a genome into
    /// the megabytes and take the process with it.
    ///
    /// The boundary itself is checked in both directions. A limit *at* its ceiling is
    /// accepted, because a cap you cannot actually use is a cap that is really one less
    /// and nobody will notice which.
    ///
    /// And the grid gets no ceiling at all, deliberately. It is the one place a limit
    /// suggests itself and does not belong: how large a resource grid this machine can
    /// hold is a question about allocating it, and Phase 2 is where that happens and
    /// where the answer can be worked out from the memory actually available. A number
    /// invented here would be a guess that later has to be argued with.
    #[test]
    fn limits_above_their_ceiling_are_rejected() {
        let over: [(&str, u32, Corruption); 4] = [
            ("limits.max_organisms", 100_000, |raw| {
                raw.limits.max_organisms = 100_001;
            }),
            ("limits.max_cells_per_organism", 64, |raw| {
                raw.limits.max_cells_per_organism = 65;
            }),
            ("limits.max_genes", 128, |raw| raw.limits.max_genes = 4096),
            ("limits.max_dev_steps", 255, |raw| {
                raw.limits.max_dev_steps = 256;
            }),
        ];

        let base = spec_defaults();

        for (field, ceiling, corrupt) in over {
            let mut raw = base.clone();
            corrupt(&mut raw);

            let complaint = raw
                .validate()
                .expect_err(&format!("{field} above its ceiling must stop the run"))
                .to_string();

            assert!(
                complaint.starts_with(&format!("{field}: ")),
                "{field} was set above its ceiling and the complaint was about something \
                 else: {complaint}"
            );
            assert!(
                complaint.ends_with(&format!("exceeds the ceiling of {ceiling}")),
                "the complaint about {field} does not say what the ceiling is: {complaint}"
            );
        }

        // At the ceiling, not over it.
        let mut at_the_ceiling = base.clone();
        at_the_ceiling.limits.max_organisms = 100_000;
        at_the_ceiling.limits.max_cells_per_organism = 64;
        at_the_ceiling.limits.max_genes = 128;
        at_the_ceiling.limits.max_dev_steps = 255;
        at_the_ceiling
            .validate()
            .expect("a limit set exactly at its ceiling is allowed");

        // The grid has no ceiling here. Whether this machine can hold a grid of four
        // million columns is Phase 2's question, asked where the memory is actually
        // committed.
        let mut enormous_grid = base;
        enormous_grid.world.grid_cols = 4_000_000;
        enormous_grid.world.grid_rows = 4_000_000;
        enormous_grid
            .validate()
            .expect("the resource grid is not capped at this stage");
    }

    /// Energy may not be told to spread faster than a quarter of the difference per tick,
    /// and the reason is arithmetic rather than taste.
    ///
    /// This is the only setting in the whole configuration whose upper end is set by
    /// *stability* rather than by what the setting means. SPEC section 3 describes
    /// `diffusion` as a lateral spread per tick, which reads like any other fraction and
    /// would suggest a bound of one; SPEC section 4 explains why that is wrong. A tile
    /// giving away a share of its difference to each of four neighbours overshoots once
    /// that share passes a quarter — it is sent past its neighbours' value, then dragged
    /// back further the next tick — and the overshoot compounds until the numbers stop
    /// being finite.
    ///
    /// **Nothing downstream would catch it.** Overshoot moves energy rather than inventing
    /// it, so every pair of tiles still trades one number both ways and the energy ledger
    /// of SPEC section 5 reports a perfectly healthy world right up until the field is
    /// nonsense. There is no second line of defence here, which is what makes this bound
    /// the whole of it.
    ///
    /// The boundary is checked in both directions. A quarter exactly is allowed, because a
    /// limit you cannot actually use is a limit one step lower that nobody can see. And a
    /// half is refused, which is the case that matters: it is a perfectly ordinary fraction
    /// that this program used to accept, so somebody's saved configuration may well contain
    /// it.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a rate at the limit must arrive as exactly the rate that was written; \
                  near enough would let a value be quietly adjusted on its way through"
    )]
    fn a_diffusion_rate_past_the_stability_limit_is_refused() {
        let base = spec_defaults();

        let mut at_the_limit = base.clone();
        at_the_limit.light.diffusion = 0.25;
        assert_eq!(
            at_the_limit
                .validate()
                .expect("a quarter is the limit itself and has to be usable")
                .light
                .diffusion,
            0.25,
            "the largest stable rate did not survive being checked"
        );

        for rate in [0.26, 0.5, 1.0] {
            let mut too_fast = base.clone();
            too_fast.light.diffusion = rate;

            let complaint = too_fast
                .validate()
                .expect_err("a rate past the stability limit must stop the run")
                .to_string();

            assert!(
                complaint.starts_with("light.diffusion: "),
                "a diffusion rate of {rate} was refused and the complaint was about \
                 something else: {complaint}"
            );
            assert!(
                complaint.contains("0..=0.25"),
                "the complaint about a diffusion rate of {rate} does not say what the \
                 limit is: {complaint}"
            );
            // The whole sentence is pinned by `errors_name_the_field_in_plain_english`.
            // What is checked here is that it does not stop at the range: a bare "outside
            // 0..=0.25" tells somebody who wrote 0.5 that they are wrong and nothing about
            // why, and this is the one bound in the file where the why is not guessable
            // from what the setting means.
            assert!(
                complaint.contains("rather than a preference"),
                "the complaint about a diffusion rate of {rate} does not explain that this \
                 is a limit of the arithmetic: {complaint}"
            );
        }
    }

    /// ⭐ A patch field told to slide faster than the light can refill what it slides off
    /// is refused, and nought is still allowed.
    ///
    /// [`PATCH_DRIFT_CEILING`] carries the derivation. What this pins is the shape of the
    /// gate, which is deliberately **not** `light.diffusion`'s even though the two read
    /// alike from outside:
    ///
    /// - **The limit itself is usable.** A limit that cannot be reached is a limit one step
    ///   lower with nobody able to tell which.
    /// - **Nought is usable**, and that is the load-bearing end. A drift of nothing is the
    ///   fixed blotches this project had before, and it is the control experiment for every
    ///   claim about drifting ones. A gate that quietly required a positive drift would take
    ///   the control away.
    /// - **The sentence explains itself.** The failure past the ceiling is a world that
    ///   empties while SPEC section 5's ledger balances to the last digit throughout, because
    ///   the energy is *accounted for* on its way out - so a person who widens this bound
    ///   gets no warning from anything else in the program. That is the same trap
    ///   `diffusion` sets and it is sprung by a completely different mechanism, which is why
    ///   the two refusals are separate sentences rather than one with a parameter.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a drift at the limit must arrive as exactly the drift that was written; \
                  near enough would let a value be quietly adjusted on its way through"
    )]
    fn a_patch_drift_faster_than_the_light_can_replace_is_refused() {
        let base = spec_defaults();

        for allowed in [0.0, 0.0001, f64::from(PATCH_DRIFT_CEILING)] {
            let mut fine = base.clone();
            fine.light.patch_drift = allowed;
            assert_eq!(
                fine.validate()
                    .expect("a drift at or below the ceiling has to be usable")
                    .light
                    .patch_drift,
                narrow("light.patch_drift", allowed).expect("these all narrow"),
                "a drift of {allowed} did not survive being checked"
            );
        }

        for drift in [0.0051, 0.01, 1.0] {
            let mut too_fast = base.clone();
            too_fast.light.patch_drift = drift;

            let complaint = too_fast
                .validate()
                .expect_err("a drift past the ceiling must stop the run")
                .to_string();

            assert!(
                complaint.starts_with("light.patch_drift: "),
                "a drift of {drift} was refused and the complaint was about something else: \
                 {complaint}"
            );
            assert!(
                complaint.contains("0..=0.005"),
                "the complaint about a drift of {drift} does not say what the limit is: \
                 {complaint}"
            );
            assert!(
                complaint.contains("the energy ledger balances perfectly throughout"),
                "the complaint about a drift of {drift} does not say that nothing else in \
                 the program will catch this, which is the whole reason the bound is a gate: \
                 {complaint}"
            );
        }

        // And a negative drift is refused by the same gate rather than quietly running the
        // field backwards, which would be a perfectly good world and not the one asked for.
        let mut backwards = base;
        backwards.light.patch_drift = -0.0006;
        assert!(
            backwards
                .validate()
                .expect_err("a negative drift must be refused")
                .to_string()
                .starts_with("light.patch_drift: "),
            "a negative drift was refused by something other than its own gate"
        );
    }

    /// ⭐⭐ **Phase 7's Group L.** The `[light]` table carries a season, and it ships **inert**.
    ///
    /// Two keys, both required. Neither has a `serde(default)`, for this file's own reason: a
    /// season that silently defaulted to absent is a run whose replay log does not describe it,
    /// and SPEC section 13 wants a recording to carry the settings that produced it.
    ///
    /// ⚠️ **The shipped amplitude is nought, and that is the discipline rather than caution.**
    /// Group H shipped a sevenfold change to every muscle in the world and could not afterwards
    /// separate *did I break anything* from *the world is now different*. The mechanism lands
    /// here switched off — `config/default.toml` is bit-for-bit the world every figure in
    /// `docs/PHASE7.md` was measured on — and `config/seasonal.toml` is the profile that turns
    /// it on. That is one line of a settings file between the two experiments.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the shipped amplitude is nought exactly, and nought is the one value in this \
                  file that must not be approximate: it is the off switch"
    )]
    fn the_light_table_carries_a_season() {
        let shipped = spec_defaults();

        assert_eq!(
            shipped.light.season_amplitude, 0.0,
            "the season does not ship inert, so `config/default.toml` is no longer the world \
             every figure in docs/PHASE7.md was measured on"
        );
        assert_eq!(
            shipped.light.season_period, 21_000,
            "the shipped period is 21,000 ticks — twelve generations, six to the half cycle, \
             and coprime with the project's 25,000-tick checkpoints"
        );

        let checked = shipped
            .validate()
            .expect("a world with no season is a world");
        assert_eq!(checked.light.season_amplitude, 0.0);
        assert_eq!(checked.light.season_period, 21_000);

        // That both keys are *required* is a claim about the document, so it is checked where a
        // document can be read: `main.rs`'s
        // `the_shipped_documents_carry_a_season_and_it_ships_inert`. This crate has no way to
        // read TOML, by design.
    }

    /// ⭐⭐ A season the water cannot follow is refused, and the refusal says why.
    ///
    /// The floor is `cap / influx` — the time a tile takes to fill from empty, 8,000 ticks at
    /// SPEC section 3's shipped light. Below it the *light* changes and the *water* does not:
    /// measured, the standing field swings 2.04% at a period of 2,000 against 6.74% at 20,000.
    /// A season nothing in the world can feel is a season that is not there, and it would look
    /// identical in the config file to one that is.
    ///
    /// ⚠️ **There is no upper bound.** A million-tick climate is a legitimate experiment, and an
    /// invented ceiling is one somebody argues with on the evening an experiment is blocked.
    ///
    /// ⚠️ **And zero is refused rather than meaning "no season".** The amplitude is the off
    /// switch. A second one, with a division by it standing behind, is two ways to say the same
    /// thing and one of them silently wrong.
    #[test]
    fn a_season_the_water_cannot_follow_is_refused() {
        let base = spec_defaults();

        for allowed in [SEASON_PERIOD_FLOOR, 21_000, 1_000_000] {
            let mut fine = base.clone();
            fine.light.season_period = allowed;
            assert_eq!(
                fine.validate()
                    .expect("a period at or above the floor has to be usable")
                    .light
                    .season_period,
                allowed,
                "a period of {allowed} did not survive being checked"
            );
        }

        for period in [0, 1, 2_000, SEASON_PERIOD_FLOOR - 1] {
            let mut too_quick = base.clone();
            too_quick.light.season_period = period;

            let complaint = too_quick
                .validate()
                .expect_err("a period below the floor must stop the run")
                .to_string();

            assert!(
                complaint.starts_with("light.season_period: "),
                "a period of {period} was refused and the complaint was about something else: \
                 {complaint}"
            );
            assert!(
                complaint.contains("8000"),
                "the complaint about a period of {period} does not say what the floor is: \
                 {complaint}"
            );
            assert!(
                complaint.contains("the water"),
                "the complaint about a period of {period} does not say *why* — that below a \
                 tile's own filling time the light changes and the water does not: {complaint}"
            );
        }
    }

    /// ⭐⭐ A season deeper than anything measured is refused, and the refusal says only that.
    ///
    /// 0.25 and 0.5 are the only amplitudes ever run. The bound is where the **evidence** stops
    /// and it is not an argument about drift: with the flat world's own second-half trough
    /// measured at 766 organisms and the largest real selection coefficient in this world at
    /// 0.85 %/generation, `N·s` is 3.6 at an amplitude of a half and still 1.8 at three
    /// quarters. "The population falls far enough that drift outruns selection" is not true
    /// anywhere this gate allows, and a refusal that said so would be a sentence somebody could
    /// disprove and then delete the bound over.
    ///
    /// A bound is not optional, though: above 1.0 the multiplier `1 + amplitude × triangle` goes
    /// negative, which is light running backwards.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "an amplitude at the ceiling must arrive as exactly the amplitude that was \
                  written; near enough would let the depth of a season be quietly adjusted"
    )]
    fn a_season_deeper_than_anything_measured_is_refused() {
        let base = spec_defaults();

        for allowed in [0.0, 0.25, f64::from(SEASON_AMPLITUDE_CEILING)] {
            let mut fine = base.clone();
            fine.light.season_amplitude = allowed;
            assert_eq!(
                fine.validate()
                    .expect("an amplitude at or below the ceiling has to be usable")
                    .light
                    .season_amplitude,
                narrow("light.season_amplitude", allowed).expect("these all narrow"),
                "an amplitude of {allowed} did not survive being checked"
            );
        }

        for amplitude in [0.5001, 0.75, 1.5, -0.25] {
            let mut too_deep = base.clone();
            too_deep.light.season_amplitude = amplitude;

            let complaint = too_deep
                .validate()
                .expect_err("an amplitude outside the measured range must stop the run")
                .to_string();

            assert!(
                complaint.starts_with("light.season_amplitude: "),
                "an amplitude of {amplitude} was refused and the complaint was about something \
                 else: {complaint}"
            );
            assert!(
                complaint.contains("0..=0.5"),
                "the complaint about an amplitude of {amplitude} does not say what the range \
                 is: {complaint}"
            );
            assert!(
                complaint.contains("measured"),
                "the complaint about an amplitude of {amplitude} does not say that the bound is \
                 where the evidence stops, which is the only thing it may say: {complaint}"
            );
            assert!(
                !complaint.contains("drift"),
                "the complaint about an amplitude of {amplitude} claims something about drift, \
                 and `N·s` is 3.6 at a half and 1.8 at three quarters — so the claim is false \
                 everywhere this gate allows: {complaint}"
            );
        }
    }

    /// `max_ticks = 0` means "no limit", and that convention stops here.
    ///
    /// SPEC section 3 writes the setting with a comment saying `0 = unbounded`, which is
    /// a perfectly ordinary way to write it in a text file and a trap the moment it gets
    /// any further. Left as a plain number, every piece of code that ever asks whether a
    /// run should stop has to know that zero is special, and the obvious way to write that
    /// check - "stop when the tick count reaches the limit" - stops immediately, before
    /// the first tick, on precisely the configuration that asked to run for ever.
    ///
    /// So the convention is translated at the gate into something with no special value
    /// in it: either there is a limit or there is not. The runner in Phase 4 cannot forget
    /// the rule, because by the time it sees the setting the rule has already been
    /// applied and there is nothing left to remember.
    #[test]
    fn max_ticks_zero_becomes_unbounded() {
        let base = spec_defaults();

        let mut forever = base.clone();
        forever.run.max_ticks = 0;
        assert_eq!(
            forever
                .validate()
                .expect("0 is a valid tick limit")
                .run
                .max_ticks,
            None,
            "a tick limit of zero must come out the other side as no limit at all"
        );

        let mut bounded = base;
        bounded.run.max_ticks = 5_000;
        assert_eq!(
            bounded
                .validate()
                .expect("5000 is a valid tick limit")
                .run
                .max_ticks,
            Some(5_000),
            "a real tick limit must survive being checked"
        );
    }

    /// The eight sentences a person is ever shown, written out in full.
    ///
    /// This is the only test in the module whose subject is the English rather than the
    /// arithmetic, and it is not decoration. The error message is the entire interface
    /// between this program and somebody trying to fix a configuration: they will never
    /// read the checking code, and if the sentence does not say which setting is wrong
    /// and what is wrong with it, the only remaining option is to edit the file at random.
    /// Pinning the literal sentence is what stops it eroding into "invalid config" three
    /// refactors from now, because a message nobody asserts on is a message nobody
    /// notices changing.
    ///
    /// Three of them are worth explaining, because all three look like unnecessary special
    /// cases and none of them is.
    ///
    /// **Negative gets its own sentence** rather than reusing the range one. A setting
    /// like `influx` has a floor of zero and no ceiling at all, so phrased as a range it
    /// reads `light.influx: -0.1 is outside 0..=340282350000000000000000000000000000000`,
    /// which is thirty-nine digits of the largest number the arithmetic can hold wrapped
    /// around the one word that actually matters.
    ///
    /// **The narrowing message uses scientific notation**, which Rust never does on its
    /// own. Left to itself it would print `1e-40` as a nought, a decimal point and thirty-
    /// nine more noughts: forty characters of zero in the one sentence whose entire job is
    /// to explain that the number's *size* is the problem.
    ///
    /// **The diffusion message is three times the length of any other**, and it is the one
    /// place that is worth it. Every other bound here follows from what its setting means,
    /// so being told that a gradient is outside `0..=1` is being told everything there is
    /// to know. A diffusion rate of a half is a perfectly ordinary fraction, is what this
    /// program used to accept, and is refused for a reason nobody could infer from the
    /// word "diffusion" — so the sentence has to carry the reason with it or the refusal
    /// reads as an arbitrary house rule.
    ///
    /// **And `patch_drift`'s is as long, for the same reason and a different mechanism.** A
    /// hundredth of a world unit per tick is a sixth of a cell's width per second; nothing
    /// about the number looks wrong. What is wrong is invisible from where the person is
    /// standing — the field would be shedding more energy to `dissipated` than the light
    /// puts in — and, like `diffusion`, it is a failure the energy ledger cannot see.
    #[test]
    fn errors_name_the_field_in_plain_english() {
        let sentences: [(&str, Corruption); 8] = [
            ("light.gradient: 1.5 is outside 0..=1", |raw| {
                raw.light.gradient = 1.5;
            }),
            (
                "light.patch_drift: 0.01 is outside 0..=0.005. The upper end is what the \
                 light can replace rather than a preference: a tile's ceiling moving out \
                 from under it sheds the difference to `dissipated`, so a field dragged \
                 sideways faster than this destroys more energy per tick than `light.influx` \
                 delivers, and the world empties while the energy ledger balances perfectly \
                 throughout",
                |raw| raw.light.patch_drift = 0.01,
            ),
            (
                "light.diffusion: 0.5 is outside 0..=0.25. The upper end is a limit of the \
                 arithmetic rather than a preference: past it energy spreads faster than \
                 it settles, the field oscillates and grows without limit, and because \
                 that still conserves energy exactly nothing else in the program will \
                 catch it",
                |raw| raw.light.diffusion = 0.5,
            ),
            ("light.influx: -0.1 must not be negative", |raw| {
                raw.light.influx = -0.1;
            }),
            ("light.cap: 0 must be greater than zero", |raw| {
                raw.light.cap = 0.0;
            }),
            ("limits.max_organisms: must not be zero", |raw| {
                raw.limits.max_organisms = 0;
            }),
            ("limits.max_genes: 4096 exceeds the ceiling of 128", |raw| {
                raw.limits.max_genes = 4096;
            }),
            (
                "light.influx: 1e-40 cannot be represented as a 32-bit float \
                 without losing its magnitude",
                |raw| raw.light.influx = 1e-40,
            ),
        ];

        let base = spec_defaults();

        for (sentence, corrupt) in sentences {
            let mut raw = base.clone();
            corrupt(&mut raw);

            let complaint = raw
                .validate()
                .expect_err("this configuration is wrong and must be refused")
                .to_string();

            assert_eq!(
                complaint, sentence,
                "the sentence a person is shown has changed"
            );
        }
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The tests above use numbers chosen by hand. This one uses numbers chosen by the
    // machine, thousands of them, including the awkward ones nobody would think to try.
    // When it fails it shrinks the failure to the smallest number that still breaks and
    // writes it to a file beside this one, so that number is re-run for ever afterwards.
    // ---------------------------------------------------------------------------------

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// The promise the narrowing makes, stated once for every number there is: it
        /// either refuses a value or keeps it.
        ///
        /// "Keeps it" cannot mean *exactly*, because almost no decimal survives being
        /// written at two different sizes exactly - that is the trap
        /// `every_spec_default_literal_narrows` exists to describe. So the promise is
        /// that a number which is accepted comes out within one step of the smaller
        /// size's own resolution, which is the best any honest conversion could do.
        ///
        /// That wording is deliberately a claim about *relative* error, and it is what
        /// makes this test bite. There is a band of very small numbers, around
        /// `1e-38` and below, that the simulation's arithmetic can still technically
        /// hold but only by giving up most of its digits. A value in that band is
        /// finite, is not zero, and passes every check a person would think to write -
        /// and it arrives carrying error of a few parts in a hundred thousand rather
        /// than a few parts in ten million. No hand-written test finds that band,
        /// because you have to already know it is there to write the number down.
        #[test]
        fn narrowing_is_rejected_or_faithful(value: f64) {
            if let Ok(narrowed) = narrow("light.influx", value) {
                let drift = (f64::from(narrowed) - value).abs();
                let one_step = value.abs() * f64::from(f32::EPSILON);

                prop_assert!(
                    drift <= one_step,
                    "{value:e} was accepted and came back as {narrowed:e}, which is \
                     off by {drift:e} - more than the {one_step:e} that one step at \
                     this size would explain",
                );
            }
        }
    }
}
