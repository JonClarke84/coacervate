//! Telling one lineage from another.
//!
//! See `SPEC.md` section 11 for the rules this implements:
//!
//! > *"Every 500 ticks, cluster the living population by distance with a threshold; a cluster
//! > that persists for 20 consecutive samples is promoted to a named species."*
//!
//! `genome.rs` supplies the distance. This is everything built on top of it: who is near whom,
//! which of those groups is the *same* group it was five hundred ticks ago, and which of them has
//! been around long enough to be worth a name.
//!
//! # ⚠️ This is an observer. It reads the world and it does not touch it
//!
//! [`Taxonomy::observe`] takes a `&World` and gives it back unchanged, exactly as
//! `coacervate_render::series::Series` does. It draws no random numbers, it visits the organism
//! arena in index order and reorders nothing, and it holds all of its own state.
//!
//! That is not tidiness, it is the whole safety argument for adding it. Everything in this
//! project is a deterministic function of a seed and a configuration, and an observer that drew
//! one number from the world's generator - or that sorted an arena to make its own work easier -
//! would leave a run perfectly deterministic and **deterministically different**. Every figure
//! recorded in `docs/PHASE4.md`, every frame dumped, and every archived run would quietly stop
//! describing the program that produced them, and nothing anywhere would say why.
//! `a_run_produces_what_it_produced_before_group_a` is the golden vector that exists to catch
//! precisely that, and `clustering_does_not_change_what_the_world_does` says it from this end.
//!
//! # Why a cluster is not recomputed from nothing at every sample
//!
//! The obvious implementation runs a clustering algorithm over the living population every 500
//! ticks and hands back a set of groups. It is also useless, and the reason is that **SPEC's
//! promotion rule is about the same cluster persisting**, and a set of groups computed twice
//! from scratch has no way of saying which of this sample's groups is which of the last one's.
//! Numbering them by size, or by the order they came out, gives a "species" that changes
//! identity whenever two lineages swap places in the population - which happens constantly.
//!
//! So identity is **carried**, not derived. Each cluster keeps a **representative genome**, and
//! at the next sample every living organism joins the cluster whose representative it is nearest
//! to, provided it is within [`THRESHOLD`] of it. A cluster survives as long as anybody joins it;
//! a genome that is further than the threshold from every representative there is mints a new
//! cluster. See [`Taxonomy::observe`] for what stops that churning.
//!
//! # The bookkeeping is dense arrays and a sorted list, on purpose
//!
//! `clippy.toml` refuses `HashMap` and `HashSet` inside this crate, and SPEC section 2 gives the
//! reason: *"no map iteration may affect simulation state"*. Iteration order over a hash map is
//! unspecified, so a clustering that walked one would put the same population into different
//! groups on two runs of the same seed. Everything here is either indexed by organism slot or
//! held in a vector kept in ascending order of identifier, which is searched rather than walked.

use crate::config::Config;
use crate::genome::Genome;
use crate::naming::{Name, Nomenclature};
use crate::world::World;

/// How many ticks pass between two samples of the population. SPEC section 11's own number.
///
/// Five times the stride of `series.rs`'s chart, which is the other periodic observer in the
/// project, and both are grids on the **world's own tick count** rather than counts of calls -
/// so a run watched through a window samples the same ticks as the same run headless.
pub const EVERY: u64 = 500;

/// How many consecutive samples a cluster must persist through before it is a species. SPEC
/// section 11's own number.
///
/// Twenty samples is ten thousand ticks, which is ten million years at SPEC section 3's shipped
/// `years_per_tick`. What it buys is worth stating plainly: **it is the whole of the defence
/// against naming things that are not there.** A single organism whose genome wandered past the
/// threshold mints a cluster, and if it leaves no descendants that cluster is gone by the next
/// sample having never been anything. Only a group that was still there twenty samples later
/// gets a name.
pub const PERSISTENCE: u32 = 20;

/// How far apart two genomes have to be to belong to different clusters.
///
/// ⭐ **SPEC section 11 asks for "a threshold" and does not say what it is, so this number decides
/// what counts as a species.** Read as a sentence it says: *two growth programs are one species
/// if they agree about at least half of themselves.* [`Genome::distance_from`] is normalised over
/// the longer gene list, so a half is literally a half - two genomes of four genes each that
/// differ entirely at two positions are exactly on the line.
///
/// # ⭐ It was measured against a real run rather than chosen
///
/// Half is a much coarser line than it first looks like it should be, and a quarter was tried
/// first and is wrong. What settled it was counting the groups a living population actually falls
/// into at SPEC's shipped `config/default.toml`, seed 42, headless:
///
/// | Tick | Population | Mean genome | at 0.25 | at 0.35 | at 0.40 | **at 0.50** | at 0.60 | at 0.70 |
/// | --- | --- | --- | --- | --- | --- | --- | --- | --- |
/// | 50,000 | 2,038 | 2.0 genes | 107 | 46 | 28 | **15** | 9 | 6 |
/// | 100,000 | 2,208 | 2.6 genes | 277 | 108 | 65 | **20** | 8 | 6 |
/// | 200,000 | 2,142 | 3.2 genes | 477 | 225 | 133 | **34** | 13 | 6 |
/// | 400,000 | 820 | 7.3 genes | 236 | 153 | 112 | **37** | 12 | 4 |
///
/// At a quarter a settled world has **several hundred** groups, and after twenty samples most of
/// them are species. That is not a finding about the world, it is a resolution failure: SPEC
/// section 11 exists so that *"lineages are things you can refer to rather than coloured dots"*,
/// and a list of four hundred names is a list nobody reads. It is also expensive - a pass is
/// `population × clusters`, so four hundred groups is fourteen times the arithmetic thirty is.
///
/// At a half the same run reads as fifteen to forty lineages and **rises slowly as the world
/// diversifies**, which is the shape worth having: a number a person can hold, that means more
/// when it goes up. At six tenths and above it collapses to under a dozen and stops moving, which
/// is a measure that has stopped resolving anything.
///
/// The line also sits just under the *median* distance between two organisms taken at random,
/// which runs from 0.53 early to 0.69 late. So a species is a group that is closer together than
/// two organisms of this world usually are - which is about the plainest thing a threshold on a
/// normalised distance can be asked to mean.
///
/// # What it costs a lineage to leave a species
///
/// On the three-gene genomes a settled run carries: a point mutation on a discrete field is about
/// 0.02 and on a numeric field far less again, one gene gained or lost is 0.25 to 0.33, and a
/// wholly different program is 1.0. So **a single indel does not split a lineage and two do**, and
/// point mutations accumulate for a very long time before they split anything. That is the
/// behaviour worth having: a species is a group that has stopped running the same program, not a
/// group that has stopped being identical.
///
/// ⚠️ **A neutral duplication is not neutral to this**, and that is a consequence rather than an
/// oversight. SPEC section 7's whole-genome duplication appends a copy of the genome and the body
/// that grows is exactly the body that grew before, but position by position the copy shares half
/// its positions with nothing - so the measure calls it half a genome away, which is exactly on
/// this line. The distance is over the *program* and not over the body, and
/// [`Genome::distance_from`] explains why it is read that way. It at least says the same thing the
/// hue does: `organism.rs`'s marker drifts by the same alignment, so a lineage that duplicated its
/// genome moves on screen as well as in the list.
pub const THRESHOLD: f32 = 0.5;

