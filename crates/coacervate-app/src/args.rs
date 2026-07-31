//! What the command line can say, and what it means.
//!
//! Five flags, written out by hand. CLAUDE.md is explicit about not over-building, and a
//! dependency for five flags would be exactly that: an argument parser is a few hundred
//! kilobytes of generality, a derive macro and a version to keep track of, in exchange for
//! about eighty lines. Written out, every rule about what an argument may be is visible in one
//! file, and the sentences a person actually reads when they mistype something are written
//! rather than generated.
//!
//! # A bad argument is a refusal, not a crash
//!
//! The same distinction `main.rs` draws about a bad configuration document, and for the same
//! reason. CLAUDE.md's rule about panicking is about invariants the *code* maintains - a ledger
//! that does not balance means the simulation is already wrong and there is nothing to be
//! gained by carrying on. A mistyped flag is not that. It is a person having typed something,
//! which is an entirely ordinary event, and what is wanted is a sentence they can act on and a
//! failing exit code so that a run started by a scheduled task at two in the morning stops
//! there rather than appearing to have worked.
//!
//! # ⚠️ A flag given twice is refused rather than resolved
//!
//! `--seed 1 --seed 2` could reasonably mean either. Taking the last quietly is how a person
//! ends up running an experiment they did not set up, which is precisely the failure
//! `typos_and_omissions_are_rejected_not_ignored` exists to prevent one file over. So it is
//! refused, and the person says which they meant.
//!
//! # ⚠️ The original bytes of the configuration are kept, and Phase 8 needs to know why
//!
//! SPEC section 13 wants `config.toml` copied **verbatim** into a run's directory, comments and
//! all, so that a recording made tonight explains itself next year. A configuration that had
//! been parsed and written back out would be correct and would have lost every comment in it -
//! `docs/PHASE1.md` recorded that as an obligation for whichever phase first read a
//! configuration from disk, and this is that phase. So [`Settings`] holds the document exactly
//! as it arrived, beside the values that were read out of it.
//!
//! ⚠️ **`--seed` and `--ticks` are therefore not in the kept document.** They are applied to the
//! values, and the document says whatever the file said. A run started with `--seed 7` against a
//! file that says `seed = 42` is a run whose verbatim configuration is *wrong about its own
//! seed*. That is recorded in `docs/PHASE5.md` as an open question rather than solved here,
//! because the only two ways out - rewriting the document, which loses the comments, or writing
//! the overrides down beside it, which invents a file format - are both decisions for Phase 8.
//! What is done in the meantime is that `main` says out loud when an override is in force.

use coacervate_sim::config::{Config, ConfigError, RawConfig};
use std::fmt;
use std::path::PathBuf;

/// What was asked for on the command line.
///
/// Everything is optional because the program has a complete configuration built into it and
/// runs perfectly well with no arguments at all - which is the ordinary case and the one that
/// must not need anything typed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    /// Show what the flags are and stop. Wins over everything else.
    pub help: bool,

    /// Read the settings from this document instead of the one built into the executable.
    pub config: Option<PathBuf>,

    /// Use this seed instead of the one the settings give.
    pub seed: Option<u64>,

    /// Stop when the world has taken this many ticks, instead of when the settings say.
    pub ticks: Option<u64>,

    /// Run the world, draw one frame of it here, and stop. See `main.rs`'s `DUMP_TICKS` for
    /// how long it runs first and why.
    pub dump_frame: Option<PathBuf>,
}

/// Why a command line could not be understood.
///
/// Four ways of getting it wrong, kept apart because they are four different things to be told.
/// Every one of them says which flag it is about, because "invalid argument" leaves a person
/// looking at the whole line.
#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentError {
    /// A flag nobody recognises.
    Unknown(String),

    /// A flag that takes a value, given without one.
    Incomplete(&'static str),

    /// A flag that takes a whole number, given something else.
    NotANumber { flag: &'static str, given: String },

    /// A flag given more than once.
    Repeated(&'static str),

    /// A bare word where a flag was expected.
    Stray(String),
}

impl fmt::Display for ArgumentError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(flag) => {
                write!(
                    out,
                    "There is no {flag} option. Run with --help to see what there is."
                )
            }
            Self::Incomplete(flag) => {
                write!(out, "{flag} needs something after it.")
            }
            Self::NotANumber { flag, given } => {
                write!(
                    out,
                    "{flag} needs a whole number and was given \"{given}\"."
                )
            }
            Self::Repeated(flag) => {
                write!(
                    out,
                    "{flag} was given more than once, and only one of them can be what was \
                     meant."
                )
            }
            Self::Stray(word) => {
                write!(
                    out,
                    "\"{word}\" is not an option and this program takes nothing else. Run with \
                     --help to see what there is."
                )
            }
        }
    }
}

