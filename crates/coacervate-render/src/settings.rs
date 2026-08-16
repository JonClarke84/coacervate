//! What a person is allowed to change while a run is going, and the gate in front of it.
//!
//! ⭐ **This module is `B3`, and `B3` is the reason it is a module rather than twenty lines
//! inside `panel.rs`.** SPEC section 3 divides its own table in two:
//!
//! > `[world]`, `[limits]` and `seed` lock at run start; the rest can be changed live, which is
//! > how environmental events work.
//!
//! and `config.rs` is emphatic about what stands between a number somebody typed and a world
//! that does not work:
//!
//! > This is the only gate. Everything downstream of it may assume its numbers are sane,
//! > because there is no other way to obtain a [`Config`].
//!
//! A slider is a new way for a number to arrive, and the whole danger of one is that it looks
//! like a *widget* rather than like a document being edited - so it is easy to write one that
//! assigns straight into the running configuration and never asks. This module is what makes
//! that impossible: **[`Dials`] holds a [`RawConfig`] - the unchecked transcript - and the only
//! way to change it is [`Dials::set`], which writes into a copy, runs
//! [`RawConfig::validate`] over the whole of it, and keeps the result only if the gate
//! accepted.** Nothing here can produce a [`Config`] by any other route, because nothing
//! anywhere can.
//!
//! # ⚠️ The bounds and the gate are two different things, and this module has both
//!
//! Every [`Dial`] carries a `least` and a `most`, and every one of those pairs is inside what
//! validation accepts - `every_dial_reaches_both_of_its_ends` drives all twenty-one of them to
//! both ends through the real gate and insists it says yes. That is a **convenience**: a slider
//! whose far end was a value the gate refuses would be a slider that fights the person using
//! it, dragging to the end and being told no.
//!
//! The gate is the **guarantee**, and it is a separate thing. Bounds written out here are bounds
//! that can be edited here, in a file `config.rs` knows nothing about; the two would drift the
//! first time somebody widened one without reading the other. So the value is checked as well as
//! bounded, and `a_value_the_gate_refuses_is_not_applied` states the consequence: what a refusal
//! does is leave the run exactly as it was.
//!
//! ⭐ **And one bound is not written out here at all.** `light.diffusion`'s upper end **is**
//! `coacervate_sim::config::DIFFUSION_STABILITY_LIMIT`, imported, because that is the one number
//! in SPEC section 3 where being wrong is silent: above a quarter the five-point stencil
//! overshoots, the field grows without limit, and *the energy ledger goes on reporting a
//! perfectly healthy world the whole way down*, because overshoot moves energy rather than
//! inventing it. It is the one bound in the program that nothing downstream would catch, which
//! makes it the one bound that must not exist in two places.

use coacervate_sim::config::{
    Config, ConfigError, DIFFUSION_STABILITY_LIMIT, DRAG_ANISOTROPY_CEILING, DRAG_ANISOTROPY_FLOOR,
    LINEAR_SCALING, PATCH_DRIFT_CEILING, RawConfig, SCALING_EXPONENT_FLOOR,
    SEASON_AMPLITUDE_CEILING, SEASON_PERIOD_FLOOR,
};

/// One setting a person may turn while the run is going.
///
/// A description rather than a value: where the setting lives in the document, what it is
/// called on the panel, how far it goes and how it is written down. The value itself is in
/// [`Dials`], because there is only one configuration and thirty-odd views of it.
///
/// The two function pointers are what stand in for a field accessor. `RawConfig` is seven
/// nested structs of plain numbers with no reflection anywhere, and the alternative to a pair of
/// closures per setting is a `match` on a string in two places - which is the same thing with an
/// extra way to get it wrong.
pub struct Dial {
    /// Which of SPEC section 3's tables it is in, which is also its heading on the panel.
    pub table: &'static str,

    /// What it is called inside that table. `panel.rs` shows this and not the full path,
    /// because the path's first half is the fold it is already sitting inside.
    pub label: &'static str,

    /// The smallest this dial goes.
    pub least: f64,

    /// The largest this dial goes.
    pub most: f64,

    /// How the number is written on the panel: how many places after the point, or none at all
    /// if it is a count.
    pub places: Option<usize>,

    /// Read it out of a document.
    read: fn(&RawConfig) -> f64,

    /// Write it into one. Nothing here checks anything - see [`Dials::set`], which is what does.
    write: fn(&mut RawConfig, f64),
}

impl Dial {
    /// The setting's full path, as `config.rs` names it in a refusal.
    #[must_use]
    pub fn field(&self) -> String {
        format!("{}.{}", self.table, self.label)
    }

    /// What this dial is set to in a document.
    #[must_use]
    pub fn of(&self, raw: &RawConfig) -> f64 {
        (self.read)(raw)
    }
}

impl std::fmt::Debug for Dial {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Dial")
            .field("field", &self.field())
            .finish()
    }
}