/// ⭐ **B3.** How far a new species has to be from its nearest named relative before it is given a
/// genus of its own rather than joining that relative's.
///
/// SPEC section 11 asks for *"a sufficiently large jump"* and, as with [`THRESHOLD`], does not say
/// how large. So this number decides how a run's list of names reads: too small and every species
/// is its own genus, which makes the first half of every binomial noise; too large and every
/// lineage in the run shares one genus, which makes it silent.
///
/// # It was read off the same measurement [`THRESHOLD`] was
///
/// `docs/PHASE7.md` records how many groups the living population of the shipped
/// `config/default.toml`, seed 42, falls into at a range of radii, at four points in one run:
///
/// | Tick | Population | at 0.50 | at 0.60 | **at 0.70** |
/// | --- | --- | --- | --- | --- |
/// | 50,000 | 2,038 | 15 | 9 | **6** |
/// | 100,000 | 2,208 | 20 | 8 | **6** |
/// | 200,000 | 2,142 | 34 | 13 | **6** |
/// | 400,000 | 820 | 37 | 12 | **4** |
///
/// The column that made this the number is the last one: **at seven tenths the run holds four to
/// six neighbourhoods at every point measured, while the species count under it climbs from
/// fifteen to thirty-seven.** That is a taxonomy - a handful of genera, several species in each,
/// and the genus count moving only when something genuinely far away appears. At six tenths there
/// are eight to thirteen, which is close enough to the species count that a genus stops grouping
/// anything.
///
/// **And then the shipped run was made and read.** Two hundred thousand ticks, seed 42, headless:
/// 60 groups, **55 of them named species, in 4 genera**. The table predicted four to six and the
/// run produced four, which is the check worth having, because the table counts a clustering done
/// from nothing at one moment and this is a run carrying its identities forward for two hundred
/// thousand ticks.
///
/// ⚠️ **One of those four genera holds 48 of the 55 species.** That is worth writing down and it
/// is not a defect: at this radius the living population of this run genuinely is one large
/// neighbourhood with three small ones outside it, which is what a world in the middle of a single
/// radiation looks like. A genus inherited from the *nearest* named relative also spreads
/// transitively - A is near B and B is near C without A being near C - so a genus is a chain of
/// relatives rather than a ball, exactly as SPEC's *"inherits its genus from its parent species"*
/// describes. If a later run ever reads as one genus and nothing else, this is the number to
/// revisit, and the table above says six tenths is where it goes.
///
/// # ⚠️ It has to be larger than [`THRESHOLD`], and that is not a stylistic preference
///
/// A cluster is minted precisely because its founder was further than [`THRESHOLD`] from every
/// representative in the world. So at the moment a lineage becomes a cluster it is already half a
/// genome from all of its neighbours, and a genus boundary at or below a half would mint a new
/// genus for very nearly every species there has ever been. The binomial would then carry exactly
/// as much information as the epithet alone.
///
/// Seven tenths also sits just above the *median* distance between two organisms of this world
/// taken at random, which `docs/PHASE7.md` measures at 0.53 early in a run and 0.69 late. Read as
/// a sentence, that is: **a lineage founds a genus when it is further from everything named than
/// two organisms of this world usually are from each other.** Which is about the plainest thing
/// "a sufficiently large jump" can be asked to mean.
pub const GENUS: f32 = 0.7;

/// Which cluster an organism was found in, and which organism that was.
///
/// ⚠️ **The serial is what makes this answerable at all.** A slot is a place in the arena and is
/// handed to whoever is born there next, so a note filed under a slot alone would come to name
/// whichever stranger moved in after the organism that was clustered died - which for most
/// organisms happens well inside the five hundred ticks between two samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Membership {
    /// Which organism this was. See [`crate::organism::Organism::serial`].
    serial: u64,

    /// Which cluster it joined, by [`Cluster::id`].
    cluster: u32,
}

/// A group of living organisms whose growth programs are near one another, and which is the same
/// group it was at the last sample.
///
/// It becomes a **species** once it has been there for [`PERSISTENCE`] consecutive samples; see
/// [`Cluster::is_species`].
#[derive(Debug)]
pub struct Cluster {
    /// Which cluster this is, for as long as the run lasts.
    ///
    /// Minted once and never reused, exactly as an organism's serial is. A cluster that goes and
    /// a cluster that arrives are two clusters even if they are the same shape, because the run
    /// of consecutive samples that SPEC section 11's promotion counts was broken in between.
    id: u32,

    /// The genome this cluster is a neighbourhood of: what an organism has to be within
    /// [`THRESHOLD`] of to belong.
    ///
    /// ⭐ **This is where a cluster's identity actually lives**, and it moves. See
    /// [`Taxonomy::observe`] for the rule that moves it and for why it is not left where it
    /// started.
    representative: Genome,

    /// How many living organisms were in it at the last sample.
    members: u32,

    /// How many consecutive samples it has had at least one member at.
    ///
    /// Never reset, because a cluster that has no members is *removed* rather than kept at
    /// nought. "Consecutive" is therefore a property of the cluster existing at all rather than a
    /// counter somebody has to remember to clear.
    seen: u32,

    /// The nearest living member found so far during the sample being taken, and how far away it
    /// was.
    ///
    /// Working room rather than a fact about the cluster: it is emptied at the start of every
    /// sample and read at the end of it, and between those two moments it is what
    /// [`Taxonomy::observe`] uses to move the representative. Kept on the cluster rather than in
    /// a second vector beside it so that there is no parallel array to fall out of step.
    nearest: Option<(f32, Genome)>,

    /// ⭐ **B1.** What this lineage is called, once it has been there long enough to be worth
    /// calling anything.
    ///
    /// Nothing at all until [`Cluster::is_species`], and never nothing again afterwards: a name is
    /// minted once, at promotion, and the cluster carries it until it goes extinct. See
    /// [`Taxonomy::christen`] for where it comes from.
    name: Option<Name>,
}

impl Cluster {
    /// Which cluster this is.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// The genome it is a neighbourhood of.
    #[must_use]
    pub const fn representative(&self) -> &Genome {
        &self.representative
    }

    /// How many living organisms were in it at the last sample.
    #[must_use]
    pub const fn members(&self) -> u32 {
        self.members
    }

    /// How many consecutive samples it has been there for.
    #[must_use]
    pub const fn seen(&self) -> u32 {
        self.seen
    }

    /// Whether it has been there long enough to be a species: SPEC section 11's twenty
    /// consecutive samples.
    #[must_use]
    pub const fn is_species(&self) -> bool {
        self.seen >= PERSISTENCE
    }

    /// ⭐ **B1.** What this lineage is called, or nothing at all if it is not a species yet.
    ///
    /// A cluster that has not persisted for [`PERSISTENCE`] samples has no name, and that is the
    /// honest answer rather than a missing feature: clusters churn - one outlier mints one and it
    /// is gone by the next sample - and a chronicle that named them would be mostly names for
    /// things that were never there.
    #[must_use]
    pub const fn name(&self) -> Option<&Name> {
        self.name.as_ref()
    }
}

