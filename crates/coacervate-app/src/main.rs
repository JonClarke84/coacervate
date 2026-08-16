//! Coacervate — the binary.
//!
//! This is the only crate that touches the filesystem, and the only one that may read a
//! clock. It reads the configuration document, hands the parsed values to `coacervate-sim`
//! to be checked, lights a world, puts the first bodies in it, and then ticks it until one
//! of SPEC section 3's bounds arrives.
//!
//! The division of labour is deliberate and it is visible in this crate's `Cargo.toml`,
//! which does not depend on `serde` at all. The shapes a configuration document can take,
//! and the rules about what its numbers may be, belong to the simulation; reading bytes
//! and turning them into those shapes belongs here. That absence in the manifest is what
//! actually enforces CLAUDE.md's rule that the simulation crate does no I/O, rather than
//! it being a convention somebody has to keep to.
//!
//! # The one allowance, and where it must not be written instead
//!
//! `clippy.toml` bans `Instant` and `SystemTime` outright, because SPEC section 2 keeps
//! wall-clock time out of simulation logic and CLAUDE.md wants the compiler doing the
//! reviewing. A runner needs a clock, so the ban is lifted for this crate — by the single
//! `allow` below and **not** by giving this package its own `[lints]` table, because a
//! package-level table *replaces* the workspace one and would silently take the five cast
//! lints with it. `clippy.toml` carries the same warning beside the ban.
//!
//! What the allowance is worth is argued in `run.rs`: nothing the clock decides can change
//! what a tick computes, only when it happens.

#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    reason = "SPEC section 3's run bounds are wall-clock bounds, so the runner needs a clock. \
              See clippy.toml, and see run.rs for why this cannot change what a run produces."
)]

mod args;
// ⭐⭐ **Phase 7's Group L.** The competition assay: what a configuration is worth, measured in
// forty minutes instead of a day. It is `#[cfg(test)]` because it is an instrument a person runs
// deliberately - `cargo test --release -- --ignored --nocapture assay` - rather than something a
// run does, and because everything in it is the public API of `coacervate-sim`, so it belongs
// beside the founding it borrows rather than inside the simulation it must not disturb.
#[cfg(test)]
mod assay;
mod founding;
mod run;

use args::{Arguments, Settings};
// ⚠️ `census` was this crate's own module until Phase 6 and now lives in `coacervate-render`.
// Phase 6's panel needs the same six numbers this progress line prints, and a panel cannot reach
// into the binary that draws it; the alternative was to compute them twice, which is the one
// thing `census.rs`'s own opening paragraph argues against. Nothing about the numbers changed.
use coacervate_render::census::{Census, millions_of_years};
use coacervate_sim::chronicle::Chronicle;
use coacervate_sim::config::RunConfig;
use coacervate_sim::species::Taxonomy;
use coacervate_sim::world::World;
use run::{Interrupt, Run, Stop};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The configuration the program ships with, built into the executable.
///
/// Read at compile time rather than looked for on disk, so the program has settings to
/// run on wherever it is copied to and there is no such thing as a missing configuration.
/// `--config` chooses a different document; this is what is used when nothing does.
const DEFAULT_CONFIG: &str = include_str!("../../../config/default.toml");

/// How many bodies the world is founded with.
///
/// Eight, spread across the width of the world — see `founding.rs` for why they are spread
/// rather than clustered. It is a small number on purpose: watching a population find its own
/// level from almost nothing is most of what there is to see in a headless run, and eight
/// founders reach it in about fifty thousand ticks at the shipped light. A run that started
/// near its equilibrium would have nothing to show for its first hour.
const FOUNDERS: u32 = 8;

/// How many ticks pass between two lines of the progress report.
///
/// Five thousand, which at SPEC section 3's thousand years to the tick is **five million
/// years** a line. A run long enough to be worth leaving alone therefore reads as a page of
/// geology rather than a number going up, which is CLAUDE.md's deep-time constraint applied to
/// the plainest output this program has.
const REPORT_EVERY: u64 = 5_000;

/// Where a run stops when it is only being run in order to draw one frame of it.
///
/// ⭐ **`--dump-frame` needs a world worth looking at, and a world is not worth looking at for
/// some time after it starts.** The count is the *world's*, so it includes the dawn -
/// `--ticks`' meaning, which Group A pinned - and at the shipped light the dawn is about ten
/// thousand ticks with nothing alive in it at all. Thirty thousand therefore means twenty
/// thousand ticks of life.
///
/// Measured rather than guessed, on the machine CLAUDE.md describes and against the shipped
/// settings:
///
/// | World tick | Alive | Mean genes | Release build |
/// | --- | --- | --- | --- |
/// | 10,000 | 8 | 1.00 | 3 s |
/// | 20,000 | 873 | 1.45 | 12 s |
/// | 30,000 | 1,713 | 1.70 | 23 s |
/// | 60,000 | 2,260 | 2.11 | 65 s |
///
/// Thirty thousand is where those two curves cross usefully. The population is well past its
/// founding - a couple of thousand bodies spread over the whole world rather than eight - and
/// mutation has visibly done something, with genomes averaging 1.7 genes and a spread of 0.85
/// around it, so what is drawn is a population that has diverged rather than eight copies of
/// `founding.rs`'s founder. And it costs twenty-three seconds, which is a wait somebody
/// checking a shader will actually sit through.
///
/// **`--ticks` overrides it**, and that is the whole of the answer to wanting an older world:
/// `--dump-frame frames/late.png --ticks 200000` draws one four times as old.
const DUMP_TICKS: u64 = 30_000;