/// ⭐ **`B1`.** Every setting SPEC section 3 does not lock, in the order that section writes
/// them.
///
/// Four tables and one line of a fifth. `[light]`, `[physics]`, `[metabolism]` and `[mutation]`
/// are what SPEC means by *"the rest"*, and `run.max_ticks_per_second` is here because `B4` asks
/// for it by name: it is what the `slow` profile is made of, and it is the only thing in `[run]`
/// that is about how a run is *watched* rather than about when it ends.
///
/// # Where each pair of ends came from
///
/// Not invented. Where SPEC gives a range, it is SPEC's; where SPEC gives a *measurement*, the
/// dial spans it; and where the gate's own bound is the interesting one, the dial takes the
/// gate's constant rather than a copy of its value.
///
/// | Setting | Ends | Why |
/// | --- | --- | --- |
/// | `light.influx` | `0 – 0.02` | SPEC section 3's measured table runs 0.0001 to 0.012, and 0.012 is the shipped `bloom` profile. A dark world is a legitimate experiment, so the low end is nought |
/// | `light.patch_drift` | `0 – 0.005` | ⭐ [`PATCH_DRIFT_CEILING`] itself, for the same reason. Nought is the fixed field this project had before Phase 7, which is the control for every claim about a drifting one |
/// | `light.diffusion` | `0 – 0.25` | ⭐ [`DIFFUSION_STABILITY_LIMIT`] itself. See this module's header |
/// | `physics.drag_anisotropy` | `1 - 3` | ⭐ [`DRAG_ANISOTROPY_FLOOR`] and [`DRAG_ANISOTROPY_CEILING`] themselves. One is isotropic water, which is the world in which nothing can swim; three is where the arithmetic stopped computing |
/// | `metabolism.upkeep_scale` | `0.01 – 8` | SPEC section 3: *"`3` and `4` both go extinct with the founder's death"*. A dial that stopped at 2 could not reach the one environmental event that measurement describes. Its low end is not nought because the gate calls it positive |
/// | `metabolism.scaling_exponent` | `0.5 – 1` | ⭐ [`SCALING_EXPONENT_FLOOR`] and [`LINEAR_SCALING`] themselves. One is exactly linear, which is the world every figure in this project was measured on; a half is `d / (d + 1)` in one dimension, the flattest exponent a distribution network can have |
/// | the `[mutation]` rates | `0 – 0.2` | All seven are fractions and the gate would take one, but a rate of one is every gene mutating at every birth. The dial covers ten times the shipped value, which is the range an experiment lives in |
/// | `run.max_ticks_per_second` | `0 – 600` | Nought is SPEC's *"0 = uncapped"*. Six hundred is about what this machine manages headless, so the far end is "as fast as it goes" and everything below it is a real slowing |
pub const DIALS: &[Dial] = &[
    Dial {
        table: "light",
        label: "influx",
        least: 0.0,
        most: 0.02,
        places: Some(4),
        read: |raw| raw.light.influx,
        write: |raw, value| raw.light.influx = value,
    },
    Dial {
        table: "light",
        label: "cap",
        least: 0.1,
        most: 32.0,
        places: Some(2),
        read: |raw| raw.light.cap,
        write: |raw, value| raw.light.cap = value,
    },
    Dial {
        table: "light",
        label: "gradient",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.light.gradient,
        write: |raw, value| raw.light.gradient = value,
    },
    Dial {
        table: "light",
        label: "patchiness",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.light.patchiness,
        write: |raw, value| raw.light.patchiness = value,
    },
    Dial {
        table: "light",
        // ⭐ Not `0.0 - 0.005`. The far end is `PATCH_DRIFT_CEILING`, imported for exactly the
        // reason `light.diffusion` imports its own: it is where the field starts shedding more
        // energy than the light delivers rather than a preference, and a copy of the number
        // written out here would be silently wrong the day somebody moves it.
        label: "patch_drift",
        least: 0.0,
        most: PATCH_DRIFT_CEILING as f64,
        places: Some(5),
        read: |raw| raw.light.patch_drift,
        write: |raw, value| raw.light.patch_drift = value,
    },
    Dial {
        table: "light",
        label: "diffusion",
        least: 0.0,
        // ⭐ Not `0.25`. The constant the gate refuses above, imported, so that the day somebody
        // finds a stable stencil and raises it, the slider follows without being found.
        //
        // ⚠️ `as` and not `f64::from`, which is what CLAUDE.md's lint table asks for everywhere
        // else. `From<f32> for f64` is not a `const fn`, and this list has to be a `const` for
        // the constant above to be the one the slider stops at rather than a copy of its value.
        // Widening a 32-bit float to a 64-bit one is exact for every value there is, which is
        // why `cast_lossless` does not fire on it.
        most: DIFFUSION_STABILITY_LIMIT as f64,
        places: Some(3),
        read: |raw| raw.light.diffusion,
        write: |raw, value| raw.light.diffusion = value,
    },
    Dial {
        table: "light",
        // ⭐ Not `8000.0`. The near end is `SEASON_PERIOD_FLOOR`, imported for the reason
        // `light.diffusion` imports its own: below it the light changes and the water does not,
        // so the world runs under a season nothing in it can feel. The far end is **not** a gate
        // constant, because the gate deliberately has no ceiling - a million-tick climate is a
        // legitimate experiment and an invented bound is one somebody argues with on the evening
        // an experiment is refused. A slider still has to stop somewhere, and ten times the
        // shipped period is four times the median species lifetime: past it a lineage lives
        // entirely inside one half cycle and what it is watching is a trend rather than a season.
        label: "season_period",
        least: SEASON_PERIOD_LEAST,
        most: SEASON_PERIOD_MOST,
        places: Some(0),
        read: |raw| period(raw.light.season_period),
        write: |raw, value| raw.light.season_period = ticks(value),
    },
    Dial {
        table: "light",
        // ⭐ Not `0.5`. The far end is `SEASON_AMPLITUDE_CEILING`, which is where the measurements
        // stop and nowhere else. Nought is no season at all, which is what ships and which is the
        // control for every claim about one.
        label: "season_amplitude",
        least: 0.0,
        most: SEASON_AMPLITUDE_CEILING as f64,
        places: Some(2),
        read: |raw| raw.light.season_amplitude,
        write: |raw, value| raw.light.season_amplitude = value,
    },
    Dial {
        table: "physics",
        label: "drag",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.physics.drag,
        write: |raw, value| raw.physics.drag = value,
    },
    Dial {
        table: "physics",
        // ⚠️ Not `0.0 - 3.0`. The low end is `DRAG_ANISOTROPY_FLOOR` and the high end is
        // `DRAG_ANISOTROPY_CEILING`, imported for the reason `light.diffusion` imports its
        // own: the ceiling is where the arithmetic stopped computing rather than a
        // preference, and a copy of `3.0` written out here would be silently wrong the day
        // somebody moves it.
        label: "drag_anisotropy",
        least: DRAG_ANISOTROPY_FLOOR as f64,
        most: DRAG_ANISOTROPY_CEILING as f64,
        places: Some(2),
        read: |raw| raw.physics.drag_anisotropy,
        write: |raw, value| raw.physics.drag_anisotropy = value,
    },
    Dial {
        table: "physics",
        label: "collision_stiffness",
        least: 0.1,
        most: 400.0,
        places: Some(1),
        read: |raw| raw.physics.collision_stiffness,
        write: |raw, value| raw.physics.collision_stiffness = value,
    },
    Dial {
        table: "physics",
        label: "spring_damping",
        least: 0.0,
        most: 4.0,
        places: Some(2),
        read: |raw| raw.physics.spring_damping,
        write: |raw, value| raw.physics.spring_damping = value,
    },
    Dial {
        table: "physics",
        // ⚠️ Not `0.0 - 1000.0`, and the far end is not a gate constant either — `config.rs`
        // deliberately puts no ceiling on a current, because the integrator is a contraction
        // under any constant force and there is nothing on the other side of a bound that
        // fails. What stops the slider here is where the **measurements** stop being about the
        // shipped world: `assay.rs`'s `a_current_buys_strangers_by_spending_contact` walked
        // eleven settings and 100 is the largest at which the population and the standing
        // biomass are still the ones every figure in SPEC was taken on — 1,867 bodies against
        // 1,753, and 26,886 units against 25,123. At 180 it is 1,336, and by 1,000 it is 650.
        // A person who wants that world can have it, and should have to write it in a file.
        label: "current",
        least: 0.0,
        most: 100.0,
        places: Some(1),
        read: |raw| raw.physics.current,
        write: |raw, value| raw.physics.current = value,
    },
    Dial {
        table: "behaviour",
        label: "resting_amplitude",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.behaviour.resting_amplitude,
        write: |raw, value| raw.behaviour.resting_amplitude = value,
    },
    Dial {
        table: "behaviour",
        // ⚠️ One is not a tidy round end. It is exactly where the shortest a spring asks to be -
        // `base_rest × (1 - stroke)` - reaches nought, and past it the rest length is negative:
        // the spring pulls at every phase of its cycle instead of oscillating about anything.
        // The gate refuses above it for that reason, so the dial stops there.
        label: "stroke",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.behaviour.stroke,
        write: |raw, value| raw.behaviour.stroke = value,
    },
    Dial {
        table: "metabolism",
        label: "upkeep_scale",
        least: 0.01,
        most: 8.0,
        places: Some(2),
        read: |raw| raw.metabolism.upkeep_scale,
        write: |raw, value| raw.metabolism.upkeep_scale = value,
    },
    Dial {
        table: "metabolism",
        // ⭐ Not `0.5 - 1.0` written out. Both ends are the gate's own constants, imported for
        // the reason `light.patch_drift` imports `PATCH_DRIFT_CEILING`: one is exactly linear —
        // the world every figure in this project was measured on — and a half is `d / (d + 1)`
        // in one dimension, the flattest exponent a distribution network can have. A copy of
        // either number written here would be silently wrong the day somebody moves it.
        label: "scaling_exponent",
        least: SCALING_EXPONENT_FLOOR as f64,
        most: LINEAR_SCALING as f64,
        places: Some(3),
        read: |raw| raw.metabolism.scaling_exponent,
        write: |raw, value| raw.metabolism.scaling_exponent = value,
    },
    Dial {
        table: "metabolism",
        label: "gene_cost",
        least: 0.0,
        most: 0.01,
        places: Some(4),
        read: |raw| raw.metabolism.gene_cost,
        write: |raw, value| raw.metabolism.gene_cost = value,
    },
    Dial {
        table: "metabolism",
        label: "movement_cost",
        least: 0.0,
        most: 2.0,
        places: Some(2),
        read: |raw| raw.metabolism.movement_cost,
        write: |raw, value| raw.metabolism.movement_cost = value,
    },
    Dial {
        table: "metabolism",
        label: "reproduction_threshold",
        least: 0.1,
        most: 10.0,
        places: Some(2),
        read: |raw| raw.metabolism.reproduction_threshold,
        write: |raw, value| raw.metabolism.reproduction_threshold = value,
    },
    Dial {
        table: "metabolism",
        label: "offspring_share",
        least: 0.0,
        most: 1.0,
        places: Some(2),
        read: |raw| raw.metabolism.offspring_share,
        write: |raw, value| raw.metabolism.offspring_share = value,
    },
    Dial {
        table: "mutation",
        label: "point_rate",
        least: 0.0,
        most: 0.5,
        places: Some(3),
        read: |raw| raw.mutation.point_rate,
        write: |raw, value| raw.mutation.point_rate = value,
    },
    Dial {
        table: "mutation",
        label: "point_sigma",
        least: 0.0,
        most: 1.0,
        places: Some(3),
        read: |raw| raw.mutation.point_sigma,
        write: |raw, value| raw.mutation.point_sigma = value,
    },
    Dial {
        table: "mutation",
        label: "duplication_rate",
        least: 0.0,
        most: 0.2,
        places: Some(3),
        read: |raw| raw.mutation.duplication_rate,
        write: |raw, value| raw.mutation.duplication_rate = value,
    },
    Dial {
        table: "mutation",
        label: "deletion_rate",
        least: 0.0,
        most: 0.2,
        places: Some(3),
        read: |raw| raw.mutation.deletion_rate,
        write: |raw, value| raw.mutation.deletion_rate = value,
    },
    Dial {
        table: "mutation",
        label: "insertion_rate",
        least: 0.0,
        most: 0.2,
        places: Some(3),
        read: |raw| raw.mutation.insertion_rate,
        write: |raw, value| raw.mutation.insertion_rate = value,
    },
    Dial {
        table: "mutation",
        label: "reorder_rate",
        least: 0.0,
        most: 0.2,
        places: Some(3),
        read: |raw| raw.mutation.reorder_rate,
        write: |raw, value| raw.mutation.reorder_rate = value,
    },
    Dial {
        table: "mutation",
        label: "genome_duplication_rate",
        least: 0.0,
        most: 0.02,
        places: Some(4),
        read: |raw| raw.mutation.genome_duplication_rate,
        write: |raw, value| raw.mutation.genome_duplication_rate = value,
    },
    Dial {
        table: "run",
        label: "max_ticks_per_second",
        least: 0.0,
        most: 600.0,
        places: None,
        read: |raw| f64::from(raw.run.max_ticks_per_second),
        write: |raw, value| raw.run.max_ticks_per_second = whole(value),
    },
];