/// Every cluster in the world, and which organism is in which.
///
/// The second periodic observer of the run, beside `series.rs`'s chart. Built once, handed every
/// tick, and does its work on one tick in [`EVERY`].
#[derive(Debug)]
pub struct Taxonomy {
    /// Every cluster there is, **in ascending order of identifier**.
    ///
    /// That order is not incidental and [`Taxonomy::species_of`] depends on it: a new cluster is
    /// always appended and its identifier is always larger than every identifier already there,
    /// and removing the empty ones preserves the order of the rest. So the list is sorted without
    /// ever being sorted, and finding a cluster by its identifier is a binary search rather than
    /// a walk.
    ///
    /// **Allocated at twice the population cap and never resized**, which is CLAUDE.md's *"a
    /// simulation that cannot allocate cannot leak"* applied to the one arena this module owns.
    ///
    /// Twice, and the factor is arithmetic rather than headroom. A cluster that survives a sample
    /// had a living member, so at rest there are never more clusters than organisms. **In the
    /// middle of a sample there can be almost twice that**: every cluster from the previous
    /// sample is still in the list - including the ones about to be removed for having nobody in
    /// them - while every organism that is far from all of them mints another. Both counts are
    /// bounded by the population cap, so the sum is bounded by twice it.
    clusters: Vec<Cluster>,

    /// Which cluster the organism in each slot of the arena was found in, in the same slots
    /// `world.rs` uses. Nothing at all for a slot that was empty at the last sample.
    of_slot: Vec<Option<Membership>>,

    /// The next cluster identifier to hand out, which is never handed out twice.
    next_id: u32,

    /// ⭐ **Group B.** Every name this run has handed out, and the genera they are in.
    ///
    /// It outlives the clusters it named: `naming.rs` explains why a name must never be reused
    /// even after the lineage that carried it is extinct.
    names: Nomenclature,

    /// How many samples have been taken, ever.
    ///
    /// ⭐ **Phase 7, `C5`.** `chronicle.rs` reads it and nothing else does. The event log has to do
    /// its own work over the population **once per clustering sample** — a species that arrived and
    /// one that went are differences between two samples, not facts about a tick — and this is the
    /// only thing that says a fresh one has happened. The alternative, testing the tick against
    /// [`EVERY`]'s grid, is the same answer in the ordinary case and a wrong one wherever the
    /// clustering did not actually run.
    taken: u64,

    /// The tick the last sample was taken at, or nothing at all if none has been.
    ///
    /// ⚠️ **This is what stops a tick being counted twice**, and here that matters more than it
    /// does for a chart. Two samples of one tick would put two records on top of each other on a
    /// chart; here they would advance every cluster's run of consecutive samples by two, and
    /// SPEC section 11's twenty samples would arrive early by however many times somebody
    /// happened to call this.
    at: Option<u64>,
}

impl Taxonomy {
    /// An observer with nothing seen yet, sized for the world a configuration describes.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        let slots = usize::try_from(config.limits.max_organisms.get())
            .expect("a population cap fits in a machine word");