fn main() -> ExitCode {
    // Everything the command line can do is decided before a tick is taken, so that a
    // mistyped flag costs nothing and `--dump-frame` does not first spend nine thousand
    // ticks on a dawn.
    let arguments = match Arguments::parse(std::env::args().skip(1)) {
        Ok(arguments) => arguments,
        // To the error stream and with a failing exit code, so that a run started by a
        // script at two in the morning stops there rather than appearing to have worked.
        Err(problem) => {
            eprintln!("{problem}");
            return ExitCode::FAILURE;
        }
    };

    if arguments.help {
        println!("{}", args::HELP);
        return ExitCode::SUCCESS;
    }

    let settings = match Settings::load(&arguments, DEFAULT_CONFIG) {
        Ok(settings) => settings,
        Err(problem) => {
            eprintln!("This configuration cannot be used: {problem}");
            return ExitCode::FAILURE;
        }
    };
    let config = settings.config().clone();

    // Plain ASCII from here down, and deliberately. A Windows console is not on a Unicode
    // code page by default, so an em dash or a plus-or-minus sign arrives as mojibake in the
    // one place this program is actually read.
    println!(
        "Coacervate - seed {}, a {} by {} world on a {} by {} grid.",
        config.world.seed,
        config.world.width,
        config.world.height,
        config.world.grid_cols,
        config.world.grid_rows,
    );

    // Where the settings came from and how much of a document there was, so that `--config`
    // can be seen to have worked rather than assumed to have. The overrides are said out loud
    // for a sharper reason: the kept document is the file as written, so a run started with
    // `--seed` is a run whose own configuration document disagrees with it. See `args.rs`.
    println!(
        "Settings: {} ({} bytes).",
        settings.source(),
        settings.document().len()
    );
    if arguments.seed.is_some() || arguments.ticks.is_some() {
        println!("The command line overrode part of them; the document above is unchanged.");
    }

    // ⚠️ A run that is only being run so that a frame can be drawn of it needs somewhere to
    // stop, and the shipped settings deliberately have no tick bound at all. `--ticks` is
    // still what decides it when it is given - so `--dump-frame` and `--ticks` together draw a
    // world of exactly the age asked for - and [`DUMP_TICKS`] is what is used when it is not.
    let bounds = if arguments.dump_frame.is_some() {
        RunConfig {
            max_ticks: Some(config.run.max_ticks.unwrap_or(DUMP_TICKS)),
            ..config.run.clone()
        }
    } else {
        config.run.clone()
    };

    // ⭐ The device is opened **before** the run, and for two reasons that are both about a
    // twenty-three-second wait. A machine with no graphics card now says so in the first second
    // rather than after the whole simulation has been computed - and, the reason this had to
    // change at all, a motion trail is a record of several moments and a run has to be *watched*
    // for one to exist. See `coacervate_render::Dump`.
    // ⭐ **Phase 6's settings, as the panel holds them.** Built from the same document `args.rs`
    // just put through the gate, which is why this cannot fail: `Dials::new` validates, and this
    // exact `RawConfig` was validated four lines ago. See `coacervate_render::settings`.
    let dials = coacervate_render::settings::Dials::new(settings.raw().clone())
        .expect("this document has already been through the gate");

    let mut filming = if arguments.dump_frame.is_some() && !arguments.window {
        let opened = if arguments.panel {
            coacervate_render::Dump::showing_panel(dials.clone())
        } else {
            coacervate_render::Dump::open()
        };

        match opened {
            Ok(dump) => Some(dump),
            Err(problem) => {
                eprintln!("{problem}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };

    let mut world = World::new(&config);
    let dawn = founding::genesis(&mut world, FOUNDERS);
    println!(
        "The light fell for {dawn} ticks; {FOUNDERS} founders are in the water. Press Enter to \
         stop.\n"
    );
    if let (Some(path), Some(until)) = (&arguments.dump_frame, bounds.max_ticks) {
        println!(
            "Running to tick {until} and then drawing one frame to {}.\n",
            path.display()
        );
    }

    let interrupt = Interrupt::new();
    listen(&interrupt);

    let mut run = Run::new(world, &bounds, &interrupt);

    let why = if arguments.window {
        // The window's own event loop drives `Run::step` from here on. It does not come back
        // until the run is over - either because a bound arrived or because somebody closed the
        // window, which asks for the same graceful stop Enter does.
        let frames = frames_of(config.world.seed);
        println!(
            "F12 writes the frame on the screen to {}.\n",
            frames.display()
        );

        match coacervate_render::window::show(
            &mut run,
            &frames,
            arguments.dump_frame.as_deref(),
            dials,
        ) {
            Ok(()) => run
                .stopped()
                .expect("a window stays open until the run it is showing is over"),
            Err(problem) => {
                eprintln!("{problem}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        heading();

        // Where the closing stretch that becomes the dumped frame's motion trail begins. A run
        // with no frame to draw never reaches it.
        let watching_from = bounds
            .max_ticks
            .map_or(u64::MAX, coacervate_render::Dump::watching_from);

        run.go(|world| {
            if world.ticks().is_multiple_of(REPORT_EVERY) {
                report(world);
            }

            // ⭐ The last hundred moments of the run, eleven ticks apart, which is what a window
            // sees in the second or so before somebody presses F12. Without this the dumped
            // frame would be a single instant and D2 would be the one visual feature of the
            // renderer that could not be checked by looking at a PNG.
            if let Some(dump) = filming.as_mut()
                && world.ticks() >= watching_from
                && (world.ticks() - watching_from).is_multiple_of(coacervate_render::TRAIL_STRIDE)
            {
                dump.watch(world);
            }
        })
    };

    // The last line of the report is printed either way, and for the windowed run it is the
    // only one: where the run got to, and what state it was in when it stopped. A window that
    // closed leaving nothing behind but a frame would be the one way of running this program
    // that produced no record of what happened.
    if arguments.window {
        heading();
    }
    report(run.world());
    println!("\n{}", composition(&Census::of(run.world())));
    println!("\n{}", lineages(run.taxonomy()));
    println!("\n{}", chronicle(run.chronicle()));
    println!("\n{}", ending(why));

    let Some(path) = &arguments.dump_frame else {
        return ExitCode::SUCCESS;
    };

    // ⚠️ A windowed run has already drawn this file, through the view the window was left at
    // and at the size it was left at. Drawing it again here would overwrite what was on the
    // screen with a picture of the whole world, which is precisely the thing `--window
    // --dump-frame` was asked for instead of.
    if arguments.window {
        println!("Drew {} as the window last showed it.", path.display());
        return ExitCode::SUCCESS;
    }

    // The world is drawn as it was left, whatever left it there. A run that went extinct before
    // its bound is drawn extinct: SPEC section 12's frame dump is a picture of what happened
    // rather than a picture of what was hoped for, and empty water is a perfectly good answer
    // to what a world looks like when nothing is in it.
    let mut dump = filming.expect("a headless run that is drawing a frame opened a device first");
    match dump.write(run.world(), run.series(), run.chronicle(), path) {
        Ok(()) => {
            println!(
                "Drew {} at {} by {}.",
                path.display(),
                coacervate_render::DUMP_WIDTH,
                coacervate_render::DUMP_HEIGHT
            );
            ExitCode::SUCCESS
        }
        Err(problem) => {
            eprintln!("{problem}");
            ExitCode::FAILURE
        }
    }
}

/// Where this run's frames go.
///
/// SPEC section 13 gives a run a directory of its own, named for when it started and what seed
/// it was, and CLAUDE.md puts `F12`'s frames in `runs/<id>/frames/`. This is that path, and
/// nothing at all is created here - the directory is made by the first frame written into it,
/// so a run that nobody takes a picture of leaves nothing behind.
///
/// ⚠️ **The timestamp is seconds since 1970 rather than a date anybody can read**, and that is
/// a deliberate deferral rather than a preference. Turning an instant into a year, a month and a
/// day is civil-calendar arithmetic - leap years, and the leap-year exceptions - or a
/// dependency, and Phase 8 owns the run directory properly because it is the phase that has to
/// write a replay log into it. The number sorts correctly in the meantime, which is the one
/// property the directory listing actually needs.
fn frames_of(seed: u64) -> PathBuf {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());

    Path::new("runs")
        .join(format!("{started}-{seed}"))
        .join("frames")
}

/// What a window is allowed to do to a run.
///
/// The whole of the join between the two halves of Group C. `coacervate-render` owns the window
/// and knows nothing about configurations, bounds or clocks; `run.rs` owns the loop and knows
/// nothing about windows. Four methods is what has to cross between them, and the important
/// thing about the list is what is *not* on it: there is no way for a window to reach the world
/// and change it, and no way for it to take a tick without the bounds being examined first.
///
/// `Run::step` is still the only loop. This makes it drivable from inside somebody else's.
impl coacervate_render::window::Watched for Run {
    fn due(&self) -> bool {
        Self::due(self)
    }

    fn tick(&mut self) -> bool {
        self.step().is_none()
    }

    fn world(&self) -> &World {
        Self::world(self)
    }

    /// ⭐ **Phase 6, `C1`.** The run's own record of itself, which the charts draw.
    fn series(&self) -> &coacervate_render::series::Series {
        Self::series(self)
    }

    /// ⭐ **Phase 7, Group C.** What has happened in this world, which the panel's last block
    /// shows and the chronicle will be written from.
    fn chronicle(&self) -> &coacervate_sim::chronicle::Chronicle {
        Self::chronicle(self)
    }

    fn ask_to_stop(&self) {
        Self::ask_to_stop(self);
    }

    /// ⭐ **Phase 6, `B1`.** The one method on this trait that changes anything, and the whole of
    /// what a slider does to a running world.
    ///
    /// It is still not a way for a window to *reach* the world: what crosses is a
    /// `coacervate_sim::config::Config`, which exists only because `RawConfig::validate` produced
    /// one, and `Run::retune` decides what to do with it. See `run.rs`.
    fn retune(&mut self, config: &coacervate_sim::config::Config) {
        Self::retune(self, config);
    }
}

/// Ask for a graceful stop as soon as anybody presses Enter.
///
/// ⚠️ **This is Enter and not `Ctrl-C`, and `run.rs` explains at length why it has to be.**
/// The short version: catching a console interrupt means either `unsafe` or a new dependency,
/// and this project forbids both. The flag set here is the seam — whatever eventually catches
/// a signal sets the same one.
///
/// The thread ends by itself if there is nothing on standard input to wait for, which is what a
/// run started by a scheduled task looks like. A read that returns nothing is an input stream
/// that has closed rather than somebody pressing a key, and it must not stop the run.
fn listen(interrupt: &Interrupt) {
    let asked = interrupt.clone();

    std::thread::spawn(move || {
        let mut line = String::new();
        if std::io::stdin()
            .read_line(&mut line)
            .is_ok_and(|read| read > 0)
        {
            asked.ask();
        }
    });
}

/// The column headings of the progress report, written once.
fn heading() {
    println!(
        "{:>12} {:>9} {:>6} {:>9} {:>8} {:>8} {:>11} {:>11} {:>9} {:>9} {:>11} {:>11} {:>6}",
        "time",
        "tick",
        "alive",
        "field",
        "biomass",
        "detritus",
        "dissipated",
        "light",
        "born",
        "died",
        "body",
        "genome",
        "depth",
    );
}

/// One line of the progress report: where the run has got to, and what state it is in.
///
/// SPEC section 5's five accounts are all here, and in the order that section lists them, so
/// that a person watching a run can see where the world's energy actually is — which is the
/// only way to tell a world that is short of light from one that is short of room.
///
/// **`born` and `died` are here for a reason that is easy to miss.** A population sitting at a
/// steady number is either turning over, with a birth for every death, or it is standing still
/// with nothing happening at all — and those are the same figure in the `alive` column and
/// completely different worlds. Only the two running totals beside it can tell them apart.
///
/// The time column is SPEC section 2's deep time: the tick count multiplied by
/// `world.years_per_tick` and read in millions of years, so a long run reads as Earth's history
/// rather than as a counter. It is presentation and it never enters the physics. The arithmetic
/// is `census::millions_of_years`, which the window's title bar and Phase 6's panel also read -
/// so the three places a person is told how far a run has got cannot disagree.
fn report(world: &World) {
    let census = Census::of(world);
    let ledger = world.ledger();
    let millions = millions_of_years(world);

    println!(
        "{:>9.1} Ma {:>9} {:>6} {:>9.0} {:>8.0} {:>8.0} {:>11.0} {:>11.0} {:>9} {:>9} \
         {:>6.2}+-{:<4.2} {:>6.2}+-{:<4.2} {:>6.0}",
        millions,
        world.ticks(),
        census.population,
        world.grid().total_energy(),
        ledger.biomass(),
        ledger.detritus(),
        ledger.dissipated(),
        ledger.influx_total(),
        census.born,
        census.deaths(),
        census.mean_cells,
        census.cell_spread,
        census.mean_genes,
        census.gene_spread,
        census.mean_depth,
    );
}

/// ⭐ **Phase 7, Group G.** What the population is *made of*, as one line of the closing report.
///
/// SPEC section 6's six kinds each have to be worth specialising into under some circumstance
/// and not others, and the only way to know whether any of them is is to count them. Every
/// reading of that in `docs/PHASE4.md` and `docs/PHASE7.md` - *"4,797 photocytes, 652
/// gonocytes, 11 sclerocytes, 6 sensocytes, 1 myocyte, 0 devorocytes"* - had to be taken by
/// hand off a debugger, which is why they appear once per phase rather than once per run.
///
/// Written in `CellKind::ALL`'s order, which is SPEC section 6's table's, so a reading here
/// can be laid straight against that table and against `series.rs`'s per-kind biomass.
fn composition(census: &Census) -> String {
    let cells: usize = census.kinds.iter().sum();
    let names = [
        "photocytes",
        "devorocytes",
        "myocytes",
        "sclerocytes",
        "sensocytes",
        "gonocytes",
    ];

    let mut said = format!("Of {cells} living cells:");
    for (name, count) in names.iter().zip(census.kinds) {
        said.push_str(&format!(" {count} {name},"));
    }
    said.pop();
    said.push('.');

    said
}

/// ⭐ **Phase 7, `A3`.** What the last clustering of the population found, as one line of the
/// closing report.
///
/// SPEC section 11's clustering happens every five hundred ticks whether anybody is watching or
/// not, and until this line existed a headless run did all of it and said nothing about any of
/// it. This is the one place a run with no window reports what was in the water.
///
/// ⚠️ **The register is `docs/PHASE7.md`'s and it is load-bearing**, which is why the wording is
/// worth reading rather than skimming: it says *what was there*, and nothing about whether that is
/// many or few, better or worse, or where it was heading. A run that ends with one group is not a
/// run that failed at anything.
fn lineages(taxonomy: &Taxonomy) -> String {
    let Some(tick) = taxonomy.sampled_at() else {
        return "The run was too short for the population to be clustered.".to_owned();
    };

    let mut said = format!(
        "At tick {tick} the population fell into {} groups by genetic distance, {} of which had \
         been there long enough to be counted as species. {} genera have been named in this run.",
        taxonomy.clusters().len(),
        taxonomy.species_count(),
        taxonomy.names().genera(),
    );

    // ⭐ **Phase 7, Group B.** The names themselves, which is the whole point of generating them:
    // a headless run that reported only a count would have named fifty lineages and shown none of
    // them. One line each, with what was alive in it at the last sample - a fact about the lineage
    // and not a ranking of it.
    for cluster in taxonomy.species() {
        if let Some(name) = cluster.name() {
            said.push_str(&format!("\n  {name} — {} alive", cluster.members()));
        }
    }

    said
}

/// ⭐⭐ **Phase 7, Group C.** The event log itself, as the closing part of the report.
///
/// This is what the whole group is for. `docs/PHASE7.md`: a run grew serially repeated bodies at
/// tick 2.8 million and it was found *by eye, in a screenshot, hours later, with no way to know
/// when it started or which lineage it happened in*. These lines are the world saying so at the
/// time.
///
/// # ⚠️ Printed at the end rather than as it happens, and only in a headless run
///
/// A headless run's output is a table - twelve aligned columns, one line every thousand ticks -
/// and a sentence appearing in the middle of it would break the alignment of everything after it.
/// A window has the panel, which shows the last few events live; a headless run gets them
/// together at the end, which is also the shape a chronicle has.
///
/// ⚠️ **A run long enough to fill the log says so.** `chronicle.rs`'s ring holds the most recent
/// thousand events and drops the oldest, so on a very long run the beginning of the history is no
/// longer in memory - and a report that quietly printed the last thousand as though they were all
/// of them would read as a complete record when it is not. SPEC section 13 asks for exactly this
/// about snapshots: *"Log what was dropped."*
fn chronicle(log: &Chronicle) -> String {
    if log.events().len() == 0 {
        return "Nothing happened in this world that the event log is watching for.".to_owned();
    }

    let mut said = match log.dropped() {
        0 => format!("The event log, all {} of it:", log.events().len()),
        dropped => format!(
            "The event log. The most recent {} events; {dropped} older ones have been dropped \
             off the front of it to keep the memory bounded:",
            log.events().len()
        ),
    };

    for event in log.events() {
        said.push_str("\n  ");
        said.push_str(&event.line());
    }

    said
}

/// What to say about a run that has stopped.
///
/// A sentence rather than the name of a variant, because these are four quite different pieces
/// of news and only one of them means the experiment finished. CLAUDE.md's rule about generated
/// text applies here as much as to the chronicle: extinction is stated and never called a
/// failure.
fn ending(why: Stop) -> &'static str {
    match why {
        Stop::OutOfTime => "Stopped: the run reached its wall-clock bound.",
        Stop::TicksDone => "Stopped: the run reached its tick bound.",
        Stop::Extinction => "Stopped: nothing is alive.",
        Stop::Asked => "Stopped: asked to.",
    }
}

#[cfg(test)]
mod tests {
    use super::DEFAULT_CONFIG;
    use crate::args::{Arguments, Settings, Source};
    use coacervate_sim::config::{Config, RawConfig};

    /// Read a configuration document and check it, with nothing overridden.
    ///
    /// The journey from text to settings a run could be started from, which `args.rs` owns
    /// because the command line can change what comes out of it. These tests are about the
    /// *document* rather than the command line, so they take the plainest route through.
    fn load(document: &str) -> Result<Config, crate::args::SettingsError> {
        Settings::read(document.to_owned(), Source::BuiltIn, &Arguments::default())
            .map(|settings| settings.config().clone())
    }

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

        assert_eq!(raw.light.influx, 0.001);
        assert_eq!(raw.light.cap, 8.0);
        assert_eq!(raw.light.gradient, 0.75);
        assert_eq!(raw.light.patchiness, 0.5);
        assert_eq!(raw.light.patch_drift, 0.0006);
        assert_eq!(raw.light.diffusion, 0.04);

        assert_eq!(raw.physics.drag, 0.92);
        assert_eq!(raw.physics.drag_anisotropy, 2.0);
        assert_eq!(raw.physics.collision_stiffness, 40.0);
        assert_eq!(raw.physics.spring_damping, 0.35);

        assert_eq!(raw.behaviour.resting_amplitude, 0.8);
        assert_eq!(raw.behaviour.stroke, 1.0);

        assert_eq!(raw.metabolism.upkeep_scale, 1.0);
        assert_eq!(raw.metabolism.scaling_exponent, 1.0);
        assert_eq!(raw.metabolism.gene_cost, 0.0001);
        assert_eq!(raw.metabolism.movement_cost, 0.0001);
        assert_eq!(raw.metabolism.reproduction_threshold, 2.2);
        assert_eq!(raw.metabolism.offspring_share, 0.45);

        assert_eq!(raw.mutation.point_rate, 0.06);
        assert_eq!(raw.mutation.point_sigma, 0.12);
        assert_eq!(raw.mutation.duplication_rate, 0.02);
        assert_eq!(raw.mutation.deletion_rate, 0.02);
        assert_eq!(raw.mutation.insertion_rate, 0.01);
        assert_eq!(raw.mutation.reorder_rate, 0.02);
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
                "[behaviour]",
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

    /// ⭐ **Phase 7, Group F.** The `dense` profile is the shipped world with a quarter of the
    /// water and the same light falling into it.
    ///
    /// SPEC section 3 lists the named presets and `config/dense.toml` is the fourth. It exists to
    /// ask whether a feeding-strategy split appears once bodies actually meet one another - SPEC
    /// section 6 makes predation emergent, and half a million ticks of the shipped world produced
    /// one devorocyte.
    ///
    /// ⚠️⚠️ **Corrected: this comment said "in contact with a stranger 13.5% of the time", and
    /// that figure does not reproduce.** Re-measured with `assay.rs`'s `what_a_mouth_meets` over
    /// 60,000 ticks at seed 42, this profile against the shipped world: the contact fraction goes
    /// **0.4723 to 0.5274**, a factor of **1.12** rather than 13.5% to 58.8%, and predation
    /// **x1.09** rather than eight times. The open-world reading is the reliable one, since
    /// 0.4723 reproduces to four decimals against
    /// `a_current_buys_strangers_by_spending_contact`'s committed assertion, so Phase 4's number
    /// is the wrong one. Bodies in the shipped world are in contact constantly; what they are in
    /// contact with is their own descendants.
    ///
    /// ⭐ **What density buys is the stranger share: 0.0004 to 0.0463, a factor of 116**, which
    /// is the statistic neither document quoted. It is still not enough to make a mouth pay - see
    /// `docs/NEXT.md` and `assay.rs`'s invasion analysis.
    ///
    /// Three claims, and each of them is a way the file could be quietly wrong.
    ///
    /// **It is the same energy.** Carrying capacity is proportional to `tiles × influx` - SPEC
    /// section 3's measured table - so a denser world is only a denser world if that product is
    /// held. Written as an equality between the two documents rather than as the number 36.864,
    /// so that retuning `light.influx` in `default.toml` moves both or fails here.
    ///
    /// **⚠️ The width is untouched.** SPEC section 8: springs are not found by the spatial hash
    /// and have no length limit, so one created across the seam works and hauls its two cells
    /// together the long way round. A body may hold `max_cells_per_organism` cells at up to
    /// `MAX_REST_LENGTH` apart, which is 870 units, and in a world narrower than about twice that
    /// a single chain reaches more than half way round it. Shrinking the height is what makes this
    /// profile safe; shrinking the width is what would make it a bug report months later.
    ///
    /// **And nothing else moved.** Every other setting is asserted equal to the shipped one,
    /// because a preset that quietly also changed the mutation rates would be an experiment with
    /// two variables in it and no way to tell which had done the work.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the two documents' numbers are pinned against each other; an approximate \
                  match would let a profile drift away from the one it is a variation on"
    )]
    fn the_dense_profile_is_the_shipped_world_with_less_water_in_it() {
        let dense: RawConfig = toml::from_str(include_str!("../../../config/dense.toml"))
            .expect("the dense profile parses");
        let shipped = coacervate_sim::config::spec_defaults();

        dense
            .clone()
            .validate()
            .expect("the dense profile is a world the program will accept");

        assert_eq!(
            dense.world.height * 4.0,
            shipped.world.height,
            "the dense profile holds {} of the shipped world's depth rather than a quarter",
            dense.world.height / shipped.world.height
        );
        assert_eq!(
            dense.world.grid_rows * 4,
            shipped.world.grid_rows,
            "the grid was not shrunk with the world, so a tile in the dense profile is not the \
             same 8 by 8 units of water it is everywhere else"
        );

        // ⚠️ The width, which is the one that matters. See the doc comment.
        assert_eq!(
            dense.world.width, shipped.world.width,
            "the dense profile is narrower than the shipped world. SPEC section 8 warns that a \
             spring has no length limit and is not found by the spatial hash, so a 64-cell body \
             at MAX_REST_LENGTH reaches 870 units - and in a world this narrow that is more than \
             half way round, where a spring hauls its cells together through the seam"
        );

        let energy = |raw: &RawConfig| {
            f64::from(raw.world.grid_cols) * f64::from(raw.world.grid_rows) * raw.light.influx
        };
        assert!(
            (energy(&dense) - energy(&shipped)).abs() < 1e-9,
            "the dense world is offered {} units of light a tick and the shipped one {}. A \
             profile that is denser *and* richer cannot say which of the two did anything",
            energy(&dense),
            energy(&shipped)
        );

        // And nothing else is different: the same document with `[world]` and `light.influx`
        // put back.
        let mut restored = dense;
        restored.world.height = shipped.world.height;
        restored.world.grid_rows = shipped.world.grid_rows;
        restored.light.influx = shipped.light.influx;
        assert_eq!(
            restored, shipped,
            "the dense profile changes something besides how much water there is and how bright \
             it is, so a run of it is an experiment with more than one variable in it"
        );
    }

    /// ⭐⭐⭐ The shipped documents carry a metabolic scaling exponent, the key is required, and
    /// **it ships inert**.
    ///
    /// The same three claims `the_shipped_documents_carry_a_season_and_it_ships_inert` makes, one
    /// table along, and the third is again the one that took the discipline.
    ///
    /// **The key is required**, with no `serde(default)`: a configuration that leaves something
    /// out is a configuration whose author did not decide that value, and SPEC section 13 wants a
    /// recording to carry the settings that produced it.
    ///
    /// **`config/kleiber.toml` is `config/default.toml` with one number changed.** Written as an
    /// equality between the two documents rather than as a list, so a profile that quietly also
    /// moved the mutation rates would be an experiment with two variables in it.
    ///
    /// **And the shipped default is one, which is exactly linear.** The multiplier is `n^0` —
    /// 1.0 for every body at every size, to the bit — so `config/default.toml` is still the world
    /// every coefficient in `SPEC.md` and every figure in `docs/PHASE7.md` was measured on, and
    /// no golden vector moves. `run.rs`'s
    /// `sub_linear_upkeep_ships_inert_and_is_a_law_the_world_can_feel` is the other half of that,
    /// taken on the world rather than on the document.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the two documents' numbers are pinned against each other; an approximate \
                  match would let a profile drift away from the one it is a variation on"
    )]
    fn the_shipped_documents_carry_a_scaling_exponent_and_it_ships_inert() {
        let kleiber: RawConfig = toml::from_str(include_str!("../../../config/kleiber.toml"))
            .expect("the kleiber profile parses");
        let shipped: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        kleiber
            .clone()
            .validate()
            .expect("the kleiber profile is a world the program will accept");

        assert_eq!(
            shipped.metabolism.scaling_exponent, 1.0,
            "the shipped world charges a body something other than the sum of its cells' \
             upkeeps, so it is no longer the world every figure in SPEC and docs/PHASE7.md was \
             measured on and every golden vector in the project has to be re-recorded"
        );
        assert_eq!(
            kleiber.metabolism.scaling_exponent, 0.75,
            "the kleiber profile does not carry the exponent the measurements were taken at"
        );

        // One number changed, and nothing else whatever.
        let mut restored = kleiber;
        restored.metabolism.scaling_exponent = shipped.metabolism.scaling_exponent;
        assert_eq!(
            restored, shipped,
            "the kleiber profile changes something besides how a body's upkeep scales with its \
             size, so a run of it is an experiment with more than one variable in it"
        );
    }

    /// ⭐⭐ **Phase 7's Group L.** The shipped documents carry a season, both keys are required,
    /// and the season **ships inert**.
    ///
    /// Three claims, and the third is the one that took the discipline.
    ///
    /// **The keys are required.** Neither has a `serde(default)`, which is this file's own rule
    /// applied to the newest setting: a configuration that leaves something out is a
    /// configuration whose author did not decide that value. A season that silently defaulted to
    /// absent would also be a run whose replay log did not describe it, and SPEC section 13 wants
    /// a recording to carry the settings that produced it.
    ///
    /// **`config/seasonal.toml` is `config/default.toml` with one number changed.** Written as an
    /// equality between the two documents rather than as a list, so a profile that quietly also
    /// moved the mutation rates would be an experiment with two variables in it and no way to
    /// tell which had done the work — the same claim `the_dense_profile_is_the_shipped_world…`
    /// makes about its own.
    ///
    /// **And the shipped default is nought.** Group H shipped a sevenfold change to every muscle
    /// in the world and could not afterwards separate *did I break anything* from *the world is
    /// now different*. Shipping the mechanism switched off means `config/default.toml` is
    /// bit-for-bit the world every figure in `docs/PHASE7.md` was measured on, six golden vectors
    /// do not have to be re-recorded, and the owner's varying world is one profile away.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the two documents' numbers are pinned against each other; an approximate \
                  match would let a profile drift away from the one it is a variation on"
    )]
    fn the_shipped_documents_carry_a_season_and_it_ships_inert() {
        let seasonal: RawConfig = toml::from_str(include_str!("../../../config/seasonal.toml"))
            .expect("the seasonal profile parses");
        let shipped: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        seasonal
            .clone()
            .validate()
            .expect("the seasonal profile is a world the program will accept");

        assert_eq!(
            shipped.light.season_amplitude, 0.0,
            "the shipped world has a season in it, so it is no longer the world every figure in \
             docs/PHASE7.md was measured on and six golden vectors have to be re-recorded"
        );
        assert_eq!(
            seasonal.light.season_amplitude, 0.25,
            "the seasonal profile does not carry the amplitude the measurements were taken at"
        );

        // One number changed, and nothing else whatever.
        let mut restored = seasonal;
        restored.light.season_amplitude = shipped.light.season_amplitude;
        assert_eq!(
            restored, shipped,
            "the seasonal profile changes something besides how deep the seasons run, so a run \
             of it is an experiment with more than one variable in it"
        );

        // Both keys are required. A document missing either is refused rather than guessed at.
        for missing in ["season_amplitude", "season_period"] {
            let without: String = DEFAULT_CONFIG
                .lines()
                .filter(|line| !line.trim_start().starts_with(missing))
                .collect::<Vec<&str>>()
                .join("\n");

            assert!(
                toml::from_str::<RawConfig>(&without).is_err(),
                "a document with no `light.{missing}` was accepted, so a run can be recorded \
                 without the season that produced it"
            );
        }
    }

    /// ⭐⭐ **`config/current.toml` is `config/default.toml` with one number changed, and the
    /// shipped default is nought.**
    ///
    /// The same claim `the_shipped_documents_carry_a_season_and_it_ships_inert` makes about the
    /// season, for the same two reasons. A profile that quietly also moved the mutation rates
    /// would be an experiment with two variables in it and no way to tell which had done the
    /// work; and shipping the mechanism switched off means `config/default.toml` is bit-for-bit
    /// the world every figure in `SPEC.md` and `docs/PHASE7.md` was measured on, so nothing has
    /// to be re-recorded and the moving world is one profile away.
    ///
    /// ⚠️ **The `current` profile is a recorded negative rather than a recommendation**, and
    /// the file says so at length. `assay.rs`'s `a_current_buys_strangers_by_spending_contact`
    /// swept eleven settings and found no value at which lineages mix without the population
    /// falling with them. What the profile is for is re-running that sweep rather than arguing
    /// about it.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the two documents' numbers are pinned against each other; an approximate \
                  match would let a profile drift away from the one it is a variation on"
    )]
    fn the_shipped_documents_carry_a_current_and_it_ships_inert() {
        let moving: RawConfig = toml::from_str(include_str!("../../../config/current.toml"))
            .expect("the current profile parses");
        let shipped: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        moving
            .clone()
            .validate()
            .expect("the current profile is a world the program will accept");

        assert_eq!(
            shipped.physics.current, 0.0,
            "the shipped world has a current running through it, so it is no longer the world \
             every figure in SPEC.md and docs/PHASE7.md was measured on"
        );
        assert_eq!(
            moving.physics.current, 600.0,
            "the current profile does not carry the value the sweep was read at"
        );

        // One number changed, and nothing else whatever.
        let mut restored = moving;
        restored.physics.current = shipped.physics.current;
        assert_eq!(
            restored, shipped,
            "the current profile changes something besides how hard the water is running, so a \
             run of it is an experiment with more than one variable in it"
        );

        // The key is required. A document missing it is refused rather than guessed at, because
        // a replay that did not record the current is a replay of a world nobody can rebuild.
        let without: String = DEFAULT_CONFIG
            .lines()
            .filter(|line| !line.trim_start().starts_with("current"))
            .collect::<Vec<&str>>()
            .join("\n");
        assert!(
            toml::from_str::<RawConfig>(&without).is_err(),
            "a document with no `physics.current` was accepted, so a run can be recorded \
             without the water that produced it"
        );
    }

    /// ⭐⭐ **The shadow's geometry is written in the document, and it ships at the geometry
    /// that was compiled in.**
    ///
    /// SPEC gives no occlusion model at all: `shadow_depth` and `shadow_spread` were both
    /// constants in `behaviour.rs` chosen in Phase 4, so making them settings is the document
    /// gaining something it never had rather than a deviation from it. What that buys is the
    /// only lever this project has found on the one strictly zero-sum resource in the world —
    /// a photon intercepted above a cell never reaches it — without paying the two thirds of
    /// the population that density costs. See `docs/NEXT.md` and SPEC section 6.
    ///
    /// ⚠️ **Both ship at exactly the old constants**, which is what keeps the blast radius at
    /// zero: `config/default.toml` is bit-for-bit the world every figure in SPEC and
    /// `docs/PHASE7.md` was measured on, and six golden vectors do not move. The same trade
    /// `light.season_amplitude` and `physics.current` both made. `run.rs`'s
    /// `the_shipped_shadow_is_the_shadow_that_was_there_before` is where that is proved against
    /// a whole population rather than against two literals.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "pinning the shipped geometry against the constants it replaced; an \
                  approximate match would let the shipped world drift"
    )]
    fn the_shipped_document_carries_a_shadow_and_it_ships_inert() {
        let shipped: RawConfig = toml::from_str(DEFAULT_CONFIG).expect("the shipped config parses");

        assert_eq!(
            shipped.light.shadow_depth, 27.2,
            "the shipped shadow no longer reaches 27.2 - `2 x genome::MAX_REST_LENGTH`, the \
             constant it replaced - so the world every figure in SPEC.md and docs/PHASE7.md was \
             measured on is not the world this document describes. **Investigate; do not paste \
             in the new number.**"
        );
        assert_eq!(
            shipped.light.shadow_spread, 0.0,
            "the shipped shadow is a cone rather than the column a disc casts in light falling \
             straight down, so occlusion is no longer what it was when every coefficient in this \
             project was taken"
        );

        // Both keys are required, for the reason `physics.current` is: a replay that did not
        // record the shape of the shadow is a replay of a world nobody can rebuild, and
        // occlusion is the one thing in this model that decides what a body's *shape* is worth.
        for key in ["shadow_depth", "shadow_spread"] {
            let without: String = DEFAULT_CONFIG
                .lines()
                .filter(|line| !line.trim_start().starts_with(key))
                .collect::<Vec<&str>>()
                .join("\n");
            assert!(
                toml::from_str::<RawConfig>(&without).is_err(),
                "a document with no `light.{key}` was accepted, so a run can be recorded \
                 without the occlusion that produced it"
            );
        }
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
