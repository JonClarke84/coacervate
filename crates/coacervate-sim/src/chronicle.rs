//! ⭐ **Group C.** The event log: what happened in this world, and when.
//!
//! > SPEC section 11: *"Events. Append-only, written in a naturalist's register. Detect and
//! > record: first adhesion (the origin of multicellularity in this run); first appearance of
//! > each cell kind; first predation event; speciation and extinction, by name; new records:
//! > body size, cell count, genome length, population; mass extinction (population falls by
//! > >50% within 5,000 ticks); environmental changes made by the user."*
//!
//! `docs/PHASE7.md` says why this is the highest-value item in the phase, and it is not a
//! feature request. At tick 2.8 million a run grew bodies with **serial repetition** in them -
//! the same structural unit built several times over - and that was found by eye, in a
//! screenshot, hours later, with no way to know when it started or which lineage it happened
//! in. The world knew everything needed to say so at the moment it happened. Nothing was
//! listening. This is the listener.
//!
//! # ⚠️ It is an observer, exactly as `species.rs` and `series.rs` are
//!
//! [`Chronicle::observe`] takes a `&World` and gives it back unchanged. It draws no random
//! number, it visits the organism arena in index order and reorders nothing, and it holds all
//! of its own state. `run.rs`'s `a_run_produces_what_it_produced_before_group_a` and
//! `the_chronicle_does_not_change_what_the_world_does` are what hold it to that.
//!
//! # Why it is in this crate and not beside `series.rs`
//!
//! `series.rs` is in `coacervate-render` because a sample is *made of* a `Census` and the panel
//! that draws it is there. An event is made of a genome's cells, a cluster's name and
//! [`crate::naming::is_permissible`], all three of which are this crate's - and a name generated
//! from Latin-ish syllables is already presentation living in the simulation crate, for the same
//! reason. Nothing here knows that rendering exists; `panel.rs` reads it, and Phase 8 writes it.
//!
//! # ⭐⭐ The bound, and what a person loses by it
//!
//! SPEC section 13 says of `events.jsonl`: *"append-only, human-readable, **keep everything**"*.
//! That is right for a **file** and wrong for memory, and the two are not the same decision.
//!
//! An unbounded log is the leak CLAUDE.md's *"allocate once, never grow"* rule exists to
//! prevent. A settled run of the shipped configuration carries about sixty clusters, of which
//! fifty-five are species, and each clustering sample can mint one and lose another - so at a
//! sample every five hundred ticks, a twelve-hour run of 32 million ticks can produce of the
//! order of a hundred thousand speciation and extinction lines. At about 160 bytes an event
//! that is **16 MB of prose**, and a run that churned faster would produce more.
//!
//! So [`Chronicle`] holds [`CAPACITY`] events and drops the oldest when it is full - a ring,
//! allocated once, and the count of what went is kept rather than the fact being hidden.
//!
//! ⚠️ **What a viewer loses is the beginning of a long run**, and that is the honest cost rather
//! than a small one: first adhesion, the first appearance of each cell kind and the first
//! predation all happen early, and they are among the most interesting lines the log ever
//! writes. On a run long enough to fill the ring they will have been pushed out of memory. A
//! ring is still the right shape for a *log* - a log is read from its end, and every alternative
//! loses something worse. Thinning, which is what `series.rs` does, cannot be done here at all:
//! a chart is a shape and half of its readings still describe it, while half of a log is a
//! sentence about a lineage whose arrival is no longer in the record.
//!
//! ⚠️ **Phase 8 must therefore write each event at the moment it is appended, and not read this
//! ring at shutdown.** `events.jsonl` is *"keep everything"* and this is the last thousand;
//! writing from here is how a twelve-hour run produces a file that begins in the middle. The
//! shape is already what a line of that file is - [`Event`] carries the tick, the deep time, a
//! stable tag and the sentence, and nothing else - so Phase 8 writes it unchanged.
//!
//! # ⭐ The register, and the one place SPEC's own wording could not be used
//!
//! `docs/PHASE7.md` marks the non-teleological rule load-bearing over every word this phase
//! generates, and [`no_event_text_uses_the_banned_vocabulary`] is the test that holds it. Two
//! things it turned up are worth writing down rather than quietly working around.
//!
//! ⚠️ **SPEC's own example line does not pass.** *"A cell has **failed** to separate from its
//! daughter"* contains *fail*, which [`crate::naming::FORBIDDEN`] refuses because of *failure* -
//! and extinction being framed as failure is the exact thing CLAUDE.md bans. The sentence means
//! the same thing without it, so the log says *"has not separated from its daughter"*. The list
//! is shared with the names on purpose and it is not being narrowed for the sake of one word.
//!
//! ⚠️ **A setting is named in words rather than by its key**, and that is the same collision:
//! `light.gradient` contains *grad*, refused for *gradus* - a step, a rank. It is also simply
//! better copy. *"How much of the light falls near the surface has been changed from 0.75 to
//! 0.40"* is a sentence about the world; `light.gradient = 0.40` is a line of a settings file.
//! See [`CONDITIONS`].
//!
//! And the harder half of the rule is not vocabulary at all. *"Only 6% of lineages survived"*
//! passes any word filter and still frames extinction as failure. Two things answer it: every
//! sentence here states **what changed** and stops, and **one detector fires on a loss** - see
//! [`Chronicle::letting_go`], which notices a lineage that has stopped building a kind of cell
//! it used to build. Without it the log could only ever celebrate gains, which CLAUDE.md says in
//! as many words is teaching something false.

use crate::cell::{Cell, CellKind};
use crate::config::Config;
use crate::naming::Name;
use crate::physics::{Spring, wrapped_offset};
use crate::species::{Cluster, Taxonomy};
use crate::world::World;
use std::collections::VecDeque;

/// How many events the log holds in memory at once.
///
/// A thousand and twenty-four, at about 160 bytes an event - eight for the tick, eight for the
/// deep time, one for the tag and a sentence of a hundred-odd characters on the heap - is
/// **about 160 KB, for ever, whatever the run does**. Against CLAUDE.md's two-gigabyte resident
/// target that is 0.008%, and it is a little over half of what `series.rs`'s chart costs.
///
/// See this module's documentation for what is lost when it fills, and for why Phase 8 must not
/// read the file out of here.
pub const CAPACITY: usize = 1_024;

/// How many ticks apart the population is put into the mass-extinction window.
///
/// `series.rs`'s own stride, which is not a coincidence: it is the finest grid anything in this
/// project records a population on, and a collapse is measured against the population and
/// nothing else.
pub const GRAIN: u64 = 100;

/// How long the mass-extinction window is. SPEC section 11's own number.
///
/// ⚠️ **This window exists because `series.rs`'s does not reach.** The chart holds a population
/// history and thins it: after eight halvings a reading is one tick in 25,600, so a fall that
/// happened and was over inside five thousand ticks is not in the series at all - it is a step
/// between two adjacent samples with no shape to measure. `series.rs` says so itself, and names
/// the event log as the thing that has to say the tick. So the log keeps its own short,
/// high-resolution window: [`WATCH`] over [`GRAIN`] is fifty-one readings, which is four hundred
/// bytes and never thins.
pub const WATCH: u64 = 5_000;

/// How many times one structure has to be built in a body before the body is repeating it.
///
/// Three. Two of anything is a pair, and a pair is what a single division makes; three is the
/// least that reads as a *series*, which is the thing being noticed. See
/// [`Chronicle::repetition`].
pub const REPEATS: usize = 3;

/// How many consecutive clustering samples a lineage has to carry a kind of cell before it is
/// counted as building it, and how many it has to go without before it is counted as having
/// stopped.
///
/// It is hysteresis, and what it is for is that a kind held by one body of a five-hundred-body
/// lineage flickers in and out of the sample as that body is born and dies. A detector without a
/// delay on both sides reports a lineage losing and regaining a cell type every few hundred ticks
/// for the length of a run, which is noise, and noise in a log is worse than silence.
///
/// ⚠️ **Four was the first answer and a real run said no.** Two hundred thousand ticks of the
/// shipped configuration produced 267 events, **130 of them a lineage letting go of a cell kind**,
/// and one lineage accounted for twenty-four of those on its own - the same sentence about the
/// same species over and over. Eight samples is four thousand ticks, and it is half of what
/// `species.rs` requires before a group is a species at all.
pub const SETTLED: u8 = 8;

/// How many cell kinds there are, as a length. `cell.rs` owns the number.
const KINDS: usize = CellKind::ALL.len();

/// A mask with every kind of cell in it: what [`Chronicle::built`] holds once the world has seen
/// all six.
///
/// Written out and checked against `cell.rs`'s own list rather than shifted into being, because a
/// kind added to the enumeration and forgotten here would leave the log looking for a seventh
/// first appearance on every tick of every run for ever.
const EVERY_KIND: u8 = 0b0011_1111;

const _: () = assert!(
    EVERY_KIND.count_ones() as usize == KINDS,
    "the mask of every cell kind has to have one bit per kind in cell.rs's own list"
);

/// The most cells a body can be made of, which `config.rs` caps at sixty-four.
///
/// Here so that [`repeated`] can count distinct parents in a fixed array on the stack rather than
/// allocating one per body per tick. A body larger than this cannot happen; if the cap is ever
/// raised, the extra cells are simply not counted towards a repeat, which is a detector that says
/// *no* slightly too often rather than one that is wrong.
const MOST_CELLS: usize = 64;

/// How many times over a body has to be wider than the widest the log has mentioned before it is
/// worth mentioning again.
///
/// ⚠️ **A ladder rather than a step, and a run is what decided that.** A body's span is not only a
/// fact about its growth program - it is a measurement of where the physics has put its cells, and
/// a large body that has just been born spends its first few ticks relaxing outwards along its
/// springs. At a fixed step of two world units, 40,000 ticks of the shipped configuration produced
/// **nine lines in twelve ticks** about one body doing exactly that, from 30.4 units across to
/// 49.1, which is nine lines saying one thing.
///
/// Half as much again is the same shape [`CROWD_STEP`] uses on the population, and it reads as the
/// sentence it is: a body half as big again as the biggest there has ever been. Over that same run
/// it is five lines from end to end.
const SPAN_GROWTH: f32 = 1.5;

/// The smallest a body has to be before its width is worth a line at all, in world units.
///
/// About two-thirds of a photocyte's width, and it is the floor under [`SPAN_GROWTH`]'s ladder: a
/// world founded on single-celled bodies has a widest body of nothing at all, and a ladder built
/// by multiplying nothing is not a ladder.
const SPAN_LEAST: f32 = 4.0;