/// ⭐ **`B2`.** The settings a run is stuck with, and what they are set to.
///
/// SPEC section 3 locks `[world]`, `[limits]` and `seed`, and the reason is CLAUDE.md's memory
/// guarantee rather than taste: *"Every arena - organisms, cells, springs, resource grid - is
/// allocated at startup at fixed capacity derived from the config, and never resized."* Every
/// one of these numbers is a size something in the world was built to.
///
/// They are **shown** because a run should be able to say what it is. A panel that reported a
/// population of 1,713 without saying whether the cap was four thousand or two thousand would be
/// reporting half a fact, and the whole argument of SPEC section 3's `influx` table turns on
/// telling a world bounded by its arena apart from one bounded by its energy.
///
/// The `unit` is the width the numeral is right-aligned against, exactly as in `panel.rs`.
#[must_use]
pub fn locked(config: &Config) -> Vec<(&'static str, String, &'static str)> {
    vec![
        ("seed", format!("{}", config.world.seed), ""),
        ("width", format!("{:.0}", config.world.width), ""),
        ("height", format!("{:.0}", config.world.height), ""),
        ("cols", format!("{}", config.world.grid_cols), ""),
        ("rows", format!("{}", config.world.grid_rows), ""),
        (
            "years",
            format!("{:.0}", config.world.years_per_tick),
            "/tick",
        ),
        (
            "organisms",
            format!("{}", config.limits.max_organisms),
            "max",
        ),
        (
            "cells",
            format!("{}", config.limits.max_cells_per_organism),
            "max",
        ),
        ("genes", format!("{}", config.limits.max_genes), "max"),
        ("steps", format!("{}", config.limits.max_dev_steps), "max"),
    ]
}

