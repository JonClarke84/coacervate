//! The runner: the thing that ticks a world, and the thing that decides when to stop.
//!
//! `world.rs` says plainly what is missing and why: *"Nothing here decides when a run ends.
//! SPEC section 3's `max_wall_clock_hours` is a wall-clock bound, and a wall clock is exactly
//! what a deterministic simulation must not read."* `clippy.toml` refuses `Instant` and
//! `SystemTime` inside `coacervate-sim` outright, so the world genuinely cannot time itself.
//!
//! **That is why this file is in `coacervate-app` and not in the simulation.** The allowance
//! that lets it read a clock is a single line at the crate root of the binary - not a
//! `[lints]` table in this crate's manifest, because a package-level table *replaces* the
//! workspace one and would silently take the five cast lints with it. `clippy.toml` carries
//! the same warning.
//!
//! The division that follows is worth stating, because it is the whole reason the arrangement
//! is safe: **nothing in here can change what a run produces.** A `Run` calls `World::tick`
//! and reads the result. It cannot reach into the world, it holds no randomness, and the only
//! two things it does with the clock are decide when to stop and decide when to *wait*.
//! Waiting changes when a tick happens and not what it computes, which is SPEC section 2's
//! "real-time speed is decoupled" - and
//! `max_ticks_per_second_actually_slows_a_run` asserts exactly that by comparing the two
//! worlds tile for tile.
//!
//! # A run ends on whichever bound arrives first
//!
//! CLAUDE.md: *"Every run terminates on whichever comes first: the wall-clock bound, the
//! generation bound, or extinction. Shutdown is graceful - finish the tick, then exit."*
//!
//! Graceful is not a promise this file has to keep by being careful. **Every bound is examined
//! between two ticks and never inside one**, so there is no state a stop can catch halfway:
//! the last thing a stopping run did was finish a tick, books checked and all. A stop that
//! could arrive mid-tick would be a world saved with the light fallen and nothing fed, and
//! Phase 8's replay log is going to be written from exactly these moments.
//!
//! # The bound is on the world's own tick count, and there is only one clock
//!
//! `run.max_ticks` is compared against `World::ticks`, which counts every tick the world has
//! ever taken - the ones `founding.rs` spent letting the light fall included. The alternative
//! was a second counter belonging to the runner, and two counts of one thing is the
//! arrangement this project keeps refusing elsewhere. A run of five hundred thousand ticks is
//! five hundred thousand ticks of the *world*, which is also what SPEC section 2's deep-time
//! display reads off, and what an archived run would report.
//!
//! # Extinction is the population reaching nothing
//!
//! A `Run` expects to be handed a world with something alive in it; `founding.rs` is how one
//! is obtained. A world with nothing in it is therefore already over, and answers
//! [`Stop::Extinction`] before it takes a single tick. That is the honest answer rather than a
//! special case: there is no run to be had in an empty world, and the alternative - ticking an
//! empty world until the wall clock runs out - is twelve hours of watching water.
//!
//! ⚠️ **`run.reseed_on_extinction` is read by nothing.** SPEC section 3 has the key and says
//! nothing else about it, and Group D deliberately has not invented a meaning for it: putting a
//! second founding population into a world whose first one died is a statement about what a run
//! *is*, and it wants deciding on purpose rather than as a side-effect of writing the loop that
//! stops. Recorded in `docs/PHASE4.md` so it is not lost.
//!
//! # `Ctrl-C` is not what stops a run gracefully, and it cannot be
//!
//! CLAUDE.md asks for `Ctrl-C` to do what the bounds do. It cannot be made to, here, and the
//! reason is worth writing down rather than quietly doing something else.
//!
//! Catching a console interrupt on Windows means `SetConsoleCtrlHandler`, which is a foreign
//! function and therefore `unsafe` - forbidden at the root of every crate in this project, with
//! no exceptions - or the `ctrlc` crate, which is a new dependency. Both are refused, so what
//! is here instead is the *seam*: [`Interrupt`], a flag the runner reads between ticks exactly
//! as it reads its bounds. `main` sets it when a line arrives on standard input, so **pressing
//! Enter stops a run gracefully**; the tests set it from the watcher. Whatever eventually
//! wires a signal to a graceful stop - Phase 5 brings a window and an event loop with it - sets
//! this same flag and needs to change nothing here.
//!
//! Until then `Ctrl-C` kills the process where it stands, which is survivable precisely because
//! there is nothing on disk yet. The moment Phase 8's replay log exists that stops being true.

use coacervate_render::series::Series;
use coacervate_sim::chronicle::Chronicle;
use coacervate_sim::config::{Config, RunConfig};
use coacervate_sim::species::Taxonomy;
use coacervate_sim::world::World;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Why a run ended.
///
/// Named rather than returned as a flag, because these are four quite different things to
/// find in the morning. A run that reached its tick bound did what it was told; a run that ran
/// out of wall clock was cut short and there is more to see; a run that went extinct is a
/// result; and a run somebody stopped is not a finding at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// `run.max_wall_clock_hours` has passed.
    OutOfTime,

    /// The world has taken the `run.max_ticks` it was allowed.
    TicksDone,

    /// Nothing is alive.
    Extinction,

    /// Somebody asked it to stop. See [`Interrupt`].
    Asked,
}

/// A request that the run stop at the end of the tick it is in.
///
/// A flag and nothing else, shared between whoever asks and the loop that reads it. It is
/// atomic because the asking usually happens on another thread - in `main` it is a thread
/// waiting on standard input - and because a run that is asked to stop must stop, rather than
/// stopping if the compiler happens to have arranged for the loop to re-read the flag.
///
/// There is no way to un-ask. A run that has been told to stop is stopping.
#[derive(Clone, Default)]
pub struct Interrupt(Arc<AtomicBool>);