/// How many times over the population has to beat what the log last mentioned before it is
/// mentioned again.
///
/// ⚠️ **Doubling, and it is the one record that needed a ladder rather than a step.** A run
/// begins with eight founders and settles near two thousand two hundred, and every one of the
/// two thousand one hundred and ninety-two populations in between is a new record - which would
/// be two thousand lines in the first twenty thousand ticks of every run, and would push
/// everything else in this module out of the ring before the first species was named. At a
/// doubling the same stretch is **eight lines**, and each of them says something: the world holds
/// twice as many bodies as it has ever held.
const CROWD_STEP: u32 = 2;

/// What kind of thing happened.
///
/// The machine-readable half of an [`Event`]. It is here rather than being left implicit in the
/// prose because Phase 8 writes these to `events.jsonl` and something reading that file back
/// should be able to find every mass extinction without matching on English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// ⭐ **C2.** A body is holding together as more than one cell.
    Adhesion,

    /// ⭐ **C3.** A kind of cell has been built for the first time in this run.
    CellKind,

    /// ⭐ **C4.** Living tissue has been eaten.
    Predation,

    /// ⭐ **C5.** A lineage has persisted long enough to be named.
    Speciation,

    /// ⭐ **C5.** A named lineage is no longer in the water.
    Extinction,

    /// ⭐ **C6.** Something is larger, longer or more numerous than it has been.
    Record,

    /// ⭐ **C7.** The population has fallen by more than half inside [`WATCH`] ticks.
    MassExtinction,

    /// ⭐ **C8.** A person changed the conditions the world is living under.
    Conditions,

    /// ⭐ **C10.** One body is building the same structure more than once.
    Repetition,

    /// ⭐ **The loss.** A named lineage has stopped building a kind of cell it used to build.
    LettingGo,
}

impl Kind {
    /// Every kind of event there is.
    ///
    /// Written out so a test can walk the whole set rather than a sample of it, exactly as
    /// `CellKind::ALL` is.
    pub const ALL: [Self; 10] = [
        Self::Adhesion,
        Self::CellKind,
        Self::Predation,
        Self::Speciation,
        Self::Extinction,
        Self::Record,
        Self::MassExtinction,
        Self::Conditions,
        Self::Repetition,
        Self::LettingGo,
    ];

    /// What this kind is called in a file, for ever.
    ///
    /// ⚠️ **These strings are a format and not a label.** Phase 8 writes them into
    /// `events.jsonl`, and an archived run is only readable for as long as they mean what they
    /// meant. Rename one and every recording ever made says something different.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Adhesion => "adhesion",
            Self::CellKind => "cell_kind",
            Self::Predation => "predation",
            Self::Speciation => "speciation",
            Self::Extinction => "extinction",
            Self::Record => "record",
            Self::MassExtinction => "mass_extinction",
            Self::Conditions => "conditions",
            Self::Repetition => "repetition",
            Self::LettingGo => "letting_go",
        }
    }
}

/// One thing that happened, and when.
///
/// ⚠️ **This is a line of Phase 8's `events.jsonl`, in memory.** Four fields, all public, none of
/// them derived from anything else: the tick, the deep time a person reads instead of the tick,
/// a tag that never changes, and the sentence. A writer turns it into
/// `{"tick":41208,"ma":41.2,"kind":"adhesion","said":"…"}` and does not have to decide anything.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Which tick of the world this happened on.
    pub tick: u64,

    /// The same moment in millions of years. CLAUDE.md's deep time, kept beside the tick rather
    /// than worked out by whoever displays it - see [`millions_of_years`].
    pub ma: f64,

    /// What kind of thing it was.
    pub kind: Kind,

    /// What happened, in a naturalist's register, as a sentence.
    pub said: String,
}

impl Event {
    /// ⭐ **C1.** The whole line, as the chronicle writes it.
    ///
    /// > *Tick 41,208 — 41.2 Ma.* A cell has not separated from its daughter.
    ///
    /// The tick and the deep time both, and in that order, because they answer two different
    /// questions: the tick is *where in the run* and is what a replay is scrubbed to, and the
    /// millions of years are what CLAUDE.md asks a person to read instead of a number going up.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "Tick {} — {:.1} Ma. {}",
            grouped(self.tick),
            self.ma,
            self.said
        )
    }

    /// The first sentence of it, for somewhere there is not room for the whole thing.
    ///
    /// `panel.rs` shows the last few events in a column two hundred points wide, which is about
    /// twenty-eight monospace characters - so a two-sentence event is nine lines of chrome. The
    /// first sentence is the one that says what happened; the second is always the detail behind
    /// it, which is what the chronicle is for.
    #[must_use]
    pub fn headline(&self) -> &str {
        match self.said.find(". ") {
            Some(stop) => &self.said[..=stop],
            None => &self.said,
        }
    }
}

/// ⭐ **CLAUDE.md's deep time, and the one place the arithmetic happens.**
///
/// > *"Tick counts are displayed as millions of years, so a long run reads as Earth's history
/// > rather than a number going up."*
///
/// `census::millions_of_years` reads a world and calls this; the event log has only a tick and a
/// configuration, because an event is recorded at a moment and read long afterwards. Two copies
/// of this multiplication is how the panel and the chronicle come to disagree about what year it
/// is.
///
/// It is presentation and it never enters the physics: nothing in a tick reads
/// `world.years_per_tick`.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    reason = "a tick count is turned into a span of geological time for a person to read; the \
              digits lost are far below the resolution of a figure printed to one decimal place"
)]
pub fn millions_of_years(ticks: u64, years_per_tick: f32) -> f64 {
    ticks as f64 * f64::from(years_per_tick) / 1e6
}

/// One of the conditions of a world that a person may change while it is running.
///
/// ⚠️ **Named in words rather than by its key**, for the two reasons this module's documentation
/// gives: `light.gradient` contains *grad*, which the shared ban list refuses, and a log is prose
/// rather than a settings file. The key is kept anyway, unprinted, and reachable through
/// [`conditions`] — so that `settings.rs`'s `every_dial_is_a_condition_the_chronicle_reports` can
/// put this list beside its own twenty sliders and insist the two name the same settings. Two
/// lists of the live settings would be two lists that come apart, and a slider whose changes
/// nothing recorded would be an environmental event that happened and was never written down.
struct Condition {
    /// Which of SPEC section 3's tables it is in.
    table: &'static str,

    /// What it is called inside that table.
    label: &'static str,

    /// What it is, in words a person reading a chronicle would recognise.
    phrase: &'static str,

    /// Read it out of a configuration.
    read: fn(&Config) -> f32,
}

/// ⭐ **C8.** Every condition of a world SPEC section 3 does not lock at run start.
///
/// > `[world]`, `[limits]` and `seed` lock at run start; the rest can be changed live, **which is
/// > how environmental events work.**
///
/// That last clause is why this is in the event log at all. Raising `metabolism.upkeep_scale` is
/// not a settings change, it is the weather turning, and SPEC section 11 lists *"environmental
/// changes made by the user"* among the things worth recording beside a mass extinction.
///
/// The four tables here are exactly the ones [`World::retune`] will accept a change to, and
/// exactly `settings.rs`'s twenty sliders less `run.max_ticks_per_second` — which is not a fact
/// about the world at all, but about how fast a person is watching it.
const CONDITIONS: [Condition; 20] = [
    Condition {
        table: "light",
        label: "influx",
        phrase: "the light reaching the water",
        read: |config| config.light.influx,
    },
    Condition {
        table: "light",
        label: "cap",
        phrase: "the most a tile of water can hold",
        read: |config| config.light.cap,
    },
    Condition {
        table: "light",
        label: "gradient",
        phrase: "how much of the light falls near the surface",
        read: |config| config.light.gradient,
    },
    Condition {
        table: "light",
        label: "patchiness",
        phrase: "how unevenly the light falls",
        read: |config| config.light.patchiness,
    },
    Condition {
        table: "light",
        label: "diffusion",
        phrase: "how fast energy spreads sideways through the water",
        read: |config| config.light.diffusion,
    },
    Condition {
        table: "physics",
        label: "drag",
        phrase: "how much of its speed the water leaves a cell",
        read: |config| config.physics.drag,
    },
    Condition {
        table: "physics",
        label: "collision_stiffness",
        phrase: "how hard two cells push each other apart",
        read: |config| config.physics.collision_stiffness,
    },
    Condition {
        table: "physics",
        label: "spring_damping",
        phrase: "how quickly an adhesion stops springing",
        read: |config| config.physics.spring_damping,
    },
    Condition {
        table: "metabolism",
        label: "upkeep_scale",
        phrase: "what it costs a cell simply to be alive",
        read: |config| config.metabolism.upkeep_scale,
    },
    Condition {
        table: "metabolism",
        label: "gene_cost",
        phrase: "what one gene costs to carry",
        read: |config| config.metabolism.gene_cost,
    },
    Condition {
        table: "metabolism",
        label: "movement_cost",
        phrase: "what a unit of work costs to do",
        read: |config| config.metabolism.movement_cost,
    },
    Condition {
        table: "metabolism",
        label: "reproduction_threshold",
        phrase: "how much a body must hold before it has a child",
        read: |config| config.metabolism.reproduction_threshold,
    },
    Condition {
        table: "metabolism",
        label: "offspring_share",
        phrase: "how much of itself a parent hands to a child",
        read: |config| config.metabolism.offspring_share,
    },
    Condition {
        table: "mutation",
        label: "point_rate",
        phrase: "how often one gene is changed as a genome is copied",
        read: |config| config.mutation.point_rate,
    },
    Condition {
        table: "mutation",
        label: "point_sigma",
        phrase: "how far a changed number moves",
        read: |config| config.mutation.point_sigma,
    },
    Condition {
        table: "mutation",
        label: "duplication_rate",
        phrase: "how often a gene is copied twice",
        read: |config| config.mutation.duplication_rate,
    },
    Condition {
        table: "mutation",
        label: "deletion_rate",
        phrase: "how often a gene is dropped",
        read: |config| config.mutation.deletion_rate,
    },
    Condition {
        table: "mutation",
        label: "insertion_rate",
        phrase: "how often a gene that was not there appears",
        read: |config| config.mutation.insertion_rate,
    },
    Condition {
        table: "mutation",
        label: "reorder_rate",
        phrase: "how often two genes side by side swap places",
        read: |config| config.mutation.reorder_rate,
    },
    Condition {
        table: "mutation",
        label: "genome_duplication_rate",
        phrase: "how often a whole genome is doubled",
        read: |config| config.mutation.genome_duplication_rate,
    },
];

