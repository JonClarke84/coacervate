//! Coacervate — the binary.
//!
//! This is the only crate that touches the filesystem. It reads the configuration
//! document, hands the parsed values to `coacervate-sim` to be checked, and reports the
//! problem in plain English if the check fails.
//!
//! The division of labour is deliberate and it is visible in this crate's `Cargo.toml`,
//! which does not depend on `serde` at all. The shapes a configuration document can take,
//! and the rules about what its numbers may be, belong to the simulation; reading bytes
//! and turning them into those shapes belongs here. That absence in the manifest is what
//! actually enforces CLAUDE.md's rule that the simulation crate does no I/O, rather than
//! it being a convention somebody has to keep to.

#![forbid(unsafe_code)]

use coacervate_sim::config::{Config, ConfigError, RawConfig};
use std::process::ExitCode;

/// The configuration the program ships with, built into the executable.
///
/// Read at compile time rather than looked for on disk, so the program has settings to
/// run on wherever it is copied to and there is no such thing as a missing configuration.
/// Choosing a different file is Phase 4's business, along with the runner that would use
/// it.
const DEFAULT_CONFIG: &str = include_str!("../../../config/default.toml");

/// Why a configuration document could not be turned into settings for a run.
///
/// Two quite different failures, kept apart because they happen at different points and
/// say different things to the person reading them. One is "this is not a configuration
/// document"; the other is "this is a configuration document, and it asks for a world
/// that cannot exist".
#[derive(Debug)]
enum LoadError {
    /// The text could not be read as a configuration document at all: broken syntax, a
    /// key nobody recognises, a setting left out, a number where a word should be.
    Unreadable(toml::de::Error),

    /// The document was read, and the simulation refused what it asked for.
    Refused(ConfigError),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(problem) => write!(out, "{problem}"),
            Self::Refused(problem) => write!(out, "{problem}"),
        }
    }
}

/// Read a configuration document and check it, in that order.
///
/// The whole journey from text to settings a run could be started from, and the only
/// route there is: reading produces the document as written, checking produces the
/// settings, and there is no way to obtain the second without the first.
fn load(document: &str) -> Result<Config, LoadError> {
    let raw: RawConfig = toml::from_str(document).map_err(LoadError::Unreadable)?;
    raw.validate().map_err(LoadError::Refused)
}