impl Interrupt {
    /// A fresh request that nobody has made yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask the run to stop at the end of the tick it is in.
    pub fn ask(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether anybody has asked.
    fn asked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A world, the bounds it runs under, and the loop between them.
pub struct Run {
    /// The world being ticked. Owned, because a run that could be handed a world somebody else
    /// was also ticking would not be reproducible.
    world: World,

    /// The moment after which the run is over, whatever else is true.
    deadline: Instant,

    /// The world's tick count at which the run is over, or nothing at all if it is to run
    /// until one of the other bounds arrives.
    last_tick: Option<u64>,

    /// How long a tick is allowed to take, or nothing at all if the run should go as fast as
    /// the machine will let it. SPEC section 3's `max_ticks_per_second`, turned into the
    /// interval it implies.
    interval: Option<Duration>,

    /// The earliest the next tick may start, when there is an interval to keep to.
    due: Instant,

    /// The flag somebody sets to stop the run.
    interrupt: Interrupt,

    /// Why the run ended, once it has.
    ///
    /// Kept because [`Run::step`] can be called by something that is not [`Run::go`] - a
    /// window's event loop, which cannot be handed a return value from a loop it does not own -
    /// and the reason a run stopped is news either way. `go` reads it from the same place.
    stopped: Option<Stop>,

    /// ⭐ **Phase 6, `C1`.** What the world has been doing, every hundred ticks.
    ///
    /// ⚠️ **It lives on the run and not on the panel**, and that is the only place it can live.
    /// SPEC section 13's records are on a grid of the world's own ticks, and the one thing in this
    /// program that knows when a tick happened is [`Run::step`] - a window draws about every
    /// eleventh tick and a headless run draws none at all, so a series sampled from anywhere else
    /// would be a different series in the two builds. It is also where Phase 8 will want it:
    /// `stats.bin` is a file in the *run's* directory.
    ///
    /// `coacervate-sim` still knows nothing about it. See `coacervate_render::series`, which
    /// explains why the type lives over there beside the census it is made of.
    series: Series,

    /// ⭐ **Phase 7, `A3` and `A4`.** Which lineages there are, every five hundred ticks.
    ///
    /// The second periodic observer of a run, and it is here for exactly the reason the series is:
    /// SPEC section 11's clustering is on a grid of the world's own ticks, and [`Run::step`] is
    /// the one place in this program a tick happens. An observer driven from a window would
    /// sample different ticks in the windowed build from the headless one, and a species is
    /// *defined* by having been there for twenty consecutive samples - so the two builds would
    /// disagree about what lives in the world rather than only about what is drawn.
    ///
    /// ⚠️ **It reads the world and changes nothing in it.** See `coacervate_sim::species`, and
    /// `a_run_produces_what_it_produced_before_group_a` below, which is what actually holds it to
    /// that.
    taxonomy: Taxonomy,

    /// ⭐ **Phase 7, Group C.** What has happened in this world, in sentences.
    ///
    /// The third periodic observer of a run, and it is here for the reason the other two are: an
    /// event happens *at a tick*, and [`Run::step`] is the one place in this program a tick
    /// happens. A log driven from a window would notice a different set of moments in the
    /// windowed build from the headless one - a window draws about every eleventh tick - so the
    /// two builds would end up disagreeing about what happened in the world rather than only
    /// about what was drawn.
    ///
    /// It is also where the *user's* half of the log has to be, because [`Run::retune`] is where
    /// a slider arrives. See there.
    ///
    /// ⚠️ **It reads the world and changes nothing in it.** See `coacervate_sim::chronicle`, and
    /// `the_chronicle_does_not_change_what_the_world_does` below, which is what holds it to that.
    chronicle: Chronicle,
}

impl Run {
    /// Take a world that is already alive, and the bounds it is to run under.
    ///
    /// The clock starts here rather than on the first tick, so `max_wall_clock_hours` measures
    /// the run and not the ticking.
    #[must_use]
    pub fn new(world: World, bounds: &RunConfig, interrupt: &Interrupt) -> Self {
        let now = Instant::now();
        let taxonomy = Taxonomy::new(world.config());
        let chronicle = Chronicle::new(world.config());

        Self {
            world,
            deadline: now + wall_clock(bounds.max_wall_clock_hours),
            last_tick: bounds.max_ticks,
            interval: bounds.max_ticks_per_second.map(|rate| {
                // A rate is a positive whole number of ticks per second, so the interval is a
                // positive fraction of a second and the division cannot go wrong.
                Duration::from_secs(1) / rate
            }),
            due: now,
            interrupt: interrupt.clone(),
            stopped: None,
            series: Series::new(),
            taxonomy,
            chronicle,
        }
    }

    /// Take one tick, or say why the run is over instead of taking one.
    ///
    /// **This is the whole of the loop, and [`Run::go`] is a `while let` around it.** Nothing
    /// else in this file decides when a tick happens.
    ///
    /// # Why the loop was turned inside out
    ///
    /// Phase 5 brings a window, and a window brings an event loop of somebody else's that
    /// insists on owning the outermost loop of the program - it wakes on a keystroke, on a
    /// resize, on the compositor asking for a frame, and it calls back into the application
    /// rather than being called by it. `go` cannot be used from inside one: it does not return
    /// until the run is over, so a windowed build calling it would show a frozen window for
    /// twelve hours and then exit.
    ///
    /// The obvious way out is to write the loop a second time inside the window's callback,
    /// which is the arrangement this project keeps refusing: **two loops over one simulation is
    /// two places for a bound to be examined**, and the day one of them gains a check the other
    /// does not is the day the windowed build and the headless build stop being the same
    /// program. Neither would report it, because nothing compares them.
    ///
    /// So there is one loop and it is here. A window's event loop calls this once per frame's
    /// worth of ticks and stops when it answers; `go` calls it until it answers. Everything
    /// either of them can say about bounds, pacing and graceful shutdown is said in this one
    /// function.
    ///
    /// # Nothing is taken when the answer is `Some`
    ///
    /// The bounds are examined first, so a run that is over does not take a further tick on its
    /// way out. That is the graceful-shutdown promise stated as a shape rather than as care:
    /// the last thing a stopping run did was finish a tick, books checked and all.
    pub fn step(&mut self) -> Option<Stop> {
        if let Some(why) = self.over() {
            self.stopped = Some(why);
            return Some(why);
        }

        self.wait();
        self.world.tick();

        // ⭐ **Phase 6's `C1` and Phase 7's `A3` and Group C.** The three lines that record the
        // run, on the one path a tick is taken by. Most of what they do is nothing at all: each
        // observer reads the world only when its own grid says to - a hundred ticks for the
        // chart, five hundred for the clustering, and for the log a walk of the population whose
        // every expensive part switches itself off once there is nothing new left to find.
        //
        // ⚠️ **The clustering goes first**, so a record taken at a tick that is on both grids
        // carries the count of species as of that tick rather than as of five hundred ticks ago.
        // Every five-hundredth tick is on both.
        //
        // ⚠️ **And the log goes last**, for the same reason turned round: a lineage promoted to a
        // species at this tick has to have been named by the time the log is asked whether
        // anything new is in the water, or the line announcing it would be a tick late for ever.
        self.taxonomy.observe(&self.world);
        self.series.observe(&self.world, &self.taxonomy);
        self.chronicle.observe(&self.world, &self.taxonomy);

        None
    }

    /// Whether a tick may be taken now, or whether the pacing is holding the run back.
    ///
    /// ⚠️ **Here so that a window never has to sleep inside its event loop.** [`Run::wait`]
    /// holds a run to `max_ticks_per_second` by sleeping, which is exactly right for a headless
    /// run with nothing else to do and exactly wrong for a windowed one: at a cap of ten ticks
    /// a second it would freeze the window for a tenth of a second at a time, and a window that
    /// does not answer the mouse is a window that has hung.
    ///
    /// So a caller that has something else to be doing asks this first and goes away and does
    /// it. The pacing is still decided here and in one place - all this says is *whether the
    /// next tick is due yet*, which is the same question [`Run::wait`] answers by waiting for
    /// the answer to become yes.
    #[must_use]
    pub fn due(&self) -> bool {
        self.interval.is_none_or(|_| Instant::now() >= self.due)
    }

    /// Ask this run to stop at the end of the tick it is in.
    ///
    /// The same request pressing Enter makes, made by whatever else has a reason to - a window
    /// being closed is the one Group C adds. There is no way to un-ask.
    pub fn ask_to_stop(&self) {
        self.interrupt.ask();
    }

    /// Why the run ended, or nothing at all if it has not.
    #[must_use]
    pub const fn stopped(&self) -> Option<Stop> {
        self.stopped
    }

    /// Tick until a bound arrives, showing `watch` the world after every tick.
    ///
    /// `watch` is how anything outside sees a run happen - it is what prints the progress line
    /// in `main` and what takes the readings in the tests below. It is handed the world after
    /// the tick rather than before, so what it sees is a world that has finished a tick, which
    /// is the same state a stop leaves behind.
    ///
    /// This is [`Run::step`] in a loop and nothing more. See that method for why the loop is
    /// written the other way up.
    pub fn go(&mut self, mut watch: impl FnMut(&World)) -> Stop {
        loop {
            if let Some(why) = self.step() {
                return why;
            }

            watch(&self.world);
        }
    }

    /// Whether the run is over, and why.
    ///
    /// Asked between ticks and never inside one, which is the whole of the graceful-shutdown
    /// promise. The order the four are examined in decides nothing - a run that is over for two
    /// reasons at once is over either way - so they are written in the order CLAUDE.md lists
    /// them, with the asking last because it is the one that is not a bound.
    fn over(&self) -> Option<Stop> {
        if Instant::now() >= self.deadline {
            return Some(Stop::OutOfTime);
        }

        if self
            .last_tick
            .is_some_and(|last| self.world.ticks() >= last)
        {
            return Some(Stop::TicksDone);
        }

        if self.world.organisms().iter().flatten().next().is_none() {
            return Some(Stop::Extinction);
        }

        if self.interrupt.asked() {
            return Some(Stop::Asked);
        }

        None
    }

    /// Hold the run back if it is going faster than `max_ticks_per_second` allows.
    ///
    /// The next tick is due at a fixed interval after the last one was, rather than an interval
    /// after this instant, so the sleeping does not accumulate the few hundred microseconds a
    /// tick actually takes into a rate that quietly drifts below the one that was asked for.
    ///
    /// A run that has fallen behind does not try to make the time back. If the machine cannot
    /// compute a tick inside the interval then the cap is not what is limiting the run, and
    /// bursting to catch up would produce exactly the thing the cap exists to prevent.
    fn wait(&mut self) {
        let Some(interval) = self.interval else {
            return;
        };

        let now = Instant::now();
        if self.due > now {
            std::thread::sleep(self.due - now);
            self.due += interval;
        } else {
            self.due = now + interval;
        }
    }

    /// The world, to read.
    #[must_use]
    pub fn world(&self) -> &World {
        &self.world
    }

    /// ⭐ **Phase 6, `C1`.** What the world has been doing, to chart.
    #[must_use]
    pub const fn series(&self) -> &Series {
        &self.series
    }

    /// ⭐ **Phase 7, `A3`.** Which lineages are in the world, to read.
    #[must_use]
    pub const fn taxonomy(&self) -> &Taxonomy {
        &self.taxonomy
    }

    /// ⭐ **Phase 7, Group C.** What has happened in this world, to read.
    #[must_use]
    pub const fn chronicle(&self) -> &Chronicle {
        &self.chronicle
    }

    /// ⭐ **Phase 6, `B1` and `B4`.** Go on under these conditions from here.
    ///
    /// SPEC section 3 divides its table into what locks at run start and what does not, and *"the
    /// rest can be changed live, which is how environmental events work"*. Two different things
    /// have to hear about a change and this is where they are kept together:
    ///
    /// - **The world**, for `[light]`, `[physics]`, `[metabolism]` and `[mutation]`.
    ///   `World::retune` walks every subsystem holding a copy of a number, and panics if
    ///   `[world]` or `[limits]` moved - the arenas were sized from those.
    /// - **The pacing**, for `run.max_ticks_per_second`, which is not a fact about the world at
    ///   all. It is what SPEC section 3 calls the `slow` profile's only lever, and it lives here
    ///   because a wall clock does.
    ///
    /// ⚠️ **The bounds are deliberately not re-read.** `max_wall_clock_hours` and `max_ticks` are
    /// the terms this run was *started* under - `--ticks` overrides one of them on the command
    /// line and `--dump-frame` supplies the other - and a deadline that could be dragged forward
    /// mid-run is a run whose closing report cannot say what it was bounded by. `panel.rs` offers
    /// neither.
    ///
    /// ⚠️ **The next tick is due immediately.** A run slowed from uncapped to ten a second would
    /// otherwise be held to whatever `due` happened to be left at, which for an uncapped run is
    /// the moment the run began - hours ago, so the first thing a newly-capped run would do is
    /// burst. See [`Run::wait`], which refuses to make time back for the same reason.
    pub fn retune(&mut self, config: &Config) {
        // ⭐ **Phase 7, `C8`.** SPEC section 3 calls a live setting change *"how environmental
        // events work"* and SPEC section 11 lists them among the things the log records, so this
        // is where the two sentences meet. Taken before the world is retuned, because afterwards
        // there is nothing left to compare against - `World::retune` replaces the configuration
        // the world was built with.
        let before = self.world.config().clone();
        self.chronicle.retuned(self.world.ticks(), &before, config);

        self.world.retune(config);
        self.interval = config
            .run
            .max_ticks_per_second
            .map(|rate| Duration::from_secs(1) / rate);
        self.due = Instant::now();
    }
}

/// The longest a run is allowed to take, as a duration.
///
/// Clamped at a century, which is not a limit anybody will meet and is not there for them. A
/// duration is counted in whole seconds and `config.rs` checks only that the hours are finite
/// and positive, so a configuration asking for `1e30` hours would otherwise take the
/// conversion past what a `Duration` can hold and stop the run at the moment it started - the
/// exact opposite of what was asked for.
fn wall_clock(hours: f32) -> Duration {
    const A_CENTURY: f64 = 100.0 * 365.25 * 24.0 * 3_600.0;

    Duration::from_secs_f64((f64::from(hours) * 3_600.0).min(A_CENTURY))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding::genesis;
    // ⚠️ `census` was this crate's own module until Phase 6. See `main.rs` for why it moved.
    use coacervate_render::census::Census;
    use coacervate_sim::config::{Config, RawConfig, RunConfig, spec_defaults};
    use coacervate_sim::species::{self, Taxonomy};
    use std::time::Instant;

    /// SPEC's default configuration with some of it changed, checked and ready to build a
    /// world from.
    ///
    /// The same fixture the simulation's own tests use, for the same reason: a test that
    /// talks about a bound is visibly not also quietly turning the light down.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// A world small enough that a test can afford to tick it, founded and alive.
    ///
    /// A sixteenth of the shipped world in each direction, with the population cap cut in the
    /// same proportion so that organisms per tile - which is what decides whether the arena or
    /// the energy budget binds first - is the shipped world's.
    ///
    /// The light is brighter than the shipped one, and only so that the dawn is short. Every
    /// test that uses this is about a **bound** - when a run stops, and how fast it is allowed
    /// to go - and none of them cares what the ecology inside does. What they do care about is
    /// that `founding.rs` has to let the field fill before it can seed anything, and at the
    /// shipped light that takes ten thousand ticks a world; eight of those, twice over in the
    /// two build profiles, is most of a minute of check suite spent watching water.
    fn a_small_living_world(change: impl FnOnce(&mut RawConfig)) -> World {
        let mut world = World::new(&a_small_config(change));

        genesis(&mut world, 8);

        world
    }

    /// The configuration [`a_small_living_world`] builds one from.
    ///
    /// Its own function since Phase 7, because `Run::retune` refuses a configuration whose
    /// `[world]` or `[limits]` differ from the ones the world was built with - so a test that
    /// changes a live setting on a small world has to be able to say *the same small world, with
    /// this one thing different*, and writing the six overrides out a second time is how the two
    /// come to disagree.
    fn a_small_config(change: impl FnOnce(&mut RawConfig)) -> Config {
        config(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 250;
            raw.light.influx = 0.012;
            change(raw);
        })
    }

    /// The bounds SPEC section 3 ships, with whatever this test is about changed.
    fn bounds(change: impl FnOnce(&mut RunConfig)) -> RunConfig {
        let mut run = RunConfig {
            max_wall_clock_hours: 12.0,
            max_ticks: None,
            max_ticks_per_second: None,
            reseed_on_extinction: false,
        };
        change(&mut run);

        run
    }

    /// The two exponents `sub_linear_scaling_raises_total_living_tissue` runs: linear, which is
    /// what ships, and Kleiber's three quarters, which is `config/kleiber.toml`.
    ///
    /// ⚠️ **The sweep in that test's doc comment has four rows and this has two**, for the
    /// reason `assay.rs`'s `a_current_buys_strangers_by_spending_contact` runs two of its
    /// eleven: the table is a measurement taken once, and the committed pair is what a check
    /// suite can afford to prove on every run.
    const EXPONENTS: [f64; 2] = [1.0, 0.75];

    /// How many ticks after the dawn the two arms are given before they are read.
    ///
    /// ⚠️ **Not 300,000, which is what the table in the doc comment was taken at.** Four runs
    /// of that length are an hour of arithmetic, and a suite that takes an hour is a suite
    /// somebody stops running — `metabolism.rs`'s own note about why the long tests are
    /// `#[ignore]`d, one order of magnitude up. Fifty thousand is where the *totals* have
    /// already separated by a third while the two worlds' mean body size is still identical to
    /// two decimal places, which makes it the cheapest tick count at which this test can fail
    /// for the right reason.
    const SETTLED: u64 = 50_000;

    /// How far the two sides of SPEC section 5's invariant have drifted apart, as a fraction
    /// of everything the world has ever contained.
    ///
    /// Written out from the specification rather than asked of the world, for the reason
    /// `world.rs`'s copy of this gives: `Ledger::check` answers "is this inside the
    /// tolerance", which is a yes or a no, and what a test wants to be able to say is *by how
    /// much*.
    fn relative_error(world: &World) -> f64 {
        let ledger = world.ledger();
        let held = world.grid().total_energy()
            + ledger.biomass()
            + ledger.detritus()
            + ledger.dissipated();
        let expected = ledger.initial_total() + ledger.influx_total();

        (held - expected).abs() / expected.abs().max(1.0)
    }

    /// ⭐ **D1.** A run ends on whichever of its bounds arrives first, and it ends between two
    /// ticks rather than inside one.
    ///
    /// CLAUDE.md: *"Every run terminates on whichever comes first: the wall-clock bound, the
    /// generation bound, or extinction. Shutdown is graceful - finish the tick, then exit."*
    ///
    /// # Four worlds, and each differs from the others in one thing
    ///
    /// The point of a test about "whichever comes first" is that the other bounds have to be
    /// *present and not reached*. So every run below is given the full set: a twelve-hour wall
    /// clock, a tick bound far past anything it will reach, a living population. Each then has
    /// exactly one of them brought within reach, and what is asserted is both the reason the
    /// run gives and the evidence for it - the tick count it stopped at, or the population it
    /// stopped with.
    ///
    /// Without the second half this would pass against a runner that always answered
    /// `TicksDone`: three of the four reasons would be wrong and one assertion each would
    /// catch it, but a runner that answered the *right* reason for the wrong cause would sail
    /// through. So the wall-clock run has to have stopped well short of its tick bound, the
    /// tick-bound run has to have stopped on the exact tick, and the extinct run has to have
    /// nothing alive in it.
    ///
    /// # What "graceful" is asserted as
    ///
    /// That the books balance at the moment the run stopped. A tick is the only thing in this
    /// project that moves energy, and it moves it in several stages - the light falls, the
    /// bodies feed, the dead are taken away - so a run stopped halfway through one would leave
    /// a world whose accounts describe a moment that never happened. Every stop below is
    /// checked for it, which is the observable form of "the last thing it did was finish a
    /// tick".
    #[test]
    fn a_run_stops_on_whichever_bound_comes_first() {
        // --- the tick bound, with hours and a population to spare ---
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 500)),
            &Interrupt::new(),
        );

        assert_eq!(run.go(|_| {}), Stop::TicksDone);
        assert_eq!(
            run.world().ticks(),
            founded + 500,
            "the run was allowed {} ticks and took {}",
            founded + 500,
            run.world().ticks()
        );
        assert!(
            run.world().organisms().iter().flatten().count() > 0,
            "the run stopped on its tick bound and everything in it was dead anyway, so this \
             case cannot tell the two bounds apart"
        );
        assert!(
            relative_error(run.world()) < 1e-6,
            "the run stopped with its books {} out, so it stopped somewhere inside a tick",
            relative_error(run.world())
        );

        // --- the wall clock, with ticks and a population to spare ---
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut run = Run::new(
            world,
            &bounds(|run| {
                run.max_wall_clock_hours = 0.1 / 3_600.0;
                run.max_ticks = Some(founded + 100_000_000);
            }),
            &Interrupt::new(),
        );

        let began = Instant::now();
        assert_eq!(run.go(|_| {}), Stop::OutOfTime);
        let took = began.elapsed();

        assert!(
            took < Duration::from_secs(10),
            "a run given a tenth of a second of wall clock ran for {took:?}"
        );
        assert!(
            run.world().ticks() > founded,
            "a run given a tenth of a second did not manage a single tick, so this case is \
             not testing that the run stopped - only that it never started"
        );
        assert!(
            run.world().ticks() < founded + 100_000_000,
            "the run reached its tick bound as well, so the wall clock is not what stopped it"
        );
        assert!(
            relative_error(run.world()) < 1e-6,
            "the run stopped somewhere inside a tick"
        );

        // --- extinction, with hours and ticks to spare ---
        //
        // The temperature is turned up until nothing can pay for itself: `metabolism.rs`
        // derives a lifespan from what a body costs to run, so a world this hot is one where
        // a founder dies of old age long before it has earned what SPEC section 10 asks of it
        // before it may breed. Group D measured that wall at an `upkeep_scale` of three.
        let world = a_small_living_world(|raw| raw.metabolism.upkeep_scale = 8.0);
        let founded = world.ticks();
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 100_000_000)),
            &Interrupt::new(),
        );

        assert_eq!(run.go(|_| {}), Stop::Extinction);
        assert_eq!(
            run.world().organisms().iter().flatten().count(),
            0,
            "the run stopped saying everything was dead and something is alive in it"
        );
        assert!(
            run.world().ticks() > founded,
            "the run reported extinction without taking a tick, so nothing died - it was \
             handed a world that was already empty"
        );
        assert!(
            run.world().ticks() < founded + 100_000_000,
            "the run reached its tick bound, so extinction is not what stopped it"
        );
        assert!(
            relative_error(run.world()) < 1e-6,
            "the run stopped somewhere inside a tick"
        );

        // --- somebody asking, with everything else to spare ---
        //
        // Asked from the watcher rather than from another thread, so the tick it is asked on
        // is a tick this test can name. The flag is the same flag `main` sets from a thread
        // waiting on standard input; what is being asserted here is when the loop reads it,
        // which is a claim about the loop and not about threads.
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let interrupt = Interrupt::new();
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 100_000_000)),
            &interrupt,
        );

        let asked_on = founded + 200;
        assert_eq!(
            run.go(|world| {
                if world.ticks() == asked_on {
                    interrupt.ask();
                }
            }),
            Stop::Asked
        );
        assert_eq!(
            run.world().ticks(),
            asked_on,
            "the run was asked to stop at the end of tick {asked_on} and finished on tick {}",
            run.world().ticks()
        );
        assert!(
            relative_error(run.world()) < 1e-6,
            "the run stopped somewhere inside a tick"
        );
    }

    /// ⭐ **A1.** A run can be taken one tick at a time, and a run taken that way is the same
    /// run.
    ///
    /// Phase 5 brings a window, and a window brings an event loop that insists on owning the
    /// program's outermost loop. [`Run::go`] owns one already. Two loops over one simulation is
    /// the arrangement where a bound gets checked in one of them and not the other, and where
    /// the windowed build and the headless build quietly stop being the same program - so
    /// there is one loop, [`Run::step`] is the body of it, and `go` is written on top.
    ///
    /// # What is actually asserted, and why the second half is the load-bearing one
    ///
    /// That a stepped run stops for the same reason on the same tick is the easy half, and it
    /// would pass against a `step` that had been written separately and happened to agree
    /// about the bounds. So the two worlds are compared **tile for tile and account for
    /// account**, exactly as `max_ticks_per_second_actually_slows_a_run` compares its pair: if
    /// the windowed build's world differed from the headless one by a single rounding, every
    /// frame Phase 5 dumps would be of a run that no recording could reproduce.
    ///
    /// # And that a step is one tick
    ///
    /// Counted, rather than assumed from the tick count at the end. A `step` that took two
    /// ticks, or that took none and left the loop to spin, would reach the same tick bound and
    /// stop for the same reason - and would make an event loop either twice as fast as it
    /// asked for or unable to make progress at all.
    ///
    /// The last case is the one an event loop meets first: a run that is **already over** must
    /// say so on the very first call, without taking a tick. That is `over` being asked
    /// between ticks rather than inside one, which is the whole of the graceful-shutdown
    /// promise, and it is what stops a window opening onto an empty world and ticking it for
    /// twelve hours.
    #[test]
    fn a_run_can_be_stepped_one_tick_at_a_time() {
        let allowance = 500;

        // The same run twice: once driven from outside a tick at a time, once told to get on
        // with it. Nothing else differs.
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut stepped = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + allowance)),
            &Interrupt::new(),
        );

        let mut steps = 0u64;
        let why = loop {
            if let Some(why) = stepped.step() {
                break why;
            }
            steps += 1;
        };

        let world = a_small_living_world(|_| {});
        let mut wholesale = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + allowance)),
            &Interrupt::new(),
        );
        let all_at_once = wholesale.go(|_| {});

        assert_eq!(why, Stop::TicksDone);
        assert_eq!(
            why, all_at_once,
            "the stepped run and the run that was left to itself stopped for different reasons"
        );
        assert_eq!(
            steps, allowance,
            "the run was allowed {allowance} ticks and {steps} calls to `step` took them, so a \
             step is not a tick and an event loop driving one would run at the wrong speed"
        );
        assert_eq!(
            stepped.world().ticks(),
            wholesale.world().ticks(),
            "the two runs did not stop on the same tick"
        );

        // ⭐ And they are the same world, not merely two worlds that stopped at the same
        // moment.
        assert_eq!(
            stepped
                .world()
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            wholesale
                .world()
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            "driving the run from outside changed the field, so the windowed build and the \
             headless build are not running the same simulation"
        );
        for (name, one, other) in [
            (
                "biomass",
                stepped.world().ledger().biomass(),
                wholesale.world().ledger().biomass(),
            ),
            (
                "detritus",
                stepped.world().ledger().detritus(),
                wholesale.world().ledger().detritus(),
            ),
            (
                "dissipated",
                stepped.world().ledger().dissipated(),
                wholesale.world().ledger().dissipated(),
            ),
            (
                "influx_total",
                stepped.world().ledger().influx_total(),
                wholesale.world().ledger().influx_total(),
            ),
        ] {
            assert!(
                one.to_bits() == other.to_bits(),
                "the stepped run left {one} in the {name} account against {other}"
            );
        }

        // A run that is over before it starts says so without taking a tick. An empty world is
        // already extinct - see this module's documentation - so this is the case a window
        // opened onto a dead world meets on its first frame.
        let empty = World::new(&config(|raw| {
            raw.world.width = 64.0;
            raw.world.height = 64.0;
            raw.world.grid_cols = 8;
            raw.world.grid_rows = 8;
        }));
        let before = empty.ticks();
        let mut over = Run::new(empty, &bounds(|_| {}), &Interrupt::new());

        assert_eq!(
            over.step(),
            Some(Stop::Extinction),
            "a run handed an empty world did not say so on its first step"
        );
        assert_eq!(
            over.world().ticks(),
            before,
            "a run that was already over took a tick anyway, so a bound is being examined \
             inside a tick rather than between two"
        );
    }

    /// ⭐ **Phase 7, `A3` at the runner's end.** The one loop that takes a tick is also what
    /// drives the clustering, and it drives it on SPEC section 11's grid.
    ///
    /// [`Run::step`] is the only place in this program a tick happens, which is what makes both
    /// periodic observers reproducible: a run watched through a window samples the same ticks as
    /// the same run headless, because neither of them is counting frames. This is that claim for
    /// the second observer, and it is worth its own test because the failure would be silent -
    /// a taxonomy that was never handed the world would simply report no species, for ever, in a
    /// world full of them.
    ///
    /// The last assertion is the seam between the two observers: the chart's record of how many
    /// species there are and the taxonomy's own count have to be one number. See `Run::step` for
    /// why the clustering is offered the world first.
    #[test]
    fn a_run_clusters_the_population_as_it_goes() {
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + species::EVERY * 2 + 1)),
            &Interrupt::new(),
        );

        assert_eq!(
            run.taxonomy().sampled_at(),
            None,
            "a run that has not taken a tick has already clustered something"
        );
        assert_eq!(run.go(|_| {}), Stop::TicksDone);

        let sampled = run
            .taxonomy()
            .sampled_at()
            .expect("a run of more than five hundred ticks has been clustered");
        assert!(
            sampled.is_multiple_of(species::EVERY),
            "the population was clustered at tick {sampled}, which is not on SPEC section 11's \
             {}-tick grid",
            species::EVERY
        );
        assert!(
            !run.taxonomy().clusters().is_empty(),
            "a run with a living population in it came out with no clusters at all"
        );

        assert_eq!(
            run.series()
                .samples()
                .last()
                .expect("a run of two thousand ticks has records")
                .species,
            run.taxonomy().species_count(),
            "the chart and the species list disagree about how many species there are"
        );
    }

    /// ⭐ **Phase 7, the cost of `A3`.** One clustering pass over a realistic population costs a
    /// small fraction of the five hundred ticks it sits between.
    ///
    /// SPEC section 11 asks for the living population to be clustered every 500 ticks, and
    /// CLAUDE.md allows up to 4,000 organisms. Compared pairwise that would be eight million
    /// distances a sample, each over two gene lists - which on this machine is seconds, at a
    /// cadence of well under a second. **A clustering pass that halved the tick rate would be a
    /// real regression on a live run**, so what it actually costs is measured rather than assumed.
    ///
    /// # What is compared, and why it is a ratio rather than a number
    ///
    /// The five hundred ticks between two samples, timed, against one sample taken over the same
    /// population. A wall-clock figure would be a number about the machine it was measured on and
    /// would fail on a slower one; the ratio is the thing that matters and is the thing a person
    /// running the simulation would notice.
    ///
    /// The bound is deliberately loose - a sample must cost less than the ticks it sits between,
    /// which is a hundred per cent of overhead - because the assertion is a guard against the
    /// *shape* of the cost changing, not a benchmark. `docs/PHASE7.md` records what it measured.
    ///
    /// ⚠️ Ignored in the debug suite. Reaching a realistic population means several thousand ticks
    /// of a full world, and a debug tick is roughly ten times a release one; the release pass runs
    /// this with `--include-ignored`.
    #[test]
    #[ignore = "several thousand ticks of a full world to reach a realistic population"]
    fn clustering_costs_little_beside_the_ticks_it_sits_between() {
        /// The population this is measured at. SPEC section 3's shipped world settles near 2,200.
        const REALISTIC: usize = 2_000;

        // ⚠️ **The shipped world's size, and not `a_small_living_world`'s.** Everything about the
        // cost of a clustering pass is a fact about how many organisms there are, and the small
        // world every other test in this file uses is a sixteenth of the area - it settles two
        // orders of magnitude short of the population this is about.
        //
        // The light is the one thing raised: `config/default.toml`'s own note records that at
        // 0.012 the population fills `limits.max_organisms` in twenty thousand ticks where the
        // shipped 0.001 takes ten times as long to settle. The population is the shipped one;
        // only the time taken to get there is not.
        let mut world = World::new(&config(|raw| raw.light.influx = 0.012));
        genesis(&mut world, 8);
        let mut taxonomy = Taxonomy::new(world.config());

        while Census::of(&world).population < REALISTIC {
            world.tick();
            assert!(
                world.ticks() < 60_000,
                "the world did not reach a population of {REALISTIC} in sixty thousand ticks, so \
                 this is not measuring what it says it is"
            );
        }

        // ⚠️ On the grid before anything is timed, or both samples below do nothing at all and
        // the measurement is of a function that returned immediately. Five hundred ticks from a
        // multiple of five hundred is another one.
        while !world.ticks().is_multiple_of(species::EVERY) {
            world.tick();
        }

        // Warm: the first sample mints every cluster there is, and it is the steady state this is
        // about rather than the first one.
        taxonomy.observe(&world);
        assert!(
            !taxonomy.clusters().is_empty(),
            "the warming sample found nothing, so what is timed below is a function that \
             returned without doing anything"
        );

        let ticking = Instant::now();
        for _ in 0..species::EVERY {
            world.tick();
        }
        let ticking = ticking.elapsed();

        let clustering = Instant::now();
        taxonomy.observe(&world);
        let clustering = clustering.elapsed();

        let census = Census::of(&world);
        println!(
            "population {}, mean genome {:.2} genes, {} clusters, {} species\n{} ticks took \
             {ticking:?}, one clustering pass took {clustering:?} - {:.2}% of the ticks it sits \
             between",
            census.population,
            census.mean_genes,
            taxonomy.clusters().len(),
            taxonomy.species_count(),
            species::EVERY,
            clustering.as_secs_f64() / ticking.as_secs_f64() * 100.0
        );

        assert!(
            clustering < ticking,
            "one clustering pass over {} organisms took {clustering:?} and the {} ticks it sits \
             between took {ticking:?}, so watching the run is costing more than the run",
            census.population,
            species::EVERY
        );
    }

    /// ⭐ **Phase 7, Group C.** A real run writes its own history, on the one path a tick is
    /// taken by.
    ///
    /// Everything about *what* the log notices is `chronicle.rs`'s and is tested there. What can
    /// only be tested here is that the observer is actually in the loop and is handed the world
    /// after the tick and after the clustering - which is the mistake this is for. A `Chronicle`
    /// built, held and never shown a world would report an empty log for the length of an
    /// overnight run, and nothing anywhere would say so.
    #[test]
    fn a_run_writes_its_own_history() {
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 1_000)),
            &Interrupt::new(),
        );

        assert_eq!(run.go(|_| {}), Stop::TicksDone);

        let log = run.chronicle();
        let said = log
            .events()
            .map(coacervate_sim::chronicle::Event::line)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            log.events().len() >= 2,
            "a thousand ticks of a living world produced {} events:\n{said}",
            log.events().len()
        );

        // ⭐ Every founder is two cells sprung together, so the origin of multicellularity in
        // this run is its first tick - and the log has to say so with a tick on it.
        let adhesion = log
            .events()
            .find(|event| event.kind == coacervate_sim::chronicle::Kind::Adhesion)
            .unwrap_or_else(|| {
                panic!(
                    "the founders are two-celled bodies and nothing recorded an adhesion:\n{said}"
                )
            });
        assert!(
            adhesion.tick > founded && adhesion.tick <= founded + 1_000,
            "the adhesion was recorded at tick {}, and the run covered {founded} to {}",
            adhesion.tick,
            founded + 1_000
        );

        // ⭐ **C8.** And a slider a person moved is an event at the tick they moved it.
        let at = run.world().ticks();
        let before = log.events().len();
        run.retune(&a_small_config(|raw| raw.metabolism.upkeep_scale = 2.5));

        let conditions: Vec<_> = run
            .chronicle()
            .events()
            .skip(before)
            .filter(|event| event.kind == coacervate_sim::chronicle::Kind::Conditions)
            .collect();
        assert_eq!(
            conditions.len(),
            1,
            "one setting was changed by hand and the log recorded {} environmental events",
            conditions.len()
        );
        assert_eq!(
            conditions[0].tick, at,
            "the change was recorded at a different tick from the one it was made on"
        );
        assert!(
            conditions[0]
                .said
                .contains("costs a cell simply to be alive"),
            "the log does not say which condition was changed: {}",
            conditions[0].said
        );
    }

    // -------------------------------------------------------------------------------------
    // A golden vector, kept apart from the red-then-green tests above
    //
    // `docs/PHASE1.md` sets the rule for these and it is worth repeating where one lives:
    // **if this ever fails, investigate - do not paste in the new numbers.** A golden vector
    // is not a test of the code that produced it. It is a record of what this program did on
    // a day somebody checked, and the only thing it can tell you is that today's program does
    // something different.
    // -------------------------------------------------------------------------------------

    /// Where every cell in the world is, as one number.
    ///
    /// An FNV-1a hash over the bit patterns of every living cell's position, walked in slot
    /// order. Bit patterns rather than the numbers themselves, for the reason `world.rs`'s
    /// `every_number_in` gives about its own comparisons: two runs that are supposed to agree
    /// exactly are not helped by a tolerance, and a difference in the last place is precisely
    /// how a determinism failure starts.
    ///
    /// A hash rather than the whole list because the whole list is a quarter of a million
    /// numbers and what a golden vector needs is something a person can write down.
    fn where_everybody_is(world: &World) -> u64 {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;

        let mut hash = OFFSET;
        for slot in 0..world.organisms().len() {
            for cell in world.cells_of(slot) {
                for bits in [cell.pos.x.to_bits(), cell.pos.y.to_bits()] {
                    hash ^= u64::from(bits);
                    hash = hash.wrapping_mul(PRIME);
                }
            }
        }

        hash
    }

    /// ⭐⭐ **A world with no current is the world that was there before one existed** — the
    /// same population, the same cells, and every one of them in exactly the same place.
    ///
    /// **This is what keeps the blast radius of `physics.current` at zero.** Every coefficient
    /// in `SPEC.md`, every figure in `docs/PHASE7.md` and the golden vector directly below this
    /// one were all measured on a world with no current in it. Shipping the mechanism switched
    /// off means that world is still the world `config/default.toml` describes, to the bit, and
    /// none of those readings has to be taken again. It is the same trade
    /// `light.season_amplitude` made — see `grid.rs`'s
    /// `a_world_with_no_season_is_the_world_that_was_there_before`, and `main.rs`'s
    /// `the_shipped_documents_carry_a_season_and_it_ships_inert`.
    ///
    /// The digest below was recorded from a build in which `physics.current` was a
    /// configuration key and **nothing read it**, which is the only moment at which "the world
    /// that was there before" can be measured rather than argued.
    ///
    /// ⚠️ **The second half is the one that makes the first half mean anything.** A test that
    /// only pinned a digest would pass just as happily against a program in which the setting
    /// is inert at *every* value — a config key nobody can tell is there — which is the failure
    /// Group H's own headline change spent a whole round hiding behind. So the same world is
    /// run again with the current turned on, and it has to come back somewhere else.
    #[test]
    #[ignore = "two shipped-world dawns and ten thousand ticks; check.ps1 runs it in release"]
    fn a_world_with_no_current_is_the_world_that_was_there_before() {
        let ran = |current: f64| {
            let mut world = World::new(&config(|raw| raw.physics.current = current));
            genesis(&mut world, 8);
            for _ in 0..5_000 {
                world.tick();
            }

            (
                world.organisms().iter().flatten().count(),
                world.living_cells().len(),
                where_everybody_is(&world),
            )
        };

        assert_eq!(
            ran(0.0),
            // 318 organisms alive holding 631 cells between them, five thousand ticks after a
            // shipped-world dawn founded with eight bodies, and every one of those cells in the
            // place this digest describes.
            (318, 631, 0x35fe_af96_9d17_98e0),
            "a shipped world with `physics.current = 0.0` is no longer the world this project \
             has been measuring all along. **Investigate; do not paste in the new numbers.** \
             Every selection coefficient in SPEC and every figure in docs/PHASE7.md was taken \
             on the world this digest describes"
        );

        // ⭐ And the current is a real force rather than a key nobody reads.
        assert_ne!(
            ran(0.6).2,
            ran(0.0).2,
            "the same world run with the current on and the current off left every cell in \
             exactly the same place, so `physics.current` is a setting a person can turn and \
             a world that cannot feel it"
        );
    }

    /// ⭐⭐ **A world with the shipped shadow is the world that was there before the shadow was
    /// a setting** — the same population, the same cells, and every one of them in exactly the
    /// same place.
    ///
    /// `light.shadow_depth` and `light.shadow_spread` were `behaviour.rs` constants until
    /// Phase 7's Group N, at `2 × MAX_REST_LENGTH` and at a shadow exactly as wide as the cell
    /// casting it. They ship at those two values, and the digest below is the same digest
    /// `a_world_with_no_current_is_the_world_that_was_there_before` pins — **recorded before
    /// either was a key, and asserted here unchanged.** That is the whole of the claim: no
    /// coefficient in `SPEC.md`, no figure in `docs/PHASE7.md` and no golden vector anywhere
    /// has to be taken again.
    ///
    /// ⚠️ **The second half is the one that makes the first half mean anything**, exactly as it
    /// is for the current: a test that only pinned a digest would pass just as happily against
    /// a program in which the setting is inert at *every* value. So the same world is run again
    /// with the shadow opened out, and it has to come back somewhere else — once for the depth
    /// and once for the width, because they are two mechanisms and a single reading could not
    /// tell which of them was doing the work.
    #[test]
    #[ignore = "three shipped-world dawns and fifteen thousand ticks; check.ps1 runs it in release"]
    fn the_shipped_shadow_is_the_shadow_that_was_there_before() {
        let ran = |change: fn(&mut RawConfig)| {
            let mut world = World::new(&config(change));
            genesis(&mut world, 8);
            for _ in 0..5_000 {
                world.tick();
            }

            (
                world.organisms().iter().flatten().count(),
                world.living_cells().len(),
                where_everybody_is(&world),
            )
        };

        assert_eq!(
            ran(|_| {}),
            // The same three numbers `a_world_with_no_current_is_the_world_that_was_there_before`
            // records, and they were recorded before the shadow was a configuration key.
            (318, 631, 0x35fe_af96_9d17_98e0),
            "a shipped world with `light.shadow_depth = 27.2` and `light.shadow_spread = 0.0` \
             is no longer the world this project has been measuring all along. **Investigate; \
             do not paste in the new numbers.** Those two values are the constants that were \
             compiled into `behaviour.rs`, so this digest moving means the arithmetic moved \
             with them rather than around them"
        );

        // ⭐ And neither of them is a key nobody reads.
        assert_ne!(
            ran(|raw| raw.light.shadow_depth = 54.4).2,
            ran(|_| {}).2,
            "the same world run at twice the shadow depth left every cell in exactly the same \
             place, so `light.shadow_depth` is a setting a person can turn and a world that \
             cannot feel it"
        );
        assert_ne!(
            ran(|raw| raw.light.shadow_spread = 1.0).2,
            ran(|_| {}).2,
            "the same world run with the shadow opened out into a cone left every cell in \
             exactly the same place, so `light.shadow_spread` is a setting a person can turn \
             and a world that cannot feel it"
        );
    }

    /// ⭐⭐⭐ **A world at `metabolism.scaling_exponent = 1.0` is the world that was there
    /// before sub-linear metabolism existed** — the same population, the same cells, and every
    /// one of them in exactly the same place.
    ///
    /// **This is what keeps the blast radius of the exponent at zero.** SPEC section 10 now
    /// charges a body `(sum of its cells' upkeeps) × n^(k − 1)`, and at `k = 1.0` that
    /// multiplier is `n^0` — exactly 1.0 for every body at every size, in this arithmetic and
    /// not merely to a tolerance. So `config/default.toml` still describes the world every
    /// coefficient in `SPEC.md` and every figure in `docs/PHASE7.md` was measured on, and the
    /// digest below is the one
    /// `a_world_with_no_current_is_the_world_that_was_there_before` and
    /// `the_shipped_shadow_is_the_shadow_that_was_there_before` already carry, unchanged. It is
    /// the same trade `light.season_amplitude`, `physics.current` and `light.shadow_spread` all
    /// made before it.
    ///
    /// ⚠️ **The second half is what makes the first half mean anything.** A test that only
    /// pinned a digest would pass just as happily against a program in which the setting is
    /// inert at *every* value — a configuration key nobody reads, which is the failure Group H
    /// spent a whole round hiding behind. So the same world is run again at three quarters, and
    /// it has to come back somewhere else.
    #[test]
    #[ignore = "two shipped-world dawns and ten thousand ticks; check.ps1 runs it in release"]
    fn sub_linear_upkeep_ships_inert_and_is_a_law_the_world_can_feel() {
        let ran = |exponent: f64| {
            let mut world = World::new(&config(|raw| raw.metabolism.scaling_exponent = exponent));
            genesis(&mut world, 8);
            for _ in 0..5_000 {
                world.tick();
            }

            (
                world.organisms().iter().flatten().count(),
                world.living_cells().len(),
                where_everybody_is(&world),
            )
        };

        assert_eq!(
            ran(1.0),
            // The same three numbers the two golden vectors above record, and they were
            // recorded before a body's size had any say in what its tissue cost.
            (318, 631, 0x35fe_af96_9d17_98e0),
            "a shipped world with `metabolism.scaling_exponent = 1.0` is no longer the world \
             this project has been measuring all along. **Investigate; do not paste in the new \
             numbers.** One is exactly linear, so the only thing that can have moved this is the \
             multiplication itself - which means the arithmetic went around SPEC section 10's \
             upkeep rather than through it"
        );

        // ⭐ And the exponent is a real law rather than a key nobody reads.
        assert_ne!(
            ran(0.75).2,
            ran(1.0).2,
            "the same world run at Kleiber's three quarters and at one left every cell in \
             exactly the same place, so `metabolism.scaling_exponent` is a setting a person can \
             turn and a world that cannot feel it"
        );
    }

    /// ⭐⭐⭐ **Sub-linear metabolism raises the total amount of living tissue in the world.**
    ///
    /// **This is the claim ten rounds of measurement have been unable to make, and it is the
    /// reason the mechanism exists.** Every "bodies got bigger" result this project has recorded
    /// was the same tissue divided among fewer bodies — four worlds measured in one week held
    /// 5,429 / 5,153 / 4,826 / 4,056 living cells while biomass went from 34,000 to 25,000 — and
    /// the arithmetic said it could not be otherwise. Biomass settles where income equals
    /// expenditure, income is fixed at `light.influx`, and upkeep was charged **linearly in cell
    /// count**, so total cells is `influx ÷ upkeep per cell` and nothing about body shape can
    /// move it. Bending upkeep to `n^(k − 1)` makes upkeep per cell a function of body size,
    /// which is the one thing that removes that constant from the denominator.
    ///
    /// ⚠️ **The total is the reading. Every quotient is reported beside it and none of them is
    /// the claim**, because a body size is `cells ÷ bodies` and has moved before without
    /// meaning anything.
    ///
    /// # The measurement: 300,000 ticks after the dawn, eight founders, seed 42
    ///
    /// ⚠️ **Taken once, as recorded.** The committed run above is [`EXPONENTS`] at
    /// [`SETTLED`] — see those two for why. The four rows below are the shipped configuration
    /// with one number changed:
    ///
    /// | `scaling_exponent` | **total living cells** | mean cells/body | biggest | at the cap | alive | biomass | field drawdown | genome | peak alive |
    /// | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
    /// | **1.00** — ships | **5,282** | 9.60 ± 3.02 | 17 | 0.00% | 550 | 33,293 | 64.7% | 5.95 | 2,164 |
    /// | 0.90 | **6,646** | 7.62 ± 4.35 | 64 | 0.11% | 872 | 40,874 | 66.3% | 4.47 | 2,426 |
    /// | **0.75** — `config/kleiber.toml` | **10,444** | 12.01 ± 11.33 | 64 | **3.34%** | 869 | 65,714 | 77.6% | 4.90 | 2,764 |
    /// | 0.60 | **25,551** | 41.01 ± 22.17 | 64 | **28.41%** | 623 | 152,641 | 88.0% | 4.21 | 2,987 |
    ///
    /// ⭐⭐ **Total living tissue rises, monotonically, by a factor of 4.8 across the range** —
    /// and it rises while the population also rises, so it is not concentration. Biomass follows
    /// it exactly (×4.6) and the field is drawn down from 64.7% to 88.0%, which is the same fact
    /// seen from the water: a world of larger bodies extracts more of the light before it spills.
    ///
    /// ⚠️⚠️ **And the runaway is real below three quarters.** At 0.60 more than a quarter of
    /// every body in the world is pressed against `limits.max_cells_per_organism`, mean body size
    /// is 41 cells of an allowed 64, and the reading is a world running into its own arena rather
    /// than one finding a size. Three quarters is where the mechanism gives most of its effect
    /// with the cap still a rarity — and it is not a number chosen to make that true, it is
    /// Kleiber's own exponent, which is why `config/kleiber.toml` carries it.
    ///
    /// The world survives all four: nothing goes extinct, the peak population never reaches
    /// `limits.max_organisms` — 2,164 to 2,987 against 4,000, so the energy budget is still what
    /// binds — and the energy ledger closes to better than 1.4 parts in a thousand million
    /// throughout.
    #[test]
    #[ignore = "two 50,000-tick runs of the shipped world; check.ps1 runs it in release"]
    fn sub_linear_scaling_raises_total_living_tissue() {
        let mut tissue = Vec::new();

        for exponent in EXPONENTS {
            let mut world = World::new(&config(|raw| raw.metabolism.scaling_exponent = exponent));
            genesis(&mut world, 8);
            let lit = world.grid().total_energy();
            let cap = usize::try_from(world.config().limits.max_cells_per_organism.get())
                .expect("a body cap fits in a machine word");
            let room = usize::try_from(world.config().limits.max_organisms.get())
                .expect("a population cap fits in a machine word");

            let mut peak = 0usize;
            for _ in 0..SETTLED {
                world.tick();
                peak = peak.max(world.organisms().iter().flatten().count());
            }

            let census = Census::of(&world);
            let sizes: Vec<usize> = world
                .organisms()
                .iter()
                .flatten()
                .map(coacervate_sim::organism::Organism::cells)
                .collect();
            let biggest = sizes.iter().copied().max().unwrap_or(0);
            let at_cap = sizes.iter().filter(|&&size| size >= cap).count();
            let field = world.grid().total_energy();
            let cells = world.living_cells().len();

            #[expect(
                clippy::cast_precision_loss,
                reason = "a count of bodies in a population of at most four thousand, turned \
                          into a fraction for a person to read"
            )]
            let crowded = at_cap as f64 / sizes.len().max(1) as f64;

            println!(
                "SCALING k={exponent:.2}: cells {cells}, body {:.3}+-{:.3}, biggest {biggest}, \
                 at cap {crowded:.4}, alive {}, biomass {:.0}, field {field:.0} \
                 (drawdown {:.1}%), genome {:.3}, peak {peak}/{room}, ledger {:.2e}",
                census.mean_cells,
                census.cell_spread,
                census.population,
                world.ledger().biomass(),
                100.0 * (1.0 - field / lit),
                census.mean_genes,
                relative_error(&world)
            );

            // The world has to still be a world for the reading to mean anything.
            assert!(
                census.population > 0,
                "the world at an exponent of {exponent} is empty"
            );
            assert!(
                peak < room,
                "the population reached {peak} against a `limits.max_organisms` of {room} at an \
                 exponent of {exponent}, so the arena is what limits this world rather than the \
                 energy budget - and every birth that failed did so for a reason unconnected \
                 with how well its parent was doing"
            );
            assert!(
                relative_error(&world) < 1e-8,
                "the run at an exponent of {exponent} finished {} out in relative terms",
                relative_error(&world)
            );

            tissue.push(cells);
        }

        // ⭐⭐⭐ The whole question. Measured at 3,865 living cells against 5,159 - a third
        // more - so a fifth is a wide margin around a large effect rather than a threshold
        // anything sits near.
        let (linear, bent) = (tissue[0], tissue[1]);
        assert!(
            bent > linear * 6 / 5,
            "the linear world holds {linear} living cells and the world at three quarters holds \
             {bent}, which is not the fifth more the mechanism was built to produce. **Total \
             living tissue is the reading**: a mean body size that rose while this did not is \
             the same tissue in fewer bodies, which is what every round before this one \
             measured"
        );
    }

    /// ⭐ **A5 group control.** A run of a fixed seed and configuration produces exactly what
    /// it produced before Phase 5 touched anything.
    ///
    /// Every accessor Group A adds is a way of *reading* the simulation, and the whole claim
    /// of the group is that reading changes nothing. That is easy to say and easy to break in
    /// a way nothing announces: an organism gained two fields, a hash is taken at every birth,
    /// and a run that ticked in a slightly different order or drew one extra random number
    /// would still be perfectly deterministic - just deterministically *different*, so that
    /// every reading in `docs/PHASE4.md` and every future recording made before today would
    /// quietly stop being reproducible.
    ///
    /// The numbers below are the bit patterns of the five quantities that summarise a run, plus
    /// the three counts. Bit patterns rather than the numbers themselves for the reason
    /// `world.rs` gives about its own comparisons: a tolerance would wave through a difference
    /// in the last place, which is exactly how a determinism failure starts.
    ///
    /// # ⚠️ **They have been re-recorded twice, and both times it was the one time it is allowed**
    ///
    /// The header above says not to paste in new numbers, and this is the exception it exists
    /// to be argued with.
    ///
    /// **Phase 7, Group G** changed three more things about what the simulation does, on
    /// purpose and each measured: `light.patchiness` from 0.15 to 0.5, a new `light.patch_drift`
    /// of 0.0006 that slides the field sideways for ever, and `CellKind::buoyancy`, which puts
    /// a standing external force on every cell in the world. Any one of those moves every draw
    /// in every stream after the first birth.
    ///
    /// **Phase 7, Group F** changed three before that: the water became anisotropic
    /// (`physics.drag_anisotropy`, defaulting to 2.0, so a body can swim at all),
    /// `metabolism.movement_cost` went from 0.15 to 0.0001, and `MAX_SENSOR_GAIN` went from 1
    /// to 8.
    ///
    /// A vector that survived either of those would have meant the changes had not landed.
    ///
    /// **What it was, through Group F**, so that a recording made between then and now can
    /// still be identified:
    ///
    /// ```text
    /// 4,000 ticks, 510 born, 472 alive
    /// field       0x40c5_00ee_58ab_0000   10,753.862
    /// biomass     0x40b9_e2c0_2eb4_762c    6,626.751
    /// detritus    0x406b_362a_1e42_a7ea      217.693
    /// dissipated  0x40b6_2fe3_11a1_7494    5,679.887
    /// influx      0x40d6_bb8c_50c7_2000   23,278.192
    /// ```
    ///
    /// **And at the end of Phase 4 and through Phases 5 and 6:**
    ///
    /// ```text
    /// 4,000 ticks, 508 born, 470 alive
    /// field       0x40c5_012f_1d23_0000   10,754.368
    /// biomass     0x40b9_dfb9_aadf_208c    6,623.725
    /// detritus    0x406b_2de6_28e9_041e      217.434
    /// dissipated  0x40b6_30de_0509_9754    5,680.867
    /// influx      0x40d6_bb19_4711_2000   23,276.395
    /// ```
    ///
    /// The world Group G describes is a **richer** one rather than merely a different one, and
    /// that is worth reading off the numbers rather than only noticing that they moved.
    /// Fourteen per cent more organisms have been born, the field holds 0.7% more and the
    /// living hold 18% more, and 16% more light has fallen. All of that is `light.patchiness`:
    /// the blotches are three times as deep, so the good tiles are much better than they were
    /// while the bad ones are worse, and a body standing on a good one earns more than the
    /// same body on an average one used to. The extra light is the same fact seen from the
    /// other side - a world of deeper blotches takes longer to fill, so more of what falls on
    /// it is still being absorbed at tick 4,000 rather than spilling straight back out.
    ///
    /// `docs/PHASE4.md`'s figures were measured on the oldest of the three.
    ///
    /// # ⭐⭐ Group H did **not** move it, and that is a result rather than a relief
    ///
    /// Phase 7's Group H multiplied the swing of every muscle in the world by nearly seven, and
    /// this vector is unchanged to the last bit. It is the first change in this project not to
    /// move it, and the reason is the whole of Group H's finding: **in four thousand ticks not
    /// one muscle anywhere fires.** A myocyte is only heard if some gene in its own genome names
    /// its `state` as a `trigger_state`, a state is one of sixty-four, and a young genome holds
    /// about three genes - almost all of which still trigger on the founder's state 0 while
    /// development scatters daughters across the whole range. Measured over 120,000 ticks of the
    /// shipped world: 56,903 spring-ticks with a myocyte on one end and no gene answering it,
    /// 874 with a gene whose `osc_freq` and `osc_phase` are both still exactly nought, and
    /// **nought in which a spring's rest length moved at all**.
    ///
    /// So a golden vector that survived a change this large is not a change that failed to
    /// land. It is the cheapest possible statement that the code path was never taken - and it
    /// was the first sign of where the real problem is. See `docs/PHASE7.md`, Group H.
    ///
    /// # ⚠️ Group I did not move it either, and the reason is a different one worth keeping
    ///
    /// Group I is the fix to Group H's finding: a cell now takes its behaviour from **the gene
    /// that built it** rather than from a gene looked up by its `state`, so every myocyte in
    /// the world is wired to a gene instead of one in fifty. This vector is still unchanged to
    /// the last bit, and this time it is **not** because the path is unreachable.
    ///
    /// It is because a myocyte needs *two* mutations before it moves anything, and 4,000 ticks
    /// is not long enough for the second. `mutation.rs` perturbs **one field of a hit gene**,
    /// so the mutation that makes a gene's `child_kind` a myocyte leaves that gene's `osc_freq`
    /// at whatever it was - and the founder's is nought. A myocyte therefore arrives silent and
    /// stays silent until the *same gene* is hit again on `osc_freq` or `osc_phase`. What Group
    /// I changed is that both of those now land on one gene in a genome of three or four,
    /// instead of one having to land on a state in a space of sixty-four; the same run measured
    /// to 313,000 ticks diverges from its own baseline at about tick 50,000 and ends with a
    /// different world. See `docs/PHASE7.md`, Group I.
    ///
    /// # ⭐⭐ Group J moved it, and it is the first change since Group F to do so
    ///
    /// Group J biases the distribution a re-drawn `trigger_state`, `child_state` or `new_state`
    /// comes from — towards the states some cell is in for the first, away from them for the other
    /// two. It changes the number of draws a point mutation takes as well as where they land, so
    /// every stream in the world parts company with its old self at the first point mutation of
    /// the first birth. **A vector that survived it would have meant the operator was not being
    /// reached**, which is exactly the reading Groups H and I had to give.
    ///
    /// **What it was through Groups G, H and I:**
    ///
    /// ```text
    /// 4,000 ticks, 583 born, 541 alive
    /// field       0x40c5_2762_ae79_0000   10,830.771
    /// biomass     0x40be_9540_3bda_7bce    7,829.251
    /// detritus    0x406f_61db_e12c_9ca0      251.058
    /// dissipated  0x40bf_d11c_ba5a_1f50    8,145.112
    /// influx      0x40da_6c0c_4c88_2000   27,056.192
    /// ```
    ///
    /// The world it describes is very nearly the same one: one more organism born, one more alive,
    /// and every account inside 4% of what it was. That is what a change to *where a state lands*
    /// ought to look like at four thousand ticks — the genomes here hold one or two genes and have
    /// had a few hundred point mutations between them, so almost nothing has yet been addressed
    /// differently. What it does over three hundred thousand ticks is in `docs/PHASE7.md`.
    ///
    /// # ⭐⭐ Group K moved exactly two of the eight numbers, by exactly the same amount
    ///
    /// A myocyte's upkeep went from 0.014 to 0.005 — `cell.rs`'s six-run sweep. **What it moved
    /// here is 1.458 units, out of `dissipated` and back into `biomass`, and nothing else at
    /// all.** The tick count, the 584 born, the 542 alive, the field's total and the light that
    /// fell are unchanged to the last bit.
    ///
    /// **What it was through Group J:**
    ///
    /// ```text
    /// 4,000 ticks, 584 born, 542 alive
    /// field       0x40c5_12d3_fabb_0000   10,789.656
    /// biomass     0x40be_a7ab_fc8d_20e9    7,847.672
    /// detritus    0x4070_5251_a3c1_ce4f      261.145
    /// dissipated  0x40bf_e53a_ca68_c230    8,165.230
    /// influx      0x40da_6dec_f5a7_2000   27,063.702
    /// ```
    ///
    /// That shape is the whole content of a pure upkeep change and is worth reading rather than
    /// merely accepting. Upkeep is the one movement in SPEC section 5 that goes `biomass →
    /// dissipated` and touches nothing else, so a world charged less for the same cells keeps
    /// the difference and every *decision* in it — who ate, who was born, who died, in what
    /// order — comes out the same. At 0.009 a tick saved per myocyte, 1.458 units is about **162
    /// myocyte-ticks** in four thousand ticks of a 542-body world, which is the cheapest
    /// available statement of how rare a muscle still is.
    ///
    /// # Why the field's total is the sensitive one
    ///
    /// It is a 64-bit sum over every tile in the world, and every body in the world has been
    /// eating out of those tiles for two thousand ticks. A perturbation anywhere - one extra
    /// draw from a stream, one organism reaped in a different order, one birth that happened a
    /// tick later - moves who ate what, and moves this number in its last bits. It is the
    /// cheapest whole-world signature there is.
    ///
    /// # ⚠️ The arena is deliberately raised out of the way
    ///
    /// `a_small_living_world` allows 250 organisms and this run reaches that in about fifteen
    /// hundred ticks. **A population pressed against its cap is the least sensitive world
    /// there is to point at a golden vector**, because `reproduction.rs` gives up on a birth
    /// the moment it finds no free slot - *before* it has drawn a single number from the
    /// parent's stream. So at the cap almost nothing touches the randomness, and a change to
    /// mutation, to development or to a per-organism stream would sail past unnoticed. Raising
    /// the arena to two thousand keeps every birth going all the way through the draw.
    #[test]
    fn a_run_produces_what_it_produced_before_group_a() {
        let mut world = a_small_living_world(|raw| raw.limits.max_organisms = 2_000);
        let founded = genesis(&mut world, 8);
        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 2_000)),
            &Interrupt::new(),
        );

        assert_eq!(run.go(|_| {}), Stop::TicksDone);

        let world = run.world();
        let census = Census::of(world);
        let ledger = world.ledger();

        assert_eq!(
            [
                world.ticks(),
                census.born,
                u64::try_from(census.population).expect("a population fits in a word"),
                world.grid().total_energy().to_bits(),
                ledger.biomass().to_bits(),
                ledger.detritus().to_bits(),
                ledger.dissipated().to_bits(),
                ledger.influx_total().to_bits(),
            ],
            [
                // 4,000 ticks: a dawn of 2,000 and the 2,000 the run was allowed.
                4_000,
                // 584 organisms have ever lived here; 542 of them are still alive.
                584,
                542,
                // The field, then SPEC section 5's four accounts, as bit patterns. The
                // quantities they stand for are written beside them so that a failure can be
                // read as a change in the world rather than only as a change in a number:
                // 10,789.656 in the water, 7,849.130 held by the living, 261.145 lying in the
                // drift, 8,163.772 spent for good, and 27,063.702 fallen as light.
                0x40c5_12d3_fabb_0000,
                0x40be_a921_3c0b_88e9,
                0x4070_5251_a3c1_ce4f,
                0x40bf_e3c5_8aea_5a30,
                0x40da_6dec_f5a7_2000,
            ],
            "this run no longer produces what it produced at the end of Phase 4. Something \
             changed what the simulation *does* rather than only what it can be asked about, \
             and every figure recorded in docs/PHASE4.md was measured on the other one"
        );
    }

    /// ⭐ **D2.** `max_ticks_per_second` really does slow a run down - and changes nothing
    /// whatever about what the run produces.
    ///
    /// SPEC section 3 calls this the `slow` profile's only lever, and SPEC section 2 says why
    /// it is safe to have one: *"Fixed timestep, `dt = 1/60` simulated seconds per tick.
    /// Real-time speed is decoupled."* Every tick is the same slice of simulated time however
    /// long the machine took over it, so holding the runner back changes when a result arrives
    /// and not what it is.
    ///
    /// # Both halves are asserted, and the second is the load-bearing one
    ///
    /// That it is slower is the easy half and could be satisfied by a runner that had simply
    /// gone wrong. What makes the lever *safe* is that the two runs come out identical, and
    /// the only way to say that is to compare them: every tile of the field and all five of
    /// SPEC section 5's accounts, exactly rather than nearly. A cap that perturbed the world
    /// by so much as a rounding would break determinism for every run made with the `slow`
    /// profile, and it would break it silently, because a slow run and a fast one are never
    /// compared by anybody except this test.
    ///
    /// # The numbers are chosen against the machine's timer rather than for tidiness
    ///
    /// Fifty ticks a second is a twentieth of a second apiece, which is comfortably longer
    /// than the roughly sixteen milliseconds a Windows sleep can be relied on to resolve.
    /// Twenty ticks is therefore about four tenths of a second of test, and the lower bound is
    /// set at three quarters of what the cap asks for - not because the runner may miss it,
    /// but because the sleep may *over*shoot and the assertion should be about the floor.
    #[test]
    fn max_ticks_per_second_actually_slows_a_run() {
        let paced = |rate: Option<u32>| {
            let world = a_small_living_world(|_| {});
            let founded = world.ticks();
            let mut run = Run::new(
                world,
                &bounds(|run| {
                    run.max_ticks = Some(founded + 20);
                    run.max_ticks_per_second = rate;
                }),
                &Interrupt::new(),
            );

            let began = Instant::now();
            let why = run.go(|_| {});

            (why, began.elapsed(), run)
        };

        let (why, quickly, fast) = paced(None);
        assert_eq!(why, Stop::TicksDone);

        let (why, slowly, slow) = paced(Some(50));
        assert_eq!(why, Stop::TicksDone);

        assert!(
            slowly >= Duration::from_millis(300),
            "twenty ticks capped at fifty a second took {slowly:?}, and twenty fiftieths of a \
             second is four tenths"
        );
        assert!(
            slowly > quickly * 4,
            "the capped run took {slowly:?} and the uncapped one took {quickly:?}, so the cap \
             is not what decided how long either of them took"
        );

        // ⭐ And the two runs are the same run. This is what makes the lever safe to ship.
        assert_eq!(
            fast.world().ticks(),
            slow.world().ticks(),
            "the two runs did not take the same number of ticks"
        );
        assert_eq!(
            fast.world()
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            slow.world()
                .grid()
                .tiles()
                .iter()
                .map(|tile| tile.to_bits())
                .collect::<Vec<u32>>(),
            "slowing the run down changed the field, so a run made with the `slow` profile is \
             not the run its seed and configuration describe"
        );
        for (name, quick, held) in [
            (
                "biomass",
                fast.world().ledger().biomass(),
                slow.world().ledger().biomass(),
            ),
            (
                "detritus",
                fast.world().ledger().detritus(),
                slow.world().ledger().detritus(),
            ),
            (
                "dissipated",
                fast.world().ledger().dissipated(),
                slow.world().ledger().dissipated(),
            ),
            (
                "influx_total",
                fast.world().ledger().influx_total(),
                slow.world().ledger().influx_total(),
            ),
        ] {
            assert!(
                quick.to_bits() == held.to_bits(),
                "slowing the run down left {quick} in the {name} account against {held}"
            );
        }
    }

    /// ⭐ **C4, at the runner's end.** A capped run can be *asked* whether the next tick is due
    /// instead of being made to wait for it, and it says why it stopped afterwards.
    ///
    /// Both of these exist for the window and neither of them changes what a run computes.
    ///
    /// `Run::wait` holds a capped run back by sleeping, which is right for a headless run with
    /// nothing else to do and wrong for a windowed one - a window whose event loop is asleep
    /// does not answer the mouse, and at a low cap it would be asleep most of the time. So
    /// `due` lets the caller go and draw a frame instead. The pacing is still decided in one
    /// place: this only reports the answer `wait` would have waited for.
    ///
    /// The second half is smaller and is the same kind of thing. `go` returns why it stopped,
    /// and a window's event loop has no return value to give back - it is somebody else's loop -
    /// so the reason is kept where either caller can read it.
    #[test]
    fn a_run_says_whether_a_tick_is_due_and_why_it_stopped() {
        // Uncapped: always due, because there is nothing holding it back. A window watching an
        // uncapped run therefore ticks for as much of each frame as it is given.
        let world = a_small_living_world(|_| {});
        let founded = world.ticks();
        let mut quick = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 3)),
            &Interrupt::new(),
        );

        assert!(quick.due(), "an uncapped run is waiting for something");
        assert_eq!(
            quick.stopped(),
            None,
            "a run that has not started has ended"
        );
        assert_eq!(quick.step(), None);
        assert!(
            quick.due(),
            "an uncapped run is not due for its next tick immediately after taking one"
        );

        assert_eq!(quick.go(|_| {}), Stop::TicksDone);
        assert_eq!(
            quick.stopped(),
            Some(Stop::TicksDone),
            "a run that has stopped does not say why"
        );

        // Capped at ten a tick a tenth of a second apart: not due again immediately, and due
        // again after the interval. Ten rather than fifty because the wait either way has to be
        // comfortably longer than the sixteen milliseconds a Windows timer resolves to.
        let world = a_small_living_world(|_| {});
        let mut slow = Run::new(
            world,
            &bounds(|run| run.max_ticks_per_second = Some(10)),
            &Interrupt::new(),
        );

        assert_eq!(slow.step(), None);
        assert!(
            !slow.due(),
            "a run capped at ten ticks a second was due again the instant after it ticked, so a \
             window watching it would sleep inside its own event loop"
        );

        std::thread::sleep(Duration::from_millis(150));
        assert!(
            slow.due(),
            "a run capped at ten ticks a second was still not due a seventh of a second later"
        );

        // And asking it to stop is the same request Enter makes.
        slow.ask_to_stop();
        assert_eq!(slow.step(), Some(Stop::Asked));
        assert_eq!(slow.stopped(), Some(Stop::Asked));
    }

    /// ⭐ **D3.** Energy is conserved across a whole living run, with every account in motion
    /// at once.
    ///
    /// SPEC section 15 asks for the ledger to balance over a hundred thousand ticks of any
    /// configuration, and `world.rs` already proves that twice - once over water and light
    /// alone, and once over eight bodies living and dying. Neither of those is this one.
    /// **This is the first test in the project where every movement `ledger.rs` knows about is
    /// happening at the same time**: light falling, tiles shedding what they cannot hold,
    /// bodies harvesting, upkeep dissipating, parents handing energy to children, the dead
    /// becoming detritus, detritus rotting back into the water - a population that is being
    /// born and dying continuously rather than a cohort that was seeded and then ran out.
    ///
    /// # ⚠️ Most of this test is the half a conservation check cannot see
    ///
    /// The lesson this project has now learned in three separate phases, and SPEC section 5
    /// states it outright: *a conservation check cannot see energy that was never declared,
    /// only energy declared wrongly.* A `tick` that had been emptied out balances its books
    /// perfectly. So does a run in which nothing was ever born, and so does one in which the
    /// bodies never ate.
    ///
    /// So the invariant is one assertion and the rest are about the accounts having **moved**.
    /// The field went down from what the light alone left it holding, so harvesting is real;
    /// `dissipated` grew, so upkeep is being paid; `detritus` was holding something at some
    /// point, so bodies died and left corpses; `influx_total` grew, so the light is still
    /// falling on a field that has room for it; and `biomass` is standing at a real quantity
    /// held by organisms that were **not the ones this run started with** - which is the
    /// assertion that says a birth transferred energy rather than conjuring it.
    #[test]
    fn energy_is_conserved_across_a_whole_living_run() {
        let mut world = World::new(&config(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 250;
            raw.light.influx = 0.001;
        }));

        let founded = genesis(&mut world, 8);
        let lit = world.grid().total_energy();
        let founders = world.born();

        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 10_000)),
            &Interrupt::new(),
        );

        let mut worst = 0.0f64;
        let mut most_dead = 0.0f64;
        let mut fewest = usize::MAX;
        run.go(|world| {
            worst = worst.max(relative_error(world));
            most_dead = most_dead.max(world.ledger().detritus());
            fewest = fewest.min(world.organisms().iter().flatten().count());
        });

        let world = run.world();
        let census = Census::of(world);
        let ledger = world.ledger();

        assert!(
            worst < 1e-8,
            "the books were out by {worst} at their worst moment of a run with a whole \
             population living and dying in it, and SPEC section 5's tolerance of 1e-3 is \
             meant to be covering the rounding in `f32` diffusion rather than a leak"
        );

        // Every account moved, which is the part the invariant is blind to.
        assert!(
            world.grid().total_energy() < lit * 0.9,
            "the field is holding {} against the {lit} the light alone left it holding, so \
             nothing has been eating out of it",
            world.grid().total_energy()
        );
        assert!(
            ledger.influx_total() > 0.0,
            "no light fell at all, so this run has established nothing"
        );
        assert!(
            ledger.dissipated() > 0.0,
            "nothing has been spent, so nobody is paying upkeep"
        );
        assert!(
            most_dead > 0.0,
            "there was never a grain of detritus in the water, so nothing died and left a body"
        );
        assert!(
            ledger.biomass() > 0.0,
            "the living hold {} between them",
            ledger.biomass()
        );

        // ⭐ And the population turned over rather than merely surviving: more organisms have
        // lived here than were founded, and more have died than were founded, so the energy in
        // `biomass` is being held by bodies that were born rather than seeded.
        assert!(
            census.born > founders * 10,
            "{} organisms have ever lived in a world founded with {founders}, so barely \
             anything was born and the birth movement is not being exercised",
            census.born
        );
        assert!(
            census.deaths() > founders,
            "only {} organisms have died in this world",
            census.deaths()
        );
        assert!(
            fewest > 0,
            "the population reached nothing at some point, so the run this test measured is \
             not the one it describes"
        );
        assert!(
            census.population > 0,
            "everything died, so this is not a living run"
        );
    }

    /// ⭐⭐ **D4.** A run of the shipped configuration settles at a living population that the
    /// **energy budget** decides, rather than filling the arena and stopping there.
    ///
    /// CLAUDE.md's phase table: *a headless run reaches equilibrium without extinction or
    /// explosion.* SPEC section 15: *a default-config headless run ends with a living,
    /// non-degenerate population - neither extinct nor a single clone filling the world. This
    /// is the test that tells you the balance is right, and it is the one most likely to
    /// fail.*
    ///
    /// # What this test had to be rewritten around, and it is the whole of Group D
    ///
    /// Group C ran the loop for the first time and found the world filling to
    /// `limits.max_organisms` in under twenty thousand ticks and sitting there, with the field
    /// **12% down** and every slot taken. A population pinned against the arena is a population
    /// where every birth that fails does so for a reason that has nothing to do with how well
    /// the parent was doing, so being better at anything buys nothing and drift is the only
    /// force acting. `docs/PHASE4.md`'s Q15 has the measurement.
    ///
    /// So "not extinct and not a clone" is not enough to assert. A world at its arena cap
    /// passes both of those and is exactly the failure CLAUDE.md warns about. **The population
    /// has to level off below the cap, at a level the light decides**, and the assertions below
    /// are in that order: alive, well clear of the arena, the field visibly eaten out of,
    /// turning over rather than standing still, and made of more than one thing.
    ///
    /// # Sixteen founders, and the number was measured rather than picked
    ///
    /// The shipped world settles at about 2,200 bodies whatever it is founded with - one
    /// founder, eight, sixteen, six hundred, all of them reach the same level. What the founder
    /// count decides is **how the world gets there**, and there is a threshold in it that is
    /// worth knowing about because it looks like a failure and is not.
    ///
    /// A world founded with many bodies at once is founded into water that is *full*, and a
    /// full field is a standing stock of a hundred and eighty thousand units. The founders eat
    /// that stock far faster than the light replaces it, so they all breed at once and the
    /// population **overshoots**, sometimes hugely, before the drawdown catches up. Measured
    /// peaks against a settled level near 2,200: eight founders **2,303**, sixteen **2,280**,
    /// fifty **2,827**, two hundred and above **4,000 - the arena** for a few thousand ticks.
    /// That is a colonisation bloom rather than the failure Q15 describes, but it would make
    /// the arena assertion below unreadable.
    ///
    /// Sixteen is therefore the largest founding that reaches the settled level without
    /// overshooting through it at all, which makes the peak below a statement about the
    /// equilibrium rather than about the transient. It is settled by about twenty thousand
    /// ticks; from a single founder the same level takes ninety thousand, which is a test
    /// nobody would run.
    ///
    /// # Measured, and why the bounds are where they are
    ///
    /// Recorded 31 July 2026, Windows 11 x86-64, release build, at the shipped configuration:
    /// a peak of **2,280** against the arena's 4,000, finishing at **2,075**, with the field
    /// holding **101,037** against the 183,791 the light alone leaves it - a drawdown of 45% -
    /// and **12,194 births against 12,333 deaths over the last ten thousand ticks**. Every
    /// assertion is written well outside the figure it is about, because this is a *stochastic*
    /// equilibrium and the numbers wander - what is being pinned is that the world is in the
    /// right regime, not that it reproduces one reading.
    ///
    /// # ⭐⭐ Two checks were run against this test, and the second is the stronger
    ///
    /// **The mutation.** `light.influx` was put back to the 0.012 SPEC first shipped and
    /// nothing else touched. The population reached **4,000 on the nose** and this test failed
    /// on the arena assertion - so what is asserted below is exactly the thing Group D changed,
    /// and not some weaker property that survived it.
    ///
    /// **The control, which says the same thing from the other side.** `limits.max_organisms`
    /// was raised tenfold, from 4,000 to 40,000, with the shipped light left alone. The run
    /// came out **identical to the digit** - peak 2,280, finishing at 2,075, field 101,037,
    /// 12,194 born - because an arena that is never reached decides nothing. That is the whole
    /// claim of this test stated as an experiment rather than as an inequality: **the energy
    /// budget is what limits this world, and the arena could be any size at all.**
    ///
    /// # Why this one is marked `ignore` and still runs on every check
    ///
    /// The same trade `world.rs` makes for its two long conservation tests. This is the
    /// shipped world with a couple of thousand bodies in it, so a tick is real work and the run
    /// is a minute of it. `scripts/check.ps1` passes `--include-ignored` to the **release**
    /// pass only, so the phase's done-criterion is proved on every check, once, in the profile
    /// where it costs a tenth of what it costs in debug.
    ///
    /// To run it in debug anyway:
    /// `cargo test -p coacervate-app -- --ignored a_headless_run_reaches_a_living_equilibrium`
    #[test]
    #[ignore = "the shipped world, for tens of thousands of ticks; check.ps1 runs it via \
                --include-ignored in the release pass"]
    fn a_headless_run_reaches_a_living_equilibrium() {
        let mut world = World::new(&config(|_| {}));
        let cap = usize::try_from(world.config().limits.max_organisms.get())
            .expect("a population cap fits in a machine word");

        let founded = genesis(&mut world, 16);
        let lit = world.grid().total_energy();

        let mut run = Run::new(
            world,
            &bounds(|run| run.max_ticks = Some(founded + 30_000)),
            &Interrupt::new(),
        );

        // Where the population went on the way, so that a world which merely *ended* in a good
        // place cannot pass: the peak says it never filled the arena, and the trough says it
        // never came close to dying out.
        let mut peak = 0usize;
        let mut trough = usize::MAX;
        let mut settling = Census::of(run.world());
        let mut settled_from = 0u64;

        run.go(|world| {
            let living = world.organisms().iter().flatten().count();
            peak = peak.max(living);
            trough = trough.min(living);

            // The last ten thousand ticks, which is about six generations and begins well
            // after the population has settled, are what the turnover claim is measured over.
            if world.ticks() == founded + 20_000 {
                settling = Census::of(world);
                settled_from = world.ticks();
            }
        });

        let world = run.world();
        let census = Census::of(world);
        let field = world.grid().total_energy();

        // 1. Alive. The one thing SPEC section 15 asks for first.
        assert!(
            census.population > 0,
            "the world is empty: a default-config run went extinct"
        );

        // 2. ⭐⭐ And the arena is not what is holding it there. This is the claim Group D
        //    exists for. At the shipped defaults before Group D the population sat at exactly
        //    4,000 for as long as anybody watched.
        assert!(
            peak < cap * 4 / 5,
            "the population reached {peak} against a `limits.max_organisms` of {cap}, so the \
             arena is what limits this world and not the energy budget - which means every \
             birth that failed did so for a reason unconnected to how well its parent was \
             doing, and drift is the only force acting"
        );
        assert!(
            census.population > cap / 20,
            "the run finished with {} organisms in a world that allows {cap}, which is a \
             population on its way out rather than one at equilibrium",
            census.population
        );

        // 3. ⭐ The field is visibly drawn down, so tiles are contested. Before Group D it
        //    fell by 12% with every slot in the world taken.
        assert!(
            field < lit * 0.75,
            "the field is holding {field} against the {lit} the light alone left it holding - \
             a drawdown of {:.0}%, and a world where the water is still nearly full is a world \
             where nothing is scarce",
            100.0 * (1.0 - field / lit)
        );

        // 4. ⭐ Turning over rather than standing still. A population that neither breeds nor
        //    dies has the same count as one that does both, and only one of them is a world
        //    evolution is happening in.
        let born = census.born - settling.born;
        let died = census.deaths() - settling.deaths();
        let over = world.ticks() - settled_from;
        assert!(
            born > 1_000 && died > 1_000,
            "{born} organisms were born and {died} died over the last {over} ticks of the run, \
             so the population is standing still"
        );

        // 5. ⭐ And it is made of more than one thing. SPEC section 15's "not a single clone
        //    filling the world".
        assert!(
            census.gene_spread > 0.1 || census.cell_spread > 0.1,
            "every organism alive has {} genes and {} cells, so the world is one genome \
             copied {} times",
            census.mean_genes,
            census.mean_cells,
            census.population
        );

        // And the books survived all of it.
        assert!(
            relative_error(world) < 1e-8,
            "the run finished {} out in relative terms",
            relative_error(world)
        );
    }
}