/// The settings of a run being watched: the document as it now stands, and the checked
/// configuration it produced.
///
/// ⭐ **The two are always in step, because the only thing that changes either changes both, and
/// only when the gate says yes.** That is the whole of this type.
#[derive(Debug, Clone)]
pub struct Dials {
    /// The document as it now stands. Unchecked by type - it is `config.rs`'s transcript - and
    /// checked in fact, because it is only ever assigned from a copy the gate accepted.
    raw: RawConfig,

    /// What that document came out of the gate as.
    checked: Config,

    /// How many changes have been accepted, so that a caller can tell whether it has yet handed
    /// the world the settings it is now living under.
    ///
    /// A count rather than a flag, because the caller that reads it is a window's event loop
    /// that may draw several frames between two changes and may take several changes between two
    /// frames. Comparing a number it kept against this one answers *"is what I last gave the
    /// world still what the panel says"* in both directions.
    accepted: u64,

    /// The last thing the gate refused, and what it said.
    ///
    /// Kept so the panel can show it. A refusal is not an error in the program - it is somebody
    /// having asked for a world that cannot exist - and the useful thing to do with it is to
    /// print the sentence `config.rs` wrote, which names the setting.
    refused: Option<ConfigError>,
}

impl Dials {
    /// Start from a configuration document that has already been through the gate.
    ///
    /// # Errors
    ///
    /// If the document is not one the simulation will accept. It cannot be, in the program as
    /// built - `args.rs` has already validated the same document - but this type's whole claim
    /// is that a `Config` inside it came out of `validate`, and building one any other way
    /// would make that claim by assertion instead of by construction.
    pub fn new(raw: RawConfig) -> Result<Self, ConfigError> {
        let checked = raw.clone().validate()?;

        Ok(Self {
            raw,
            checked,
            accepted: 0,
            refused: None,
        })
    }