/// What to print when somebody asks what the options are.
///
/// Plain ASCII and no box drawing, for the reason `main.rs` gives about its progress line: a
/// Windows console is not on a Unicode code page by default, so anything clever arrives as
/// mojibake in the one place this program is actually read.
pub const HELP: &str = "\
Coacervate - an open-ended evolution simulator. Not a game.

Usage: coacervate [options]

  --config <file>      Read the settings from this TOML document instead of the
                       one built into the program.
  --seed <n>           Run with this seed instead of the one the settings give.
                       One seed and one configuration are always the same run.
  --ticks <n>          Stop once the world has taken this many ticks. This is
                       the world's own count and it includes the ticks the light
                       spent falling before anything was seeded. 0 means no tick
                       bound, which is what the shipped settings ask for.
  --dump-frame <path>  Run the world, draw one frame of it to this PNG, and
                       stop. Without --ticks it runs to tick 30000, which at
                       the shipped settings is about twenty thousand ticks of
                       life after the dawn and takes around twenty seconds.
  --help               Show this and stop.

With no options at all it runs the settings built into the program until it is
stopped. Press Enter to stop a run gracefully; a run also ends on its own at
whichever of its bounds arrives first.";

impl Arguments {
    /// Read a command line.
    ///
    /// Takes the arguments *after* the program's own name, so that a test can hand it a line
    /// without having to invent one.
    ///
    /// # Errors
    ///
    /// If a flag is not recognised, is given twice, is missing the value it needs, or is given
    /// a value that is not the kind of thing it takes. See [`ArgumentError`].
    pub fn parse(line: impl IntoIterator<Item = String>) -> Result<Self, ArgumentError> {
        let mut parsed = Self::default();
        let mut line = line.into_iter();

        while let Some(word) = line.next() {
            match word.as_str() {
                "--help" => parsed.help = true,
                "--config" => {
                    once(&mut parsed.config, "--config")?;
                    parsed.config = Some(PathBuf::from(value(&mut line, "--config")?));
                }
                "--dump-frame" => {
                    once(&mut parsed.dump_frame, "--dump-frame")?;
                    parsed.dump_frame = Some(PathBuf::from(value(&mut line, "--dump-frame")?));
                }
                "--seed" => {
                    once(&mut parsed.seed, "--seed")?;
                    parsed.seed = Some(number(&mut line, "--seed")?);
                }
                "--ticks" => {
                    once(&mut parsed.ticks, "--ticks")?;
                    parsed.ticks = Some(number(&mut line, "--ticks")?);
                }
                flag if flag.starts_with('-') => {
                    return Err(ArgumentError::Unknown(word));
                }
                _ => return Err(ArgumentError::Stray(word)),
            }
        }

        Ok(parsed)
    }
}

/// Refuse a flag that has already been given.
fn once<T>(slot: &mut Option<T>, flag: &'static str) -> Result<(), ArgumentError> {
    if slot.is_some() {
        return Err(ArgumentError::Repeated(flag));
    }

    Ok(())
}

/// The word after a flag, which has to be there.
fn value(
    line: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, ArgumentError> {
    line.next().ok_or(ArgumentError::Incomplete(flag))
}

/// The whole number after a flag, which has to be there and has to be one.
fn number(
    line: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<u64, ArgumentError> {
    let given = value(line, flag)?;

    given
        .parse()
        .map_err(|_| ArgumentError::NotANumber { flag, given })
}

/// Where a configuration document came from.
///
/// Kept because it is what an error message has to name. "This configuration cannot be used"
/// is a very different sentence depending on whether the person chose the file or the program
/// did, and only one of those is something they can go and fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The document compiled into the executable.
    BuiltIn,

    /// A document read from here, because `--config` said so.
    File(PathBuf),
}

impl fmt::Display for Source {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn => write!(out, "the settings built into the program"),
            Self::File(path) => write!(out, "{}", path.display()),
        }
    }
}

