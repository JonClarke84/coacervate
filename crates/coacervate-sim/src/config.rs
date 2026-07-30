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
    pub diffusion: f64,
}

/// The `[physics]` table as written: how the soup pushes back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawPhysics {
    pub drag: f64,
    pub collision_stiffness: f64,
    pub spring_damping: f64,
}

/// The `[metabolism]` table as written: what living costs, and what reproducing costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMetabolism {
    pub upkeep_scale: f64,
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
            influx: 0.012,
            cap: 8.0,
            gradient: 0.75,
            patchiness: 0.15,
            diffusion: 0.04,
        },
        physics: RawPhysics {
            drag: 0.92,
            collision_stiffness: 40.0,
            spring_damping: 0.35,
        },
        metabolism: RawMetabolism {
            upkeep_scale: 1.0,
            movement_cost: 0.15,
            reproduction_threshold: 2.2,
            offspring_share: 0.45,
        },
        mutation: RawMutation {
            point_rate: 0.06,
            point_sigma: 0.12,
            duplication_rate: 0.02,
            deletion_rate: 0.02,
            insertion_rate: 0.01,
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
/// of thirty-one settings it came from, sends you hunting through the file by hand.
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
    pub diffusion: f32,
}

/// The checked `[physics]` table: how the soup pushes back.
#[derive(Debug, Clone, PartialEq)]
pub struct PhysicsConfig {
    pub drag: f32,
    pub collision_stiffness: f32,
    pub spring_damping: f32,
}

/// The checked `[metabolism]` table: what living costs, and what reproducing costs.
#[derive(Debug, Clone, PartialEq)]
pub struct MetabolismConfig {
    pub upkeep_scale: f32,
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
                // "lateral spread per tick".
                diffusion: fraction("light.diffusion", self.light.diffusion)?,
            },
            physics: PhysicsConfig {
                // "velocity retained per tick" - a proportion of what was there before.
                drag: fraction("physics.drag", self.physics.drag)?,
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
            metabolism: MetabolismConfig {
                // The document calls it a "temperature": scale it to zero and nothing
                // costs anything to be alive.
                upkeep_scale: positive("metabolism.upkeep_scale", self.metabolism.upkeep_scale)?,
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
            ("light.diffusion", raw.light.diffusion),
            ("physics.drag", raw.physics.drag),
            (
                "physics.collision_stiffness",
                raw.physics.collision_stiffness,
            ),
            ("physics.spring_damping", raw.physics.spring_damping),
            ("metabolism.upkeep_scale", raw.metabolism.upkeep_scale),
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
    /// written", which sounds like precisely the right standard and rejects fourteen of
    /// the twenty-two numbers in SPEC's own defaults - `influx`, `drag`, `patchiness` and
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
            22,
            "SPEC section 3 has twenty-two decimal settings; this list has {}, so one has \
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
            "{} of the 22 numbers in SPEC's own default configuration cannot be loaded:\n  {}",
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
    /// Every one of the thirty-one settings is checked, not a sample, and the reason is
    /// the shape of the code being tested rather than thoroughness for its own sake.
    /// Turning a document into a checked configuration is thirty-one hand-written
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

        assert_eq!(config.light.influx, 0.012);
        assert_eq!(config.light.cap, 8.0);
        assert_eq!(config.light.gradient, 0.75);
        assert_eq!(config.light.patchiness, 0.15);
        assert_eq!(config.light.diffusion, 0.04);

        assert_eq!(config.physics.drag, 0.92);
        assert_eq!(config.physics.collision_stiffness, 40.0);
        assert_eq!(config.physics.spring_damping, 0.35);

        assert_eq!(config.metabolism.upkeep_scale, 1.0);
        assert_eq!(config.metabolism.movement_cost, 0.15);
        assert_eq!(config.metabolism.reproduction_threshold, 2.2);
        assert_eq!(config.metabolism.offspring_share, 0.45);

        assert_eq!(config.mutation.point_rate, 0.06);
        assert_eq!(config.mutation.point_sigma, 0.12);
        assert_eq!(config.mutation.duplication_rate, 0.02);
        assert_eq!(config.mutation.deletion_rate, 0.02);
        assert_eq!(config.mutation.insertion_rate, 0.01);
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
        // be nothing but not less gets -0.1.
        let corruptions: [(&str, Corruption); 22] = [
            ("world.width", |raw| raw.world.width = 0.0),
            ("world.height", |raw| raw.world.height = 0.0),
            ("world.years_per_tick", |raw| raw.world.years_per_tick = 0.0),
            ("light.influx", |raw| raw.light.influx = -0.1),
            ("light.cap", |raw| raw.light.cap = 0.0),
            ("light.gradient", |raw| raw.light.gradient = 1.5),
            ("light.patchiness", |raw| raw.light.patchiness = 1.5),
            ("light.diffusion", |raw| raw.light.diffusion = 1.5),
            ("physics.drag", |raw| raw.physics.drag = 1.5),
            ("physics.collision_stiffness", |raw| {
                raw.physics.collision_stiffness = 0.0;
            }),
            ("physics.spring_damping", |raw| {
                raw.physics.spring_damping = -0.1;
            }),
            ("metabolism.upkeep_scale", |raw| {
                raw.metabolism.upkeep_scale = 0.0;
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

    /// The six sentences a person is ever shown, written out in full.
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
    /// Two of them are worth explaining, because both look like unnecessary special
    /// cases and neither is.
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
    #[test]
    fn errors_name_the_field_in_plain_english() {
        let sentences: [(&str, Corruption); 6] = [
            ("light.gradient: 1.5 is outside 0..=1", |raw| {
                raw.light.gradient = 1.5;
            }),
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