    /// The settings the world should now be running under.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.checked
    }

    /// The document as it now stands.
    #[must_use]
    pub const fn document(&self) -> &RawConfig {
        &self.raw
    }

    /// What this dial is currently set to.
    #[must_use]
    pub fn value(&self, dial: &Dial) -> f64 {
        dial.of(&self.raw)
    }

    /// How many changes have been accepted since this was built.
    #[must_use]
    pub const fn accepted(&self) -> u64 {
        self.accepted
    }

    /// The last thing the gate refused, if it has refused anything.
    #[must_use]
    pub const fn refused(&self) -> Option<&ConfigError> {
        self.refused.as_ref()
    }

    /// ⭐⭐ **`B3`, and it is these eleven lines.** Turn a dial, if the gate will have it.
    ///
    /// The change is made to a **copy** of the document, the copy is put through
    /// [`RawConfig::validate`] whole, and only a copy that came back accepted is kept. So the
    /// three things that could go wrong all cannot:
    ///
    /// - A value the gate refuses does not reach the world, because the world is handed
    ///   [`Dials::config`] and that is only ever assigned from a validated document.
    /// - A value the gate refuses does not reach the *panel* either. The slider springs back,
    ///   because the slider reads [`Dials::value`], which reads the kept document.
    /// - And it is the **whole** document that is checked rather than the one field, which
    ///   matters for the settings whose bound is about a pair rather than about a number. There
    ///   are none today; there is no version of this that has to be found and changed when there
    ///   is one.
    ///
    /// # Errors
    ///
    /// Whatever `config.rs` said, in a sentence naming the setting. The run is untouched.
    pub fn set(&mut self, dial: &Dial, value: f64) -> Result<(), ConfigError> {
        let mut asked = self.raw.clone();
        (dial.write)(&mut asked, value);

        match asked.clone().validate() {
            Ok(checked) => {
                self.raw = asked;
                self.checked = checked;
                self.accepted += 1;
                self.refused = None;
                Ok(())
            }
            Err(problem) => {
                self.refused = Some(problem.clone());
                Err(problem)
            }
        }
    }
}

/// A dial's value, as the whole number a `[run]` setting is written in.
///
/// Only `run.max_ticks_per_second` reaches this, and the value has already been held inside
/// that dial's own range - nought to six hundred - by the widget that produced it and by the
/// clamp below. Every whole number in that range is exactly representable, and the floor and the
/// negative are both taken before the conversion, so there is nothing left to truncate and
/// nothing left to lose a sign. `panel.rs`'s `whole` carries the same note about pixels.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rate in ticks per second, clamped to 0..=600 on the line above; f32 and f64 both \
              hold every whole number in that range exactly"
)]
fn whole(value: f64) -> u32 {
    value.clamp(0.0, 600.0).round() as u32
}

/// A dial's value, as the whole number of ticks `light.season_period` is written in.
///
/// Clamped into the slider's own range before the conversion, so the floor and the negative are
/// both taken and there is nothing left to truncate or to lose a sign. Every whole number in that
/// range is exactly representable at either width.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a season's period in ticks, clamped to the slider's own 8,000..=210,000 on the line \
              above; f64 holds every whole number in that range exactly"
)]
fn ticks(value: f64) -> u64 {
    value.clamp(SEASON_PERIOD_LEAST, SEASON_PERIOD_MOST).round() as u64
}

/// The same period back out again, for the slider to sit at.
///
/// A period is a count of ticks and a slider is a `f64`. Exact for every value the dial can
/// express, and for every value a document could carry short of nine quadrillion ticks.
#[expect(
    clippy::cast_precision_loss,
    reason = "a season's period is a count of ticks; every one the dial can express is exact as a \
              64-bit float, and this is a number being put on a slider"
)]
fn period(ticks: u64) -> f64 {
    ticks as f64
}

/// The near end of the `light.season_period` slider, which **is** the gate's own floor.
///
/// ⚠️ A named constant rather than the cast written inline, because a `const` item is the only
/// place an `#[expect]` can be hung in a `const` array — and CLAUDE.md's rule is that a lossy
/// `as` is either restructured away or annotated with why it is not lossy. Eight thousand is
/// exact at every width there is.
#[expect(
    clippy::cast_precision_loss,
    reason = "the gate's own floor, which is eight thousand and is exact as a 64-bit float. The \
              cast is here rather than at the dial so that the dial's end is this constant \
              rather than a copy of its value"
)]
const SEASON_PERIOD_LEAST: f64 = SEASON_PERIOD_FLOOR as f64;