/// A configuration document, where it came from, and the checked settings read out of it.
///
/// The three are kept together rather than the document being thrown away once it has been
/// parsed, and this module's documentation gives the reason: SPEC section 13 wants the document
/// copied verbatim into the run's directory, and a configuration written back out from its
/// parsed values would have lost every comment in it.
#[derive(Debug)]
pub struct Settings {
    /// The document exactly as it arrived, byte for byte.
    ///
    /// A `String` rather than a vector of bytes because TOML is defined to be UTF-8, so a
    /// document that is not valid UTF-8 is not a configuration document and the read fails with
    /// a sentence saying so. Reading it as text therefore loses nothing: no line endings are
    /// changed, no byte-order mark is stripped, and what comes out is what was on disk.
    document: String,

    /// Where it came from.
    source: Source,

    /// What was read out of it, checked, with any overrides from the command line applied.
    config: Config,
}

/// Why a run could not be started from what was asked for.
#[derive(Debug)]
pub enum SettingsError {
    /// The file named by `--config` could not be read.
    Unreadable {
        path: PathBuf,
        problem: std::io::Error,
    },

    /// The text is not a configuration document at all: broken syntax, a key nobody
    /// recognises, a setting left out, a number where a word should be.
    NotADocument {
        source: Source,
        problem: toml::de::Error,
    },

    /// The document was read, and the simulation refused what it asked for.
    Refused {
        source: Source,
        problem: ConfigError,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, problem } => {
                write!(out, "{} could not be read: {problem}", path.display())
            }
            Self::NotADocument { source, problem } => {
                write!(out, "{source} is not a configuration document: {problem}")
            }
            Self::Refused { source, problem } => {
                write!(
                    out,
                    "{source} asks for a world that cannot exist: {problem}"
                )
            }
        }
    }
}

impl Settings {
    /// Find the configuration these arguments ask for, read it, and check it.
    ///
    /// `built_in` is the document compiled into the program, which is what is used when no
    /// `--config` was given. Passing it in rather than reaching for it keeps the one piece of
    /// this that touches the disk in one function and leaves everything else testable without
    /// a file.
    ///
    /// # Errors
    ///
    /// If the file cannot be read, is not a configuration document, or asks for a world the
    /// simulation refuses. See [`SettingsError`].
    pub fn load(arguments: &Arguments, built_in: &str) -> Result<Self, SettingsError> {
        let (document, source) = match &arguments.config {
            None => (built_in.to_owned(), Source::BuiltIn),
            Some(path) => (
                std::fs::read_to_string(path).map_err(|problem| SettingsError::Unreadable {
                    path: path.clone(),
                    problem,
                })?,
                Source::File(path.clone()),
            ),
        };

        Self::read(document, source, arguments)
    }

    /// The same, given the document rather than somewhere to find it.
    ///
    /// Split out so that everything about *interpreting* a configuration can be tested without
    /// a file on disk, which leaves [`Settings::load`] as the one function whose whole job is
    /// the reading.
    ///
    /// # Where the overrides go in, and why it is here rather than after the checking
    ///
    /// `--seed` and `--ticks` are written into the document's values **before** they are
    /// checked, so that they go through exactly the gate every setting in the file goes
    /// through. A seed applied afterwards would be a number nothing had looked at, and a tick
    /// bound applied afterwards would have to re-implement `config.rs`'s rule about what a zero
    /// means.
    ///
    /// # Errors
    ///
    /// If the document is not a configuration document, or asks for a world the simulation
    /// refuses.
    pub fn read(
        document: String,
        source: Source,
        arguments: &Arguments,
    ) -> Result<Self, SettingsError> {
        let mut raw: RawConfig =
            toml::from_str(&document).map_err(|problem| SettingsError::NotADocument {
                source: source.clone(),
                problem,
            })?;

        if let Some(seed) = arguments.seed {
            raw.world.seed = seed;
        }
        if let Some(ticks) = arguments.ticks {
            raw.run.max_ticks = ticks;
        }

        let config = raw.validate().map_err(|problem| SettingsError::Refused {
            source: source.clone(),
            problem,
        })?;

        Ok(Self {
            document,
            source,
            config,
        })
    }