        Self {
            clusters: Vec::with_capacity(slots * 2),
            of_slot: vec![None; slots],
            next_id: 0,
            names: Nomenclature::new(config.world.seed),
            taken: 0,
            at: None,
        }
    }

    /// ⭐ **A3 and A4.** Take a sample of the living population if one is due at this tick.
    ///
    /// Called after every tick by whatever is doing the ticking, exactly as `Series::observe` is.
    /// Most calls do nothing at all: the world is read only when its own tick count lands on
    /// [`EVERY`]'s grid.
    ///
    /// # What one sample does, in order
    ///
    /// 1. Every cluster is emptied of members. The clusters themselves stay, with their
    ///    representatives and their runs of consecutive samples.
    /// 2. Every living organism, **in arena order**, joins the cluster whose representative it is
    ///    nearest to, if that is within [`THRESHOLD`]. An organism further than the threshold from
    ///    every representative there is mints a new cluster with its own genome as the
    ///    representative, and that cluster is available to every organism examined after it.
    /// 3. Any cluster nobody joined is removed: that lineage is over, and its run of consecutive
    ///    samples is over with it.
    /// 4. Every surviving cluster has persisted one sample longer, and one that reaches
    ///    [`PERSISTENCE`] is a species.
    /// 5. Each surviving cluster's representative becomes **the living member nearest to it**.
    ///
    /// # ⭐ The four decisions that stop species churning
    ///
    /// This is the part that a naive implementation gets wrong, and every one of these is load-
    /// bearing rather than a refinement.
    ///
    /// **Nearest wins, not first.** An organism within the threshold of two clusters joins the
    /// one it is closer to. Under first-match the arena order would decide, and arena order is a
    /// consequence of who happened to die and free a slot - so two lineages sitting near each
    /// other would trade members every time a birth landed in a different slot.
    ///
    /// **The representative moves to the nearest living member, and no further.** Left where it
    /// started, a cluster would die the moment the whole population had drifted a threshold away
    /// from a genome that no longer exists - and a *new* cluster would be minted in the same
    /// place, so a lineage quietly drifting would be recorded as an unbroken succession of
    /// species arriving and going extinct without anything having split. Moved to the *closest*
    /// member rather than to an arbitrary one, the representative does not move at all while any
    /// organism still carries it, which is the common case, and otherwise moves by the least the
    /// living population allows.
    ///
    /// **A cluster is removed only when it is genuinely empty**, so a lineage that dips to one
    /// member and recovers keeps its identity and its run of samples.
    ///
    /// **And promotion is what turns all of that into a name.** A cluster minted by a single
    /// outlier that leaves no descendants is gone by the next sample having never been anything,
    /// and it takes twenty consecutive samples - ten thousand ticks - before anything is called a
    /// species. Churn in the *clusters* is expected and harmless; churn in the species list is
    /// what SPEC section 11's twenty samples exist to prevent.
    ///
    /// # Two clusters that meet do not merge, and nothing has to handle it
    ///
    /// If two representatives drift onto the same genome, every organism near them joins whichever
    /// is nearer, and ties go to the **older** cluster, because the list is in ascending order of
    /// identifier and the first of an equal pair wins. The younger one is left with no members and
    /// is removed by step 3. That is a merge, resolved toward the identity that has been there
    /// longer, and it falls out of the rules rather than needing one of its own.
    pub fn observe(&mut self, world: &World) {
        if !world.ticks().is_multiple_of(EVERY) || self.at == Some(world.ticks()) {
            return;
        }

        self.at = Some(world.ticks());
        self.sample(
            world
                .organisms()
                .iter()
                .enumerate()
                .filter_map(|(slot, organism)| {
                    organism
                        .as_ref()
                        .map(|living| (slot, living.serial(), living.genome()))
                }),
        );
    }

    /// One sample, over a population given as slots, serials and genomes.
    ///
    /// Separated from [`Taxonomy::observe`] because the promotion rule is about **twenty
    /// consecutive samples**, which is ten thousand ticks of a real world - and a test that had
    /// to tick a world that far to find out whether a cluster is promoted would be a test nobody
    /// runs. Everything about identity and persistence is decided here, from a population that
    /// can be written down.
    ///
    /// ⚠️ **Visible to the rest of the crate since Phase 7 Group C, and only for that reason.**
    /// `chronicle.rs`'s tests have exactly the problem this split was made for: speciation and
    /// extinction *by name* need a promoted species, a promoted species needs twenty samples, and
    /// twenty samples is ten thousand ticks of a real world. Nothing outside a test calls it —
    /// [`Taxonomy::observe`] is still the only thing a run drives.
    pub(crate) fn sample<'a>(
        &mut self,
        population: impl Iterator<Item = (usize, u64, &'a Genome)>,
    ) {
        self.taken += 1;

        for cluster in &mut self.clusters {
            cluster.members = 0;
            cluster.nearest = None;
        }
        for slot in &mut self.of_slot {
            *slot = None;
        }

        for (slot, serial, genome) in population {
            let (at, apart) = match self.nearest(genome) {
                Some(found) => found,
                None => (self.mint(genome), 0.0),
            };

            let cluster = &mut self.clusters[at];
            cluster.members += 1;
            if cluster
                .nearest
                .as_ref()
                .is_none_or(|(sofar, _)| apart < *sofar)
            {
                cluster.nearest = Some((apart, genome.clone()));
            }
            let joined = cluster.id;

            self.of_slot[slot] = Some(Membership {
                serial,
                cluster: joined,
            });
        }

        self.clusters.retain(|cluster| cluster.members > 0);
        for cluster in &mut self.clusters {
            cluster.seen += 1;
            if let Some((_, nearest)) = cluster.nearest.take() {
                cluster.representative = nearest;
            }
        }

        // ⭐ Group B, and it is deliberately the last thing that happens. A cluster is named from
        // the representative it has *after* this sample moved it, and against the other clusters
        // as they are now - so a lineage promoted at the same sample as its neighbour is named
        // against where that neighbour actually is.
        for at in 0..self.clusters.len() {
            if self.clusters[at].is_species() && self.clusters[at].name.is_none() {
                self.clusters[at].name = Some(self.christen(at));
            }
        }
    }

    /// ⭐ **B2 and B3.** A name for the cluster at this index, which has just become a species.
    ///
    /// > SPEC section 11: *"A new species inherits its genus from its parent species and receives a
    /// > new epithet; a sufficiently large jump mints a new genus."*
    ///
    /// # ⚠️ There is no parent species to inherit from, so the nearest one is used instead
    ///
    /// Nothing in Group A records which cluster came out of which, and nothing should. A cluster is
    /// minted by whichever organism happened to be further than [`THRESHOLD`] from every
    /// representative in the world; that organism's own parent may be in any cluster at all, or
    /// dead, or in a cluster that has since been removed. A parentage built on that would be a
    /// record of which slot of the arena was free, which is not ancestry.
    ///
    /// So the genus is inherited from the thing that is actually known, and it is the same measure
    /// the clustering itself is built on: **the nearest named species there is**, if it is within
    /// [`GENUS`]. That is a claim about relatedness rather than about descent, which is what a
    /// genus has always been - a group of species that resemble each other more than they resemble
    /// anything else.
    ///
    /// **Living species only**, and that is a decision. A genus could go on accepting new members
    /// after everything in it was extinct, which would need every named genome kept for the length
    /// of the run - `docs/PHASE7.md`'s cap is 128 genes, so that is kilobytes per name and
    /// unbounded in total. Reading the living clusters costs nothing and says something defensible:
    /// a lineage joins the genus of a relative that is *there*.
    ///
    /// # What the name is made of
    ///
    /// The cluster's identifier and the fingerprint of the representative genome, which
    /// `naming.rs` mixes with the run's seed. **No random number is drawn**, from this observer's
    /// state or from anybody's stream - see that module and this one's opening paragraph for why
    /// that is the constraint the whole group is built around.
    ///
    /// The identifier is in it because it is unique for the life of the run, so two lineages that
    /// are running identical programs still get two names. The genome is in it so that a name is a
    /// fingerprint of the lineage rather than of a counter - two runs that produce the same growth
    /// program in the same place in the sequence call it the same thing.
    fn christen(&mut self, at: usize) -> Name {
        let mine = &self.clusters[at];
        let of = u64::from(mine.id) ^ mine.representative.hash();

        let mut nearest: Option<(f32, &Name)> = None;
        for (other, cluster) in self.clusters.iter().enumerate() {
            let Some(name) = cluster.name.as_ref() else {
                continue;
            };
            if other == at {
                continue;
            }

            let apart = mine.representative.distance_from(&cluster.representative);
            if apart > GENUS {
                continue;
            }

            if nearest.is_none_or(|(sofar, _)| apart < sofar) {
                nearest = Some((apart, name));
            }
        }

        // Cloned so that the borrow of the cluster list ends before the nomenclature is written
        // to. A name is a couple of short words and this happens once in a lineage's existence.
        let parent = nearest.map(|(_, name)| name.clone());

        self.names.mint(of, parent.as_ref())
    }

    /// Which cluster this genome is nearest to and how far away it is, or nothing at all if it is
    /// further than [`THRESHOLD`] from every one of them.
    ///
    /// A ranking rather than a search for the first match; see [`Taxonomy::observe`] for why.
    /// Ties go to the earlier cluster in the list, which is the older one.
    fn nearest(&self, genome: &Genome) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;

        for (at, cluster) in self.clusters.iter().enumerate() {
            let apart = genome.distance_from(&cluster.representative);
            if apart > THRESHOLD {
                continue;
            }

            if best.is_none_or(|(_, sofar)| apart < sofar) {
                best = Some((at, apart));
            }
        }

        best
    }

    /// A new cluster around this genome, appended, and where it landed.
    ///
    /// Appended rather than inserted, which is what keeps [`Taxonomy::clusters`] in ascending
    /// order of identifier without ever sorting it.
    fn mint(&mut self, genome: &Genome) -> usize {
        self.clusters.push(Cluster {
            id: self.next_id,
            representative: genome.clone(),
            members: 0,
            seen: 0,
            nearest: None,
            name: None,
        });
        self.next_id += 1;

        self.clusters.len() - 1
    }

    /// Every cluster in the world, promoted or not, in ascending order of identifier.
    #[must_use]
    pub fn clusters(&self) -> &[Cluster] {
        &self.clusters
    }

    /// ⭐ **A4.** The clusters that have persisted long enough to be species.
    pub fn species(&self) -> impl Iterator<Item = &Cluster> {
        self.clusters.iter().filter(|cluster| cluster.is_species())
    }

    /// How many species there are now.
    ///
    /// The figure `series.rs`'s `Sample::species` records, and the conversion lives here because
    /// this is the module that knows the count is bounded by the population cap.
    #[must_use]
    pub fn species_count(&self) -> u32 {
        u32::try_from(self.species().count())
            .expect("a species needs a member, so there are never more than there are organisms")
    }

    /// ⭐ **A5.** Which species the organism in this slot belongs to, or nothing at all.
    ///
    /// Nothing at all means one of three things, and they are deliberately not told apart: the
    /// slot was empty at the last sample, or a different organism is in it now, or the organism's
    /// cluster has not persisted long enough to be a species. In every one of them the honest
    /// answer to *"which species is this?"* is that there is not one to give.
    ///
    /// ⚠️ **The serial is checked against the one that was clustered**, so an organism born into a
    /// slot since the last sample is not handed the species of whoever was there before it. See
    /// [`Membership`].
    #[must_use]
    pub fn species_of(&self, slot: usize, serial: u64) -> Option<u32> {
        let membership = (*self.of_slot.get(slot)?)?;
        if membership.serial != serial {
            return None;
        }

        let at = self
            .clusters
            .binary_search_by_key(&membership.cluster, Cluster::id)
            .ok()?;

        self.clusters[at].is_species().then_some(membership.cluster)
    }

    /// ⭐ **Group B.** Every name this run has handed out, and the genera they are in.
    #[must_use]
    pub const fn names(&self) -> &Nomenclature {
        &self.names
    }

    /// The tick the last sample was taken at, or nothing at all if none has been.
    #[must_use]
    pub const fn sampled_at(&self) -> Option<u64> {
        self.at
    }

    /// ⭐ **Phase 7, `C5`.** How many samples this observer has taken, ever.
    ///
    /// The event log's cadence. See [`Taxonomy::taken`].
    #[must_use]
    pub const fn samples(&self) -> u64 {
        self.taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{CellKind, Vec2};
    use crate::config::{LimitsConfig, RawConfig, spec_defaults};
    use crate::genome::{Action, Gene, SensorTarget, State};

    /// SPEC's default configuration with some of it changed, checked and ready to build a world
    /// from.
    fn config(change: impl FnOnce(&mut RawConfig)) -> Config {
        let mut raw = spec_defaults();
        change(&mut raw);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
    }

    /// A small, bright world with light already in it and bodies standing in the light.
    ///
    /// ⚠️ **A sixteenth of the shipped world in each direction, and twelve times the light**, which
    /// is `run.rs`'s `a_small_living_world` and `series.rs`'s `seeded` and is here for their
    /// reason: `World::seed` takes a founder's energy out of the tiles it stands on and refuses if
    /// they have not got it, and at the shipped light the dawn is ten thousand ticks of watching
    /// water. No test in this module is about the ecology inside.
    ///
    /// Each founder is seeded with a chain of the length its entry in `founders` names, so the
    /// population starts out as more than one growth program rather than as a clone of a single
    /// one.
    fn a_world_of(founders: &[u8]) -> World {
        let config = config(|raw| {
            raw.world.width = 512.0;
            raw.world.height = 288.0;
            raw.world.grid_cols = 64;
            raw.world.grid_rows = 36;
            raw.limits.max_organisms = 250;
            raw.light.influx = 0.012;
        });
        let mut world = World::new(&config);

        for _ in 0..2_000 {
            world.tick();
        }

        for (at, segments) in founders.iter().enumerate() {
            let along = config.world.width
                * (f32::from(u8::try_from(at).expect("a few founders")) + 0.5)
                / f32::from(u8::try_from(founders.len()).expect("a few founders"));
            world
                .seed(
                    a_chain(*segments, &config.limits),
                    Vec2::new(along, config.world.height * 0.5),
                    2.0,
                )
                .expect("a world lit for two thousand ticks has water for a few small bodies");
        }

        world
    }

    /// A genome that grows a chain of `segments` adhered cells: `world.rs`'s own fixture, cut
    /// down to what this module needs.
    ///
    /// The length of the chain is the length of the genome, which is what makes two of these
    /// genuinely far apart: a chain of two and a chain of five share two gene positions of five,
    /// so three of the five are unmatched and they are well past [`THRESHOLD`].
    fn a_chain(segments: u8, limits: &LimitsConfig) -> Genome {
        let genes = (0..segments)
            .map(|segment| Gene {
                trigger_state: State::new(segment),
                min_step: 0,
                max_step: u8::MAX,
                action: Action::Divide,
                angle: 0.0,
                adhere: true,
                child_state: State::new(segment + 1),
                child_kind: CellKind::Photocyte,
                rest_length: 8.0,
                stiffness: 10.0,
                new_kind: CellKind::Photocyte,
                new_state: State::ZERO,
                osc_freq: 0.0,
                osc_phase: 0.0,
                sensor_gain: 0.0,
                sensor_target: SensorTarget::Light,
            })
            .collect();

        Genome::new(genes, limits)
    }

    /// A gene that disagrees with every gene [`a_chain`] builds about all sixteen of its fields.
    ///
    /// Written out rather than derived so that the drift in
    /// `a_species_survives_drift_and_splits_only_when_the_population_does` is a genuinely large
    /// step: replacing one gene of eight with this moves the genome about an eighth, and replacing
    /// all eight moves it nearly the whole way - which is what makes "the lineage drifted right
    /// across a threshold and kept its identity" a claim about something.
    fn a_wholly_different_gene() -> Gene {
        Gene {
            trigger_state: State::new(40),
            min_step: u8::MAX,
            max_step: 0,
            action: Action::Terminate,
            angle: std::f32::consts::PI,
            adhere: false,
            child_state: State::new(41),
            child_kind: CellKind::Devorocyte,
            rest_length: crate::genome::MAX_REST_LENGTH,
            stiffness: crate::genome::MAX_STIFFNESS,
            new_kind: CellKind::Sclerocyte,
            new_state: State::new(42),
            osc_freq: crate::genome::MAX_OSC_FREQ,
            osc_phase: std::f32::consts::PI,
            sensor_gain: crate::genome::MAX_SENSOR_GAIN,
            sensor_target: SensorTarget::Detritus,
        }
    }

    /// A population written down: slot, serial and genome, in arena order.
    fn population(genomes: &[Genome]) -> Vec<(usize, u64, &Genome)> {
        genomes
            .iter()
            .enumerate()
            .map(|(slot, genome)| {
                (
                    slot,
                    u64::try_from(slot).expect("a slot is a small number"),
                    genome,
                )
            })
            .collect()
    }

    /// A taxonomy with room for a few hundred organisms and nothing seen yet.
    fn a_taxonomy() -> Taxonomy {
        a_taxonomy_of_seed(42)
    }

    /// The same, for a run of a given seed.
    ///
    /// The seed is what the names come out of - see [`crate::naming`] - so it is the one setting
    /// the naming tests have to be able to change.
    fn a_taxonomy_of_seed(seed: u64) -> Taxonomy {
        Taxonomy::new(&config(|raw| {
            raw.limits.max_organisms = 250;
            raw.world.seed = seed;
        }))
    }

    /// A population of chains, sampled until every group in it has been there long enough to be a
    /// species, and what each of those species ended up being called.
    ///
    /// ⚠️ **Chain lengths are how these tests control distance exactly.** Gene *i* of
    /// [`a_chain`] depends only on *i*, so two chains agree at every position they share and the
    /// distance between them is the number of positions they do not share over the longer of the
    /// two: a chain of eight and a chain of three are exactly `5/8` apart. That is a rational
    /// number written in the test rather than a consequence of what
    /// [`crate::genome::MAX_REST_LENGTH`] happens to be.
    fn named(seed: u64, chains: &[u8]) -> Vec<String> {
        let limits = config(|_| {}).limits;
        let genomes: Vec<Genome> = chains
            .iter()
            .map(|segments| a_chain(*segments, &limits))
            .collect();

        let mut taxonomy = a_taxonomy_of_seed(seed);
        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(&genomes).into_iter());
        }

        taxonomy
            .clusters()
            .iter()
            .map(|cluster| {
                cluster
                    .name()
                    .expect("every cluster here has been a species for a sample")
                    .to_string()
            })
            .collect()
    }

    /// ⭐ **B1.** A species gets a binomial name.
    ///
    /// > SPEC section 11: *"Binomial, generated from Latin-ish syllables."*
    ///
    /// CLAUDE.md says what the name is for, and it is not decoration: *"Lineages get generated
    /// binomial names (`Coacervus primus`) **so they are things you can refer to rather than
    /// coloured dots**."* Group A gave every lineage an identifier; this is what makes a sentence
    /// about one readable.
    ///
    /// Three claims here, and the second is the one with teeth.
    ///
    /// **A species has a name, and a cluster that is not one yet does not.** A name that appeared
    /// before SPEC section 11's twenty samples would be a name for something that mostly turns out
    /// not to exist - `Taxonomy::observe` explains why clusters churn and species do not - and a
    /// chronicle full of lineages that were named and never seen again is a chronicle nobody
    /// trusts.
    ///
    /// **A name is minted once and never changes.** The whole value of a name is that the sentence
    /// written about a lineage ten thousand ticks ago still refers to the same thing.
    ///
    /// **And it is a binomial**: two words, a capitalised genus and a lower-case epithet, which is
    /// the shape every reader of a species name recognises. What makes those two words read like
    /// Latin rather than like line noise is `naming.rs`'s own test.
    #[test]
    fn a_species_gets_a_binomial_name() {
        let limits = config(|_| {}).limits;
        let steady = vec![a_chain(3, &limits)];
        let mut taxonomy = a_taxonomy();

        for sample in 1..PERSISTENCE {
            taxonomy.sample(population(&steady).into_iter());
            assert!(
                taxonomy.clusters()[0].name().is_none(),
                "a cluster that has been there for {sample} samples has been named, and SPEC \
                 section 11 asks for {PERSISTENCE} before anything is a species"
            );
        }

        taxonomy.sample(population(&steady).into_iter());
        let name = taxonomy.clusters()[0]
            .name()
            .expect("a cluster that has persisted long enough to be a species has no name")
            .clone();

        let written = name.to_string();
        assert_eq!(
            written,
            format!("{} {}", name.genus(), name.epithet()),
            "a name does not read as a binomial"
        );
        assert_eq!(
            written.split(' ').count(),
            2,
            "{written} is not two words, so it is not a binomial"
        );
        assert!(
            name.genus().starts_with(char::is_uppercase) && !name.epithet().is_empty(),
            "{written} is not a capitalised genus and a lower-case epithet"
        );

        // ⚠️ And it is minted once. A lineage whose name changed as it went would make every
        // sentence already written about it wrong.
        for _ in 0..3 {
            taxonomy.sample(population(&steady).into_iter());
            assert_eq!(
                taxonomy.clusters()[0].name(),
                Some(&name),
                "a species that went on existing was renamed"
            );
        }
    }

    /// ⭐ **B2.** A new species inherits its genus from its nearest named relative and gets an
    /// epithet of its own.
    ///
    /// > SPEC section 11: *"A new species inherits its genus from its parent species and receives
    /// > a new epithet."*
    ///
    /// **Group A does not record which cluster came out of which**, and it should not: a cluster is
    /// minted by whichever organism was far from everything, and that organism's parent may be in
    /// any cluster at all or dead. So the parent species is read off the thing that is actually
    /// known, which is the same distance the clustering itself is built on - **the nearest named
    /// species there is**, provided it is within [`GENUS`].
    ///
    /// A chain of eight and a chain of three are `5/8` apart: past [`THRESHOLD`], so they are two
    /// clusters and two species, and inside [`GENUS`], so the second is a relative of the first
    /// rather than something new.
    #[test]
    fn a_new_species_inherits_its_genus_and_gets_a_new_epithet() {
        let limits = config(|_| {}).limits;
        let eight = a_chain(8, &limits);
        let three = a_chain(3, &limits);

        let apart = eight.distance_from(&three);
        assert!(
            apart > THRESHOLD && apart <= GENUS,
            "the two lineages are {apart} apart, which is not the case this test is about: past \
             {THRESHOLD} so that they are two species, and inside {GENUS} so that they are one \
             genus"
        );

        let mut taxonomy = a_taxonomy();
        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(std::slice::from_ref(&eight)).into_iter());
        }
        let elder = taxonomy.clusters()[0]
            .name()
            .expect("the first lineage is a species")
            .clone();

        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(&[eight.clone(), three.clone()]).into_iter());
        }
        assert_eq!(
            taxonomy.clusters().len(),
            2,
            "the two lineages did not come out as two clusters"
        );
        let younger = taxonomy.clusters()[1]
            .name()
            .expect("the second lineage has been there long enough to be a species")
            .clone();

        assert_eq!(
            younger.genus(),
            elder.genus(),
            "{younger} did not inherit the genus of {elder}, which is {} away - inside {GENUS}",
            apart
        );
        assert_ne!(
            younger.epithet(),
            elder.epithet(),
            "{younger} and {elder} are the same name, so the chronicle cannot tell them apart"
        );
    }

    /// ⭐ **B3.** A large enough jump mints a new genus.
    ///
    /// > SPEC section 11: *"a sufficiently large jump mints a new genus."*
    ///
    /// How large is [`GENUS`], and that constant is where the reasoning and the measurement are.
    /// This is the test that says the rule is applied: a chain of eight and a chain of two are
    /// `6/8` apart, which is past it, so the second lineage is not put in the first's genus.
    ///
    /// ⚠️ **And the first species of a run founds a genus too**, having nothing to be a relative
    /// of. Checked here because it is the case that happens in every run and never in a test that
    /// only ever looks at the second species.
    #[test]
    fn a_large_enough_jump_mints_a_new_genus() {
        let limits = config(|_| {}).limits;
        let eight = a_chain(8, &limits);
        let two = a_chain(2, &limits);

        let apart = eight.distance_from(&two);
        assert!(
            apart > GENUS,
            "the two lineages are {apart} apart, which is not past {GENUS}, so this test is not \
             about a large enough jump"
        );

        let mut taxonomy = a_taxonomy();
        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(std::slice::from_ref(&eight)).into_iter());
        }
        let elder = taxonomy.clusters()[0]
            .name()
            .expect("the first lineage of a run is a species and founds a genus")
            .clone();

        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(&[eight.clone(), two.clone()]).into_iter());
        }
        let younger = taxonomy.clusters()[1]
            .name()
            .expect("the second lineage has been there long enough to be a species")
            .clone();

        assert_ne!(
            younger.genus(),
            elder.genus(),
            "{younger} was put in the genus of {elder}, which is {apart} away - past {GENUS}"
        );
    }

    /// ⭐ **B5.** The same run gives the same names.
    ///
    /// Without this a replayed run's chronicle disagrees with the original, and SPEC section 2's
    /// whole determinism argument is about being able to go back and watch something again: *"When
    /// something interesting happens you will want to see it again."* A chronicle that said
    /// *Coacervus vorax* last night and something else this morning would make every screenshot,
    /// every note and every archived run stop describing the run it came from.
    ///
    /// So a name is a function of **the run's seed and the lineage**, and of nothing else. It is
    /// not a function of the wall clock, of what order the arena happened to be in, or of a draw
    /// from anybody's generator - see [`crate::naming`] for why that last one would be far worse
    /// than a naming bug.
    ///
    /// Three lineages rather than one, because the second and third are where a generator that
    /// drew from a shared stream would come apart.
    #[test]
    fn names_are_reproducible_from_the_seed() {
        let chains = [3, 8, 20];

        assert_eq!(
            named(42, &chains),
            named(42, &chains),
            "one seed named the same three lineages two different things"
        );

        // ⚠️ And a different seed is a different run. If the names did not move with the seed they
        // would not be coming from it, and every run ever made would carry the same list.
        assert_ne!(
            named(42, &chains),
            named(43, &chains),
            "two seeds produced the same names, so the names are not coming from the seed"
        );
    }

    /// ⭐ **A3.** The living population is clustered by distance, every 500 ticks.
    ///
    /// > SPEC section 11: *"Every 500 ticks, cluster the living population by distance with a
    /// > threshold."*
    ///
    /// Three claims, and the third is what makes the first two mean anything.
    ///
    /// **The grid is the world's own tick count.** Not a count of calls: a run watched through a
    /// window and the same run headless take samples at the same ticks, and a tick offered twice
    /// is one sample. The second of those matters more here than it does for a chart - see
    /// [`Taxonomy::at`].
    ///
    /// **Near genomes are one cluster and far ones are several.** Four founders growing chains of
    /// two, two, five and five make two groups: the two-gene genomes are identical and the
    /// five-gene ones are identical, and either pair is three unmatched gene positions of five
    /// from the other.
    ///
    /// **And it reads the living population.** A world whose organisms have all died has no
    /// clusters, however many it had a moment before - which is also the shape of what an
    /// extinction has to look like from here.
    #[test]
    fn the_living_population_clusters_by_distance() {
        let mut world = a_world_of(&[2, 2, 5, 5]);
        let mut taxonomy = Taxonomy::new(world.config());

        // Nothing is sampled off the grid, however many times it is offered.
        while !world.ticks().is_multiple_of(EVERY) {
            taxonomy.observe(&world);
            assert_eq!(
                taxonomy.sampled_at(),
                None,
                "a sample was taken at tick {}, which is not on SPEC section 11's {EVERY}-tick \
                 grid",
                world.ticks()
            );
            world.tick();
        }

        taxonomy.observe(&world);
        let sampled = taxonomy.sampled_at();
        assert_eq!(sampled, Some(world.ticks()));

        assert_eq!(
            taxonomy.clusters().len(),
            2,
            "four founders growing two distinct bodies came out as {} clusters",
            taxonomy.clusters().len()
        );
        assert_eq!(
            taxonomy
                .clusters()
                .iter()
                .map(Cluster::members)
                .collect::<Vec<u32>>(),
            vec![2, 2],
            "the two pairs of identical genomes did not come out as two pairs"
        );

        // A tick offered twice is one sample, which is what keeps SPEC section 11's twenty
        // consecutive samples a count of ticks rather than a count of calls.
        taxonomy.observe(&world);
        assert_eq!(
            taxonomy
                .clusters()
                .iter()
                .map(Cluster::seen)
                .collect::<Vec<u32>>(),
            vec![1, 1],
            "the same tick was sampled twice, so twenty consecutive samples would arrive early \
             by however many times somebody happened to call this"
        );

        // ⚠️ And it is the *living* population. Nothing alive is no clusters.
        let empty = Taxonomy::new(world.config());
        let mut empty = empty;
        empty.sample(std::iter::empty());
        assert!(
            empty.clusters().is_empty(),
            "a world with nothing alive in it has clusters in it"
        );
    }

    /// ⭐ **A4.** A cluster that persists for twenty consecutive samples is promoted to a species,
    /// and one that does not is not.
    ///
    /// > SPEC section 11: *"a cluster that persists for 20 consecutive samples is promoted to a
    /// > named species."*
    ///
    /// Driven through [`Taxonomy::sample`] rather than through a world, because twenty samples is
    /// ten thousand ticks and this test is about the counting rather than about the ecology. See
    /// that method for why it is separate.
    ///
    /// # What is actually being checked here
    ///
    /// That the count is **consecutive**, which is the whole of the rule. A cluster that is there,
    /// gone, and there again is two clusters that each lasted a while, not one that lasted twice
    /// as long - so the second one starts its twenty from the beginning and gets an identifier of
    /// its own. Without that, a lineage that flickered in and out of existence would be promoted
    /// on the twentieth flicker and named as though it had been there all along.
    ///
    /// And that promotion happens on the twentieth sample rather than the nineteenth or the
    /// twenty-first. A boundary that is one out is the failure that would never be noticed: every
    /// species in every run would simply be named five hundred ticks early or late, for ever.
    #[test]
    fn a_cluster_that_persists_is_promoted_to_a_species() {
        let limits = config(|_| {}).limits;
        let steady = vec![a_chain(3, &limits)];
        let mut taxonomy = a_taxonomy();

        for sample in 1..PERSISTENCE {
            taxonomy.sample(population(&steady).into_iter());

            assert_eq!(
                taxonomy.clusters().len(),
                1,
                "one genome sampled {sample} times over is more than one cluster"
            );
            assert_eq!(taxonomy.clusters()[0].seen(), sample);
            assert!(
                taxonomy.species().next().is_none(),
                "a cluster seen {sample} times is already a species, and SPEC section 11 asks \
                 for {PERSISTENCE}"
            );
        }

        taxonomy.sample(population(&steady).into_iter());
        assert_eq!(
            taxonomy.species_count(),
            1,
            "a cluster that has been there for {PERSISTENCE} consecutive samples is not a species"
        );
        let named = taxonomy.clusters()[0].id();

        // ⚠️ It stays one. Persisting longer does not promote it a second time.
        taxonomy.sample(population(&steady).into_iter());
        assert_eq!(taxonomy.species_count(), 1);
        assert_eq!(
            taxonomy.clusters()[0].id(),
            named,
            "a species that went on existing was given a new identifier, so nothing downstream \
             could follow it"
        );

        // --- ⭐ and "consecutive" means consecutive ---
        //
        // The world empties for one sample. What comes back is the same shape and is not the same
        // cluster: its run was broken, so it starts its twenty again with an identifier of its
        // own, and the species that was there is gone.
        taxonomy.sample(std::iter::empty());
        assert!(
            taxonomy.clusters().is_empty(),
            "a cluster with no members at a sample was kept, so 'consecutive' means 'ever'"
        );

        taxonomy.sample(population(&steady).into_iter());
        assert_eq!(
            taxonomy.clusters().len(),
            1,
            "the population came back and was not clustered"
        );
        assert_ne!(
            taxonomy.clusters()[0].id(),
            named,
            "a cluster that had gone and come back kept its old identifier, so a lineage that \
             flickered would be promoted on the twentieth flicker and named as though it had \
             been there all along"
        );
        assert_eq!(
            taxonomy.clusters()[0].seen(),
            1,
            "the returning cluster carried the run of samples the old one had built up"
        );
        assert_eq!(
            taxonomy.species_count(),
            0,
            "the species that went extinct is still being counted"
        );
    }

    /// ⭐ **A4, the hard half.** A cluster keeps its identity from one sample to the next while
    /// the population under it drifts, and it splits only when the population genuinely splits.
    ///
    /// This is the claim [`Taxonomy::observe`]'s four rules exist for, and the one a naive
    /// implementation gets wrong. A clustering recomputed from scratch every sample has no way of
    /// saying which of this sample's groups is which of the last one's, so identity has to be
    /// carried - and the way it is carried decides whether a lineage that is merely *drifting* is
    /// reported as one species or as an endless procession of them.
    ///
    /// # Three things happen to one population here
    ///
    /// It is sampled steadily until it is a species. Then it **drifts**: the genome it was
    /// clustered around is replaced, gene by gene, by one a little further away each sample, far
    /// enough overall that the lineage ends up well past a threshold from where it started. And
    /// then it **splits**: half of it stays and half of it becomes something a threshold away
    /// from the other half at once.
    ///
    /// The drift must not change the species. The split must produce a second one.
    #[test]
    fn a_species_survives_drift_and_splits_only_when_the_population_does() {
        let limits = config(|_| {}).limits;
        let mut taxonomy = a_taxonomy();

        // A genome of eight genes, rewritten one gene at a time into a gene that disagrees with
        // the one it replaces about all sixteen of its fields. One step is therefore about an
        // eighth of the genome - comfortably inside the threshold - and eight steps are nearly
        // the whole of it, which is well outside.
        let base: Vec<Gene> = a_chain(8, &limits).genes().to_vec();
        let drifted = |steps: usize| {
            let mut genes = base.clone();
            for gene in genes.iter_mut().take(steps) {
                *gene = a_wholly_different_gene();
            }
            Genome::new(genes, &limits)
        };

        for _ in 0..PERSISTENCE {
            taxonomy.sample(population(&[drifted(0)]).into_iter());
        }
        assert_eq!(taxonomy.species_count(), 1);
        let named = taxonomy.clusters()[0].id();

        // --- it drifts, one gene at a time, right across a threshold and further ---
        for step in 1..=8 {
            taxonomy.sample(population(&[drifted(step)]).into_iter());

            assert_eq!(
                taxonomy.clusters().len(),
                1,
                "a lineage that drifted by one gene came out as {} clusters at step {step}",
                taxonomy.clusters().len()
            );
            assert_eq!(
                taxonomy.clusters()[0].id(),
                named,
                "a lineage that drifted one gene at a time was recorded as a new species at \
                 step {step}, so a population that is merely changing would be an endless \
                 procession of species arriving and going extinct with nothing having split"
            );
        }

        // The drift really did carry it past a threshold from where it started, or the claim
        // above is a claim about nothing.
        assert!(
            drifted(0).distance_from(&drifted(8)) > THRESHOLD,
            "the drifted genome is {} from where it began, which is inside the threshold of \
             {THRESHOLD} - so this test would pass against a cluster that never moved",
            drifted(0).distance_from(&drifted(8))
        );

        // --- and it splits when the population does ---
        let far = a_chain(2, &limits);
        assert!(
            far.distance_from(&drifted(8)) > THRESHOLD,
            "the two halves of the split are not a threshold apart"
        );

        taxonomy.sample(population(&[drifted(8), far.clone()]).into_iter());
        assert_eq!(
            taxonomy.clusters().len(),
            2,
            "half a population moved a threshold away and was not recorded as a second cluster"
        );
        assert_eq!(
            taxonomy.clusters()[0].id(),
            named,
            "the half that did not move lost its identity to the half that did"
        );
        assert_eq!(
            taxonomy.species_count(),
            1,
            "the new cluster is a species already, and it has been there for one sample"
        );
    }

    /// ⭐ **A6.** Watching a world cluster does not change what the world does.
    ///
    /// Two worlds of one seed and one configuration are ticked side by side, one with a
    /// [`Taxonomy`] observing it at every tick and one with nothing watching at all, and then
    /// compared **number for number**: every tile of the resource field, and the position,
    /// velocity, radius, energy flow and developmental state of every cell of every body, as bit
    /// patterns.
    ///
    /// # Why this is the test the whole group needs and not a formality
    ///
    /// The failure it exists to catch is silent. An observer that drew a single random number
    /// from the world's generator, or that sorted an arena to make its own work easier, would
    /// leave a run **perfectly deterministic and deterministically different** - every reading in
    /// `docs/PHASE4.md`, every frame dumped, and every archived run would stop describing the
    /// program that produced them, and nothing would report it. `run.rs`'s
    /// `a_run_produces_what_it_produced_before_group_a` says the same thing from the other end,
    /// against numbers recorded before any of this existed.
    ///
    /// The run is taken past a sample boundary rather than stopping short of one, because an
    /// observer that never did any work could not perturb anything either.
    ///
    /// ⚠️ **Bit patterns rather than a tolerance.** A difference in the last place is exactly how
    /// a determinism failure starts, and it is the only kind this could produce.
    #[test]
    fn clustering_does_not_change_what_the_world_does() {
        let founders = [2, 2, 5];

        let mut watched = a_world_of(&founders);
        let mut alone = a_world_of(&founders);
        let mut taxonomy = Taxonomy::new(watched.config());

        // Well past the first sample, and past the second, so the observer has genuinely done
        // its work - minted clusters, moved representatives - before anything is compared.
        for _ in 0..EVERY * 2 + 100 {
            watched.tick();
            taxonomy.observe(&watched);
            alone.tick();
        }

        assert!(
            !taxonomy.clusters().is_empty(),
            "the observer found nothing to cluster, so this test would pass against an observer \
             that never ran"
        );
        assert_eq!(
            watched.ticks(),
            alone.ticks(),
            "the two worlds did not take the same number of ticks"
        );
        assert_eq!(
            every_number_in(&watched),
            every_number_in(&alone),
            "a world that was being clustered is not the world that was not, so the species \
             observer is changing the run rather than reading it"
        );
    }

    /// Every number in a world, as bit patterns: `world.rs`'s own whole-world signature.
    ///
    /// The living cells slot by slot rather than the whole arena, because the arena is mostly
    /// empty slots holding cells nothing has ever written to - and two runs would agree about
    /// those however wrong everything else had gone.
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

    /// ⭐ **A5.** An organism knows which species it belongs to.
    ///
    /// Until this existed an organism carried a marker that drifts and nothing else, so nothing
    /// could count the members of a lineage or say when one had ended. The answer is kept here
    /// rather than on the organism, which is what keeps the simulation's state exactly what it
    /// was - see this module's opening paragraph.
    ///
    /// # ⚠️ The serial is the load-bearing part
    ///
    /// A slot is a place in the arena and is handed to whoever is born there next. Five hundred
    /// ticks is long enough for most organisms to die, so a note filed under a slot alone would
    /// hand a newborn the species of the stranger who used to live there. That is not a rare
    /// edge: at the shipped configuration it is what would happen to a large fraction of the
    /// population at every sample.
    #[test]
    fn an_organism_knows_its_species() {
        let limits = config(|_| {}).limits;
        let steady = vec![a_chain(3, &limits), a_chain(3, &limits)];
        let mut taxonomy = a_taxonomy();

        // Before promotion there is no species to belong to, and saying so is the honest answer.
        taxonomy.sample(population(&steady).into_iter());
        assert_eq!(
            taxonomy.species_of(0, 0),
            None,
            "an organism in a cluster that has been there for one sample was handed a species"
        );

        for _ in 1..PERSISTENCE {
            taxonomy.sample(population(&steady).into_iter());
        }

        let named = taxonomy.clusters()[0].id();
        assert_eq!(
            taxonomy.species_of(0, 0),
            Some(named),
            "an organism in a species does not know it is in one"
        );
        assert_eq!(
            taxonomy.species_of(1, 1),
            Some(named),
            "two organisms of one species were given different answers"
        );

        // ⚠️ A slot that has been taken over by somebody else since the sample.
        assert_eq!(
            taxonomy.species_of(0, 4_000),
            None,
            "a newborn was handed the species of the organism that used to live in its slot, \
             which at five hundred ticks between samples is most of the population"
        );

        // A slot nothing was in, and a slot outside the arena.
        assert_eq!(taxonomy.species_of(2, 2), None);
        assert_eq!(
            taxonomy.species_of(100_000, 0),
            None,
            "a slot outside the arena has to answer rather than panic"
        );
    }
}