/// The far end of the same slider, and the one dial end in this file that is **not** a gate
/// constant.
///
/// The gate deliberately has no ceiling on a season's period: a very slow climate is a legitimate
/// experiment, and an invented bound is one somebody argues with on the evening an experiment is
/// refused. A slider still has to stop somewhere. Ten times the shipped period is four times the
/// median species lifetime, and past that a lineage lives entirely inside one half cycle — so
/// what a person is watching is a trend rather than a season, and the way to ask for one is the
/// configuration file rather than the slider.
const SEASON_PERIOD_MOST: f64 = 210_000.0;

#[cfg(test)]
mod tests {
    use super::{DIALS, Dials, locked};
    use coacervate_sim::chronicle;
    use coacervate_sim::config::{
        DIFFUSION_STABILITY_LIMIT, SEASON_AMPLITUDE_CEILING, SEASON_PERIOD_FLOOR, spec_defaults,
    };

    /// ⭐ **Phase 7, `C8`.** Every slider a person can turn is a condition the event log reports
    /// a change to, and every condition it reports is a slider.
    ///
    /// SPEC section 3: *"the rest can be changed live, **which is how environmental events
    /// work**"*, and SPEC section 11 lists *"environmental changes made by the user"* among the
    /// things the log records. Those two sentences are only one feature if the two lists are the
    /// same list.
    ///
    /// ⚠️ **They cannot be one list, and this is what stands in for that.** The sliders live here
    /// because they are made of bounds, formatting and a `RawConfig` - the *unchecked* document a
    /// panel edits; the log's live in `coacervate-sim`, because a `Config` and the sentence
    /// describing it are the simulation's. So the failure to guard against is somebody adding a
    /// twenty-third slider: without this, that setting would be changeable by hand and the change
    /// would never appear in the log, which is an environmental event that happened and was not
    /// written down.
    ///
    /// `run.max_ticks_per_second` is the one exception, and it is on the *dial* side only,
    /// because it is not a fact about the world at all - it is how fast a person is watching one,
    /// and slowing a run down is not weather.
    #[test]
    fn every_dial_is_a_condition_the_chronicle_reports() {
        let mut dials: Vec<String> = DIALS
            .iter()
            .map(super::Dial::field)
            .filter(|field| field != "run.max_ticks_per_second")
            .collect();
        let mut conditions: Vec<String> = chronicle::conditions().collect();

        dials.sort();
        conditions.sort();

        assert_eq!(
            dials, conditions,
            "the settings a person can change and the settings the event log describes are not \
             the same settings, so there is a slider whose change is an environmental event \
             nothing records - or a line the log can print about something no panel offers"
        );
    }

    /// The dials of a shipped run.
    fn shipped() -> Dials {
        Dials::new(spec_defaults()).expect("SPEC section 3's defaults are a world")
    }

    /// ⭐⭐ **`B3`, the first half.** Every dial reaches both of its ends, and the gate accepts
    /// what it finds there.
    ///
    /// This is the test that keeps the bounds and the gate in step, and it is worth being clear
    /// about which of the two it is testing. It is not testing `config.rs` - that has its own
    /// suite. It is testing that **the ranges written in this file do not describe worlds the
    /// program refuses**, which is a claim about two files agreeing, and the only way to state
    /// it is to drive one and ask the other.
    ///
    /// Both ends, and every dial rather than a sample: the ends are where a range is wrong, and
    /// the twenty-two entries above are twenty-two hand-written pairs of numbers of which
    /// several are neighbours with the same type. `config.rs`'s own
    /// `spec_defaults_convert_into_a_validated_config` lists all thirty-five settings for
    /// exactly this reason.
    #[test]
    fn every_dial_reaches_both_of_its_ends() {
        for dial in DIALS {
            assert!(
                dial.least < dial.most,
                "{} runs from {} to {}",
                dial.field(),
                dial.least,
                dial.most
            );

            for (which, value) in [("bottom", dial.least), ("top", dial.most)] {
                let mut dials = shipped();

                dials.set(dial, value).unwrap_or_else(|problem| {
                    panic!(
                        "{}'s {which} end is {value}, and a world with it there is one the \
                         program refuses: {problem}. The dial's range and the gate's have come \
                         apart, so dragging this slider all the way would spring back",
                        dial.field()
                    )
                });

                assert!(
                    (dials.value(dial) - value).abs() < f64::EPSILON,
                    "{} was set to {value} and reads back as {}",
                    dial.field(),
                    dials.value(dial)
                );
                assert_eq!(dials.accepted(), 1, "the change was not counted");
                assert!(
                    dials.refused().is_none(),
                    "an accepted change left a refusal"
                );
            }
        }
    }