/// Every condition of a world this log will report a change to, as `table.label`.
///
/// The paths rather than the phrases, because the one thing worth checking from outside this
/// crate is that the list is the same list `settings.rs` puts sliders on. See [`Condition`].
pub fn conditions() -> impl ExactSizeIterator<Item = String> {
    CONDITIONS
        .iter()
        .map(|condition| format!("{}.{}", condition.table, condition.label))
}

/// A named lineage, and which kinds of cell it has settled into building.
///
/// ⭐ **This is the whole of the loss detector's state.** See [`Chronicle::letting_go`].
#[derive(Debug)]
struct Watched {
    /// Which cluster this is. `species.rs` mints these once and never reuses them.
    id: u32,

    /// What it is called.
    name: Name,

    /// How many consecutive samples each kind of cell has been carried by somebody in it.
    carried: [u8; KINDS],

    /// How many consecutive samples each kind has been carried by nobody in it.
    without: [u8; KINDS],

    /// Which kinds this lineage has carried for [`SETTLED`] consecutive samples, as a bit per
    /// kind, and so is counted as *building* rather than as happening to have one about.
    builds: u8,

    /// Which kinds the log has already said this lineage stopped building.
    ///
    /// ⚠️ **A loss is a landmark and not a status, and a real run is what settled that.** A
    /// lineage of several hundred bodies takes up a cell kind again and lets it go again, and
    /// over two hundred thousand ticks of the shipped configuration one species said *"has
    /// stopped building sclerocytes"* twenty-four times. Said once, it is the same class of thing
    /// as SPEC section 11's *first appearance of each cell kind* - the moment a lineage's bodies
    /// stopped containing something they used to contain - and it is the mirror of it, which is
    /// exactly what CLAUDE.md asks the log to have.
    told: u8,
}

/// Everything that has happened in one world, and the state it takes to notice it happening.
///
/// The third periodic observer of a run, beside `series.rs`'s chart and `species.rs`'s
/// clustering, and the only one of the three that is written in sentences.
#[derive(Debug)]
pub struct Chronicle {
    /// The events, oldest first. **Allocated once at [`CAPACITY`] and never resized.**
    events: VecDeque<Event>,

    /// How many events have been dropped off the front to make room.
    ///
    /// Kept rather than the fact being silent. SPEC section 13 says the same thing about
    /// snapshots: *"Log what was dropped; silent truncation reads as complete history when it
    /// isn't."*
    dropped: u64,

    /// `world.years_per_tick`, which is locked at run start.
    years_per_tick: f32,

    /// How wide the world is, for measuring a body the short way round it.
    width: f32,

    /// ⭐ **C2.** Whether anything in this world has ever held together as more than one cell.
    adhesion: bool,

    /// ⭐ **C4.** Whether living tissue has ever been eaten.
    predation: bool,

    /// ⭐ **C3.** Which kinds of cell have ever been built, by anybody, as a bit per kind.
    ///
    /// A mask rather than six flags so that the whole question *"is there still a kind of cell
    /// this world has never grown?"* is one comparison against [`EVERY_KIND`] - which is asked
    /// once per living body per tick, and which after the first hour of a run is always no.
    built: u8,

    /// ⭐ **C10.** Whether one body has ever been seen building the same structure three times.
    repetition: bool,

    /// ⭐ **C6.** Whether the records below hold a reading yet.
    ///
    /// ⚠️ **The first reading primes them and is not announced**, and that is a decision rather
    /// than an oversight. A record is news that something has changed, and the first population
    /// a world ever has has not changed from anything. Without this every run would open with
    /// four lines announcing that its founders were the largest bodies, the longest genomes and
    /// the greatest population there had ever been, which is true and is not news.
    primed: bool,

    /// The most cells any body has had.
    most_cells: u32,

    /// The most genes any genome has had.
    most_genes: u32,

    /// The furthest across any body has been, in world units.
    widest: f32,

    /// The furthest across the log has said a body was. See [`SPAN_GROWTH`].
    told_span: f32,

    /// The largest the population has been.
    crowd: u32,

    /// The largest population the log has mentioned. See [`CROWD_STEP`].
    told_crowd: u32,

    /// ⭐ **C7.** The population at each of the last [`WATCH`] ticks' worth of readings, oldest
    /// first, on [`GRAIN`]'s grid.
    window: VecDeque<(u64, u32)>,

    /// ⭐ **C5.** Every named lineage the log knows about, **in ascending order of identifier**.
    ///
    /// The same order `species.rs` keeps its clusters in, which is what makes the two walkable
    /// side by side without either being searched.
    watched: Vec<Watched>,

    /// Which kinds of cell each of the taxonomy's clusters was carrying at the last sample, as a
    /// bit per kind, indexed by that cluster's position in `Taxonomy::clusters`.
    ///
    /// Working room, allocated once at twice the population cap - which is `species.rs`'s own
    /// bound on how many clusters there can be - and cleared at the start of every sample.
    carrying: Vec<u8>,

    /// How many clustering samples had been taken when the lineages were last looked at.
    samples: u64,
}

impl Chronicle {
    /// A log with nothing in it, for a run of this configuration.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let slots = usize::try_from(config.limits.max_organisms.get())
            .expect("a population cap fits in a machine word");

