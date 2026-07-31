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

use coacervate_sim::config::RunConfig;
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
}

impl Run {
    /// Take a world that is already alive, and the bounds it is to run under.
    ///
    /// The clock starts here rather than on the first tick, so `max_wall_clock_hours` measures
    /// the run and not the ticking.
    #[must_use]
    pub fn new(world: World, bounds: &RunConfig, interrupt: &Interrupt) -> Self {
        let now = Instant::now();

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
            return Some(why);
        }

        self.wait();
        self.world.tick();

        None
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
    use crate::census::Census;
    use crate::founding::genesis;
    use coacervate_sim::config::{Config, RawConfig, RunConfig, spec_defaults};
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
        let mut world = World::new(&config(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 250;
            raw.light.influx = 0.012;
            change(raw);
        }));

        genesis(&mut world, 8);

        world
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

    // -------------------------------------------------------------------------------------
    // A golden vector, kept apart from the red-then-green tests above
    //
    // `docs/PHASE1.md` sets the rule for these and it is worth repeating where one lives:
    // **if this ever fails, investigate - do not paste in the new numbers.** A golden vector
    // is not a test of the code that produced it. It is a record of what this program did on
    // a day somebody checked, and the only thing it can tell you is that today's program does
    // something different.
    // -------------------------------------------------------------------------------------

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
    /// The numbers below were recorded from the code as it stood at the end of Phase 4, before
    /// a line of Group A was written. They are the bit patterns of the five quantities that
    /// summarise a run, plus the three counts. Bit patterns rather than the numbers themselves
    /// for the reason `world.rs` gives about its own comparisons: a tolerance would wave
    /// through a difference in the last place, which is exactly how a determinism failure
    /// starts.
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
                // 508 organisms have ever lived here; 470 of them are still alive.
                508,
                470,
                // The field, then SPEC section 5's four accounts, as bit patterns. The
                // quantities they stand for are written beside them so that a failure can be
                // read as a change in the world rather than only as a change in a number:
                // 10,754.368 in the water, 6,623.725 held by the living, 217.434 lying in the
                // drift, 5,680.867 spent for good, and 23,276.395 fallen as light.
                0x40c5_012f_1d23_0000,
                0x40b9_dfb9_aadf_208c,
                0x406b_2de6_28e9_041e,
                0x40b6_30de_0509_9754,
                0x40d6_bb19_4711_2000,
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