    /// The settings a run would be started from.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Where they came from.
    #[must_use]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The document exactly as it arrived. SPEC section 13's verbatim copy, waiting for Phase 8.
    #[must_use]
    pub fn document(&self) -> &str {
        &self.document
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_CONFIG;

    /// A command line, written the way it would be typed.
    fn line(words: &[&str]) -> Vec<String> {
        words.iter().map(|&word| word.to_owned()).collect()
    }

    /// Whatever `parse` said, as the sentence a person would see.
    fn refusal(words: &[&str]) -> String {
        Arguments::parse(line(words))
            .expect_err("this line must be refused")
            .to_string()
    }

    /// ⭐ **A6.** The five flags are read, and a line with nothing on it is a complete
    /// instruction.
    ///
    /// The last part is the one worth stating. This program has a full configuration built into
    /// it and every flag below is a way of departing from it, so `coacervate` with nothing after
    /// it has to be a run of the shipped settings - not a usage message, not a complaint about
    /// a missing argument. That is the ordinary case and it is the one somebody double-clicking
    /// the executable gets.
    #[test]
    fn the_binary_takes_arguments() {
        assert_eq!(
            Arguments::parse(line(&[])).expect("an empty command line is a complete request"),
            Arguments::default(),
            "a run with nothing typed after it is not a plain run of the shipped settings"
        );

        let asked = Arguments::parse(line(&[
            "--config",
            "worlds/dim.toml",
            "--seed",
            "7",
            "--ticks",
            "12345",
            "--dump-frame",
            "frames/one.png",
        ]))
        .expect("a line with all four options on it must be understood");

        assert_eq!(asked.config, Some(PathBuf::from("worlds/dim.toml")));
        assert_eq!(asked.seed, Some(7));
        assert_eq!(asked.ticks, Some(12_345));
        assert_eq!(asked.dump_frame, Some(PathBuf::from("frames/one.png")));
        assert!(!asked.help);

        // The order they are given in is not information.
        let backwards = Arguments::parse(line(&["--seed", "7", "--config", "worlds/dim.toml"]))
            .expect("options in any order must be understood");
        assert_eq!(backwards.seed, Some(7));
        assert_eq!(backwards.config, Some(PathBuf::from("worlds/dim.toml")));

        // `--help` on its own, and beside other things.
        assert!(
            Arguments::parse(line(&["--help"]))
                .expect("--help is a request")
                .help
        );
        assert!(
            Arguments::parse(line(&["--seed", "3", "--help"]))
                .expect("--help beside other options is still a request")
                .help,
            "--help stopped being noticed because something else was on the line"
        );
    }

    /// ⭐ **A6.** A command line that is wrong produces a sentence and a refusal. It does not
    /// panic, and it does not carry on.
    ///
    /// This is the same argument `a_bad_config_is_reported_not_crashed_on` makes about a
    /// configuration document, applied one layer further out, and it matters for the same
    /// reason: a person typing `--tick 500` and getting a twelve-hour run of the shipped
    /// settings has been told nothing at all, and finds out in the morning.
    ///
    /// Five ways of getting it wrong are checked, and every message is required to **name the
    /// flag it is about**. A refusal that says only "invalid argument" leaves somebody looking
    /// at the whole line, and the whole line is exactly what they have already checked.
    ///
    /// The repeated-flag case is the one that is a choice rather than an obligation. `--seed 1
    /// --seed 2` could be resolved by taking either, and both are guesses about which of two
    /// contradictory instructions was meant. Refusing costs the person one keystroke and cannot
    /// run the wrong experiment.
    #[test]
    fn a_bad_command_line_is_reported_not_crashed_on() {
        for (words, must_mention) in [
            (&["--tick", "500"][..], "--tick"),
            (&["--seed"][..], "--seed"),
            (&["--seed", "many"][..], "many"),
            (&["--seed", "1", "--seed", "2"][..], "--seed"),
            (&["config.toml"][..], "config.toml"),
        ] {
            let complaint = refusal(words);

            assert!(
                complaint.contains(must_mention),
                "the refusal of {words:?} does not mention {must_mention}: {complaint}"
            );
            assert!(
                complaint.len() > 20,
                "the refusal of {words:?} is not a sentence anybody can act on: {complaint}"
            );
        }

        // And the specific ones, so that the four failures are told apart rather than merely
        // all being failures.
        assert_eq!(
            Arguments::parse(line(&["--tick", "500"])),
            Err(ArgumentError::Unknown("--tick".to_owned()))
        );
        assert_eq!(
            Arguments::parse(line(&["--ticks"])),
            Err(ArgumentError::Incomplete("--ticks"))
        );
        assert_eq!(
            Arguments::parse(line(&["--ticks", "-1"])),
            Err(ArgumentError::NotANumber {
                flag: "--ticks",
                given: "-1".to_owned()
            }),
            "a negative tick bound is not a whole number of ticks, and passing it through would \
             be a run that stops at a moment that cannot arrive"
        );
        assert_eq!(
            Arguments::parse(line(&["--config", "a.toml", "--config", "b.toml"])),
            Err(ArgumentError::Repeated("--config"))
        );

        // ⚠️ A flag's value is taken as a value even when it looks like a flag, so that a path
        // beginning with a dash is a path. The alternative - refusing anything that looks like
        // a flag - would make some filenames unusable for no benefit.
        assert_eq!(
            Arguments::parse(line(&["--config", "--odd-name.toml"]))
                .expect("a value that looks like a flag is still a value")
                .config,
            Some(PathBuf::from("--odd-name.toml"))
        );
    }

    /// ⭐ **A6.** `--seed` and `--ticks` reach the settings a run is actually started from, and
    /// go through the same gate every setting in the file goes through.
    ///
    /// Without this, either flag could be parsed perfectly and then ignored, and the whole suite
    /// would stay green while a person ran an experiment with a seed they did not choose. It is
    /// the same claim `the_bundled_config_seeds_the_world_rng` makes about the document, one
    /// layer out.
    ///
    /// The last assertion is the one that says the override goes in *before* the checking rather
    /// than after: `config.rs` turns a `max_ticks` of nought into "no bound at all", and a
    /// `--ticks 0` that had been applied afterwards would have to know that rule for itself.
    #[test]
    fn the_seed_and_the_tick_bound_can_be_overridden() {
        let plain = Settings::read(
            DEFAULT_CONFIG.to_owned(),
            Source::BuiltIn,
            &Arguments::default(),
        )
        .expect("the shipped settings must be ones the program accepts");

        assert_eq!(plain.config().world.seed, 42, "the shipped seed");
        assert_eq!(
            plain.config().run.max_ticks,
            None,
            "the shipped settings ask to run until they are stopped"
        );

        let overridden = Settings::read(
            DEFAULT_CONFIG.to_owned(),
            Source::BuiltIn,
            &Arguments {
                seed: Some(1_234),
                ticks: Some(50_000),
                ..Arguments::default()
            },
        )
        .expect("changing the seed and the tick bound leaves settings the program accepts");

        assert_eq!(
            overridden.config().world.seed,
            1_234,
            "--seed was read and then ignored, so a person choosing a seed gets the shipped one"
        );
        assert_eq!(
            overridden.config().run.max_ticks,
            Some(50_000),
            "--ticks was read and then ignored"
        );

        // Nought means no bound, which is `config.rs`'s rule and not this module's.
        let unbounded = Settings::read(
            DEFAULT_CONFIG.to_owned(),
            Source::BuiltIn,
            &Arguments {
                ticks: Some(0),
                ..Arguments::default()
            },
        )
        .expect("a tick bound of nought is what the shipped document itself says");

        assert_eq!(
            unbounded.config().run.max_ticks,
            None,
            "a --ticks of nought did not come out meaning what the same number written in the \
             document means, so the override is being applied after the checking rather than \
             before it"
        );
    }

    /// ⭐ **A6.** The document is kept exactly as it arrived - comments, blank lines and all.
    ///
    /// SPEC section 13: *"`config.toml` - verbatim copy - the run is self-describing."*
    /// `docs/PHASE1.md` recorded the trap that goes with it as an obligation for this phase: a
    /// configuration that has been parsed and written back out is *correct* and has lost every
    /// comment in it, so a person opening an archived run next year finds thirty-three numbers
    /// and none of the reasoning that went with them.
    ///
    /// The comment written into the document below is deliberately something no re-serialisation
    /// could reproduce. A `Settings` that had kept `toml::to_string` of its parsed values would
    /// hold every one of those numbers and not one word of the sentence.
    #[test]
    fn the_original_document_is_kept_word_for_word() {
        let annotated = format!(
            "# Turned the light down on purpose - see the note in docs/PHASE4.md.\n\n{}",
            DEFAULT_CONFIG
        );

        let settings = Settings::read(
            annotated.clone(),
            Source::BuiltIn,
            &Arguments {
                // An override is applied as well, because the document must be kept as it was
                // written whatever the command line said about it.
                seed: Some(99),
                ..Arguments::default()
            },
        )
        .expect("a document with a comment on the top of it is still a document");

        assert_eq!(
            settings.document(),
            annotated,
            "the document changed on its way through, so SPEC section 13's verbatim copy \
             would be a copy of something else"
        );
        assert!(
            settings.document().contains("docs/PHASE4.md"),
            "the comment is gone, which is what re-serialising a parsed configuration does \
             and what docs/PHASE1.md recorded as the trap of this phase"
        );
        assert_eq!(
            settings.config().world.seed,
            99,
            "the override did not reach the settings"
        );
        assert!(
            settings.document().contains("seed = 42"),
            "⚠️ the kept document says {} rather than what the file said. See this module's \
             documentation: the overrides deliberately do not rewrite it, and Phase 8 has to \
             decide what an archived run says about a seed that came from the command line",
            settings.config().world.seed
        );
    }

    /// ⭐ **A6.** `--config` actually reads the file it names, and says something useful when it
    /// cannot.
    ///
    /// The one test here that touches the disk, and it is the only way to establish that the
    /// flag does anything at all: every other claim about `--config` can be made against a
    /// document handed over directly, and would pass just as well against a program that parsed
    /// the flag and then read its own built-in settings anyway.
    ///
    /// The file is written somewhere the operating system offers for the purpose and is taken
    /// away afterwards. Its name carries the process's own number so that two copies of the
    /// suite running at once cannot collide.
    #[test]
    fn a_config_file_is_read_from_where_it_was_asked_for() {
        let path = std::env::temp_dir().join(format!(
            "coacervate-{}-a-config-file-is-read.toml",
            std::process::id()
        ));
        let written = DEFAULT_CONFIG.replace("seed = 42", "seed = 8675309");
        std::fs::write(&path, &written).expect("the temporary directory must be writable");

        let arguments = Arguments {
            config: Some(path.clone()),
            ..Arguments::default()
        };
        let settings = Settings::load(&arguments, DEFAULT_CONFIG);
        let missing = Settings::load(
            &Arguments {
                config: Some(path.join("no-such-file.toml")),
                ..Arguments::default()
            },
            DEFAULT_CONFIG,
        );

        std::fs::remove_file(&path).expect("the file this test wrote must be removable");

        let settings = settings.expect("the file this test wrote is a configuration document");
        assert_eq!(
            settings.config().world.seed,
            8_675_309,
            "the seed came from the built-in settings rather than from the file --config named"
        );
        assert_eq!(
            settings.document(),
            written,
            "what was kept is not what was on disk"
        );
        assert_eq!(settings.source(), &Source::File(path));

        let complaint = missing
            .expect_err("a --config naming a file that is not there must stop the run")
            .to_string();
        assert!(
            complaint.contains("no-such-file.toml"),
            "the refusal does not say which file could not be read: {complaint}"
        );
    }

    /// ⭐ **A6.** `--help` describes every flag there is, and nothing that does not exist.
    ///
    /// A help text is the one piece of documentation in a program that is guaranteed to be read,
    /// and it is also the one that goes stale silently: a flag added and not mentioned is a
    /// feature nobody finds, and a flag removed and still mentioned is worse. So both directions
    /// are checked here.
    ///
    /// It is also where `--dump-frame`'s one surprise is written down. The flag does not draw
    /// the world as it is seeded - it runs it first, for some tens of seconds - and a person
    /// who expects an immediate PNG and gets a silence is entitled to have been told.
    #[test]
    fn help_describes_every_flag_and_only_the_flags() {
        for flag in ["--config", "--seed", "--ticks", "--dump-frame", "--help"] {
            assert!(
                HELP.contains(flag),
                "{flag} is an option this program takes and --help does not mention it"
            );
        }

        assert!(
            HELP.contains("30000"),
            "--dump-frame runs the world before it draws anything, and --help does not say how \
             far - so the wait looks like a program that has hung"
        );

        // Nothing invented. Every word in the text beginning with two dashes has to be a flag
        // `parse` actually understands, or the help is describing a program that does not
        // exist - which is the direction that wastes somebody's afternoon.
        //
        // A flag the program takes, given on its own, either parses or says it needs something
        // after it. Only a flag it does not take comes back as unknown.
        for word in HELP.split_whitespace() {
            let flag = word.trim_end_matches(|c: char| !c.is_alphanumeric());
            if !flag.starts_with("--") {
                continue;
            }

            assert_ne!(
                Arguments::parse(line(&[flag])),
                Err(ArgumentError::Unknown(flag.to_owned())),
                "--help offers {flag} and the program does not take it"
            );
        }

        assert!(
            HELP.is_ascii(),
            "the help text is not plain ASCII, and a Windows console is not on a Unicode code \
             page by default - so this is the one thing a person is guaranteed to read arriving \
             as mojibake"
        );
    }
}