        Self {
            events: VecDeque::with_capacity(CAPACITY),
            dropped: 0,
            years_per_tick: config.world.years_per_tick,
            width: config.world.width,
            adhesion: false,
            predation: false,
            built: 0,
            repetition: false,
            primed: false,
            most_cells: 0,
            most_genes: 0,
            widest: 0.0,
            told_span: 0.0,
            crowd: 0,
            told_crowd: 0,
            // Fifty-one readings span SPEC's five thousand ticks; the fifty-second is the room
            // the newest one needs while the oldest is still in the ring.
            window: VecDeque::with_capacity(
                usize::try_from(WATCH / GRAIN + 2).expect("fifty-two readings"),
            ),
            watched: Vec::with_capacity(slots),
            carrying: Vec::with_capacity(slots * 2),
            samples: 0,
        }
    }

    /// Look at the world and write down anything that has happened since the last look.
    ///
    /// Called after every tick by whatever is doing the ticking - `Run::step`, which is the one
    /// place in the program a tick happens - immediately after `Taxonomy::observe`, so that a
    /// lineage that arrived at this tick is named by the time the log is asked about it.
    ///
    /// # ⚠️ It is idempotent, and that is deliberate rather than incidental
    ///
    /// `series.rs` and `species.rs` both refuse a tick they have already been shown, because a
    /// second reading of one tick would be a second point on a chart and a second sample towards
    /// a promotion. Nothing here has that shape: a *first* fires once by construction, a record
    /// is a maximum, the mass-extinction window is keyed on the tick, and the lineages are done
    /// once per clustering sample rather than once per call. So there is no guard, and a caller
    /// that offers the same world twice gets the same log.
    pub fn observe(&mut self, world: &World, taxonomy: &Taxonomy) {
        let population = self.survey(world, taxonomy);
        self.collapse(world.ticks(), population);

        // ⭐ **C5 and the loss.** Once per *clustering sample*, and not once per tick: a lineage
        // that arrived and one that went are differences between two samples of the population,
        // and `species.rs` takes one every five hundred ticks. See `Taxonomy::samples`.
        if taxonomy.samples() != self.samples {
            self.samples = taxonomy.samples();
            self.lineages(world, taxonomy);
        }
    }

    /// ⭐ **C2, C3, C6 and C10.** One walk of the living population, and everything that can be
    /// seen in one.
    ///
    /// # ⚠️ What is done for every body and what is not
    ///
    /// The population has to be counted, so the arena is walked whatever else is true. Everything
    /// beyond that is gated on being able to say anything new, and after the opening minutes of a
    /// run nearly all of it is switched off:
    ///
    /// | | Costs | Stops when |
    /// | --- | --- | --- |
    /// | The cell and gene records | two comparisons | never - they are the point |
    /// | First adhesion | one comparison | the first body of two cells |
    /// | Every kind of cell | one pass over the body | all six have been built |
    /// | The body-size record | one pass over the body, and its pairs only if that pass says it might beat the record | never, but the second pass is rare |
    /// | Serial repetition | the body's springs against each other | the first body that repeats a unit |
    ///
    /// A settled run of the shipped configuration carries about 2,200 bodies of three cells, so
    /// the steady state is 2,200 slot reads, 4,400 comparisons and 6,600 squared distances.
    fn survey(&mut self, world: &World, taxonomy: &Taxonomy) -> u32 {
        let tick = world.ticks();
        let width = self.width;
        let mut population = 0_u32;

        // ⭐ **C4.** The ledger counts every mouthful of living tissue and nothing else in the
        // world does. See `ledger.rs`.
        if !self.predation && world.ledger().predation_total() > 0.0 {
            self.predation = true;
            self.note(tick, Kind::Predation, said_predation());
        }

        for (slot, organism) in world.organisms().iter().enumerate() {
            let Some(organism) = organism else {
                continue;
            };
            population += 1;

            let genes = u32::try_from(organism.genome().genes().len())
                .expect("a genome is capped at 128 genes");
            if genes > self.most_genes {
                if self.primed {
                    self.note(
                        tick,
                        Kind::Record,
                        said_record(Record::Genes, &grouped(u64::from(genes))),
                    );
                }
                self.most_genes = genes;
            }

            let cells = u32::try_from(organism.cells()).expect("a body is capped at 64 cells");
            if cells > self.most_cells {
                if self.primed {
                    self.note(
                        tick,
                        Kind::Record,
                        said_record(Record::Cells, &grouped(u64::from(cells))),
                    );
                }
                self.most_cells = cells;
            }

            if !self.adhesion && organism.springs() > 0 {
                self.adhesion = true;
                self.note(tick, Kind::Adhesion, said_adhesion(cells));
            }

            let body = world.cells_of(slot);

            if self.built != EVERY_KIND {
                for cell in body {
                    let at = cell.kind as usize;
                    if self.built & bit(at) == 0 {
                        self.built |= bit(at);
                        self.note(tick, Kind::CellKind, said_kind(cell.kind));
                    }
                }
            }

            if body.len() > 1 && reach(body, width) > self.widest {
                let across = span(body, width);
                if across > self.widest {
                    if self.primed && across >= self.told_span.max(SPAN_LEAST) * SPAN_GROWTH {
                        self.note(
                            tick,
                            Kind::Record,
                            said_record(Record::Span, &format!("{across:.1}")),
                        );
                        self.told_span = across;
                    }
                    self.widest = across;
                }
            }

            if !self.repetition
                && organism.springs() >= REPEATS
                && let Some((times, on, of)) = repeated(body, world.springs_of(slot))
            {
                self.repetition = true;
                let name = taxonomy
                    .species_of(slot, organism.serial())
                    .and_then(|id| {
                        taxonomy
                            .clusters()
                            .binary_search_by_key(&id, Cluster::id)
                            .ok()
                    })
                    .and_then(|at| taxonomy.clusters()[at].name());
                self.note(tick, Kind::Repetition, said_repetition(name, times, on, of));
            }
        }

        if population > self.crowd {
            if self.primed && population >= self.told_crowd.saturating_mul(CROWD_STEP) {
                self.note(
                    tick,
                    Kind::Record,
                    said_record(Record::Crowd, &grouped(u64::from(population))),
                );
                self.told_crowd = population;
            }
            self.crowd = population;
        }

        // The first reading a run ever takes primes the records instead of announcing them. See
        // [`Chronicle::primed`].
        if !self.primed && population > 0 {
            self.primed = true;
            self.told_crowd = population;
            self.told_span = self.widest;
        }

        population
    }

    /// ⭐ **C7.** Whether the population has fallen by more than half inside [`WATCH`] ticks.
    ///
    /// The window is its own, short and never thinned - see [`WATCH`] for why `series.rs`'s
    /// history cannot answer this. A reading goes in every [`GRAIN`] ticks of the world's own
    /// clock, readings older than the window are dropped off the front, and a fall is measured
    /// against the largest reading still in it.
    ///
    /// ⚠️ **Firing clears the window**, which is what stops one collapse being reported over and
    /// over for the next five thousand ticks: the peak that the fall was measured against is
    /// gone, and the next line has to be a fall from where the world is *now*.
    fn collapse(&mut self, tick: u64, population: u32) {
        if !tick.is_multiple_of(GRAIN) {
            return;
        }

        // A world offered twice at one tick is one reading. See [`Chronicle::observe`].
        if self.window.back().is_some_and(|(at, _)| *at == tick) {
            return;
        }

        while self
            .window
            .front()
            .is_some_and(|(at, _)| tick.saturating_sub(*at) > WATCH)
        {
            self.window.pop_front();
        }

        // The largest reading in the window, and when it was. Started at the present one, so a
        // window with nothing older than now in it can never report a fall.
        let mut peak = (tick, population);
        for &(at, count) in &self.window {
            if count > peak.1 {
                peak = (at, count);
            }
        }
        self.window.push_back((tick, population));

        if u64::from(population) * 2 >= u64::from(peak.1) {
            return;
        }

        self.note(
            tick,
            Kind::MassExtinction,
            said_collapse(peak.1, population, tick - peak.0),
        );
        self.window.clear();
        self.window.push_back((tick, population));
    }

    /// ⭐ **C5 and the loss.** Which named lineages have arrived, which have gone, and which have
    /// stopped building something they used to build.
    ///
    /// Its own walk of the population rather than a share of [`Chronicle::survey`]'s, because it
    /// happens on the clustering's five-hundred-tick grid and the survey happens every tick.
    /// Asking every organism which species it is in costs a search of the cluster list, and four
    /// hundred and ninety-nine ticks in five hundred there would be nothing to do with the
    /// answer.
    fn lineages(&mut self, world: &World, taxonomy: &Taxonomy) {
        let tick = world.ticks();
        let clusters = taxonomy.clusters();

        // Which kinds of cell each cluster's living members carry between them. Indexed by the
        // cluster's position in the list, which is stable for the length of this call.
        self.carrying.clear();
        self.carrying.resize(clusters.len(), 0);

        for (slot, organism) in world.organisms().iter().enumerate() {
            let Some(organism) = organism else {
                continue;
            };
            let Some(id) = taxonomy.species_of(slot, organism.serial()) else {
                continue;
            };
            let Ok(at) = clusters.binary_search_by_key(&id, Cluster::id) else {
                continue;
            };

            for cell in world.cells_of(slot) {
                self.carrying[at] |= bit(cell.kind as usize);
            }
        }

        // Anything the log was watching that is not a named cluster any more is over. Walked from
        // the front with `remove` rather than filtered, because the name has to be read out
        // before it goes.
        let mut at = 0;
        while at < self.watched.len() {
            let id = self.watched[at].id;
            let still = clusters
                .binary_search_by_key(&id, Cluster::id)
                .is_ok_and(|found| clusters[found].name().is_some());

            if still {
                at += 1;
                continue;
            }

            let gone = self.watched.remove(at);
            self.note(tick, Kind::Extinction, said_extinction(&gone.name));
        }

        // And every named cluster is either one the log already knows about - in which case what
        // it is building is brought up to date - or one that has just been named.
        for (at, cluster) in clusters.iter().enumerate() {
            let Some(name) = cluster.name() else {
                continue;
            };
            let carrying = self.carrying[at];

            match self
                .watched
                .binary_search_by_key(&cluster.id(), |watched| watched.id)
            {
                Ok(found) => self.building(tick, found, carrying),
                Err(insert) => {
                    let said = said_speciation(name, cluster.members());
                    self.watched.insert(
                        insert,
                        Watched {
                            id: cluster.id(),
                            name: name.clone(),
                            carried: [0; KINDS],
                            without: [0; KINDS],
                            builds: 0,
                            told: 0,
                        },
                    );
                    self.note(tick, Kind::Speciation, said);
                }
            }
        }
    }

    /// ⭐ **The loss.** Bring one lineage's record of what it builds up to date, and say so if it
    /// has stopped building something.
    ///
    /// ⚠️ **Both directions have [`SETTLED`] samples of hysteresis on them and both are needed.**
    /// A kind held by one body of a fifty-body lineage comes and goes from the sample every time
    /// that body dies and another is born, so a detector without a delay would report the same
    /// lineage letting go of the same cell type every few hundred ticks for the length of a run -
    /// which is noise, and noise in a log is worse than silence.
    ///
    /// ⚠️ **And it says so once per lineage per kind.** See [`Watched::told`]: a lineage takes a
    /// cell kind up again and lets it go again, and a log that reported every crossing would say
    /// the same sentence about the same species two dozen times in one run.
    ///
    /// See [`said_letting_go`] for why this detector exists at all: without one that fires on a
    /// *loss*, the log is structurally biased however carefully every sentence in it is worded.
    fn building(&mut self, tick: u64, at: usize, carrying: u8) {
        for (kind, of) in CellKind::ALL.into_iter().enumerate() {
            let watched = &mut self.watched[at];

            if carrying & bit(kind) != 0 {
                watched.carried[kind] = watched.carried[kind].saturating_add(1);
                watched.without[kind] = 0;
                if watched.carried[kind] >= SETTLED {
                    watched.builds |= bit(kind);
                }
                continue;
            }

            watched.carried[kind] = 0;
            watched.without[kind] = watched.without[kind].saturating_add(1);
            if watched.builds & bit(kind) == 0 || watched.without[kind] < SETTLED {
                continue;
            }

            watched.builds &= !bit(kind);
            if watched.told & bit(kind) != 0 {
                continue;
            }

            watched.told |= bit(kind);
            let name = watched.name.clone();
            self.note(tick, Kind::LettingGo, said_letting_go(&name, of, carrying));
        }
    }

    /// ⭐ **C8.** Write down that a person changed the conditions of the world.
    ///
    /// Handed the configuration as it was and as it now is; every one of [`CONDITIONS`] that
    /// differs is one line. Several sliders moved at once is therefore several events at one
    /// tick, which is what happened.
    #[expect(
        clippy::float_cmp,
        reason = "the question is whether the number a person is now running the world under is \
                  the number it was running under before, and that is exact equality of the value \
                  the simulation actually charges - not a nearness. A tolerance here would be a \
                  slider a person could move and the log would not mention"
    )]
    pub fn retuned(&mut self, tick: u64, before: &Config, after: &Config) {
        for condition in &CONDITIONS {
            let was = (condition.read)(before);
            let now = (condition.read)(after);

            if was != now {
                self.note(tick, Kind::Conditions, said_conditions(condition, was, now));
            }
        }
    }

    /// The events, oldest first.
    #[must_use]
    pub fn events(&self) -> impl ExactSizeIterator<Item = &Event> {
        self.events.iter()
    }

    /// The most recent events, newest last, at most `many` of them.
    #[must_use]
    pub fn latest(&self, many: usize) -> impl ExactSizeIterator<Item = &Event> {
        let from = self.events.len().saturating_sub(many);

        self.events.range(from..)
    }

    /// How many events have been dropped off the front of the log to make room for newer ones.
    ///
    /// Nought for every run short enough to fit in [`CAPACITY`]. See this module's documentation
    /// for what a viewer loses when it is not.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Put an event in, dropping the oldest if the ring is full.
    fn note(&mut self, tick: u64, kind: Kind, said: String) {
        if self.events.len() >= CAPACITY {
            self.events.pop_front();
            self.dropped += 1;
        }

        self.events.push_back(Event {
            tick,
            ma: millions_of_years(tick, self.years_per_tick),
            kind,
            said,
        });
    }
}

/// ⭐ **C2.** *"A cell has not separated from its daughter."*
///
/// ⚠️ **SPEC's own sentence says "has failed to separate", and it cannot be used.** See this
/// module's documentation: *fail* is on the shared ban list because of *failure*, and the
/// register of the log and the register of the names are one register.
fn said_adhesion(cells: u32) -> String {
    format!(
        "A cell has not separated from its daughter. There is a body of {cells} cells in the \
         water, held together by adhesion, where every body before it was one cell on its own."
    )
}

/// ⭐ **C3.** A kind of cell has been built for the first time in this run.
fn said_kind(kind: CellKind) -> String {
    format!(
        "A {} has been built for the first time in this world: {}.",
        one(kind),
        does(kind)
    )
}

/// ⭐ **C4.** *"Living tissue is being eaten."*
///
/// ⚠️ **It does not say whose.** The ledger counts the mouthful and cannot say which body took
/// it, and finding out would mean walking every devorocyte in the world against every cell near
/// it - a second implementation of the one rule CLAUDE.md's decision log is most insistent must
/// not be written twice. What the log can say exactly is the thing that changed about the world.
fn said_predation() -> String {
    "A devorocyte has taken energy out of another body. Living tissue is being eaten in this \
     world as well as the water and the dead."
        .to_owned()
}