    /// ⭐⭐ **`B3`, the second half, and the one the whole module exists for.** A value the gate
    /// refuses is *refused*, not applied.
    ///
    /// ⚠️ `light.diffusion` at 0.5 is the case that matters and it is the first one here. SPEC
    /// section 4: past a quarter the five-point stencil overshoots, the field oscillates and
    /// grows without limit, **and energy stays perfectly conserved the whole way down** - so the
    /// invariant this project checks every thousand ticks reports a healthy world right up until
    /// the numbers stop being finite. It is the one setting where a slider going round the gate
    /// would produce a run that fails silently, and 0.5 is a perfectly ordinary-looking fraction
    /// that somebody would type.
    ///
    /// Three claims per case, and the second and third are the ones with teeth: the change is
    /// refused, **the document is left as it was**, and **the checked configuration the world is
    /// handed is left as it was**. A `set` that refused and assigned anyway would pass the first.
    #[test]
    fn a_value_the_gate_refuses_is_not_applied() {
        let dial = |field: &str| {
            DIALS
                .iter()
                .find(|dial| dial.field() == field)
                .unwrap_or_else(|| panic!("there is no {field} dial"))
        };

        // The setting, a value its meaning or the arithmetic excludes, and what the complaint
        // has to be about. One per kind of bound `config.rs` has.
        for (field, refused) in [
            // ⚠️ Above the stability limit. Nothing downstream would catch it.
            ("light.diffusion", 0.5),
            // Below nothing: tiles draining into no account.
            ("light.influx", -0.1),
            // Not a fraction: a light field blotchier than the light it is blotching.
            ("light.patchiness", 1.5),
            // Not positive: a world where nothing costs anything to be alive.
            ("metabolism.upkeep_scale", 0.0),
            // Not a fraction: an offspring taking more than its parent had.
            ("metabolism.offspring_share", 1.5),
        ] {
            let dial = dial(field);
            let mut dials = shipped();
            let before = dials.value(dial);
            let world_was = dials.config().clone();

            let complaint = dials
                .set(dial, refused)
                .expect_err(&format!(
                    "{field} was set to {refused}, which the configuration gate refuses, and the \
                     slider took it anyway - so a slider is a way round the only thing standing \
                     between a typed number and a broken world"
                ))
                .to_string();

            assert!(
                complaint.starts_with(&format!("{field}: ")),
                "{field} was refused and the complaint was about something else: {complaint}"
            );
            assert!(
                (dials.value(dial) - before).abs() < f64::EPSILON,
                "{field} was refused and the document kept the refused value: it reads {} and \
                 was {before}",
                dials.value(dial)
            );
            assert_eq!(
                dials.config(),
                &world_was,
                "{field} was refused and the settings the world is handed changed anyway"
            );
            assert_eq!(
                dials.accepted(),
                0,
                "a refused change was counted as one the world should be told about"
            );
            assert!(
                dials.refused().is_some(),
                "a refusal left nothing for the panel to say"
            );
        }
    }

    /// ⭐ **`B1` and `B2` as a pair: exactly SPEC section 3's live half has a dial, and exactly
    /// its locked half does not.**
    ///
    /// Two lists that have to add up to one table. Written as a count as well as a membership
    /// test, so that a setting added to SPEC and forgotten here shows up as a number that no
    /// longer matches rather than as a slider nobody noticed was missing - which is the same
    /// device `config.rs`'s `every_spec_default_literal_narrows` uses on the same table.
    #[test]
    fn the_live_settings_have_dials_and_the_locked_ones_do_not() {
        // SPEC section 3: `[world]`, `[limits]` and `seed` lock at run start.
        for dial in DIALS {
            assert!(
                dial.table != "world" && dial.table != "limits",
                "{} is in a table SPEC section 3 locks at run start, and every arena in the \
                 world was sized from it",
                dial.field()
            );
        }

        // And the five tables SPEC calls "the rest" are here in full. Counted per table, so a
        // setting dropped out of one of them is a number that changes.
        for (table, settings) in [
            // ⭐ Eight since Phase 7's Group L: `season_period` and `season_amplitude`.
            ("light", 8),
            // ⭐ Five since `physics.current`: a depth-dependent sideways force on every cell.
            ("physics", 5),
            ("behaviour", 2),
            // ⭐ Six since `metabolism.scaling_exponent`: the power a body's summed upkeep is
            // raised to as it grows.
            ("metabolism", 6),
            ("mutation", 7),
            ("run", 1),
        ] {
            let found = DIALS.iter().filter(|dial| dial.table == table).count();
            assert_eq!(
                found, settings,
                "SPEC section 3's [{table}] has {settings} live settings and {found} of them \
                 have a dial"
            );
        }
        assert_eq!(
            DIALS.len(),
            29,
            "the dials do not add up to the tables above"
        );

        // ⚠️ Every dial's path is one `config.rs` would name in a refusal. A dial whose `table`
        // and `label` did not spell the field's real path would produce a complaint pointing at
        // a setting that does not exist, which is worse than no complaint - and the panel prints
        // that sentence verbatim. Not-a-number is the probe because it is the one value every
        // decimal setting refuses whatever its meaning, so one line covers all twenty-one of
        // them.
        //
        // ⚠️ The count dials are left out and there is exactly one: `run.max_ticks_per_second`
        // is a whole number and *every* whole number is a rate SPEC section 3 allows, nought
        // included - that is its way of writing "uncapped". There is no value it can be given
        // that the gate would refuse, so there is no refusal to read a field name out of.
        let shipped = shipped();
        for dial in DIALS.iter().filter(|dial| dial.places.is_some()) {
            let mut dials = shipped.clone();
            let complaint = dials
                .set(dial, f64::NAN)
                .expect_err("not-a-number is not a setting")
                .to_string();

            assert!(
                complaint.starts_with(&format!("{}: ", dial.field())),
                "the {} dial writes into a field the gate calls something else: {complaint}",
                dial.field()
            );
        }

        // The locked half is shown rather than hidden - a run should be able to say what it is.
        let shown = locked(shipped.config());
        assert_eq!(
            shown.len(),
            10,
            "`[world]` has six settings and `[limits]` four, and {} are shown",
            shown.len()
        );
        assert!(
            shown
                .iter()
                .any(|(name, value, _)| *name == "seed" && value == "42"),
            "the panel does not say what seed the run is: {shown:?}"
        );
    }