fn main() -> ExitCode {
    match load(DEFAULT_CONFIG) {
        Ok(config) => {
            println!(
                "Configuration accepted: seed {}, a {} by {} world on a {} by {} grid.",
                config.world.seed,
                config.world.width,
                config.world.height,
                config.world.grid_cols,
                config.world.grid_rows,
            );
            ExitCode::SUCCESS
        }
        // To the error stream and with a failing exit code, so that a run started by a
        // script at two in the morning stops there rather than appearing to have worked.
        Err(problem) => {
            eprintln!("This configuration cannot be used: {problem}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_CONFIG, load};
    use coacervate_sim::config::RawConfig;

    /// The `[world]` table reads back the numbers written in it: seed 42, a 2048 × 1152
    /// world sampled by a 256 × 144 resource grid, and a thousand years to the tick.
    ///
    /// These six values are the ones SPEC section 3 locks at run start, so they are also
    /// the six a run can never be talked out of afterwards.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "pinning the literal written in the document; an approximate match here \
                  would defeat the purpose of the test"
    )]
    fn the_world_table_parses() {
        let raw: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        assert_eq!(raw.world.seed, 42);
        assert_eq!(raw.world.width, 2048.0);
        assert_eq!(raw.world.height, 1152.0);
        assert_eq!(raw.world.grid_cols, 256);
        assert_eq!(raw.world.grid_rows, 144);
        assert_eq!(raw.world.years_per_tick, 1000.0);
    }

    /// Every remaining value in the shipped document is the literal SPEC section 3 writes.
    ///
    /// This is the test that pins the program's idea of a configuration to the
    /// specification document rather than to whatever happened to be typed. If SPEC and
    /// the shipped file ever disagree, one of them was edited and the other was not, and
    /// this is where that shows up.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "pinning the literals written in the document; an approximate match here \
                  would defeat the purpose of the test"
    )]
    fn the_default_profile_matches_spec_section_3() {
        let raw: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        assert_eq!(raw.light.influx, 0.012);
        assert_eq!(raw.light.cap, 8.0);
        assert_eq!(raw.light.gradient, 0.75);
        assert_eq!(raw.light.patchiness, 0.15);
        assert_eq!(raw.light.diffusion, 0.04);

        assert_eq!(raw.physics.drag, 0.92);
        assert_eq!(raw.physics.collision_stiffness, 40.0);
        assert_eq!(raw.physics.spring_damping, 0.35);

        assert_eq!(raw.metabolism.upkeep_scale, 1.0);
        assert_eq!(raw.metabolism.movement_cost, 0.15);
        assert_eq!(raw.metabolism.reproduction_threshold, 2.2);
        assert_eq!(raw.metabolism.offspring_share, 0.45);

        assert_eq!(raw.mutation.point_rate, 0.06);
        assert_eq!(raw.mutation.point_sigma, 0.12);
        assert_eq!(raw.mutation.duplication_rate, 0.02);
        assert_eq!(raw.mutation.deletion_rate, 0.02);
        assert_eq!(raw.mutation.insertion_rate, 0.01);
        assert_eq!(raw.mutation.genome_duplication_rate, 0.0008);

        assert_eq!(raw.limits.max_organisms, 4000);
        assert_eq!(raw.limits.max_cells_per_organism, 64);
        assert_eq!(raw.limits.max_genes, 128);
        assert_eq!(raw.limits.max_dev_steps, 16);

        assert_eq!(raw.run.max_wall_clock_hours, 12.0);
        assert_eq!(raw.run.max_ticks, 0);
        assert_eq!(raw.run.max_ticks_per_second, 0);
        assert!(!raw.run.reseed_on_extinction);
    }

    /// Delete the line that sets `key`, so the document is missing that setting.
    fn without(document: &str, key: &str) -> String {
        let kept: Vec<&str> = document
            .lines()
            .filter(|line| !line.trim_start().starts_with(key))
            .collect();
        assert_eq!(
            kept.len() + 1,
            document.lines().count(),
            "no single line in the shipped config sets {key}"
        );
        kept.join("\n")
    }

    /// A mistyped setting stops the run. It is not quietly ignored.
    ///
    /// This is the most valuable test in the group, and the reason is what the failure
    /// costs rather than how likely it is. Type `influks` for `influx` and, with nothing
    /// checking, the program reads a file that mentions nothing it recognises, shrugs,
    /// and runs the world on some other number entirely. It does not crash and it does
    /// not complain. You find out in the morning, from eight hours of results belonging
    /// to an experiment you did not set up.
    ///
    /// Four ways of getting a document wrong are checked. A misspelled key, where the
    /// error has to name both what was typed and what was meant, because "unknown field"
    /// on its own leaves you hunting. A key nobody recognises sitting in a table that is
    /// otherwise complete, which is the case a check on missing keys alone would sail
    /// past. A whole table nobody recognises. And a key simply left out, because a
    /// setting the author never decided is not a setting the program should decide for
    /// them.
    ///
    /// **Every table gets the stray-key treatment, and that loop is load-bearing.** The
    /// setting that produces this behaviour has to be repeated on each table
    /// individually - it is not inherited from the document as a whole - so testing one
    /// table proves nothing whatsoever about the other six. Checked by removing it from
    /// `[mutation]` alone: with only `[light]` probed here, the entire suite stayed green
    /// while every mutation rate in the document became a key that could be silently
    /// misspelled.
    #[test]
    fn typos_and_omissions_are_rejected_not_ignored() {
        let misspelled = DEFAULT_CONFIG.replace("influx =", "influks =");
        let message = toml::from_str::<RawConfig>(&misspelled)
            .expect_err("a misspelled key must stop the run")
            .to_string();
        assert!(
            message.contains("influks"),
            "the error does not say which key was not recognised: {message}"
        );
        assert!(
            message.contains("influx"),
            "the error does not offer the key that was meant: {message}"
        );

        for table in [
            "world",
            "light",
            "physics",
            "metabolism",
            "mutation",
            "limits",
            "run",
        ] {
            let stray = DEFAULT_CONFIG.replace(
                &format!("[{table}]\n"),
                &format!("[{table}]\nsparkle = 1.0\n"),
            );
            assert_ne!(
                stray, DEFAULT_CONFIG,
                "the shipped config has no [{table}] table, so this case tested nothing"
            );

            toml::from_str::<RawConfig>(&stray).expect_err(&format!(
                "a stray key in the otherwise complete [{table}] table must stop the run"
            ));
        }

        let unknown_table = format!("{DEFAULT_CONFIG}\n[weather]\nrain = true\n");
        toml::from_str::<RawConfig>(&unknown_table)
            .expect_err("a table nobody recognises must stop the run");

        let incomplete = without(DEFAULT_CONFIG, "diffusion");
        toml::from_str::<RawConfig>(&incomplete).expect_err("a missing setting must stop the run");
    }

    /// A configuration survives being written out and read back in.
    ///
    /// SPEC section 13 has the replay log carry the settings that produced a run, so a
    /// recording made tonight can be understood next year. That only works if writing a
    /// configuration down and reading it back gives the same configuration, and this is
    /// the test that says so - every field, not a spot check, because `assert_eq!` on the
    /// whole document compares all of them and cannot be fooled by a field somebody
    /// forgot to look at.
    ///
    /// Three claims, and the second two are the ones worth explaining.
    ///
    /// **Writing twice gives the same bytes.** Without this, a run's settings could be
    /// stable in meaning while wandering in form, and every archived configuration would
    /// differ from every other for no reason anyone could point at.
    ///
    /// **The tables come back in SPEC's order.** A document that reorders itself is still
    /// correct and is much harder to read next to the specification it came from, and
    /// this is the file a person opens when they want to know what a run was.
    #[test]
    fn a_config_round_trips() {
        let parsed: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        let written = toml::to_string(&parsed).expect("a parsed config can be written back out");
        let read_back: RawConfig =
            toml::from_str(&written).expect("what was written can be read again");

        assert_eq!(
            parsed, read_back,
            "a configuration changed on its way out to a file and back"
        );

        let written_again =
            toml::to_string(&read_back).expect("a parsed config can be written back out");
        assert_eq!(
            written, written_again,
            "writing the same configuration twice produced two different documents"
        );

        let headers: Vec<&str> = written
            .lines()
            .filter(|line| line.starts_with('['))
            .collect();
        assert_eq!(
            headers,
            [
                "[world]",
                "[light]",
                "[physics]",
                "[metabolism]",
                "[mutation]",
                "[limits]",
                "[run]",
            ],
            "the tables came back in a different order from the one SPEC section 3 writes"
        );
    }

    /// The configuration the program ships with is one the program will accept.
    ///
    /// Everything before this tested a piece of the journey: the document parses, the
    /// numbers narrow, the bounds hold. This walks the whole of it, from the bytes that
    /// are built into the executable through to a configuration a run could be started
    /// from - which is the thing that actually has to work, and the thing none of the
    /// pieces guarantee on their own.
    ///
    /// It is also the test that catches the most ordinary failure there is: somebody
    /// edits `config/default.toml` to try something, mistypes it, and ships an executable
    /// that refuses its own settings on startup.
    #[test]
    fn the_bundled_default_config_validates() {
        let config = load(DEFAULT_CONFIG).expect("the configuration we ship must be one we accept");

        assert_eq!(config.world.seed, 42);
        assert_eq!(config.world.grid_cols.get(), 256);
        assert_eq!(config.limits.max_genes.get(), 128);
        assert_eq!(
            config.run.max_ticks, None,
            "the shipped configuration asks to run until it is stopped"
        );
    }

    /// A configuration that is wrong is reported. The program does not fall over.
    ///
    /// This is the distinction CLAUDE.md draws between the two kinds of failure, applied
    /// here. The simulation crashes on a broken invariant, because a violation there means
    /// the code is already wrong and there is nothing to be gained by carrying on. A bad
    /// configuration is not that. It is a person having typed something, which is an
    /// entirely ordinary event, and what is wanted is a sentence they can act on.
    ///
    /// Both ways of being wrong are checked, because they fail at different stages and it
    /// would be easy to handle one and not the other: a document that is not valid TOML at
    /// all never reaches the checking, and a document that parses perfectly and asks for an
    /// impossible world never reaches it any other way.
    #[test]
    fn a_bad_config_is_reported_not_crashed_on() {
        let not_a_document = "this is not a configuration, it is a sentence";
        let complaint = load(not_a_document)
            .expect_err("a document that is not a configuration must be refused")
            .to_string();
        assert!(
            !complaint.is_empty(),
            "a refusal with nothing to say is no use to anybody"
        );

        let impossible = DEFAULT_CONFIG.replace("gradient = 0.75", "gradient = 1.5");
        let complaint = load(&impossible)
            .expect_err("a gradient of 1.5 must be refused")
            .to_string();
        assert!(
            complaint.contains("light.gradient"),
            "the refusal does not say which setting is at fault: {complaint}"
        );
    }

    /// The two copies of SPEC's defaults - the shipped file and the fixture the
    /// simulation's own tests are written against - say the same thing.
    ///
    /// SPEC section 3's numbers are necessarily written down twice. The simulation crate
    /// cannot read a TOML document, by design, so its tests need a configuration built in
    /// Rust; and the program has to ship an actual file for anyone to edit. Two copies of
    /// the same numbers, maintained by hand, in different files, in different crates.
    ///
    /// The failure that invites is quiet and specific. Someone tunes `influx` in the
    /// shipped file, the simulation's entire test suite stays green because it never looks
    /// at that file, and from then on every test in the project is asserting things about
    /// a configuration nobody runs. This is the one assertion that ties the two together,
    /// and it is why the fixture is public rather than hidden inside the test module.
    #[test]
    fn spec_defaults_fixture_matches_the_shipped_file() {
        let from_the_file: RawConfig =
            toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        assert_eq!(
            from_the_file,
            coacervate_sim::config::spec_defaults(),
            "config/default.toml and coacervate_sim::config::spec_defaults() have drifted \
             apart: the tests and the shipped program are no longer describing the same \
             configuration"
        );
    }

    /// The number written in the configuration document is the number the randomness
    /// actually comes from.
    ///
    /// This is the one test in the project where the two halves of this phase meet, and it
    /// exists because everything else can pass without them being connected at all. The
    /// configuration tests prove a document is read correctly. The simulation's own
    /// generator tests prove that a given seed always produces the same numbers - but every
    /// one of them writes that seed as a literal in the test itself. Nothing, anywhere,
    /// checked that the seed in the file is the seed that reaches the generator.
    ///
    /// So without this test you could rename the setting, drop it from the checked
    /// configuration, or simply never pass it on, and the entire suite would stay green
    /// while every run of the simulation ignored the one number the person running it
    /// chose. CLAUDE.md's wording for this phase is "config, seeded RNG"; this assertion is
    /// what puts the "seeded" and the "config" in the same sentence.
    ///
    /// The four numbers below are the ones seed 42 produces. They are the same values
    /// pinned in the simulation's own `golden_vectors_pin_the_seed_to_stream_mapping`,
    /// repeated here deliberately: this test is asserting that the *route* from the file to
    /// the generator arrives at them, which is a different claim from the generator
    /// producing them when handed the number directly.
    #[test]
    fn the_bundled_config_seeds_the_world_rng() {
        use coacervate_sim::rng::WorldRng;
        use rand::Rng;

        let config = load(DEFAULT_CONFIG).expect("the configuration we ship must be one we accept");
        let mut world = WorldRng::from_seed(config.world.seed);

        let drawn: Vec<u64> = (0..4).map(|_| world.world_stream().next_u64()).collect();

        assert_eq!(
            drawn,
            vec![
                0x27fc_e323_2899_4b4c,
                0xf8c2_b3da_3a86_e191,
                0x6a61_49f8_f870_b678,
                0xa2d2_3c69_ecb1_ee64,
            ],
            "the randomness in a run is not coming from the seed written in the \
             configuration document"
        );

        // The assertion above, on its own, cannot tell the difference between reading the
        // seed from the document and ignoring the document to write 42 into the code -
        // because the shipped document happens to say 42. So change the document and
        // require the numbers to change with it. Editing the seed is the whole reason the
        // setting exists, and this is the only test that proves editing it does anything.
        let other_seed = DEFAULT_CONFIG.replace("seed = 42", "seed = 43");
        assert_ne!(
            other_seed, DEFAULT_CONFIG,
            "the shipped document no longer says `seed = 42`, so this test is not \
             changing what it thinks it is changing"
        );

        let config =
            load(&other_seed).expect("changing only the seed leaves a valid configuration");
        let mut world = WorldRng::from_seed(config.world.seed);
        let drawn_from_43: Vec<u64> = (0..4).map(|_| world.world_stream().next_u64()).collect();

        assert_ne!(
            drawn, drawn_from_43,
            "changing the seed in the configuration document changed nothing about the \
             run, so the seed is being ignored"
        );
    }
}