/// ⭐ **C5.** A cluster has persisted long enough to be worth a name.
fn said_speciation(name: &Name, members: u32) -> String {
    // A lineage promoted on its last member is a perfectly ordinary thing for this world to do,
    // and "1 of them are alive" is the sentence a run of the shipped configuration produced.
    let alive = match members {
        1 => "one of them is alive".to_owned(),
        several => format!("{} of them are alive", grouped(u64::from(several))),
    };

    format!(
        "{name}. A group of bodies has been running one growth program for twenty consecutive \
         samples and is recorded here under that name; {alive}."
    )
}

/// ⭐ **C5.** A named lineage is no longer in the water.
///
/// ⚠️ **Extinction is not failure and is not written as one.** It says what is the case and
/// stops.
fn said_extinction(name: &Name) -> String {
    format!("{name} is no longer in the water. Nothing alive is running its growth program.")
}

/// ⭐ **C6.** Something is larger, longer or more numerous than it has been in this run.
fn said_record(what: Record, now: &str) -> String {
    match what {
        Record::Cells => {
            format!("A body of {now} cells. No body in this world has been made of more.")
        }
        Record::Genes => {
            format!("A genome of {now} genes. No growth program in this world has been longer.")
        }
        Record::Span => {
            format!("A body {now} world units across. No body in this world has reached further.")
        }
        Record::Crowd => format!(
            "There are {now} organisms in the water, more than at any earlier point in this run."
        ),
    }
}

/// Which of SPEC section 11's four records a line is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Record {
    /// How many cells a body is made of.
    Cells,

    /// How many genes a growth program has.
    Genes,

    /// How far across a body reaches, in world units.
    Span,

    /// How many organisms are alive at once.
    Crowd,
}

impl Record {
    /// All four, for the test that walks every sentence this module can write.
    #[cfg(test)]
    const ALL: [Self; 4] = [Self::Cells, Self::Genes, Self::Span, Self::Crowd];
}

/// ⭐ **C7.** The population has fallen by more than half inside [`WATCH`] ticks.
fn said_collapse(from: u32, to: u32, over: u64) -> String {
    format!(
        "A mass extinction. The population has fallen from {} to {} over {} ticks, which is more \
         than half of it.",
        grouped(u64::from(from)),
        grouped(u64::from(to)),
        grouped(over)
    )
}

/// ⭐ **C8.** A person changed the conditions of the world.
fn said_conditions(condition: &Condition, from: f32, to: f32) -> String {
    format!(
        "The conditions of this world have been changed by hand: {} is now {to}, where it was \
         {from}.",
        condition.phrase
    )
}

/// ⭐ **C10.** One body is building the same structure more than once.
///
/// ⚠️ **Phrased as what changed, and never as an achievement.** `docs/PHASE7.md` is explicit
/// about the wording - *"a structure is being built more than once in one body"* - and about the
/// reason: serial repetition is the origin of segmentation, which is a landmark in how a body is
/// organised, and a sentence that called it a step upward would be teaching the one thing
/// CLAUDE.md marks load-bearing against.
fn said_repetition(name: Option<&Name>, times: usize, on: CellKind, of: CellKind) -> String {
    let whose = match name {
        Some(name) => format!("{name} is"),
        None => "A lineage that has not been named yet is".to_owned(),
    };

    format!(
        "A structure is being built more than once in one body. {whose} growing {times} copies \
         of one unit along a single body: a {} with a {} attached to it.",
        one(on),
        one(of)
    )
}

/// ⭐ **The loss.** A named lineage has stopped building a kind of cell it used to build.
///
/// ⚠️ **This is the detector that stops the log being structurally biased**, and CLAUDE.md is
/// why it exists rather than SPEC:
///
/// > *"Loss of structure is a legitimate and common outcome: a lineage that abandons
/// > photosynthesis to parasitise its neighbours has not regressed. If the event log can only
/// > celebrate gains, it is teaching something false - and it will make the simulation less
/// > interesting to watch, because half of what happens will go unremarked."*
///
/// So it says *has stopped building*, which is what changed, and then says what the bodies are
/// made of now - which is a description and not a reckoning.
fn said_letting_go(name: &Name, kind: CellKind, carrying: u8) -> String {
    let left = CellKind::ALL
        .iter()
        .enumerate()
        .filter(|(at, _)| carrying & bit(*at) != 0)
        .map(|(_, kind)| many(*kind))
        .collect::<Vec<_>>();

    let made_of = match left.len() {
        0 => "Nothing is left of the bodies it used to grow.".to_owned(),
        1 => format!("Its bodies are made of {} now.", left[0]),
        _ => format!(
            "Its bodies are made of {} and {} now.",
            left[..left.len() - 1].join(", "),
            left[left.len() - 1]
        ),
    };

    format!("{name} has stopped building {}. {made_of}", many(kind))
}

/// One cell of this kind, written the way a sentence wants it.
const fn one(kind: CellKind) -> &'static str {
    match kind {
        CellKind::Photocyte => "photocyte",
        CellKind::Devorocyte => "devorocyte",
        CellKind::Myocyte => "myocyte",
        CellKind::Sclerocyte => "sclerocyte",
        CellKind::Sensocyte => "sensocyte",
        CellKind::Gonocyte => "gonocyte",
    }
}

/// Several cells of this kind.
const fn many(kind: CellKind) -> &'static str {
    match kind {
        CellKind::Photocyte => "photocytes",
        CellKind::Devorocyte => "devorocytes",
        CellKind::Myocyte => "myocytes",
        CellKind::Sclerocyte => "sclerocytes",
        CellKind::Sensocyte => "sensocytes",
        CellKind::Gonocyte => "gonocytes",
    }
}

/// What a cell of this kind does, from SPEC section 6.
///
/// ⚠️ **A sensocyte "reads how the light lies about it" and not "reads the gradient"**, which is
/// the collision this module's documentation describes: *grad* is refused by the shared ban list
/// because of *gradus*, a step or a rank.
const fn does(kind: CellKind) -> &'static str {
    match kind {
        CellKind::Photocyte => "it takes energy out of the light",
        CellKind::Devorocyte => "it drains whatever it is touching that is not its own body",
        CellKind::Myocyte => "it contracts, and so a body that has one can swim",
        CellKind::Sclerocyte => "it is stiff, and it is nine times harder to bite than a photocyte",
        CellKind::Sensocyte => "it reads how the light lies about it, and says so",
        // ⚠️ "holds energy *towards* a child" was the first wording and `toward` is banned - see
        // this module's header. It is the right ban and this is the right rewrite: a gonocyte
        // holds energy *for* a child, which is what it does, where *towards* is a direction.
        CellKind::Gonocyte => "it holds energy for a child, and a body without one has none",
    }
}

/// Which bit of a kind mask stands for the kind at this index of [`CellKind::ALL`].
const fn bit(at: usize) -> u8 {
    1 << at
}

/// A number with its thousands separated, which is how SPEC section 11's own examples write a
/// tick.
fn grouped(count: u64) -> String {
    let digits = count.to_string();
    let mut written = String::with_capacity(digits.len() + digits.len() / 3);

    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            written.push(',');
        }
        written.push(digit);
    }

    written
}

/// How far across a body reaches: the greatest distance between two of its cells, measured the
/// short way round a world that joins up sideways.
///
/// Compared as squares, so there is one square root per body rather than one per pair.
fn span(body: &[Cell], width: f32) -> f32 {
    let mut most = 0.0_f32;

    for (at, one) in body.iter().enumerate() {
        for other in &body[at + 1..] {
            let apart = wrapped_offset(one.pos, other.pos, width);
            most = most.max(apart.dot(apart));
        }
    }

    most.sqrt()
}

/// ⭐⭐ **C10.** Whether one body is building the same structure more than once, and if so how
/// many times and what the structure is.
///
/// # ⭐ Why this is in the log at all, given that SPEC's list does not have it
///
/// SPEC section 11's list of events is deliberate, and this is not on it. It goes in anyway, and
/// the reasoning is the shape of the list rather than an exception to it.
///
/// **SPEC's own list already contains a landmark of exactly this class.** *First adhesion* is not
/// an event in the ledger - nothing is gained or lost, no lineage arrives or goes - and CLAUDE.md
/// calls it *"the origin of multicellularity in this run"*. It is on the list because a change in
/// how bodies are **organised** is worth writing down. Serial repetition is the same kind of
/// thing: it is the origin of **segmentation**, and it is the visible signature of
/// duplicate-and-diverge, which CLAUDE.md's decision log calls *"the single most important
/// decision in the project"* and *"the engine behind essentially all real biological
/// complexity"*. A log that records the first two cells stuck together and says nothing when a
/// lineage starts building the same organ three times over is recording the less interesting of
/// the two.
///
/// And it is cheap, which is the other half of the argument: a body whose structure repeats is
/// knowable from the body, without a search, a history or a second pass over the world.
///
/// # What counts as a repeated structure
///
/// A **unit** is an adhered pair: a cell, and a cell attached to it that is a different kind or
/// in a different developmental state. A body repeats a unit when the same pair - the same two
/// kinds and states, in the same order - hangs off [`REPEATS`] **different** parent cells.
///
/// ⚠️ **A pair whose two ends are the same is not a unit, and that exclusion is what makes this
/// mean anything.** A gene that divides a cell into another cell of its own kind and state makes
/// a chain, and a chain of eight identical photocytes repeats nothing that a chain of two does
/// not. What is being looked for is *a spine with something on it*, which is what
/// `docs/PHASE7.md` records being found by eye at tick 2.8 million: a horizontal spine with
/// regularly spaced branches.
///
/// The kinds and states are read off the **finished** body rather than off the genes that grew
/// it, which is the honest reading - it is a description of the thing standing in the water, and
/// a cell that was differentiated after it was budded is the kind it now is.
fn repeated(body: &[Cell], springs: &[Spring]) -> Option<(usize, CellKind, CellKind)> {
    for (at, spring) in springs.iter().enumerate() {
        let (parent, child) = (body[spring.a], body[spring.b]);
        if parent.kind == child.kind && parent.state == child.state {
            continue;
        }

        // Which parent cells have already been counted for this unit, so that a cell carrying
        // two identical branches counts once. Sixty-four of them is `MOST_CELLS`, on the stack.
        let mut counted = [false; MOST_CELLS];
        let mut times = 0;

        for other in &springs[at..] {
            let (one, two) = (body[other.a], body[other.b]);
            if one.kind != parent.kind
                || one.state != parent.state
                || two.kind != child.kind
                || two.state != child.state
            {
                continue;
            }

            let Some(seen) = counted.get_mut(other.a) else {
                continue;
            };
            if *seen {
                continue;
            }

            *seen = true;
            times += 1;
        }

        if times >= REPEATS {
            return Some((times, parent.kind, child.kind));
        }
    }

    None
}