    /// The one bound that is not a number in this file.
    ///
    /// ⚠️ Stated on its own because it is the only bound in SPEC section 3 that nothing
    /// downstream would catch - see this module's header and `config.rs`'s
    /// `DIFFUSION_STABILITY_LIMIT`. A copy of `0.25` written out here would pass every test
    /// above on the day it was written and would be silently wrong the day the limit moved.
    #[test]
    fn the_diffusion_dial_stops_where_the_gate_does_and_not_at_a_copy_of_it() {
        let dial = DIALS
            .iter()
            .find(|dial| dial.field() == "light.diffusion")
            .expect("light.diffusion is a live setting");

        assert!(
            (dial.most - f64::from(DIFFUSION_STABILITY_LIMIT)).abs() < f64::EPSILON,
            "the diffusion dial stops at {} and the gate refuses above {DIFFUSION_STABILITY_LIMIT}",
            dial.most
        );

        // And a hair past it is refused, which is what makes the limit a limit rather than a
        // label. `config.rs` allows the quarter itself: a limit that cannot be reached is a
        // limit one step lower with nobody able to tell which.
        let mut dials = shipped();
        dials
            .set(dial, f64::from(DIFFUSION_STABILITY_LIMIT))
            .expect("the stability limit itself is allowed");
        dials
            .set(dial, f64::from(DIFFUSION_STABILITY_LIMIT) + 0.001)
            .expect_err("a hair past the stability limit is a field that grows without limit");
    }

    /// ⭐⭐ **Group L.** The two season dials stop where the gate does, and at the gate's own
    /// constants rather than at copies of their values.
    ///
    /// The same claim `the_diffusion_dial_stops_where_the_gate_does_and_not_at_a_copy_of_it`
    /// makes, for the two settings added last — and it matters more for these two than for any
    /// other pair in the file, because **neither bound can be guessed from the name of the
    /// setting.** A period's floor is `light.cap / light.influx`, which is a fact about the field
    /// and not about the season; an amplitude's ceiling is where the *measurements* stop, which
    /// is not a fact about anything at all except what has been run.
    ///
    /// ⚠️ **One end here is deliberately not a gate constant, and this is where that is written
    /// down.** The gate has no ceiling on a period: a very slow climate is a legitimate
    /// experiment, and an invented bound is one somebody argues with on the evening an experiment
    /// is refused. A slider still has to stop somewhere, so its far end is [`SEASON_PERIOD_MOST`]
    /// — ten times the shipped period — and the way to ask for a slower season is the
    /// configuration file.
    #[test]
    fn the_two_season_dials_stop_where_the_gate_does() {
        let dial = |field: &str| {
            DIALS
                .iter()
                .find(|dial| dial.field() == field)
                .expect("both season settings are live")
        };

        let period = dial("light.season_period");
        assert!(
            (period.least - super::SEASON_PERIOD_LEAST).abs() < f64::EPSILON,
            "the season-period dial starts at {} and the gate refuses below \
             {SEASON_PERIOD_FLOOR}",
            period.least
        );

        let amplitude = dial("light.season_amplitude");
        assert!(
            (amplitude.most - f64::from(SEASON_AMPLITUDE_CEILING)).abs() < f64::EPSILON,
            "the season-amplitude dial stops at {} and the gate refuses above \
             {SEASON_AMPLITUDE_CEILING}",
            amplitude.most
        );
        assert!(
            amplitude.least.abs() < f64::EPSILON,
            "the season-amplitude dial cannot be turned off; it starts at {}",
            amplitude.least
        );

        // And both ends of both are reachable, and a hair past each is refused. The gate allows
        // the ends themselves — a limit that cannot be reached is a limit one step lower with
        // nobody able to tell which — so the refusal has to be tested just outside them.
        let mut dials = shipped();
        dials
            .set(period, super::SEASON_PERIOD_LEAST)
            .expect("the floor itself is a season the water can follow");
        dials
            .set(period, super::SEASON_PERIOD_MOST)
            .expect("the far end of the slider is a world");
        dials
            .set(amplitude, f64::from(SEASON_AMPLITUDE_CEILING))
            .expect("the deepest season anything has been measured at is allowed");
        dials
            .set(amplitude, 0.0)
            .expect("no season at all is the control for every claim about one");
        dials
            .set(amplitude, f64::from(SEASON_AMPLITUDE_CEILING) + 0.001)
            .expect_err("a season deeper than anything measured is not a world to run");

        // ⚠️ The period's floor cannot be crossed through the slider at all, because `ticks`
        // clamps into the dial's own range before the gate sees it — so the refusal is checked
        // through the document instead, which is the route a person editing a file takes.
        let mut document = shipped();
        assert!(
            (document.value(period) - 21_000.0).abs() < f64::EPSILON,
            "the shipped period is not the one SPEC section 4 measured"
        );
        document
            .set(period, super::SEASON_PERIOD_LEAST - 1.0)
            .expect("the slider clamps rather than refusing, so this is the floor itself");
        assert!(
            (document.value(period) - super::SEASON_PERIOD_LEAST).abs() < f64::EPSILON,
            "a slider dragged below the floor left the period at {} rather than at the floor",
            document.value(period)
        );
    }
}