/// An upper bound on [`span`], in one pass over the body rather than over its pairs.
///
/// ⭐ **This is what keeps the body-size record cheap.** `span` is quadratic in the size of a
/// body, which at SPEC's cap of sixty-four cells is two thousand measurements per body per tick -
/// affordable for the two- and three-celled bodies a settled run actually carries and not for a
/// world of large ones. Every cell is within `d` of the first, so no two of them are more than
/// `2d` apart; a body whose bound does not beat the record cannot beat the record, and nearly
/// every body in a run is one of those.
fn reach(body: &[Cell], width: f32) -> f32 {
    let mut most = 0.0_f32;

    for cell in &body[1..] {
        let apart = wrapped_offset(body[0].pos, cell.pos, width);
        most = most.max(apart.dot(apart));
    }

    2.0 * most.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Vec2;
    use crate::config::{Config, LimitsConfig, spec_defaults};
    use crate::genome::{Action, Gene, Genome, SensorTarget, State};
    use crate::naming::is_permissible;
    use crate::species::PERSISTENCE;

    /// SPEC section 3's own configuration, as a world can be built from it.
    fn shipped() -> Config {
        spec_defaults()
            .validate()
            .expect("SPEC section 3's defaults are a world")
    }

    /// A gene that divides a cell in one state into a daughter of a given kind in another.
    fn divide(from: u8, step: u8, kind: CellKind, into: u8, adhere: bool) -> Gene {
        Gene {
            trigger_state: State::new(from),
            min_step: step,
            max_step: step,
            action: Action::Divide,
            angle: 0.0,
            adhere,
            child_state: State::new(into),
            child_kind: kind,
            rest_length: 8.0,
            stiffness: 10.0,
            new_kind: CellKind::Photocyte,
            new_state: State::ZERO,
            osc_freq: 0.0,
            osc_phase: 0.0,
            sensor_gain: 0.0,
            sensor_target: SensorTarget::Light,
        }
    }

    /// A genome that grows a chain with one cell of every kind in it.
    ///
    /// The seed cell is a photocyte, which SPEC section 7 fixes, so five genes buy the other
    /// five kinds: each fires on the state the one before it handed out, on the step after it.
    fn every_kind(limits: &LimitsConfig) -> Genome {
        Genome::new(
            vec![
                divide(0, 0, CellKind::Devorocyte, 1, true),
                divide(1, 1, CellKind::Myocyte, 2, true),
                divide(2, 2, CellKind::Sclerocyte, 3, true),
                divide(3, 3, CellKind::Sensocyte, 4, true),
                divide(4, 4, CellKind::Gonocyte, 5, true),
            ],
            limits,
        )
    }

    /// A dark world with nothing in it. Every seeding into one has to ask for nothing at all.
    fn dark() -> World {
        World::new(&shipped())
    }

    /// A world the light has been falling on for long enough that a body can be seeded with
    /// something to live on.
    fn lit(ticks: u64) -> World {
        let mut world = dark();
        for _ in 0..ticks {
            world.tick();
        }

        world
    }

    /// Everything the log has said, run together, so a test can look for a phrase in it.
    fn said(log: &Chronicle) -> String {
        log.events().map(Event::line).collect::<Vec<_>>().join("\n")
    }

    /// The events of one kind.
    fn of(log: &Chronicle, kind: Kind) -> Vec<&Event> {
        log.events().filter(|event| event.kind == kind).collect()
    }

    /// The living population, as `species.rs` wants to be handed one.
    fn population(world: &World) -> impl Iterator<Item = (usize, u64, &Genome)> {
        world
            .organisms()
            .iter()
            .enumerate()
            .filter_map(|(slot, organism)| {
                organism
                    .as_ref()
                    .map(|living| (slot, living.serial(), living.genome()))
            })
    }

    /// ⭐ **C1.** An event knows which tick it happened on and what that is in deep time.
    ///
    /// > *Tick 41,208 — 41.2 Ma.*
    ///
    /// Both, and in that order. The tick is what a replay is scrubbed to and the millions of
    /// years are what CLAUDE.md asks a person to read instead of a number going up, and an event
    /// that carried only one of them would be useless to one of the two readers.
    #[test]
    fn an_event_is_recorded_with_its_tick_and_its_deep_time() {
        let world = dark();
        let mut log = Chronicle::new(world.config());

        log.retuned(41_208, world.config(), &louder(0.005));

        let event = log
            .events()
            .next()
            .expect("a condition was changed, so something happened");

        assert_eq!(event.tick, 41_208, "the event does not know its own tick");
        assert!(
            (event.ma - 41.208).abs() < 1e-9,
            "tick 41,208 of a world of a thousand years a tick is 41.208 Ma, and the event says \
             {}",
            event.ma
        );
        assert!(
            event.line().starts_with("Tick 41,208 — 41.2 Ma. "),
            "the line does not read as SPEC section 11's own examples do:\n{}",
            event.line()
        );
    }

    /// SPEC section 3's configuration with the light turned up.
    fn louder(influx: f64) -> Config {
        let mut raw = spec_defaults();
        raw.light.influx = influx;

        raw.validate().expect("a brighter world is still a world")
    }

    /// ⭐ **C2.** A body that is more than one cell is noticed the moment there is one.
    #[test]
    fn first_adhesion_is_noticed() {
        let mut world = dark();
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());

        log.observe(&world, &taxonomy);
        assert!(
            of(&log, Kind::Adhesion).is_empty(),
            "an empty world has had an adhesion event"
        );

        let limits = world.config().limits.clone();
        world
            .seed(every_kind(&limits), Vec2::new(100.0, 100.0), 0.0)
            .expect("an empty world has room for one body asking for nothing");
        log.observe(&world, &taxonomy);

        let noticed = of(&log, Kind::Adhesion);
        assert_eq!(
            noticed.len(),
            1,
            "a body of six adhered cells is in the water and the log says {}:\n{}",
            noticed.len(),
            said(&log)
        );
        assert!(
            noticed[0]
                .said
                .contains("has not separated from its daughter"),
            "the sentence is not SPEC section 11's: {}",
            noticed[0].said
        );
    }

    /// ⭐ **C3.** Every one of SPEC section 6's six kinds of cell is noticed the first time it is
    /// built, and only the first time.
    #[test]
    fn the_first_appearance_of_each_cell_kind_is_noticed() {
        let mut world = dark();
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());

        let limits = world.config().limits.clone();
        world
            .seed(every_kind(&limits), Vec2::new(100.0, 100.0), 0.0)
            .expect("an empty world has room for one body asking for nothing");

        for _ in 0..4 {
            log.observe(&world, &taxonomy);
        }

        let noticed = of(&log, Kind::CellKind);
        assert_eq!(
            noticed.len(),
            KINDS,
            "a body with one cell of every kind in it produced {} first appearances rather than \
             {KINDS}:\n{}",
            noticed.len(),
            said(&log)
        );

        for kind in CellKind::ALL {
            assert!(
                noticed.iter().any(|event| event.said.contains(one(kind))),
                "no line says a {} was ever built:\n{}",
                one(kind),
                said(&log)
            );
        }
    }

    /// ⭐ **C4.** The first mouthful of living tissue anywhere in the world is noticed.
    ///
    /// ⚠️ **A victim with nothing in it is not eaten**, which is what makes this test need a lit
    /// world rather than the dark one every other test here uses: `behaviour.rs` shares out a
    /// body's energy between the mouths on it, and a body holding nothing gives every one of them
    /// nothing. So the light is left to fall first and both bodies are seeded with something.
    #[test]
    fn the_first_predation_is_noticed() {
        let mut world = lit(900);
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());

        let limits = world.config().limits.clone();

        // A photocyte with a devorocyte budded off it, unadhered, so the mouth sits exactly
        // touching its own body and a little way from anything else.
        let mouth = Genome::new(vec![divide(0, 0, CellKind::Devorocyte, 1, false)], &limits);
        world
            .seed(mouth, Vec2::new(400.0, 400.0), 0.2)
            .expect("a lit world can pay for a body");

        // And a plain single photocyte, placed so that it is inside the devorocyte's reach.
        world
            .seed(
                Genome::new(Vec::new(), &limits),
                Vec2::new(410.0, 400.0),
                0.2,
            )
            .expect("a lit world can pay for a body");

        log.observe(&world, &taxonomy);
        assert!(
            of(&log, Kind::Predation).is_empty(),
            "nothing has been eaten yet and the log says it has"
        );

        world.tick();
        assert!(
            world.ledger().predation_total() > 0.0,
            "the two bodies are not close enough to be eating each other, so this test proves \
             nothing"
        );

        log.observe(&world, &taxonomy);
        assert_eq!(
            of(&log, Kind::Predation).len(),
            1,
            "the first mouthful of living tissue in the world was not noticed:\n{}",
            said(&log)
        );

        world.tick();
        log.observe(&world, &taxonomy);
        assert_eq!(
            of(&log, Kind::Predation).len(),
            1,
            "predation is being noticed every time it happens rather than the first time"
        );
    }

    /// A lineage of two bodies, one of which carries a devorocyte and one of which does not, and
    /// the taxonomy that has watched them long enough to name them.
    ///
    /// The two genomes differ in one field of one gene, which
    /// `species.rs`'s threshold puts at a sixteenth of a genome apart - well inside the half that
    /// makes two bodies one species - so both are members of the same cluster.
    fn a_named_lineage() -> (World, Taxonomy, Chronicle) {
        let mut world = dark();
        let limits = world.config().limits.clone();

        world
            .seed(
                Genome::new(vec![divide(0, 0, CellKind::Devorocyte, 1, true)], &limits),
                Vec2::new(100.0, 100.0),
                0.0,
            )
            .expect("an empty world has room");
        world
            .seed(
                Genome::new(vec![divide(0, 0, CellKind::Photocyte, 1, true)], &limits),
                Vec2::new(300.0, 100.0),
                0.0,
            )
            .expect("an empty world has room");

        let taxonomy = Taxonomy::new(world.config());
        let log = Chronicle::new(world.config());

        (world, taxonomy, log)
    }

    /// ⭐ **C5.** A lineage that has been there long enough is recorded **by name**, and so is a
    /// named lineage that is no longer there.
    #[test]
    fn speciation_and_extinction_are_recorded_by_name() {
        let (world, mut taxonomy, mut log) = a_named_lineage();

        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(&world));
            log.observe(&world, &taxonomy);
        }

        let born = of(&log, Kind::Speciation);
        assert_eq!(
            born.len(),
            1,
            "two bodies running near-identical programs were sampled twenty times and the log \
             records {} speciations:\n{}",
            born.len(),
            said(&log)
        );

        let name = taxonomy
            .species()
            .next()
            .and_then(|cluster| cluster.name())
            .expect("twenty samples is SPEC section 11's promotion rule")
            .to_string();
        assert!(
            born[0].said.contains(&name),
            "the speciation line does not name the lineage it is about: {}",
            born[0].said
        );

        // And then nothing is left running that program at all.
        taxonomy.sample(std::iter::empty());
        log.observe(&world, &taxonomy);

        let gone = of(&log, Kind::Extinction);
        assert_eq!(
            gone.len(),
            1,
            "the only lineage in the world is gone and the log records {} extinctions:\n{}",
            gone.len(),
            said(&log)
        );
        assert!(
            gone[0].said.contains(&name),
            "the extinction line does not name the lineage it is about: {}",
            gone[0].said
        );
    }

    /// ⭐ **The loss.** A lineage that has stopped building a kind of cell is noticed.
    ///
    /// ⚠️ **This is the detector that keeps the log from being structurally biased**, and it is
    /// the one test in this module that is about the *register* rather than about a mechanism.
    /// CLAUDE.md: *"If the event log can only celebrate gains, it is teaching something false -
    /// and it will make the simulation less interesting to watch, because half of what happens
    /// will go unremarked."*
    ///
    /// The lineage here is two bodies whose growth programs differ in one field: one grows a
    /// devorocyte and one does not. While both are in the sample the lineage builds devorocytes;
    /// when the one that does is no longer in the population, it does not - and the log has to
    /// say so.
    #[test]
    fn a_lineage_that_stops_building_a_cell_kind_is_noticed() {
        let (world, mut taxonomy, mut log) = a_named_lineage();

        for _ in 0..PERSISTENCE + u32::from(SETTLED) {
            taxonomy.sample(population(&world));
            log.observe(&world, &taxonomy);
        }
        assert!(
            of(&log, Kind::LettingGo).is_empty(),
            "the lineage still has a devorocyte in it:\n{}",
            said(&log)
        );

        // The body that grows the devorocyte is no longer in the population. Slot 1 is the one
        // that never had one.
        for _ in 0..=SETTLED {
            taxonomy.sample(population(&world).filter(|(slot, _, _)| *slot == 1));
            log.observe(&world, &taxonomy);
        }

        let let_go = of(&log, Kind::LettingGo);
        assert_eq!(
            let_go.len(),
            1,
            "the lineage has stopped building devorocytes and the log records {} such lines:\n{}",
            let_go.len(),
            said(&log)
        );
        assert!(
            let_go[0].said.contains("has stopped building devorocytes"),
            "the line does not say what changed: {}",
            let_go[0].said
        );
        assert!(
            let_go[0].said.contains("photocytes"),
            "the line does not say what the bodies are made of now: {}",
            let_go[0].said
        );
    }

    /// ⭐ **C6.** SPEC section 11's four records are noticed when they are broken.
    #[test]
    fn new_records_are_noticed() {
        let mut world = dark();
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());
        let limits = world.config().limits.clone();

        // One body of two cells, which primes the records rather than announcing them: a run's
        // first reading has not changed from anything.
        world
            .seed(
                Genome::new(vec![divide(0, 0, CellKind::Gonocyte, 1, true)], &limits),
                Vec2::new(100.0, 100.0),
                0.0,
            )
            .expect("an empty world has room");
        log.observe(&world, &taxonomy);
        assert!(
            of(&log, Kind::Record).is_empty(),
            "the first bodies of a run were announced as records:\n{}",
            said(&log)
        );

        // And then a body that is longer, larger and further across than that one.
        world
            .seed(every_kind(&limits), Vec2::new(500.0, 100.0), 0.0)
            .expect("an empty world has room");
        log.observe(&world, &taxonomy);

        let records = of(&log, Kind::Record);
        let all = records
            .iter()
            .map(|event| event.said.clone())
            .collect::<Vec<_>>()
            .join("\n");
        for wanted in ["cells", "genes", "world units"] {
            assert!(
                all.contains(wanted),
                "nothing in the log records a new {wanted} figure:\n{all}"
            );
        }

        // The population doubles from one body to two, which is the ladder `CROWD_STEP` sets.
        assert!(
            all.contains("organisms in the water"),
            "the population doubled and the log did not say so:\n{all}"
        );
    }

    /// ⭐ **C7.** A population that halves inside five thousand ticks is noticed.
    ///
    /// ⚠️ **It has its own window and this is why.** `series.rs` thins its population history to
    /// a stride of 25,600 ticks late in a run, so a collapse that happened and was over inside
    /// SPEC's five thousand would be a single step between two adjacent samples with no shape to
    /// measure at all.
    #[test]
    fn a_mass_extinction_is_noticed() {
        let mut world = dark();
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());
        let limits = world.config().limits.clone();

        for founder in 0_u8..8 {
            world
                .seed(
                    Genome::new(vec![divide(0, 0, CellKind::Gonocyte, 1, true)], &limits),
                    Vec2::new(100.0 + f32::from(founder) * 100.0, 100.0),
                    0.0,
                )
                .expect("an empty world has room");
        }

        // Tick zero, with everybody in the water.
        log.observe(&world, &taxonomy);

        // A dark world gives a body holding nothing no way to pay its upkeep, so the whole
        // population is gone within a tick or two. A hundred ticks later the window has a second
        // reading in it.
        for _ in 0..GRAIN {
            world.tick();
        }
        assert_eq!(
            world.organisms().iter().flatten().count(),
            0,
            "the founders are still alive, so nothing has collapsed and this test proves nothing"
        );
        log.observe(&world, &taxonomy);

        let collapse = of(&log, Kind::MassExtinction);
        assert_eq!(
            collapse.len(),
            1,
            "a population of eight fell to nothing inside a hundred ticks and the log records {} \
             mass extinctions:\n{}",
            collapse.len(),
            said(&log)
        );
        assert!(
            collapse[0].said.contains("fallen from 8 to 0"),
            "the line does not say what the population did: {}",
            collapse[0].said
        );
    }

    /// ⭐ **C8.** A change a person made to the conditions is an event like any other.
    ///
    /// SPEC section 3: *"the rest can be changed live, **which is how environmental events
    /// work**"*, and SPEC section 11 lists them among the things worth recording.
    #[test]
    fn a_change_the_user_made_is_recorded() {
        let world = dark();
        let mut log = Chronicle::new(world.config());

        let before = world.config().clone();
        let mut raw = spec_defaults();
        raw.light.influx = 0.004;
        raw.metabolism.upkeep_scale = 3.0;
        let after = raw.validate().expect("a warmer, brighter world is a world");

        log.retuned(120_000, &before, &after);

        let changes = of(&log, Kind::Conditions);
        assert_eq!(
            changes.len(),
            2,
            "two sliders moved and the log records {} changes:\n{}",
            changes.len(),
            said(&log)
        );

        let all = said(&log);
        assert!(
            all.contains("the light reaching the water"),
            "the log does not say the light was changed:\n{all}"
        );
        assert!(
            all.contains("what it costs a cell simply to be alive"),
            "the log does not say the upkeep was changed:\n{all}"
        );

        // And a retune that changed nothing is not an event.
        log.retuned(120_100, &after, &after);
        assert_eq!(
            of(&log, Kind::Conditions).len(),
            2,
            "a configuration reapplied unchanged was recorded as an environmental event"
        );
    }

    /// ⭐ **C10.** A body that builds the same structure three times over is noticed.
    ///
    /// This is what `docs/PHASE7.md` records Jonathan finding by eye at tick 2.8 million: a spine
    /// with regularly spaced branches, the same unit built several times.
    ///
    /// The genome here grows one: three steps of a state-nought photocyte dividing into another
    /// state-nought photocyte make a spine of eight, and a fourth step in which every one of them
    /// buds a myocyte makes eight copies of one two-cell unit. The spine's own springs join two
    /// cells of the same kind and state and are not a repeated *structure* - see
    /// [`Chronicle::repetition`].
    #[test]
    fn serial_repetition_is_noticed() {
        let mut world = dark();
        let mut log = Chronicle::new(world.config());
        let taxonomy = Taxonomy::new(world.config());
        let limits = world.config().limits.clone();

        let plain = Genome::new(vec![divide(0, 0, CellKind::Photocyte, 0, true)], &limits);
        world
            .seed(plain, Vec2::new(100.0, 100.0), 0.0)
            .expect("an empty world has room");
        log.observe(&world, &taxonomy);
        assert!(
            of(&log, Kind::Repetition).is_empty(),
            "a body of two identical cells is not a repeated structure:\n{}",
            said(&log)
        );

        let mut segmented = divide(0, 0, CellKind::Photocyte, 0, true);
        segmented.max_step = 2;
        let branch = divide(0, 3, CellKind::Myocyte, 1, true);
        world
            .seed(
                Genome::new(vec![segmented, branch], &limits),
                Vec2::new(600.0, 100.0),
                0.0,
            )
            .expect("an empty world has room");
        log.observe(&world, &taxonomy);

        let noticed = of(&log, Kind::Repetition);
        assert_eq!(
            noticed.len(),
            1,
            "a body carrying eight copies of one two-cell unit was noticed {} times:\n{}",
            noticed.len(),
            said(&log)
        );
        assert!(
            noticed[0]
                .said
                .contains("A structure is being built more than once in one body"),
            "the wording is not `docs/PHASE7.md`'s: {}",
            noticed[0].said
        );
        assert!(
            noticed[0].said.contains("myocyte"),
            "the line does not say what is being built repeatedly: {}",
            noticed[0].said
        );
    }

    /// ⚠️ **The log is bounded, and it says what it dropped.**
    ///
    /// An unbounded log is the leak CLAUDE.md's *"allocate once, never grow"* rule exists to
    /// prevent, and SPEC section 13's *"keep everything"* is a rule about a file rather than about
    /// memory. SPEC section 13 is also explicit about the other half: *"Log what was dropped;
    /// silent truncation reads as complete history when it isn't."*
    #[test]
    fn the_log_is_bounded_and_says_what_it_dropped() {
        let world = dark();
        let mut log = Chronicle::new(world.config());

        let over = CAPACITY + 200;
        let mut before = world.config().clone();
        for at in 0..over {
            let mut raw = spec_defaults();
            // Every step is a different light, so every one of them is a change.
            // Every step is a light this world has not had before, so every one of them is a
            // change. From `at + 1`, because the world already has the light of step nought and
            // a configuration reapplied unchanged is deliberately not an event.
            raw.light.influx =
                0.001 + f64::from(u32::try_from(at + 1).expect("a countable number")) * 1e-6;
            let after = raw
                .validate()
                .expect("a slightly brighter world is a world");

            log.retuned(
                u64::try_from(at).expect("a countable number"),
                &before,
                &after,
            );
            before = after;
        }

        assert_eq!(
            log.events().len(),
            CAPACITY,
            "the log holds {} events, and it is allocated once at {CAPACITY} and never resized",
            log.events().len()
        );
        assert_eq!(
            log.dropped(),
            u64::try_from(over - CAPACITY).expect("a countable number"),
            "the log dropped events off its front and does not say how many"
        );
        assert_eq!(
            log.events().next().expect("the log is full").tick,
            u64::try_from(over - CAPACITY).expect("a countable number"),
            "the log dropped something other than its oldest events"
        );
    }

    /// ⭐⭐ **C9. The register test.**
    ///
    /// CLAUDE.md marks the non-teleological rule load-bearing, and `docs/PHASE7.md` says why a
    /// test rather than care: *"A banned-vocabulary test over every generated string is cheap and
    /// is the only thing that stops the register drifting as copy is added later."*
    ///
    /// ⭐ **It calls `naming::is_permissible` rather than growing a list of its own.** Two lists
    /// of banned words are two lists that come apart, and the register of the event log and the
    /// register of the names are one register. `naming.rs`'s
    /// `the_banned_vocabulary_is_claude_mds_whole_list` is what holds the list itself to
    /// CLAUDE.md's ten phrases.
    ///
    /// ⚠️ **Every sentence this module can write is generated here**, and not merely the ones a
    /// short run happens to produce: all six cell kinds, all twenty conditions, all four records,
    /// both halves of every named line and both the named and unnamed forms of the ones that can
    /// go either way. A test that only checked a run's own log would pass for years and then let
    /// a sentence through the first time some rare event fired.
    #[test]
    fn no_event_text_uses_the_banned_vocabulary() {
        let mut every = vec![
            said_adhesion(2),
            said_adhesion(64),
            said_predation(),
            said_collapse(2_140, 812, 4_900),
        ];

        for kind in CellKind::ALL {
            every.push(said_kind(kind));
            every.push(one(kind).to_owned());
            every.push(many(kind).to_owned());
            every.push(does(kind).to_owned());
        }

        for what in Record::ALL {
            every.push(said_record(what, "1,024"));
        }

        for condition in &CONDITIONS {
            every.push(said_conditions(condition, 0.5, 0.25));
            every.push(condition.phrase.to_owned());
        }

        // Names, which are the other half of what a line of this log is made of. A real one,
        // taken from the naming machinery rather than written here, so the register test covers
        // the words a run will actually print.
        let mut nomenclature = crate::naming::Nomenclature::new(42);
        let name = nomenclature.mint(0x9e37_79b9_7f4a_7c15, None);

        every.push(said_speciation(&name, 47));
        every.push(said_extinction(&name));
        for kind in CellKind::ALL {
            every.push(said_letting_go(&name, kind, 0));
            every.push(said_letting_go(&name, kind, 0b0011_1111));
            every.push(said_letting_go(&name, kind, 0b0000_0001));
            every.push(said_repetition(Some(&name), 8, kind, CellKind::Myocyte));
            every.push(said_repetition(None, 3, CellKind::Photocyte, kind));
        }

        for said in &every {
            assert!(
                is_permissible(&said.to_lowercase()),
                "the event log can print this, and it uses a word this project may not:\n{said}"
            );
        }

        // ⚠️ And the filter is doing something. A test that ran a permissive check over a
        // hundred strings would pass identically if `is_permissible` answered yes to everything.
        assert!(
            !is_permissible("this lineage is more advanced than the last"),
            "the filter these sentences were checked against accepts everything"
        );
    }

    /// Every kind of event has a tag that will not change and a place in [`Kind::ALL`].
    ///
    /// The tags go into Phase 8's `events.jsonl`, so an archived run is readable for exactly as
    /// long as they mean what they meant.
    #[test]
    fn every_kind_of_event_has_its_own_tag() {
        let mut tags = Kind::ALL.map(Kind::tag).to_vec();
        tags.sort_unstable();
        tags.dedup();

        assert_eq!(
            tags.len(),
            Kind::ALL.len(),
            "two kinds of event write the same tag into the replay log"
        );
        for tag in tags {
            assert!(!tag.is_empty(), "a kind of event has no tag at all");
        }
    }

    /// Looking at one world twice writes one log.
    ///
    /// [`Chronicle::observe`] has no guard against being handed a tick twice - see its own note -
    /// so the claim is that it does not need one.
    #[test]
    fn nothing_is_noticed_twice() {
        let mut world = dark();
        let limits = world.config().limits.clone();
        world
            .seed(every_kind(&limits), Vec2::new(100.0, 100.0), 0.0)
            .expect("an empty world has room");

        let taxonomy = Taxonomy::new(world.config());
        let mut once = Chronicle::new(world.config());
        let mut twice = Chronicle::new(world.config());

        for _ in 0..8 {
            once.observe(&world, &taxonomy);
        }
        for _ in 0..16 {
            twice.observe(&world, &taxonomy);
        }

        assert_eq!(
            said(&once),
            said(&twice),
            "a world looked at twice as often produced a different log"
        );
        assert!(
            !said(&once).is_empty(),
            "neither log says anything at all, so comparing them proves nothing"
        );
    }

    /// ⭐⭐ **Watching a world write its own history does not change what the world does.**
    ///
    /// Two worlds of one seed and one configuration are ticked side by side, one with a
    /// [`Chronicle`] and a [`Taxonomy`] reading it after every tick and one with nothing watching
    /// at all, and then compared **number for number**: every tile of the resource field, and the
    /// position, velocity, radius, energy flow and developmental state of every cell of every
    /// body, as bit patterns.
    ///
    /// This is `species.rs`'s `clustering_does_not_change_what_the_world_does` said again for the
    /// third observer, and it exists for the reason that one does: the failure is silent. A log
    /// that drew a single random number from the world's generator, or that sorted an arena to
    /// make its own work easier, would leave a run **perfectly deterministic and
    /// deterministically different**, and nothing anywhere would say why.
    /// `run.rs`'s `a_run_produces_what_it_produced_before_group_a` says it from the other end,
    /// against numbers recorded before any of Phase 7 existed.
    ///
    /// ⚠️ **Bit patterns rather than a tolerance.** A difference in the last place is exactly how
    /// a determinism failure starts, and it is the only kind this could produce.
    #[test]
    fn the_chronicle_does_not_change_what_the_world_does() {
        let mut watched = a_small_living_world();
        let mut alone = a_small_living_world();
        let mut taxonomy = Taxonomy::new(watched.config());
        let mut log = Chronicle::new(watched.config());

        for _ in 0..1_200 {
            watched.tick();
            taxonomy.observe(&watched);
            log.observe(&watched, &taxonomy);
            alone.tick();
        }

        assert!(
            log.events().len() > 2,
            "the log noticed almost nothing, so this would pass against an observer that never \
             ran:\n{}",
            said(&log)
        );
        assert_eq!(
            watched.ticks(),
            alone.ticks(),
            "the two worlds did not take the same number of ticks"
        );
        assert_eq!(
            every_number_in(&watched),
            every_number_in(&alone),
            "a world that was writing its own history is not the world that was not, so the \
             event log is changing the run rather than reading it"
        );
    }

    /// A world small enough to tick in a test, with a population in it.
    ///
    /// The light is brighter than the shipped configuration's, and only so that the dawn is
    /// short: what this is for is having bodies feeding, moving, breeding and dying while
    /// something watches, and none of the claims made about it care what the ecology does.
    fn a_small_living_world() -> World {
        let mut raw = spec_defaults();
        raw.world.width = 512.0;
        raw.world.height = 288.0;
        raw.world.grid_cols = 64;
        raw.world.grid_rows = 36;
        raw.limits.max_organisms = 250;
        raw.light.influx = 0.012;

        let config = raw.validate().expect("a small world is a world");
        let mut world = World::new(&config);
        for _ in 0..1_500 {
            world.tick();
        }

        for founder in 0_u8..6 {
            world
                .seed(
                    Genome::new(
                        vec![divide(0, 0, CellKind::Gonocyte, 1, true)],
                        &config.limits,
                    ),
                    Vec2::new(40.0 + f32::from(founder) * 70.0, 140.0),
                    2.0,
                )
                .expect("a lit world can pay for six founders");
        }

        world
    }

    /// Every number in a world, as bit patterns: `species.rs`'s own whole-world signature.
    fn every_number_in(world: &World) -> Vec<u32> {
        let mut written: Vec<u32> = world
            .grid()
            .tiles()
            .iter()
            .map(|tile| tile.to_bits())
            .collect();

        for slot in 0..world.organisms().len() {
            for cell in world.cells_of(slot) {
                written.extend([
                    cell.pos.x.to_bits(),
                    cell.pos.y.to_bits(),
                    cell.vel.x.to_bits(),
                    cell.vel.y.to_bits(),
                    cell.radius.to_bits(),
                    cell.energy_flow.to_bits(),
                    u32::from(cell.state),
                ]);
            }
        }

        written
    }

    /// A tick is written with its thousands separated, as SPEC section 11's own examples are.
    #[test]
    fn a_tick_is_written_the_way_spec_writes_one() {
        for (count, written) in [
            (0_u64, "0"),
            (7, "7"),
            (999, "999"),
            (1_000, "1,000"),
            (41_208, "41,208"),
            (210_880, "210,880"),
            (2_800_000, "2,800,000"),
        ] {
            assert_eq!(grouped(count), written, "{count} is written wrongly");
        }
    }
}
