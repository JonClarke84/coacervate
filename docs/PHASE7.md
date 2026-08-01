# Phase 7 — species, names, and the event log

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *speciation is visible and named*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 7** | in progress |
| **Current group** | D — Darwin in the margin (A, B, C, F, G, H, I, J, K and L are done) |
| **Suite** | green — **287 tests** |

⚠️ **Groups F, G and H are out of order and all three had to be.** F is the swimming work, taken
out of turn because Jonathan's live run had reached tick 2.8 million with one myocyte in it and
the diagnostic found that **nothing in this world could move, and nothing ever could have**. G is
what F's own closing paragraph asked for: swimming was made possible and was still worth
nothing, because the resource field never moved. H is what G's asked for: the field moved and
swimming was still too slow to catch it, so the stroke was measured and made seven times bigger.
They sit in the step ledger below between Groups D and E, which are the two still to do.

⚠️⚠️ **Group H is a negative result, and it is the most useful thing in this document.** The
stroke was the right lever and it was not the binding constraint. The diagnostic that went with
it found that a myocyte is only heard if some gene in its own genome names its `state`, that a
state is one of sixty-four against a genome of about three genes, and that **over 120,000 ticks
of the shipped world not one muscle anywhere moved a spring**. Three phases of work on the
*payoff* have all acted on a code path the world essentially never takes. See Group H.

⚠️⚠️ **Group I is the wiring Group H asked for, and it took Group H's third candidate**: a cell
now takes its behaviour from **the gene that built it**. Muscles fire — 9,901 spring-ticks of
genuine movement against nought. What the measurement that preceded it found is bigger than
muscle, and it is in Group I and in SPEC section 7: **only 2.2% of grown cells sit in a state
their own genome names**, so development stops at nearly every cell it visits, and that is why
bodies in this world are two cells for the first hundred thousand ticks of every run ever
measured.

⚠️⚠️⚠️ **Group L is the round that stopped arguing and built an instrument, and it is the most
useful thing in this document.** Six rounds each removed one reason a lineage could not swim and
each ended in a null, because nothing could measure what a configuration was *worth* without
waiting for evolution to produce one. The **competition assay** removes the wait: two founder sets
one mutation apart, seeded alternately, 42,000 ticks, and the ratio of living descendants is the
selection coefficient — about four minutes a run against a day. Its table of coefficients is in
Group L and it **refuted the seventh round's design before a line of it was written**: a muscle
must earn +2.5 %/generation to break even, the entire measured value of shape in this world is
+0.85, and `rest_length` already collects that for nothing. The round shipped the instrument, a
real pre-existing bug in what a muscle is charged for moving, and a season on the light.

⚠️⚠️ **Group J is Q31, and it is half a result and half a correction.** The distribution a re-drawn
state comes from is now biased — towards states some cell is in for `trigger_state`, away from them
for `child_state` and `new_state` — and the fraction of grown cells a genome addresses went from
**4.6% to 17.7%** over a 300,000-tick run. **And mean body size did not move**: 6.09 cells against
6.62, with living cells and biomass inside 2.4% of each other. The addressing was a real defect and
it was not what was holding bodies at two cells. See Group J.

---

## Why this phase matters more than its position suggests

Jonathan is running a live simulation. At tick 2.8 million he noticed, by eye, that bodies
had developed **serial repetition** — a horizontal spine with regularly spaced branches, the
same structural unit built several times over. That is the signature of duplicate-and-diverge
and it is the most interesting thing this project has produced.

He found it in a screenshot, hours after it happened, with no way to know when it started or
which lineage it happened in.

**That is the gap this phase closes.** The world already knows everything needed to say
*"tick 1,204,880 — a structure is being built more than once in one body"* at the moment it
occurs. Nothing is listening.

---

## The rule that governs every word this phase generates

CLAUDE.md marks this **load-bearing**, and it applies to the event log, species descriptions,
the inspector, tooltips and the chronicle alike:

> Every piece of generated copy has to be rigorously non-teleological. Evolution has no goal,
> no ladder and no destination, and the belief that it does is the single most common
> misconception about it.

Concretely, and these are bans:

- Never *evolving toward*, *progress*, *advanced*, *primitive*, *higher*, *improved*,
  *better*, *more evolved*, *trying to*, *succeeded in*.
- Fitness is relative to **current** conditions only. A lineage that thrives and then dies
  when the light dims was never worse — the conditions changed.
- **Extinction is not failure** and must never be framed as such.
- **Loss of structure is a legitimate and common outcome.** A lineage that abandons
  photosynthesis to parasitise its neighbours has not regressed. *If the event log can only
  celebrate gains, it is teaching something false — and it will make the simulation less
  interesting to watch, because half of what happens will go unremarked.*
- State **what changed**, never whether it was an improvement.

⚠️ **This wants a test, not just care.** A banned-vocabulary test over every generated string
is cheap and is the only thing that stops the register drifting as copy is added later.

---

## Step ledger

### Group A — telling one lineage from another — **done**

- [x] **A1. `two_genomes_have_a_distance`** — `Genome::distance_from`, in `genome.rs`.
  Positional alignment; each of a gene's sixteen fields scaled onto nought-to-one and
  averaged; an unmatched position costs `UNMATCHED = 1.0`; the whole divided by the longer
  gene list. The three things SPEC leaves open are decided in the table below.
- [x] **A2. `distance_is_a_metric`** *(property test)* — **it is a true metric, triangle
  inequality included.** Written up below, because that was not a foregone conclusion.
- [x] **A3. `the_living_population_clusters_by_distance`** — `species.rs`, every 500 ticks on
  the world's own tick grid.
- [x] **A4. `a_cluster_that_persists_is_promoted_to_a_species`** — 20 consecutive samples.
  `Taxonomy` is the second periodic observer, beside `Series`, and identity is carried by a
  **representative genome that moves to its nearest living member**. Also
  `a_species_survives_drift_and_splits_only_when_the_population_does`, which is the anti-churn
  claim stated as a test.
- [x] **A5. `an_organism_knows_its_species`** — `Taxonomy::species_of(slot, serial)`. The
  answer is kept on the **observer**, not on the organism, so the simulation's state is
  exactly what it was.
- [x] **A6. `clustering_does_not_change_what_the_world_does`** — two worlds ticked side by
  side, compared tile for tile and cell for cell. **`a_run_produces_what_it_produced_before_group_a`
  passes untouched**, with the observer in the loop. And, from the cost measurement below, a
  third piece of evidence nobody asked for: the 200,000-tick run before Group A and the same
  run with the clustering in it print **the same forty-six lines** — every population, every
  account, every mean, at every reported tick — differing only by the one new line at the end.
- [x] **`Sample.species`**, carried in from Phase 6, is filled. The record grew from 64 bytes
  to **72** — see the note in `series.rs` about the four bytes of tail padding Phase 8 must
  write as zeroes.

#### ⭐ There were already two measures of "how far apart", and only one of them is SPEC's

Phase 5 Group D built **`Genome::divergence_from`** for the hue drift: the share of gene
positions at which two genomes disagree at all. SPEC section 11 asks for something finer, and
two independent notions of distance would have been a genuine bug — the colours on screen
saying one thing about a split and the species panel another, with nothing to say which was
right.

They are **not** two measures. `divergence_from` is now literally `distance_from` with the
per-gene cost replaced by *"any difference at all"*: one private `Genome::aligned` walks the
two gene lists and both public methods hand it a cost function. So they use one matching rule,
one normalisation and one range by construction, they are nought together and one together,
and the fine one is never larger than the coarse one. `divergence_from`'s value is unchanged
to the bit — the sum of exact `1.0`s is the integer count it used to compute — so the marker
drift, and every golden vector downstream of it, is untouched.

`Organism::genome_hash` was not a candidate for either: it jumps rather than drifts, which is
what `docs/PHASE5.md` records as the reason it stopped being used for hue.

#### What Group A decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐ Genes are matched by position, not by a sequence alignment with gaps** | `genome.rs` | Two reasons and both are load-bearing. SPEC section 7's development takes the **first** gene whose trigger matches, so a gene inserted at the front is read before everything behind it and shifts the whole reading frame — an alignment that slid the lists back into register would report that nothing much happened at the one moment when a great deal might have. And a gapped alignment costs the product of the lengths, 16,384 dynamic-programming cells per pair at SPEC's cap, where this costs the shorter list. It is also the rule `divergence_from` already used, which is what keeps the hue and the species list one opinion. |
| **⭐ Sixteen fields, equally weighted, each scaled onto nought-to-one** | `genome.rs` | Eight discrete fields are nought-or-one (a myocyte is not a third of the way to a sclerocyte, and a state is a *name*); six numbers are the gap over the width of the range a gene is drawn from, truncated at one; **the two angles are angles**, compared the short way round the circle. Equal weights because `mutation.rs` picks the field a point mutation changes by drawing uniformly from the same constant — the measure privileges no field because the operator that creates the differences privileges none. `FIELDS_IN_A_GENE` moved to `genome.rs` so there is one sixteen rather than two. |
| **`min_step` and `max_step` are scaled over the byte, not over `max_dev_steps`** | `genome.rs` | Scaling over the run's development budget would make the distance between two fixed genomes depend on the configuration they were compared under — the same pair one species in a sixteen-step world and two in a four-step one, and an archived genome not comparable with a living one without carrying the settings it grew under. The cost, stated rather than buried: at SPEC's budget a re-drawn step moves the distance by about a fiftieth of a field, so timing barely reaches the species boundary. |
| **⭐ The unmatched penalty is exactly a whole gene** | `genome.rs` | It is the honest reading — there is nothing there to be like — and it is what keeps the triangle inequality. See below. |
| **⭐ The threshold is `0.5` — *"one species if they agree about at least half of themselves"*** | `species.rs` | The number that decides what a species is, and SPEC gives none. **A quarter was tried first and is wrong**; the measurement that settled it is below. On the three-gene genomes a settled run carries, a point mutation on a discrete field is about 0.02, one gene gained or lost is 0.25 to 0.33, and a wholly different program is 1.0 — so at a half **one indel does not split a lineage and two do**, and point mutations accumulate for a very long time before they split anything. A species is a group that has stopped running the same program, not one that has stopped being identical. |
| **⚠️ A neutral duplication is not neutral to the distance** | `species.rs` | SPEC section 7's whole-genome duplication grows exactly the body that grew before, and position by position it shares half its positions with nothing — so the measure calls it half a genome away. Recorded rather than fixed: the distance is over the **program**, not the body, and the alternative is a gapped alignment, which the first row of this table rejects for a stronger reason. The hue says the same thing, so at least the screen and the list agree. |
| **⭐ Cluster identity is carried by a representative genome that moves** | `species.rs` | The four rules that stop species churning, in the order they matter: **nearest wins, not first** (arena order is a consequence of who happened to die, so first-match would trade members between neighbouring lineages on every birth); **the representative moves to its nearest living member and no further** (left where it started, a lineage that merely drifts is recorded as an endless procession of species arriving and going extinct with nothing having split — measured, by pinning it: the drift test then reports a new species at step 5); **a cluster is removed only when genuinely empty**, so a dip to one member keeps its run of samples; and **promotion is what turns all of that into a name** — a cluster minted by one outlier that leaves no descendants is gone by the next sample having never been anything. |
| **Two clusters that meet do not need a merge rule** | `species.rs` | Ties in "nearest" go to the earlier entry, the list is in ascending order of identifier, so a younger cluster whose representative drifted onto an older one's loses every member and is removed. A merge resolved toward the identity that has been there longer, out of the rules rather than a rule of its own. |
| **A species is looked up by slot *and serial*** | `species.rs` | A slot is a place and is handed to whoever is born there next. Five hundred ticks is long enough for most organisms to die, so a note filed under a slot alone would hand a newborn the species of the stranger who used to live there — not an edge case but a large fraction of the population at every sample. |
| **The observer lives in `coacervate-sim`, unlike `Series`** | `species.rs` | `Series` is in `coacervate-render` because a sample is *made of* a `Census` and the panel that draws it is there. Clustering is made of genomes and a distance, which are the simulation's, and `clippy.toml`'s ban on `HashMap` and `HashSet` — SPEC section 2's *"no map iteration may affect simulation state"* — is exactly the discipline the bookkeeping wanted. Dense arrays by slot, and a vector kept in ascending order of identifier that is binary-searched rather than walked. |
| **A headless run says what it found** | `main.rs` | One line of the closing report: how many groups the last clustering found and how many had been there long enough to be species. Without it the phase does all of its work and a run with no window reports none of it. Phrased to `docs/PHASE7.md`'s register — what was there, and nothing about whether that is many or few. |

#### ⭐ The distance **is** a metric — including the triangle inequality

Worth writing out, because it was the open question of the group and because the answer turns
on a decision that looks like a detail.

The unnormalised cost is a sum over positions of a per-position cost, where a position past the
end of the shorter genome costs `UNMATCHED`. Each field's own measure is a metric bounded by
one — the discrete metric on the eight choices, the truncated ratio on the six numbers, the
short way round the circle on the two angles — and a mean of metrics is a metric, so the sum is
an ℓ¹ metric on genomes padded with "nothing".

**The division by the longer gene list is where an alignment cost usually stops being a
metric**, because the three sides of a triangle are divided by three different numbers. It
survives here, and the condition is that a gene facing empty space costs at least *half* what
the worst matched pair costs. Writing `p` for the penalty and `m` for the longer of two gene
lists: when the third genome is no longer than `m`, every divisor is at most `m` and the ℓ¹
inequality carries straight through; when it is longer, by `t − m`, it pays the penalty twice
over — once against each of the other two — and `(t − m)(2pm − D) / tm ≥ 0` holds for every
`D ≤ m` exactly when `p ≥ ½`.

At `p = 1` there is room to spare. **At `p = 0.4` it is not a metric**, and that is measured
rather than argued: `[x]` against `[y]` for two maximally different genes costs 1, and going by
way of `[x, x]` costs 0.9. `distance_is_a_metric` carries that attack as a fixed family added
to every case — a family of *mutated* genomes never finds it, because two random genes still
agree about half their discrete fields by chance and are never a whole gene apart.

The property test's tolerance is `1e-6` and it is there for one reason: the inequality is exact
in the reals, and what is compared is a 32-bit sum of up to 128 terms divided by a gene count.

#### ⭐⭐ The threshold was measured, and the first answer was wrong

A quarter was picked from first principles — just below where one gene gained or lost lands on a
short genome — and the shipped world was then run for 200,000 ticks to see what it produced.
**485 groups, 315 of them promoted to species.** That is not a finding about the world; it is a
resolution failure. SPEC section 11 exists so that *"lineages are things you can refer to rather
than coloured dots"*, and a list of three hundred names is a list nobody reads. Group B would
have generated three hundred binomials for it. It is also expensive: a pass is
`population × clusters`, so it cost **7% of the tick rate** where the shipped threshold costs
0.85%.

So the groups a living population actually falls into were counted at a range of thresholds, at
four points in one run of the shipped `config/default.toml`, seed 42:

| Tick | Population | Mean genome | at 0.25 | at 0.35 | at 0.40 | **at 0.50** | at 0.60 | at 0.70 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 50,000 | 2,038 | 2.0 genes | 107 | 46 | 28 | **15** | 9 | 6 |
| 100,000 | 2,208 | 2.6 genes | 277 | 108 | 65 | **20** | 8 | 6 |
| 200,000 | 2,142 | 3.2 genes | 477 | 225 | 133 | **34** | 13 | 6 |
| 400,000 | 820 | 7.3 genes | 236 | 153 | 112 | **37** | 12 | 4 |

At a half the run reads as **fifteen to forty lineages, rising slowly as the world diversifies**,
which is the shape worth having: a number a person can hold, that means more when it goes up. At
six tenths and above it collapses under a dozen and stops moving — a measure that has stopped
resolving anything. The half also sits just under the *median* distance between two organisms
taken at random, which runs from 0.53 early to 0.69 late: **a species is a group closer together
than two organisms of this world usually are.**

⚠️ Those counts are a *from-scratch* clustering of one moment, and the observer that actually
runs is incremental: a group survives while anybody is near it, so a real run carries a few more
than the table says. Measured, at 200,000 ticks: the table says 34 and the run reports **60
groups, 55 of them species**. Still a list a person can read, and the difference is the identity
being carried rather than recomputed — which is the point of carrying it.

⚠️ Worth noticing in that table quite apart from the threshold: at 400,000 ticks the population
has fallen to 820 and the mean genome has more than doubled to 7.3 genes. Nothing in this group
explains that and nothing in this group should — it is recorded because it is the first time
anybody has looked at this run past 200,000 ticks.

#### ⭐ What it costs, measured

Two hundred thousand ticks of the shipped `config/default.toml`, seed 42, headless, on the same
machine, with the same live simulation running alongside every time. The world reaches a
population of about 2,200:

| | Ticks per second | 200,000 ticks in | Groups at the end |
| --- | --- | --- | --- |
| Before Group A | **748.6** | 267.1 s | — |
| Clustering in the loop, at the threshold as shipped | **742.3** | 269.4 s | 60, of which 55 species |
| *(the same, at the quarter that was tried first)* | *696.1* | *287.3 s* | *485, of which 315 species* |

**0.85% of the tick rate**, which is 2.3 seconds over 200,000 ticks, or about **5.7 ms per
clustering pass** at 2,200 organisms in 60 groups. The middle row is worth keeping because it
shows what the threshold does to the cost as well as to the reading: eight times the groups is
eight times the arithmetic, and a quarter cost 7%.

And the pass timed directly rather than by difference
(`clustering_costs_little_beside_the_ticks_it_sits_between`, ignored in debug and run by the
release pass): **2,816 organisms in 4 groups, one clustering pass 210 µs against 1.28 s for the
500 ticks it sits between — 0.02%.**

⚠️ **The naive shape the phase warned about — 4,000 organisms compared pairwise, eight million
distances a sample — is not what happens, and nothing is sampled to avoid it.** Clustering is
**leader-style**: each organism is compared against the *representatives*, of which there are a
few dozen, so a pass is `population × clusters` rather than `population²`. The whole living
population is examined at every sample.

The worst case is still `population²`, and it is reached only by a world in which every organism
is more than a threshold from every other — a world in which the word "species" has nothing to
describe. It is worth knowing that the threshold is what stands between the measured cost and
that case: at a quarter the cost was already eight times what it is, on the same world.

#### ⚠️ Two things Group A did **not** do

**No early exit on the distance, and no length pre-filter.** Both are cheap and exact — a pair
whose gene counts differ by more than `threshold × longer` cannot be within the threshold — and
at 0.85% of the tick rate neither is worth the code today. Written down so that whoever looks at
this cost next knows the obvious optimisation is available rather than tried and rejected. It is
the thing to reach for if the group count ever climbs into the hundreds on a long run.

**No sorting, no random numbers, no map iteration**, which is A6 stated as three prohibitions
rather than as a test.

### Group B — names — **done**

⚠️ **What Group A left on the doorstep.** A settled run of the shipped configuration carries
**about sixty groups, fifty-five of them species**, and the count rises slowly as the world
diversifies. That is the size of list B has to name and B4 has to keep unique. `Cluster::id` is
minted once and never reused, which is the hook — a name attaches to an identifier that has one
meaning for the life of the run. `Cluster::representative` is the genome to name *from*, if a
name is to be generated from anything but the identifier.

- [x] **B1. `a_species_gets_a_binomial_name`** — generated from Latin-ish syllables, in the new
  `naming.rs`. A name is minted at promotion, once, and never changes. `naming.rs`'s
  `a_name_reads_like_a_latin_binomial` holds the structure a person's judgement rests on:
  capitalised genus, lower-case epithet, two syllables at least, **no run of four consonants**,
  and the epithet agreeing with its genus.
- [x] **B2. `a_new_species_inherits_its_genus_and_gets_a_new_epithet`** — from its *nearest named
  living relative*, because Group A does not record which cluster came out of which and should
  not. See the table below.
- [x] **B3. `a_large_enough_jump_mints_a_new_genus`** — `species::GENUS = 0.7`, read off the same
  measurement `THRESHOLD` was and then confirmed against a real run. Below.
- [x] **B4. `a_name_is_never_reused_in_one_run`** — a `BTreeSet` of every binomial ever handed
  out, which outlives the lineages in it. Proved over five thousand names, three quarters of them
  minted into a genus that already existed.
- [x] **B5. `names_are_reproducible_from_the_seed`** — **no random number is drawn anywhere in
  the group.** Below.
- [x] **A headless run says what it named.** `main.rs`'s closing report gained the genus count and
  the list of named species with what was alive in each. Without it the phase generated fifty-five
  names a run and displayed none of them.

#### ⭐ Where the names come from, given that nothing may draw a number

The constraint is `species.rs`'s and it is the whole shape of the group: an observer that drew one
number from the world's generator would leave a run **perfectly deterministic and deterministically
different**, and `a_run_produces_what_it_produced_before_group_a` is the golden vector that would
catch it. **It passes untouched.**

So a name is *computed*, not drawn. `Taxonomy::christen` mixes three numbers that already exist —
the run's **seed**, the cluster's **identifier**, and the **fingerprint of its representative
genome** (`Genome::hash`) — through the finalising step of `splitmix64`, which holds no state. The
identifier is in it because it is unique for the life of the run, so two lineages running identical
programs still get two names; the genome is in it so that a name is a fingerprint of the lineage
rather than of a counter.

⚠️ **`grid.rs` writes out the same mixer and the two are deliberately not shared.** Both are
numbers that must mean the same thing for ever and they must mean it for different reasons — one is
what a run *looks like*, the other is what its lineages are *called*. Sharing them would mean a
change made for the noise field silently renames every species in every archived run.

#### The syllables, and how they were made to read Latin rather than random

Every word is `onset · nucleus · medial [· nucleus · medial] · ending`, so consonants and vowels
alternate by construction and `strkth` is unreachable. SPEC's own two examples fall straight out:
`v·o·r·ax` is *vorax* and `pr·i·m·us` is *primus*.

⭐ **The tables are distributions, not sets, and that is what actually made it read.** The first
version drew uniformly over the *set* of legal Latin groups and produced

> Quauralum spoettuntens · Cloettannum saefilmosum · Thauclaclum natructosum

— every syllable individually plausible and the whole thing obviously machine-made. Three things
were wrong and all three were distributional:

| Wrong | Why | Fix |
| --- | --- | --- |
| **Nearly every stem was three syllables** | Laying two-syllable stems out first and three-syllable ones after multiplies the positions by 300, so 300 words in 301 are the long kind | The stem length is **its own digit**, so it is an even split |
| Two words in five began with a diphthong, three in five with a cluster | Uniform over a set where 3 of 8 nuclei are diphthongs | Plain vowels listed three times, plain consonants twice |
| Seven epithets in eight were `-osus` or `-idus` | Eight endings drawn evenly | `-us`/`-a`/`-um` listed three times |

After that: *Virnus defens · Grophus soxidus · Taetha nafina · Bruscirum drophinum · Thenna
firmens*.

One spelling rule survived the alternation and had to be written down: **`qu` is never followed by
`u`** in Latin, and `quuprax` is the one word in thirty a reader stops at.

#### What Group B decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐ The genus threshold is `0.7`** | `species.rs` | SPEC asks for *"a sufficiently large jump"* and gives no number. Read off Group A's own table: at seven tenths the shipped run holds **four to six neighbourhoods at every point measured** while the species count under it climbs from 15 to 37 — a handful of genera with several species in each. At six tenths it is 8 to 13, close enough to the species count that a genus stops grouping anything. It also **has** to be above `THRESHOLD`: a cluster is minted precisely because it was half a genome from everything, so a genus boundary at or below a half would mint a genus for very nearly every species and the binomial would carry no more than the epithet alone. Seven tenths also sits just above the *median* distance between two organisms taken at random (0.53 early, 0.69 late) — **a lineage founds a genus when it is further from everything named than two organisms of this world usually are from each other.** |
| **⚠️ Measured, and it came out at four** | — | 200,000 ticks of the shipped `config/default.toml`, seed 42, headless: 60 groups, **55 named species in 4 genera**. The table predicted four to six. **One of the four holds 48 of the 55**, which is recorded rather than fixed: at this radius the population genuinely is one large neighbourhood with three small ones outside it, and a genus inherited from the *nearest* relative also spreads transitively — A near B and B near C without A near C. That is a chain of relatives, which is what SPEC's *"inherits from its parent species"* describes. If a run ever reads as one genus and nothing else, six tenths is where this goes. |
| **⭐ The parent species is the nearest *named living* one, not an ancestor** | `species.rs` | Group A records no cluster parentage and should not: a cluster is minted by whichever organism happened to be far from everything, and that organism's own parent may be in any cluster, or dead, or in one since removed. A parentage built on that is a record of which arena slot was free. So the genus is inherited from the thing actually known — the nearest named species within `GENUS` — which is a claim about *relatedness* rather than descent, and that is what a genus has always been. **Living only**: keeping every named genome for the length of a run is kilobytes per name and unbounded, and reading the live clusters costs nothing. |
| **A name is minted at promotion and never changes** | `species.rs` | The whole value of a name is that a sentence written ten thousand ticks ago still refers to the same thing. Clusters churn — one outlier mints one and it is gone by the next sample — so a name before SPEC's twenty samples would mostly be a name for something that was never there. |
| **⭐ Three genders, and the epithet agrees** | `naming.rs` | *Coacervus prima* is the mistake a classicist notices immediately, and it is what a scheme that draws its two endings independently produces in a third of all names. The gender **is** which ending the genus was given, and every epithet in that genus is then drawn from the endings that agree with it. `-ax` and `-ens` are third-declension adjectives of one termination — *vorax*, *virens* — so they appear unchanged in all three lists. |
| **Uniqueness is enforced on the written binomial, by walking rather than redrawing** | `naming.rs` | Redrawing on a collision cannot say what happens when the words run out, because a sequence of draws is not a sequence of *different* draws. Stepping one along from where the draw landed visits every position in the language exactly once. **When a genus runs out of epithets the lineage founds a new one** — a genus minted a moment ago has all of its epithets free. It cannot be reached: 38 million epithets in each gender and 14 million genera, against a run that names of the order of fifty lineages an hour. |
| **⚠️ The name registry is the one thing here that grows with the length of a run** | `naming.rs` | B4 is *never reused in one run*, so a name has to be remembered after its lineage is extinct. A name is about twenty bytes and a twelve-hour run keeps a few hundred kilobytes of them. Recorded because CLAUDE.md's arenas are all allocated once and this one is not — it is an observer's bookkeeping rather than simulation state, and the bound is the number of species a run has ever had. |
| **A ban list, checked before every name is handed out** | `naming.rs` | CLAUDE.md marks the non-teleological rule load-bearing and a name is the most repeated generated copy in the project. *maximus* is `m·a·x·imus` — four draws from perfectly innocent — so the rule needs a filter and not care. 78 fragments in three groups: **Latin that ranks** (*optimus*, *summus*, *melior*, *dominus*, *victor*, *sapiens*, *gradus*…), **the English CLAUDE.md bans outright**, and **words that would be read as something other than Latin** — a generated name is going into a document somebody may show to other people. ⚠️ `primus` is deliberately *not* banned: it is a fact about the order a lineage appeared in, not a rank, and it is the project's own example. |

### Group C — the event log ⭐

The highest-value item in the phase. Append-only, human-readable, in a naturalist's register.

⚠️ **What Group B leaves on the doorstep.** `Cluster::name()` is `Option<&Name>` — `Some` exactly
when the cluster is a species — and `Name` displays as *Coacervus primus*. C5 has its names.
`Taxonomy::names()` reaches the whole registry, including the genus count. **C9 should call
`naming::is_permissible` rather than growing a second list of banned words**: two such lists are
two lists that come apart, and the register of the log and the register of the names are one
register.

**Done.** `chronicle.rs`, in `coacervate-sim`, plus a fourth block on the panel.

- [x] **C1. `an_event_is_recorded_with_its_tick_and_its_deep_time`** — *"Tick 41,208 — 41.2
  Ma."* An `Event` carries the tick, the deep time, a stable tag and the sentence, and nothing
  else — which is a line of Phase 8's `events.jsonl` in memory. ⭐ `millions_of_years` moved
  into `chronicle.rs` and `census.rs` now calls it, so the arithmetic is in one place rather
  than three.
- [x] **C2. `first_adhesion_is_noticed`** — the origin of multicellularity in this run.
  ⚠️ **With the shipped founder it fires on the run's first tick**, because `founding.rs` seeds
  a photocyte with a gonocyte sprung to it. That is the honest answer: this world *begins*
  multicellular, and the log says so at tick 10,001 rather than pretending otherwise.
- [x] **C3. `the_first_appearance_of_each_cell_kind_is_noticed`** — with a one-clause gloss of
  what the kind does, from SPEC section 6.
- [x] **C4. `the_first_predation_is_noticed`** — ⭐ **`Ledger` gained a `predated` counter**,
  and it had to. `Ledger::predate` is `biomass → biomass`, so no total in the world moves and
  no balance can ever notice one happening; an observer outside the tick would have to redo
  `behaviour.rs`'s neighbour search over every devorocyte on every tick to find out, which is a
  second implementation of the one rule CLAUDE.md's decision log is most insistent must not be
  scripted. One addition inside a loop that has already done the arithmetic answers it exactly.
  It is on neither side of the invariant and `Ledger::check` destructures it as `predated: _`.
  ⚠️ **The line does not say whose devorocyte it was**, for the same reason.
- [x] **C5. `speciation_and_extinction_are_recorded_by_name`** — on the clustering's own grid.
  `Taxonomy` gained `samples()`, a count of samples taken, which is what tells the log a fresh
  one has happened; testing the tick against `species::EVERY` would be the same answer in the
  ordinary case and a wrong one wherever the clustering did not actually run.
- [x] **C6. `new_records_are_noticed`** — body size, cell count, genome length, population.
  See the table below for the two that needed a ladder rather than a step, and why.
- [x] **C7. `a_mass_extinction_is_noticed`** — its own window: fifty-one readings a hundred
  ticks apart, four hundred bytes, never thinned. Firing clears it, so one collapse is one
  line rather than fifty.
- [x] **C8. `a_change_the_user_made_is_recorded`** — twenty conditions, named **in words**.
  `settings.rs`'s `every_dial_is_a_condition_the_chronicle_reports` puts the list beside the
  twenty sliders and insists they are the same settings.
- [x] **C9. `no_event_text_uses_the_banned_vocabulary`** ⭐ — over every sentence the phase can
  generate, not the ones a run happens to produce: all six cell kinds, all twenty conditions,
  all four records, and both the named and unnamed forms of everything that has one. It calls
  `naming::is_permissible`, and `naming.rs` gained
  `the_banned_vocabulary_is_claude_mds_whole_list`. Below, because the list was missing two of
  CLAUDE.md's ten and the register test caught three sentences.
- [x] **C10. `serial_repetition_is_noticed`** — ⭐ **built.** The reasoning is in `repeated`'s
  own doc comment and in the table below.
- [x] **⭐ The loss detector**, which SPEC does not ask for and CLAUDE.md requires:
  `a_lineage_that_stops_building_a_cell_kind_is_noticed`.
- [x] **The panel shows the last events**, hidden by `Chrome::compose`'s one screensaver line
  with no new `if`. `the_panel_shows_the_most_recent_events` is what proves the second half.
- [x] **A headless run prints the log**, which is the whole point of the group.

#### ⭐⭐ The bound, and what a viewer loses

SPEC section 13 says of `events.jsonl` *"append-only, human-readable, **keep everything**"*.
That is right for a **file** and wrong for memory, and they are not the same decision. A
settled run carries about sixty clusters; each clustering sample can mint one and lose another,
so a twelve-hour run of 32 million ticks can produce of the order of a hundred thousand
speciation and extinction lines — **16 MB of prose**, unbounded, and worse on a run that churns
faster.

So the log is a **ring of 1,024 events, allocated once, oldest dropped** — about 160 KB, or
0.008% of CLAUDE.md's two-gigabyte target and a little over half what `series.rs`'s chart
costs. The count of what was dropped is kept, and the headless report prints it: SPEC section
13 says the same thing about snapshots — *"Log what was dropped; silent truncation reads as
complete history when it isn't."*

⚠️ **What a viewer loses is the beginning of a long run** — first adhesion, the first appearance
of each cell kind, the first predation — which are among the most interesting lines the log ever
writes. A ring is still right for a *log*, which is read from its end, and thinning (which is
what `series.rs` does) cannot be done here at all: half a chart still describes the shape, while
half a log is a sentence about a lineage whose arrival is no longer in the record.

⚠️ **Phase 8 must write each event as it is appended and must not read this ring at shutdown.**
`events.jsonl` is *keep everything* and this is the last thousand; writing from here is how a
twelve-hour run produces a file that begins in the middle. `Event` is already shaped as a line
of that file — `{tick, ma, kind, said}` — so Phase 8 writes it unchanged, and `Kind::tag` is a
**format**: rename one and every archived recording says something different.

#### ⭐ What Group C decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐ The log lives in `coacervate-sim`, unlike `Series`** | `chronicle.rs` | `series.rs` is in `coacervate-render` because a sample is *made of* a `Census` and the panel that draws it is there. An event is made of a body's cells, a cluster's `Name` and `naming::is_permissible`, all three of which are this crate's — and a binomial generated from Latin-ish syllables is already presentation living in the simulation crate, for exactly the same reason. Nothing in it knows that rendering exists. |
| **⭐⭐ `repeated` is C10, and the argument is the shape of SPEC's list rather than an exception to it** | `chronicle.rs` | SPEC's list already contains a landmark of this class: *first adhesion* is not an event in the ledger — nothing is gained or lost, no lineage arrives or goes — and CLAUDE.md calls it *"the origin of multicellularity in this run"*. It is on the list because a change in how bodies are **organised** is worth writing down. Serial repetition is the same thing: the origin of **segmentation**, and the visible signature of duplicate-and-diverge, which CLAUDE.md's decision log calls *"the single most important decision in the project"*. A log that records the first two cells stuck together and says nothing when a lineage starts building the same organ three times over is recording the less interesting of the two. |
| **What counts as a repeated structure** | `chronicle.rs` | A **unit** is an adhered pair whose two ends differ in kind or in developmental state; a body repeats one when the same pair hangs off **three different parent cells**. ⚠️ **A pair whose two ends are the same is excluded and that exclusion is what makes it mean anything**: a gene that divides a cell into another cell of its own kind makes a chain, and a chain of eight identical photocytes repeats nothing a chain of two does not. What is looked for is *a spine with something on it* — which is what was found by eye at tick 2.8 million. Read off the finished body, so it is a description of the thing standing in the water. |
| **⚠️ The wording, and it is `docs/PHASE7.md`'s own** | `chronicle.rs` | *"A structure is being built more than once in one body. **Coacervus primus** is growing 8 copies of one unit along a single body: a photocyte with a myocyte attached to it."* What changed, and then what it is. Nothing about it being a step. |
| **⭐⭐ The loss detector fires on a lineage that stops building a cell kind** | `chronicle.rs` | CLAUDE.md is why it exists rather than SPEC: *"Loss of structure is a legitimate and common outcome… If the event log can only celebrate gains, it is teaching something false."* It is the mirror of C3 — the first appearance of a kind in the world, and the moment a lineage's bodies stopped containing one — and it is the trigger SPEC section 11's Darwin table already names for *rudimentary and atrophied organs*, so Group D has it waiting. Chosen over *bodies get smaller* because that one is a number going down, which is much harder to write about without implying a direction; *has stopped building sclerocytes* states a fact and stops. |
| **⚠️⚠️ It says so once per lineage per kind, and a real run is what settled that** | `chronicle.rs` | 200,000 ticks of the shipped configuration, seed 42, first version: **267 events, 130 of them a lineage letting go of a cell kind, and one species said the same sentence twenty-four times.** A lineage of several hundred bodies takes a cell kind up again and lets it go again, so a detector reporting every crossing reports churn. Two changes: the hysteresis went from four samples to **eight** on both sides, and each `(species, kind)` pair is announced **once** — a landmark, not a status. The same run then produced **149 events: 77 speciations, 30 records, 22 extinctions, 11 losses, 8 firsts**. |
| **⭐ Records: two of the four needed a ladder rather than a step** | `chronicle.rs` | Cell count and genome length are small integers that climb rarely, so every new maximum is a line and there are at most 192 of them in a run. **Population is not**: a run goes from 8 founders to about 2,200 and every one of the 2,192 populations in between is a new record, which would be two thousand lines in the first twenty thousand ticks and would push everything else out of the ring before the first species was named. At a **doubling** the same stretch is eight lines, each saying something. **Body span needed one too, for a different reason**: a span is a measurement of where the physics has put a body's cells, and a large body spends its first ticks relaxing outwards along its springs — at a fixed step of two world units one body produced **nine lines in twelve ticks**. At half as much again it is five lines end to end. |
| **The first reading primes the records rather than announcing them** | `chronicle.rs` | A record is news that something changed, and the first population a world ever has has not changed from anything. Without it every run opens with four lines announcing that its founders were the largest bodies, the longest genomes and the greatest population there had ever been — all true, and not news. |
| **⭐ `observe` is idempotent and has no guard against a repeated tick** | `chronicle.rs` | `series.rs` and `species.rs` both refuse a tick they have seen, because a second reading is a second chart point and a second sample towards a promotion. Nothing here has that shape: a *first* fires once by construction, a record is a maximum, the collapse window is keyed on the tick and the lineages are done once per clustering sample. So there is no guard, and `nothing_is_noticed_twice` says the absence is safe rather than an oversight. |
| **⚠️ SPEC's own example sentence could not be used** | `chronicle.rs` | *"A cell has **failed** to separate from its daughter"* contains *fail*, which the shared ban list refuses because of *failure* — and extinction framed as failure is exactly what CLAUDE.md bans. The log says *"has not separated from its daughter"*, which means the same thing. The list is shared with the names on purpose and is not being narrowed for one word. |
| **⚠️ A setting is named in words rather than by its key** | `chronicle.rs` | Same collision: `light.gradient` contains *grad*, refused for *gradus* — a step, a rank. It is also better copy. *"How much of the light falls near the surface has been changed"* is a sentence about the world; `light.gradient = 0.40` is a line of a settings file. |
| **⚠️ The ban list was missing two of CLAUDE.md's ten, and nothing had noticed** | `naming.rs` | *succeeded* does not contain *success* — the letters differ from the fifth onwards — and there is no `y` in the syllable tables, so *trying* was never reachable by a name and was never noticed to be absent. Group B's list was written against what a *name* could accidentally say; a log is written in English, where both are one slip away. Seven were added and one widened (`worst` → `wors`); **not one of them is spellable by the syllable tables**, so no name any earlier version of this project generated is changed. |
| **The register test then caught three sentences** | `chronicle.rs` | *"how often two **neighbouring** genes swap places"* — `neighbouring` contains *urin*. *"it holds energy **towards** a child"* — `toward` was one of the seven just added, and it is the right ban: a gonocyte holds energy *for* a child, where *towards* is a direction. And *"it reads the **gradient** it sits in"*, which is the `grad` collision again. Three in about forty sentences, written by somebody who had the rule open in front of him. That is the argument for the test in one line. |
| **⚠️ `Taxonomy::sample` is `pub(crate)` for the tests** | `species.rs` | Speciation and extinction *by name* need a promoted species, a promoted species needs twenty samples, and twenty samples is ten thousand ticks of a real world. `species.rs` split `sample` out of `observe` for exactly this and wrote down why; Group C is the second caller. Nothing outside a test calls it. |
| **The panel shows the first sentence of each event and not the whole one** | `panel.rs` | The column is 208 points across, which is about twenty-eight monospace characters — a two-sentence event is nine lines of chrome and three of them would be taller than the readings, the controls and the charts together. The first sentence of every event this project writes is the one that says what happened; the rest is the detail, and the detail is what the chronicle is for. |
| **⚠️ A fixed-height box filled from the bottom with *whole* events** | `panel.rs` | Unlike the readings and the charts there is no arithmetic that says in advance how tall a sentence wants to be, and a block that took whatever height its contents asked for would change size every time something happened — on a screen CLAUDE.md asks to be *visually calm*, that is the worst possible behaviour. The first version was a scroll area held at its end, which slices the top line through the middle of its letters; a dumped frame is what caught it. Measuring each event and dropping the one that will not fit costs eight lines and leaves a block with nothing broken in it. |

#### ⭐ What it costs, measured

Two hundred thousand ticks of the shipped `config/default.toml`, seed 42, headless, on the same
machine, with the same live simulation running alongside — which is how Group A's figures were
taken, so the numbers are directly comparable:

| | Ticks per second | 200,000 ticks in |
| --- | --- | --- |
| Group A: clustering in the loop | 742.3 | 269.4 s |
| **Group C: clustering and the event log in the loop** | **740.5** | **270.1 s** |

**About a quarter of one per cent**, which is inside the noise of two runs on a machine doing
other things. The reason it is that cheap is the shape of the survey rather than care: the
population has to be counted, so the arena is walked whatever else is true, and **everything
beyond that switches itself off once there is nothing new left to find**. A *first* stops
looking the moment it has fired; the body-size record does one pass per body and only looks at
its pairs when that pass says it might beat the record; the lineages are done once per
clustering sample rather than once per tick. A settled run of 2,200 three-celled bodies costs
2,200 slot reads, 4,400 comparisons and 6,600 squared distances a tick.

⚠️ **The chrome grew from 4.9% of a dumped frame to 6.9%**, and to 8.3% of the 1280 by 720
window this program opens, against the 10% bound
`the_chrome_is_a_small_part_of_whatever_it_is_drawn_into` holds. That is a fifth more panel for
a fourth block, and it is the closest the chrome has come to its own bound. **A fifth block
would not fit**, which is worth knowing before Group D's marginalia is drawn anywhere near this
column.

#### ⚠️ Three things Group C did **not** do

**No `events.jsonl`.** That is Phase 8's, and the note above says what it has to do differently.

**No `Ledger` figure for anything but predation.** The counter is there because C4 is otherwise
unanswerable from outside a tick; nothing else in the log needed one, and a ledger that grew a
counter per detector would be a summary of the world kept inside the world, which is what
`census.rs` exists to argue against.

**No naming of the first predator.** See C4.

### Group D — Darwin in the margin

⚠️ **What Group C leaves on the doorstep.** `Chronicle::events` and `Chronicle::latest` are the
stream of things that happened, each with a `Kind` — and SPEC section 11's trigger table is
written in exactly those terms: *first predation event*, *speciation*, *mass extinction*, *a
lineage loses a cell kind*, *deep-time milestone*. Five of the eight triggers are `Kind`s that
already exist, and *a lineage loses a cell kind* is `Kind::LettingGo`, which was built for
CLAUDE.md's reason and turns out to be Darwin's *rudimentary and atrophied organs* trigger as
well. ⚠️ **And the chrome is at 6.9% of a dumped frame against a 10% bound**, so the marginalia
wants somewhere other than the left-hand column — see Group C's cost note.

- [ ] **D1. `a_quote_fires_only_when_its_trigger_happens`** — ⚠️ SPEC section 11 is emphatic:
  *"quotes are captions on events, not decoration."* A rotating quote box would be a fortune
  cookie — disconnected, and tiresome within an hour.
- [ ] **D2. `each_trigger_fires_at_most_once_per_run`** — a handful across several hours, not
  a stream.
- [ ] **D3. `darwin_toml_loads_and_is_attributed`** — `{ text, work, year, trigger }`, always
  with the work and the year.
- [ ] **D4. `nothing_is_quoted_that_darwin_could_not_have_said`** — ⚠️ **anachronism
  discipline.** He knew nothing of genes, mutation or molecular heredity, and deliberately
  avoided the origin of life in *Origin* — the warm little pond was a private letter, not a
  published claim. Selection, struggle, divergence, extinction, rudimentary organs and deep
  time are safely his. Quoting him beside a mutation event is a category error a biologist
  would notice immediately.
- [ ] **D5. `the_marginalia_is_typeset_quietly`** — serif, generous leading, low contrast,
  slow fade. ⚠️ The chrome is currently one monospace face; this needs a second. Disableable.

### Group F — making swimming possible — **done**

⚠️ **Out of the phase's plan, and it could not wait.** Jonathan's live run had reached tick 2.8
million with **one myocyte** in it, and the diagnostic that went looking for why found something
larger than a tuning problem: **nothing in this world could move, and nothing ever could have.**

Three changes, all measured, all landing together because the first of them moves every golden
vector in the project and there is no sense in doing that twice.

- [x] **F1. `a_travelling_wave_carries_a_body_through_the_water`** ⭐⭐ — `physics.drag_anisotropy`,
  a new key in `[physics]`, shipping at **2.0**. See below.
- [x] **F2. `a_straight_body_and_a_reciprocal_stroke_both_go_nowhere`** — the two ways the new
  water still, correctly, refuses to move a body.
- [x] **F3. `drag_is_anisotropic_across_a_body_axis`** — the mechanism, and the load-bearing half:
  a cell with fewer than two adhesions has no axis and keeps the plain drag.
- [x] **F4. `the_drag_anisotropy_range_is_closed_at_both_ends`** — `1.0..=3.0`.
- [x] **F5. `metabolism.movement_cost` 0.15 → 0.0001** — a thousandfold, and the reasoning is in
  SPEC section 3 beside the value.
- [x] **F6. The light sensor's *normalisation*** — a fixed reference of 0.02 rather than the tile's
  own energy, and `MAX_SENSOR_GAIN` from 1 to 8.
- [x] **F7. `config/dense.toml`** — `the_dense_profile_is_the_shipped_world_with_less_water_in_it`.

#### ⭐⭐ The conservation law, which is the whole finding

Every internal force is `+f` on one cell and `−f` on another; there is no mass; and `drag` is one
scalar applied identically to every cell. So for any free body, `Σv ← Σv × drag` — **the total
velocity is a conserved quantity of the integrator and it decays to nothing.** Measured `|Σv|` of
5.96e-7 over 2,000 ticks, and a twelve-cell travelling-wave undulator moving 0.00015 units per
1,000 ticks, which is `f32` noise.

**This is stronger than the scallop theorem**, and that is why nobody caught it: a travelling wave
is the textbook escape from the scallop theorem, section 9's controller is built to produce one,
and it escaped the theorem and not the law. The law is not about strokes at all. It is about the
integrator.

The fix is what real swimming at this scale works on — **drag anisotropy**, the fact that a
slender body resists motion across its axis about twice as hard as along it. The full argument,
the measurements and the bounds are in SPEC section 8, which had described the physics as if
locomotion were possible and now says why it was not.

#### ⭐ What the measurements actually say, including the part that is not good news

Over 1,000 ticks, hand-built bodies through the real physics:

| Body | `k = 1` (as shipped until now) | `k = 2` (shipped now) |
| --- | --- | --- |
| One myocyte, one spring | 0 | 0 |
| Two springs at π/2, cells in a line | 0.0005 | 0.0005 |
| Six-cell travelling wave, cells in a line | 0.0003 | 0.0003 |
| **Eight-cell zig-zag, resting stroke** | 0.0005 | **0.154** |
| **Eight-cell zig-zag, driven to full amplitude** | 0.0004 | **1.896** |
| Three-cell zig-zag, two springs at π/2, full stroke | 0.0005 | **1.085** |

**A body whose cells lie in a straight line still cannot swim, and that is correct** — all its
motion is along its own axis, so the sideways drag never engages, and nothing one-dimensional
swims in any fluid. What a lineage has to find is a **shape**, not a rhythm.

⚠️ **And swimming is now possible without yet being worth anything.** 0.154 units per 1,000 ticks
is a three-hundred-fold improvement on noise and it is still **a fortieth of a cell's diameter per
1,000 ticks**, against a body that lives 571 to 2,000 ticks. A lineage that grew a perfect
undulator would move less than its own width in a lifetime. That is the honest reading of the
300,000-tick run below, where myocytes appear repeatedly and never persist: the mechanism exists
now and the *payoff* does not. Whatever comes next — a faster beat, a larger swing, or SPEC
section 8's buoyancy-by-`CellKind` — has a working substrate to act on, which it did not before.

#### The 300,000-tick run at the shipped configuration

Eight founders, seed 42, `config/default.toml` as it now ships. Ticks are the world's, so the
first 10,000 are the dawn.

| Tick | Alive | Field | Biomass | Mean cells | Mean genes | Mean depth | Myocytes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 20,000 | 879 | 170,506 | 12,435 | 1.97 | 1.42 | 296 | 0 |
| 50,000 | 2,070 | 110,605 | 29,035 | 1.98 | 1.90 | 400 | 2 |
| 100,000 | 2,159 | 87,880 | 30,739 | 2.03 | 2.45 | 434 | 0 |
| 150,000 | 2,007 | 86,946 | 30,900 | 2.22 | 2.89 | 435 | 1 |
| 200,000 | 1,498 | 79,388 | 32,094 | 3.21 | 3.86 | 460 | 0 |
| 250,000 | 844 | 69,300 | 32,958 | 6.07 | 6.28 | 484 | 0 |
| **310,000** | **650** | **65,755** | **33,961** | **8.39** | **8.75** | **483** | **1** |

The five accounts at the end: **65,755** in the field, **33,961** held by the living, **4,475**
lying in the drift, **6,468,877** spent for good, **6,573,069** fallen as light. The ledger checked
itself every thousand ticks of all 310,000 and never once disagreed.

Cell-kind composition at tick 310,000, out of 5,467 living cells: **4,797 photocytes, 652
gonocytes, 11 sclerocytes, 6 sensocytes, 1 myocyte, 0 devorocytes.**

**Nothing degenerated, and neither of the two failure modes the change was watched for appeared.**

*Myocytes did not accumulate as neutral bloat.* They are present in fourteen of the thirty-one
readings and never number more than five at once, in a population of four to five thousand cells.
Making movement nearly free did not make a muscle free: a myocyte's upkeep is **0.014** against a
photocyte's **0.004**, so it is three and a half times as expensive to own and earns nothing.
`movement_cost` was never what was pricing it.

*The population did not migrate to the surface and form a mat.* Mean depth drifts from 296 to 483
in a world 1,152 deep — into the brighter half and nowhere near the top of it. Nothing can swim
upwards to any useful degree, so what that drift measures is where the lineages that bred fastest
happened to be, which is what it measured before Group F.

**The ecology is the one Phase 4 measured, arriving sooner.** Living biomass sits between 30,000
and 34,000 for the whole run while the population falls by two thirds and bodies quadruple — the
carrying-capacity claim holding exactly while everything else moves. Phase 4's half-million-tick
run ended at 794 organisms of 6.73 cells and 10.80 genes; this one is at 650, 8.39 and 8.75 by tick
310,000, having taken a little over half as long to get further.

⚠️ **And myocytes are still at one, which is the number Group F set out to change.** The answer to
why is in the table above it: the mechanism works and the payoff does not yet exist. A body that
lives 571 to 2,000 ticks and swims 0.154 units per 1,000 has moved a fortieth of its own width by
the time it dies. Selection has nothing to see. **That is a real result rather than a failure of
the change** — before it, the payoff was not small but exactly zero and provably so — and it says
where to look next: at the *speed*, which means the beat, the swing, or SPEC section 8's
buoyancy-by-`CellKind`, and not at the water.

*(Group G below took a third answer that is not in that list — **the field**, which never moved
— and it produced devorocytes and not myocytes. The reading above is still the reading: what
myocytes are short of is speed.)*

#### `config/dense.toml`, run the same distance

300,000 ticks, same seed, same eight founders. The dawn is 3,000 ticks rather than 10,000,
because the light is four times as bright per tile.

| Tick | Alive | Mean cells | Mean genes | Mean depth (of 288) |
| --- | --- | --- | --- | --- |
| 30,000 | 1,848 | 1.99 | 1.45 | 107 |
| 100,000 | 1,571 | 2.66 | 2.37 | 120 |
| 200,000 | 666 | 8.17 | 5.90 | 147 |
| 303,000 | 542 | 9.90 | 8.47 | 143 |

**It is alive, it is denser, and it is smaller.** 542 organisms against the shipped profile's 650
in a quarter of the water, which is about **3.3 times** the bodies per unit of water; the
population falls further and the bodies grow faster and larger (9.90 cells against 8.39 at the
same age), which is what packing the same energy into less room ought to do. Ending composition:
4,797 photocytes, 556 gonocytes, 11 sclerocytes, 1 sensocyte, 0 myocytes, 0 devorocytes.

⚠️ **Devorocytes are still transient**, though they reach 7 at once here against the shipped
world's 3. This is a run, not the diagnostic's measurement — whether the contact rate turns into a
feeding-strategy split is the question the profile exists to be pointed at over an overnight run,
and 300,000 ticks is not that.

#### What Group F decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **The axis is the line between a cell's first and last adhered partner** | `physics.rs`, `axis` | Exactly right for a cell in a chain, which is the case that matters. A branch point has no single direction the body runs and gets a deterministic one; it is also a place a body genuinely is not slender. |
| **Fewer than two adhesions means isotropic drag** | `physics.rs`, `axis` | ⚠️ The load-bearing half. Using the one spring a cell does have, or breaking the tie the way `direction` does, would hold every loose cell in the world harder in one direction than another — thrust with no muscle behind it, in a direction decided by storage order. |
| **Every axis is read before any cell moves** | `physics.rs`, `axes` | A cell's axis is a line between two *other* cells, half of which the loop has already moved. Read from a half-moved world it would depend on storage order, which SPEC section 2 forbids. |
| **`across_drag` is worked out once, not per cell** | `physics.rs` | A power is the most expensive operation in the module and it is the same number for every cell. |
| **The bound is `1.0..=3.0`** | `config.rs`, `DRAG_ANISOTROPY_CEILING` | One is isotropic water, kept reachable because it is the control for every claim about swimming. Three is where a prototype produced not-a-number at `collision_stiffness = 5,000` — an unequal damping of two components is a *rotation* of the velocity towards the axis, and past three the correction and the overshoot stop cancelling. |
| **`LIGHT_REFERENCE` is 0.02 and is not a config key** | `behaviour.rs` | It is the background gradient of the shipped world, derived from SPEC section 4's own formula. A cell cannot know what `light.cap` is set to, and a sensor whose meaning moved with the weather is one whose evolved gain means something different after every change of conditions. |
| **⚠️ `SENSOR_GAIN_SPREAD` does not follow `MAX_SENSOR_GAIN`** | `genome.rs` | The genetic distance was scaled by `2 × MAX_SENSOR_GAIN`, and genetic distance is the unit every species boundary in every chronicle is measured in. Raising the gain would have silently redrawn all of them. One is a sensor's range; the other is a unit of measurement. |
| **The golden vector was re-recorded, and said so** | `run.rs` | The one time it is allowed. The old values are written out beside the new ones so a recording made before today can still be identified. |
| **`dense` shrinks the height** | `config/dense.toml` | ⚠️ SPEC section 8: a 64-cell body at `MAX_REST_LENGTH` reaches 870 units, so in a world narrower than about 1,200 a chain reaches more than half way round and the spring-through-the-seam warning goes live. |

#### ⚠️ Three tests moved, and none of them was weakened

- **`a_run_produces_what_it_produced_before_group_a`** — re-recorded, deliberately. Any one of the
  three changes moves every draw in every stream after the first birth. A golden vector that
  survived would have meant the changes had not landed.
- **`a_myocyte_oscillates_its_springs_and_pays_for_the_work`** — 2.790 → 0.00186 over the same 130
  ticks, which is exactly the ratio `movement_cost` moved by. The old figure is kept in the comment
  because it is the argument for the change: **one spring, at rest, cost one and a half times a
  myocyte's entire upkeep.**
- **`hue_comes_from_lineage_and_drifts_with_it`** — the band widened from 0.15 to 0.2, measured at
  0.1512. The cause is `MAX_SENSOR_GAIN`: at one, a gain spent most of its life pressed against its
  own clamp, where a point mutation changes nothing and `divergence_from` sees an identical gene.
  Off the clamp the same mutations are real, so lineages drift faster. **The test was made stronger
  rather than looser**: the counterfactual it rejects is now *measured* on the same population, by
  colouring it from the genome fingerprint instead, rather than being a number in a comment.

### Group G — making swimming *pay*, and making depth a thing a body is — **done**

⚠️ **Out of the phase's plan for the same reason Group F was**, and it is the answer to Group
F's own last paragraph: *"the mechanism exists now and the payoff does not"*. Two changes,
landing together because both move every golden vector and there is no sense in doing that
twice.

- [x] **G1. `the_light_blotches_drift_sideways_without_being_redrawn`** ⭐⭐ — `light.patch_drift`,
  a new key in `[light]`, shipping at **0.0006** world units per tick. `PatchNoise::at` gains a
  coordinate offset; the offset advances every tick; the ceilings are re-read every 100 ticks
  through the same routine a live `[light]` retune uses.
- [x] **G2. `a_drifting_field_sheds_what_its_ceiling_slides_off`** — what the drift costs,
  measured rather than argued.
- [x] **G3. `a_patch_drift_faster_than_the_light_can_replace_is_refused`** — `0..=0.005`, and a
  fourth kind of refusal because the sentence is a different one.
- [x] **G4. `light.patchiness` 0.15 → 0.5** — a drifting field is worth following only if there
  is somewhere better to arrive at.
- [x] **G5. `buoyancy_is_what_a_kind_is_made_of`** — a sixth number on SPEC section 6's table.
- [x] **G6. `a_body_floats_or_sinks_according_to_what_it_is_made_of`** — the physics, and the
  founder's exact neutrality.
- [x] **G7. `Census` gains `mean_depth` and `kinds`** — every per-kind count in this document
  and in `docs/PHASE4.md` had been taken by hand off a debugger. A headless run now prints them.

#### ⭐⭐ Three runs, because a global change with a feedback nobody priced is Phase 4's failure shape

310,000 ticks, eight founders, seed 42, release build, measured 1 August 2026. The first
10,000 ticks are the dawn, so these are directly comparable with Group F's table above.

| | **Group F baseline** | **C** — patchiness only | **B** — buoyancy only | **A** — as shipped |
| --- | --- | --- | --- | --- |
| `patchiness` | 0.15 | 0.5 | 0.15 | **0.5** |
| `patch_drift` | — | 0 | 0 | **0.0006** |
| buoyancy | — | yes | yes | **yes** |
| Alive | 650 | 882 | 677 | **777** |
| Field | 65,755 | 66,319 | 59,668 | **64,303** |
| Biomass | 33,961 | 33,506 | 35,524 | **33,135** |
| Detritus | 4,475 | 4,751 | 4,330 | **4,811** |
| Dissipated | 6,468,877 | 6,651,288 | 6,551,850 | **6,696,681** |
| Light | 6,573,069 | 6,755,863 | 6,651,373 | **6,798,930** |
| Mean cells | 8.39 | 6.00 | 8.23 | **6.66** |
| Mean genes | 8.75 | 5.81 | 7.41 | **4.75** |
| Mean depth (of 1,152) | 483 | 486 | 476 | **553** |
| Living cells | 5,467 | 5,292 | 5,574 | **5,177** |
| Photocytes | 4,797 | 4,365 | 4,889 | **4,371** |
| Gonocytes | 652 | 889 | 676 | **773** |
| Sclerocytes | 11 | 27 | 8 | **12** |
| Sensocytes | 6 | 8 | 0 | **13** |
| **Myocytes** | **1** | 3 | 1 | **2** |
| **Devorocytes** | **0** | 0 | 0 | **6** |

The shipped run, over time — the ledger checked itself every thousand ticks of all 310,000 and
never once disagreed:

| Tick | Alive | Field | Biomass | Mean cells | Mean genes | Mean depth |
| --- | --- | --- | --- | --- | --- | --- |
| 20,000 | 467 | 169,494 | 6,527 | 1.98 | 1.08 | 309 |
| 50,000 | 1,760 | 110,462 | 24,450 | 1.98 | 1.89 | 349 |
| 100,000 | 2,127 | 88,480 | 29,790 | 2.01 | 2.56 | 427 |
| 150,000 | 2,138 | 84,504 | 31,362 | 2.10 | 3.02 | 439 |
| 200,000 | 1,970 | 81,354 | 32,042 | 2.37 | 3.36 | 451 |
| 250,000 | 1,407 | 74,578 | 32,690 | 3.50 | 4.09 | 531 |
| **310,000** | **777** | **64,303** | **33,135** | **6.66** | **4.75** | **553** |

Peak population **2,219** against a `limits.max_organisms` of 4,000, and 292,752 organisms
born against 291,975 dead. **Nothing degenerated**: no extinction, nothing near the arena, and
mean depth in the *lower* half of the water rather than a mat against the surface.

#### ⭐ Devorocytes appeared, and the drift is the only thing they can be attributed to

**Six at tick 310,000, against a baseline of nought**, and — the reading that makes it a result
rather than a number — **nought in both of the other two runs**. C differs from A in exactly one
setting, `patch_drift`, and it has none. Group F's `dense` profile, which was built specifically
to provoke a feeding split by quadrupling the contact rate, also ended with nought.

The mechanism is the one the two changes were designed to produce together. A drifting field
takes the *sitting still* strategy off the table for a photocyte; a body that cannot follow the
light is a body with a reason to eat something that can.

⚠️ **It is one reading of six cells in a population of five thousand.** It says the strategy is
now reachable, not that it is established. The question of whether it persists belongs to an
overnight run, which is what `dense` exists to be pointed at.

#### ⚠️ Myocytes did not appear, and that is the honest answer

**Two, against a baseline of one**, with three in the run that had no drift at all. There is no
signal here. Making the field move has not yet made a muscle worth owning, and the arithmetic
Group F recorded still stands: a myocyte costs **0.014** a tick against a photocyte's 0.004,
three and a half times as much, and a full-stroke undulator manages 1.9 units per 1,000 ticks
against a body that lives 571 to 2,000 of them. A body that grew a perfect undulator and swam
flat out for its whole life would cover about **four units** — two thirds of one of its own
cells — while the field it is chasing moves 1.2. The margin is there and it is thin, and
nothing has found it in 310,000 ticks.

**What that says about where to look next** is that the drift is the right *shape* of pressure
and the wrong *magnitude* of reward, and the lever is the stroke rather than the water: a body
that swam two or three times faster would be chasing something it could visibly catch. That is
`osc_freq`, the 0.4 amplitude coefficient in section 9's controller, and `MAX_REST_LENGTH` —
none of which this group touched.

#### ⭐ What the drift costs, measured twice

**On a full field with nothing living in it** — the worst case, since a population has already
eaten most of the field down below the level a falling ceiling can reach — the drift sheds
**0.0179 a tick** against the 23.04 the light offers the world, which is **0.08%**. That is
`a_drifting_field_sheds_what_its_ceiling_slides_off`.

**In the living world**, run A against run C, which differ in nothing else: `dissipated` is
45,393 higher over 310,000 ticks, or **+0.146 a tick** — 0.63% of the world's gross income.
⭐ And `influx_total` is 43,067 higher, or **+0.139 a tick**, which is the other half of the
same fact and was not expected: a ceiling that slides *up* under a tile makes room the light
immediately fills. **The drift very nearly pays for itself**, at a net cost to the world of
about 0.007 a tick, or three hundredths of one per cent.

**Carrying capacity is untouched.** Living cells 5,177 against C's 5,292, and biomass 33,135
against 33,506 — both inside 2%. The population count is 12% lower and the bodies are 11%
larger, which is the same biomass arranged differently.

#### ⚠️ Buoyancy at the shipped magnitude changed nothing measurable, which is what gentle means

Run B is the shipped world with buoyancy and nothing else. Against the baseline: 677 organisms
against 650, mean depth **476 against 483**, 8.23 cells against 8.39. That is the same world.

The measured drift per lifetime is in SPEC section 6 and the largest figure in it is **2.87
world units over 2,000 ticks** — half of one cell's width. A lineage crosses this world over
hundreds of generations or not at all, and 310,000 ticks is about 180 generations.

**This is a deliberate result rather than a disappointment, and it is the half of the change
that was most likely to break the world.** SPEC section 8's diagnostic measured a uniform sink
of `g ≈ 5` putting a population on the floor in forty generations; the whole risk in this change
was magnitude, so it was started an order of magnitude below that and measured. What has been
established is that the mechanism is in place, the founder is exactly neutral, and no world
drowned. **If depth is wanted as a live pressure, this column is where the number moves**, and
the next step is to double it and re-run rather than to add anything.

#### What Group G decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **The drift is a coordinate offset, never a reseed** | `grid.rs`, `PatchNoise::slide` | ⚠️ The load-bearing half. Re-drawing the lattice from a moving seed also makes a field that changes, and it would delete what a lineage had found rather than tow it away - so there would be nothing to follow and no advantage in following it. |
| **Sideways only** | `grid.rs`, `PatchNoise::at` | The lattice wraps sideways because the world does, so a horizontal drift is seamless at every offset. Downwards it does not wrap, so a vertical drift would push blotches through the floor and invent replacements at the surface - the reseed above, applied at the two edges. |
| **The offset is accumulated, not `drift × ticks`** | `grid.rs`, `Grid::drift` | `[light]` is live. A `drift × ticks` offset would teleport the whole field sideways the instant somebody turned the dial; adding a step per tick means the dial changes the *speed* and nothing else. |
| **The ceilings are recomputed every 100 ticks, through `relight`'s own routine** | `grid.rs`, `Grid::retarget` | There must be exactly one place a ceiling is computed, or a drifting world and a retuned one would disagree about what a ceiling is. The interval does not change what the drift costs - the loss per retarget and the retargets per tick are inversely proportional - so it buys smoothness only. |
| **`PATCH_DRIFT_CEILING` is 0.005, and it is a bound on the *shipped* light** | `config.rs` | Unlike `DIFFUSION_STABILITY_LIMIT`, which is a fact about a stencil at any configuration. Written as one number anyway, because a bound that moved with four other settings is one nobody can reason about and the interesting range is a ninth of it. |
| **A fourth kind of refusal rather than reusing `Unstable`** | `config.rs`, `OutrunsTheLight` | The arithmetic does not stop working. The *world* empties, while the ledger balances to the last digit throughout, because the loss goes through an account. That is a different sentence and the sentence is the whole value of a refusal. |
| **Buoyancy is a force, not a velocity** | `physics.rs`, `Physics::lift` | So it goes through the two drags. A cell in a flat chain has its axis across the pull, so a long flat body settles at about half the rate a loose cell does - which is right, and which came out of Group F's anisotropy rather than being written down. |
| **A myocyte weighs exactly nothing** | `cell.rs` | Chosen against the other five rather than from what the tissue is. A muscle that floated would be a way of changing depth *without swimming*, which is the thing the drifting field exists to make worth doing. |
| **The founder's two kinds sum to exactly zero** | `cell.rs` | Checked with `==`. Every run begins with one photocyte and one gonocyte; a pair that came to a thousandth would start every world drifting and would have moved every depth in this document for a reason that is not selection. |
| **`Census` gained the counts rather than reading them off `Sample::biomass_of`** | `census.rs` | Per-kind biomass is apportioned out of each organism's single pool, so it answers "how much energy is held in muscle" and cannot answer "how many myocytes are there". `KINDS` moved to `census.rs` and `series.rs` re-exports it, so the two readings cannot end up with different numbers of columns. |

#### ⚠️ Six tests moved, and none of them was weakened

- **`a_run_produces_what_it_produced_before_group_a`** — re-recorded, deliberately, for the
  second time. Both previous sets are kept beside the new one. The world it describes is a
  *richer* one: 14% more organisms born, 18% more biomass, 16% more light fallen, all of which
  is `patchiness` making the good tiles better.
- **`energy_is_conserved_over_100k_ticks`** — its four pinned figures re-recorded, both previous
  sets kept. Three moved in one direction for one reason: deeper blotches mean tiles below the
  depth average fill sooner and then shed everything reaching them, so the world takes in twice
  the light, sheds two and a half times as much, and holds *less*. The biological pump running
  harder, which is what a blotchier ocean is.
- **`the_field_reaches_a_ceiling`** — three pinned figures re-recorded, and the settling
  criterion **strengthened**: it now requires a hundred still ticks in a row rather than one.
  Diffusion carries sub-representable movements to the next tick, so a single still tick can
  happen on the way to somewhere and does - the first still tick of that world is 2,310 and the
  last moving one is 2,320. The old form was passing on a coincidence.
- **`motion_is_viscous_not_ballistic`** and **`overlapping_cells_push_apart`** — the "has it
  stopped" claim is now about the axis the shove was along, because what a loose cell is left
  with vertically is its own buoyancy and is *supposed* to still be there. The first gained a
  new assertion pinning that speed to SPEC section 8's `f × dt × drag / (1 − drag)`, which is
  strictly more than it checked before.
- **`the_world_wraps_sideways_and_is_closed_top_and_bottom`** — the sinking case is now a
  sclerocyte, so each boundary is tested against a cell whose own composition holds it there
  for the whole hundred ticks rather than against a shove that decays in ten.
- **`the_live_settings_can_be_changed_while_a_run_is_going`** — `hot > cold × 2` replaced by an
  exact figure. `dissipated` is dominated by the field's ceiling, not by metabolism, and deeper
  blotches brought three hundred units of spill into a comparison about two units of upkeep.
  The two worlds are identical apart from the temperature, so everything else cancels in the
  *difference* and the test now works out what the retune should have cost from SPEC section 6's
  table and insists on it to a thousandth.
- **`a_tick_feeds_the_bodies_in_the_world`** and **`the_drift_can_be_read`** — a longer dawn
  (the field takes a third longer to reach deeper ceilings) and a body's position read at the
  tick it died rather than the tick it was seeded (three photocytes float about a unit and a
  half over a lifetime). Neither claim changed.

### Group H — making the stroke big enough to matter — **done, and the result is negative**

⚠️ **Out of the phase's plan for the third time, and it is the answer to Group G's own last
paragraph**: *"the lever is the stroke rather than the water"*. It is. The stroke was measured,
moved, and shipped, and **it did not produce myocytes**, because the diagnostic that went with
it found that the controller the stroke belongs to is almost never executed at all.

- [x] **H1. `a_bigger_stroke_is_what_makes_swimming_worth_doing`** ⭐⭐ — the sensitivity
  measurement, meaned over nine body shapes, and the three claims it supports.
- [x] **H2. `[behaviour]`, a new table in SPEC section 3** — `resting_amplitude` at **0.8**
  (was a constant 0.3) and `stroke` at **1.0** (was a constant 0.4). Both `fraction`-gated.
- [x] **H3. `the_stroke_cannot_take_a_rest_length_below_nothing`** — why one is the ceiling.
- [x] **H4. `a_myocyte_works_through_the_stroke_the_settings_give_it`** — the controller obeys
  the document, and SPEC's original 0.3 and 0.4 are still reachable *as a configuration*.
- [x] **H5. The 310,000-tick run**, and the diagnostic that explains it.

#### ⭐ The sensitivity table, which is the part worth keeping

Nine hand-built bodies — 6, 8 and 12 cells by 2, 3 and 4 units of kink — driven by a travelling
wave through the real physics for a **2,000-tick lifetime**, and meaned. Meaned because a single
undulator is strongly resonant: at one stroke and one beat the same body kinked three units
travels three times what it does kinked four, since the shape it settles into under its own
springs is not the shape it was built in. The first reading of the stroke was taken on one shape
and had a factor of five of noise in it.

| Lever | Walked from → to | Units per lifetime | Work per tick | Verdict |
| --- | --- | --- | --- | --- |
| **the stroke, unsensed** | 0.12 → 0.8 | **0.3 → 11.7** | ×24 | ⭐⭐ shipped |
| **the stroke, driven** | 0.4 → 1.0 | **3.7 → 41.1** | ×2.8 | ⭐⭐ shipped |
| `physics.drag_anisotropy` | 2.0 → 2.5 → 3.0 | 41.1 → 44.4 → 46.2 | ×1.0 | not taken |
| `osc_freq` | 1 → 2.5 → 5 rad/s | 6.0 → 31.9 → 15.5 | ×12 | range already right |
| segment length | 8 → 13.6 → 27.2 | ×1.7 → ×3.4 | ×2.9 → ×11.6 | reachable already |
| `physics.drag` | 0.92 → 0.95 → 0.99 | ×2.7 → ×9 | ×1.7 | rejected |
| spring stiffness | 10 → 40 → 144 | ×1.0 → ×0.65 | ×2.8 | already near optimum |

**Distance goes as roughly the cube of the stroke and as the square root of everything else.**
Nothing else is close, and that is the whole of the argument for `[behaviour]`.

Three levers were rejected on their own terms rather than on their size. **`physics.drag` at
0.99** buys a factor of nine and lets a cell coast a hundred units off one shove, and SPEC
section 8 is explicit that momentum is not a strategy here — that is a different world, not a
faster one. **`osc_freq`** peaks between two and three and a half radians a second and falls away
either side, so `MAX_OSC_FREQ` of 5 already contains the optimum and widening it would only add
draws that are worse; a gene drawn uniformly lands in the useful band about half the time.
**Segment length** is already reachable — a gene may draw `rest_length` up to `MAX_REST_LENGTH`
of 13.6 without anything changing — and raising the bound would eat the seam headroom SPEC
section 8 warns about (64 cells at 13.6 is 870 units against a half-world of 1,024).

⚠️ **And the cost is not the constraint, which was the thing most likely to make this pointless.**
A body driven flat out does about 95 units of work a tick across six to twelve springs; at
`movement_cost` of 1e-4 that is **0.0095 a tick**, against the **0.084** the same six myocytes
pay simply to be alive. Swimming costs about a ninth of standing still. And the ratio moved the
right way: distance per unit of work is *higher* at the shipped stroke than at the old one, so a
body that swims well now pays less per unit travelled than one that twitched.

#### ⚠️ `drag_anisotropy`'s NaN at three does not reproduce, and the ceiling is not where SPEC says

Group F recorded `DRAG_ANISOTROPY_CEILING` as *"where the arithmetic stopped: at a
`collision_stiffness` of 5,000 a pile of cells produced not-a-number within a few hundred
ticks."* Re-measured, on a pile of 64 cells **bonded into chains so that every interior cell has
an axis** — the plain pile-up has no springs at all, so no cell has an axis and the anisotropy is
arithmetically absent from it:

| `k` | 40 | 1,000 | 3,000 | 5,000 | 12,000 |
| --- | --- | --- | --- | --- | --- |
| 1.0 | settles | settles | settles | settles | **jitters** |
| 2.0 | settles | settles | settles | settles | **jitters** |
| 3.0 | settles | settles | settles | settles | **jitters** |

Nothing produced a not-a-number at any of them, and the edge is the same at every anisotropy, so
**it is the collision stiffness and not the anisotropy**. Higher anisotropy in fact settles the
pile *harder*: at `k = 3` the crowd's peak motion is 22% below `k = 1`'s, which is what thicker
water sideways ought to do. The ceiling was left at 3.0 anyway, because nothing wants to go above
it — the whole range from 2 to 3 is worth 12% — but the *reason* written beside it is wrong and
is now recorded as such.

#### ⭐⭐ The 310,000-tick run, and why it is a negative result

Eight founders, seed 42, `config/default.toml` as it now ships, against Group G's run of the
same length.

| | **Group G** | **Group H** |
| --- | --- | --- |
| Alive | 777 | **812** |
| Field | 64,303 | **67,271** |
| Biomass | 33,135 | **32,548** |
| Mean cells | 6.66 | **6.28** |
| Mean genes | 4.75 | **5.46** |
| Mean depth (of 1,152) | 553 | **534** |
| Living cells | 5,177 | **5,096** |
| **Myocytes** | **2** | **1** |
| **Devorocytes** | **6** | **0** |

**The world survived and nothing degenerated** — no extinction, a peak population far below
`max_organisms`, biomass inside 2% of every run since Phase 4, and mean depth in the lower half
of the water rather than a mat against the surface. **And there is no myocyte signal, and no
devorocyte signal.** One against two, and nought against six, are both single readings of single
figures in a population of five thousand cells.

⚠️ **The run is bit-identical to Group G's for its first 200,000 ticks** — 467, 1,760, 2,127,
2,138 and 1,970 organisms at the five checkpoints, and every one of the five accounts equal to
the last digit. A change that made every muscle in the world eleven times stronger left the world
*exactly* where it was for two thirds of the run. That is not a change failing to matter. It is a
change never being applied.

#### ⭐⭐ What the diagnostic found: the muscle is disconnected, not weak

Counted over 120,000 ticks of the shipped world, every spring with a myocyte on one end:

| | Spring-ticks |
| --- | --- |
| A myocyte on a spring, and **no gene in its genome names its state** | **56,903** |
| A gene answers it, but that gene's `osc_freq` and `osc_phase` are both still exactly nought | 874 |
| **A muscle that moved a spring by any amount at all** | **0** |

`behaviour.rs`'s `first_match` maps a cell's `state` to the first gene whose `trigger_state` is
that state, and a myocyte with no such gene is skipped entirely. A state is one of **64**. A
genome at that age holds about **three genes**, and the probe shows their trigger states are
almost all `[0]`, `[0, 0]`, `[0, 0, 0]` — because zero is the founder's and `trigger_state` is
not where mutation spends its time. Meanwhile development scatters daughters across the whole
state space through `child_state`: the silent myocytes are sitting in states 1, 5, 8, 11, 20, 44,
46, 51.

So a myocyte is grown into a state nothing in its own genome is listening to. It is
**anatomically present and behaviourally disconnected** — a muscle with no nerve to it — and it
pays a myocyte's 0.014 a tick, three and a half times a photocyte's, to do absolutely nothing.
The 1.5% that do find a gene meet a second gate immediately: that gene is a copy of the
founder's, whose `osc_freq` and `osc_phase` are both zero, so `sin(0)` is zero and the rest
length is multiplied by exactly one.

**This is why three separate changes to the payoff have come back null.** Group F made
locomotion arithmetically possible, Group G gave it somewhere to go, Group H made it eleven times
faster, and all three act on a code path this world takes about once in every two hundred
thousand spring-ticks. Each was necessary; none could have been sufficient; and the shipped
stroke is correct and stays, because on the rare occasion the path *is* taken it now moves a body
five body-lengths in a lifetime instead of half of one.

#### What to try next, in the order of what it costs

**The wiring, not the reward.** Three candidates, and the first is much the smallest:

1. **A mutation operator that moves a gene's `trigger_state` onto a state some cell in the body
   actually occupies.** `mutation.rs` mutates `trigger_state` as one field among fifteen and to a
   uniformly random state; nothing biases it towards the states development produces. This is one
   operator and it does not change what anything means.
2. **A smaller state space.** 64 states against three genes is what makes the miss near-certain.
   This is a one-line change with a very wide blast radius: every genetic distance, every species
   boundary and every golden vector moves.
3. **A myocyte with no gene answering it falls back to the gene that built it.** This is the most
   direct and it changes what a *state* means, so it should be argued before it is written.

The measurement to take first is the cheapest one: **what fraction of cells in the shipped world
sit in a state their own genome names?** If that number is low for photocytes too, then it is not
a fact about muscle at all — it is a fact about the genome, and it has been quietly shaping
everything since Phase 3.

⭐⭐ *(Taken, in Group I. It is low for photocytes too — **2.2% of grown cells**, of every kind —
so the last sentence above was right, and Group H's third candidate was the one written. Group I
has the number, the change and the run.)*

#### What Group H decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **The two coefficients are one table, not two settings in two places** | `config.rs`, `RawBehaviour` | They multiply together into a single stroke, and a resting amplitude read from one configuration against a stroke read from another is a muscle nobody specified. `behaviour.rs` carries them as one `Drive`. |
| **`fraction` rather than a fifth kind of refusal** | `config.rs` | ⚠️ `stroke`'s ceiling of one *is* the fraction bound, and that is not a coincidence: `base_rest × (1 − stroke)` reaches nought at exactly one. A `stable`-style gate would have said the arithmetic stops working, and it does not — the arithmetic works perfectly and the model stops meaning what it says. |
| **The measurement is meaned over nine shapes** | `physics.rs`, `swims_and_works` | A hand-built undulator is strongly resonant and a single shape gave a factor of five of noise. Three lengths by three kinks is enough for the mean to be about the stroke. |
| **`drag_anisotropy` stays at 2.0** | not changed | It is worth 12% across its whole remaining range and 2.0 is where slender-body theory puts a real slender body. Buying 8% by leaving the physical justification behind is a bad trade. |
| **`physics.drag` was rejected despite being the second-largest lever** | not changed | ×9 at 0.99, and a cell then coasts a hundred units off one shove. SPEC section 8: *momentum is not a strategy here.* That is a different world, not a faster one. |
| **`a_body_that_senses` takes its resting amplitude as a parameter** | `behaviour.rs` | ⚠️ At the shipped 0.8 there are two tenths of room below the clamp, so a body four units from a neighbour and one eight units away would read *the same number* and the tests would be measuring the clamp. Nought for the tests about what a sensocyte reports, a half for the test about the sign. Both got stronger. |
| **The golden vector did not move, and that is reported rather than assumed** | `run.rs` | It runs 4,000 ticks, and in 4,000 ticks no muscle anywhere fires. The one change in this project so far that did not move it — which, read against the diagnostic above, was the first sign of what was wrong. |

#### ⚠️ Three tests moved, and none of them was weakened

- **`a_myocyte_oscillates_its_springs_and_pays_for_the_work`** — the swing re-recorded from
  **7.04–8.96 to 1.6–14.4** on a spring asked to be eight units long, and the cost from
  **0.00186 to 0.0827** over the same 130 ticks. Both previous figures are kept beside the new
  ones. The cost is 44 times higher because work is force through distance and both halves scale
  with the swing; it is 4.5% of a myocyte's own upkeep, which is the number that says the change
  is affordable.
- **`a_travelling_wave_carries_a_body_through_the_water`** — the unsensed row re-recorded from
  **0.154 to 2.3** units per 1,000 ticks. The driven row is unchanged, because it passes its
  stroke in explicitly.
- **`a_myocyte_that_does_nothing_pays_nothing`** — the held rest length is now worked out from
  the settings rather than written down. A number there would have been a second, silent copy of
  the shipped stroke that a retune leaves behind.
- **`a_sensocyte_reports_a_gradient_towards_its_target`** and
  **`both_attraction_and_avoidance_are_reachable_for_a_sensocyte`** — both now build their scenes
  at a stated resting amplitude instead of the shipped one, for the reason in the table above.
  The first reads the signal directly rather than through an offset, and the second sits exactly
  half way up the clamp so the two directions have identical room.

### Group I — connecting a cell to its own genome — **done, and it is half a positive result**

⚠️ **Out of the phase's plan for the fourth time, and it is Group H's own closing instruction**:
*"the wiring, not the reward"*. Group H named three candidates and asked for one measurement
first. The measurement was taken, and it is larger than the muscle question.

- [x] **I1. The measurement Group H asked for** ⭐⭐ — what fraction of *all* cells sit in a state
  their own genome names, by kind, over a 300,000-tick run of the shipped world.
- [x] **I2. `a_cell_remembers_which_gene_built_it`** — development stamps the position of the
  gene that made or re-made a cell onto that cell. `Divide` and `Differentiate` stamp;
  `Terminate` does not, because stopping a cell says nothing about what it is.
- [x] **I3. `a_cell_with_no_gene_is_the_seed_cell_and_needs_none`** ⭐ *(property test)* — the
  whole justification for giving the seed cell nothing rather than gene zero.
- [x] **I4. `a_myocyte_takes_its_rhythm_from_the_gene_that_built_it`** ⭐⭐ — the headline, and
  it is written so that it can only pass under the new rule: a myocyte in state 44 that no gene
  names, built by a gene whose trigger state nothing in the body is in.
- [x] **I5. The 313,000-tick run**, and the spring-tick count re-taken on it.

#### ⭐⭐ I1 — the measurement, and it is a fact about the genome rather than about muscle

Shipped world, seed 42, eight founders, 300,000 ticks after the dawn, sampled every 200 ticks —
**6.46 million cell-observations**. *Does this cell's own genome contain a gene whose
`trigger_state` is this cell's `state`?*

| | Named | Of | |
| --- | --- | --- | --- |
| **Every living cell** | 2,610,738 | 6,458,391 | **40.4%** |
| **Every cell except the seed cell it grew from** | 86,418 | 3,905,138 | **2.2%** |

and by kind, over grown cells:

| | photocyte | devorocyte | myocyte | sclerocyte | sensocyte | gonocyte |
| --- | --- | --- | --- | --- | --- | --- |
| Grown cells in a state their genome names | **2.5%** | **0.9%** | **2.1%** | **3.4%** | **2.7%** | **2.0%** |

**Group H's "if that number is low for photocytes too" is answered: it is low for everything.**
The 40% in the first row is almost entirely the seed cell — every body has exactly one, it is in
state 0 because SPEC section 7 puts it there, and the founder's own gene triggers on state 0. It
is the one cell in the world that is connected by construction. **Take the seed cells out and a
genome answers to 2% of its own body.**

⚠️ **So it was never only a fact about muscle.** Development uses the same lookup, so a cell
whose state no gene names is a cell development can do nothing further with — and that is why
**mean cell count sits at 1.98 to 2.13 for the first 140,000 ticks of every run this project has
ever measured**, and why the founder is two cells: its gene hands its daughter `child_state = 1`
and nothing names state 1. Bodies have been small for the same arithmetic reason behaviour was
absent. It has been shaping the project since Phase 3.

#### The fix, and what it does not touch

**A cell takes its behaviour from the gene that built it.** `Cell` and `BodyCell` gain
`gene: Option<u8>`; `development.rs` writes it; `behaviour.rs` reads it in place of
`first_match`, which is deleted along with its `STATES` table. The argument is in SPEC section 7
and at length on `develop`.

**The seed cell is given nothing**, rather than gene zero or a default rhythm, and nothing is
lost by it: the only two ways a cell can become a myocyte or a sensocyte are a gene's
`child_kind` and a gene's `new_kind`, and both stamp — so a cell with no gene is always a
photocyte, which needs none. I3 is that as a property test.

**Development is not changed.** See the open question below.

#### ⭐⭐ I5 — do muscles fire? Yes. Are there myocytes? No.

Same world, same seed, same 120,000-tick window, every spring with a myocyte on one end:

| | Before | After |
| --- | --- | --- |
| A myocyte on a spring and **no gene speaks for it** | **70,352** | **0** |
| A gene answers, but its `osc_freq` is still exactly nought | 874 | 68,571 |
| **A muscle whose rest length is a moving function of time** | **0** | **9,901** |
| Total myocyte spring-ticks | 71,226 | 78,472 |

*(70,352 against Group H's 56,903 is the same count over a longer window — 120,000 ticks after
the founding rather than 120,000 of the world's, which includes a 13,000-tick dawn. **The 874 is
identical in both**, which is what says the two instruments agree.)*

And the wiring itself, over the same 6.5 million cell-observations after the change: **100% of
every grown cell, and 100% of every myocyte, devorocyte, sclerocyte, sensocyte and gonocyte in
the world, is attached to a gene.** The only cells that are not are seed photocytes.

⚠️ **The second row is what is left, and it is a different problem with the same shape.** A gene
that has just become a myocyte-maker still carries the founder's `osc_freq` of nought, and
`sin(0)` is nought. `mutation.rs` perturbs **one field of a hit gene**, so a moving muscle needs
two mutations. What changed is that both now have to land on **one gene out of three or four**
rather than one of them having to land on a state in a space of **sixty-four**.

The run, against Group H's:

| | **Group H** | **Group I** |
| --- | --- | --- |
| Alive | 812 | **797** |
| Mean cells | 6.28 | **6.62** |
| Mean genes | 5.46 | **6.17** |
| Mean depth (of 1,152) | 534 | **488** |
| Living cells | 5,096 | **5,277** |
| Biomass | 32,548 | **33,468** |
| **Myocytes** | **1** | **4** |
| **Devorocytes** | **0** | **0** |
| Mean displacement per lifetime | — | **2.028** units (1.938 before) |

**The world survived and nothing degenerated**: no extinction, a peak of 2,131 against a
`max_organisms` of 4,000, biomass inside 2% of every run since Phase 4, and a mean depth in the
lower half of the water. **The first 38,000 ticks are bit-identical to the run before the
change** and then they part, which is the change landing.

⚠️⚠️ **And there is still no myocyte signal.** Four against one is a single reading of a single
figure, and this run passed through checkpoints holding nought and one on its way there while
the run it replaced passed through **ten**. The displacement figure moved by a twentieth of one
cell's width across 263,000 lifetimes and is dominated by bodies with no muscle at all. **Nothing
in this world swims.** What the change bought is that the question is now askable: before it, no
experiment on the payoff could return anything, because the controller was never executed.

#### ⚠️ Does development have the same bug? Yes — and it does not have the same fix

**It is the same arithmetic and the same near-certain miss.** 2.2% is a fact about
`trigger_state` matching, and development is the other thing that matches on it. Every run this
project has measured has had bodies of exactly two cells for its first hundred thousand ticks,
and this is why.

**But it is not the same *kind* of mistake, which is why it was left alone here.** Behaviour had
a genuine choice between two readings of one record and Phase 4 picked the one that fails; SPEC
was silent and the silence has now been filled. Development has no second reading available:
SPEC section 7's pseudo-code says in as many words *"find the FIRST gene where
`gene.trigger_state == cell.state`"*, and section 7's own justification for the entire genome
design rests on it — *"because conditions key on `state`, duplicating a gene and changing its
`trigger_state` creates a new body part"*. Take the keying away and the operator the project is
built on goes with it. **The fix for behaviour was to stop addressing; there is no equivalent
for development, because a rule has to say which cells a gene acts on.**

So the recommendation is Group H's *first* candidate rather than its third, and it is now much
better motivated than it was:

1. ⭐ **Bias where a `trigger_state` or `child_state` mutation lands.** `mutation.rs` re-draws a
   discrete field uniformly over all 64 states; drawing instead from the states the parent's own
   genome already mentions would make duplicate-and-diverge land on something. It changes **no
   meaning at all** — SPEC says "re-draw uniformly" of a *field*, and which distribution a
   discrete re-draw uses is exactly the kind of thing it leaves open. One operator, no golden
   vector argument beyond the usual one.
2. **A smaller state space.** 64 against three genes is what makes the miss near-certain. One
   line, and it moves every genetic distance, every species boundary and every recorded figure
   in the project.
3. **Not first-match-wins**, and not `trigger_state` itself.

⭐⭐ *(Taken, in **Group J**, and it is the first candidate exactly as recommended. The two state
fields turned out to want **opposite** biases, which this paragraph did not anticipate: a
`trigger_state` towards the states cells are in, a `child_state` mostly away from them, or the space
of addressable identities can never grow. The addressing went from 4.6% to 17.7% of grown cells —
and **mean body size did not move**, which is the more interesting half. Group J has the run.)*

#### What Group I decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **A cell's behaviour comes from the gene that built it** | `development.rs`, `develop`; `behaviour.rs`, `contract` and `look` | The four behaviour fields sit in the same fixed record as `child_kind` and `new_kind`, so one record describes one thing. Phase 4's state lookup connected **2.2%** of grown cells. Now recorded in SPEC section 7. |
| **`Differentiate` re-stamps and `Terminate` does not** | `development.rs`, `develop` | A cell takes its behaviour from what it was last *made into*. Stopping a cell says nothing about what it is, so a `Terminate` gene does not take a cell's behaviour over — asserted in I2. |
| **The seed cell is given nothing at all** | `development.rs`, `develop` | Not gene zero, which is a perfectly ordinary answer and would be indistinguishable from one; not a default rhythm, which is a tune nobody selected. The same answer `organism.rs` gives a founder's missing parent, and I3 proves nothing is lost by it. |
| **It is a position in the genome, not a copy of the gene** | `cell.rs`, `Cell::gene` | A byte, because `config.rs` caps `max_genes` at 128. A copy would be sixteen fields per cell and would have to be kept in step with a genome that never changes during a life anyway. |
| **The determinism comparison reads it** | `world.rs`, `every_number_in` | A field on `Cell` that the comparison omits is a field two runs could disagree about silently. A cell with no gene is written as `u32::MAX`, because nought is gene zero. |
| **The test `Scene` says which gene built a cell, and no longer says which state it is in** | `behaviour.rs`, `Scene::add` | After the change nothing in that module reads a `state`, so a scene that still set one would be stating something the code cannot act on. |
| **`a_swimmer`'s rhythm moved onto the gene that buds the myocyte** | `world.rs` | It used to be a second, silent `Terminate` gene answering to the daughter's state. The silent gene is kept, unchanged, as a second claim: a gene naming a myocyte's state does **not** take that myocyte's behaviour over. |
| **`the_state_table_covers_every_state_a_gene_can_name` is deleted** | `behaviour.rs` | It guarded the size of `first_match`'s table against `State::COUNT`. There is no table. |

#### ⚠️ What moved, and what did not

- **Nothing was re-recorded.** Every golden vector and every pinned figure in the suite is
  unchanged to the last bit, including `a_run_produces_what_it_produced_before_group_a` — and
  that is a *finding* rather than a relief, for a **different** reason than Group H's. Group H
  did not move it because the code path was unreachable; Group I does not move it because a
  myocyte needs a second mutation on the same gene and 4,000 ticks of a 583-organism world is
  not long enough for one. `run.rs` records both readings side by side.
- **`a_headless_run_reaches_a_living_equilibrium`** (D4, 30,000 ticks of the shipped world)
  passes unchanged, which is the same fact at a longer range.
- **Six behaviour tests went red and were made green by the change rather than by editing them**
  — the four myocyte tests and the two sensocyte tests — because their scenes now say which gene
  built each cell and the old code ignored it. That red is what the change was written against.
- **Three tests added, one deleted: 260 → 262.**

### Group J — the other half of the addressing problem — **done, and the result is mixed**

⚠️ **Out of the phase's plan for the fifth time, and it is Q31.** Group I found that development
stops at 97.8% of the cells it visits, recorded it as a decision nobody had taken, and named the
candidate: **bias where a discrete re-draw of a state lands.** This is that, taken.

- [x] **J1. `a_re_drawn_trigger_state_lands_on_a_state_some_cell_is_in`** ⭐⭐ — three quarters of
  them, and the mask itself checked before the proportions that rest on it.
- [x] **J2. `a_re_drawn_child_state_can_still_name_a_state_nothing_answers_to`** ⭐⭐ — the
  opposite bias, and why getting it backwards is the worse failure.
- [x] **J3. `every_state_a_body_reaches_is_one_its_genome_writes_down`** — the superset claim,
  which is what makes reading the alphabet off the gene list legitimate rather than merely cheap.
- [x] **J4. `a_lineage_now_finds_a_body_that_uniform_re_draws_did_not`** — the consequence, with
  the uniform counterfactual measured on the same fixture rather than argued.
- [x] **J5. The 300,000-tick run**, against Group I's, on one instrument run twice.

#### ⭐⭐ The decision: two fields, two opposite biases

SPEC section 7 says *"discrete fields re-draw uniformly"*. **That is a sentence about a field** —
it says a state is re-drawn rather than nudged — and the distribution it re-draws from is exactly
what it leaves open. Development's rule is untouched, and had to be: section 7 rests the whole
justification for a variable-length rule list on conditions keying on `state`.

| Field | What it is | Biased towards | Shipped |
| --- | --- | --- | --- |
| `trigger_state` | which cells a gene **acts on** | states some cell of the body is in | **0.75** |
| `child_state`, `new_state` | the identity a gene **hands out** | states some gene answers to | **0.25** |

The second row is the one that matters most to get right. A `child_state` biased the way the
trigger is would leave a genome able to hand out only names it already answers to — **no state
nothing yet names could ever be minted, no new body part could ever be invented**, and a lineage
would collapse onto the closed set of three or four states its founder was given. Small bodies are
slow; a closed alphabet is the design not working.

Three quarters and a quarter, and neither is one or nought: at or below a half the miss stays the
ordinary case and the change is unmeasurable, and at one a gene can never be pointed at a state
nothing occupies, which is one of the two ways a gene goes silent — the neutral material section 7
says duplication feeds on. It is also the dial against the blob failure the change was watched for.

**The alphabet is read off the genome, not off a developed body.** Two 64-bit masks, one pass over
the gene list, no allocation. A development pass per reproduction would be exact and would cost the
whole of `develop` on top of the one reproduction already does; it would also be *less* stable,
because which states a body reaches depends on the step windows and on first-match-wins as well as
on the states. What is used is a **superset** of what a pass would report, provably: a cell's state
is written in exactly two places — a gene's `child_state` when it is budded and a gene's `new_state`
when it is re-made — and the one cell neither touches is the seed, which is in state 0, which the
mask therefore always carries. J3 is that as a test.

#### ⭐⭐ J5 — the named fraction moved by a factor of four, and nothing else moved

300,000 ticks after the dawn, shipped world, seed 42, eight founders, release build. **Both columns
are the same instrument run twice**, which matters: Group I's 2.2% was measured on the program
*before* Group I landed, and the same measurement on the program as it shipped afterwards is 4.6%.
The like-for-like comparison is the one below.

| | **Group I** | **Group J** |
| --- | --- | --- |
| **Grown cells in a state their genome names** | **4.56%** | **17.68%** |
| Every living cell, including seeds | 37.97% | 45.34% |
| Alive | 797 | **848** |
| Mean cells | 6.62 | **6.09** |
| **Largest body** | **32** | **17** |
| **Bodies at `max_cells_per_organism`** | **0.00%** | **0.00%** |
| Mean genes | 6.17 | **6.10** |
| Mean depth (of 1,152) | 488 | **483** |
| Field | 63,128 | **68,663** |
| Biomass | 33,468 | **32,687** |
| Living cells | 5,276 | **5,164** |
| **Myocytes** | **4** | **0** |
| **Devorocytes** | **0** | **0** |
| Mean displacement per lifetime | 4.023 | **3.966** |
| Lifetimes closed | 262,771 | 252,522 |

and by kind, over grown cells — the table Group I's I1 gave, re-taken:

| | photocyte | devorocyte | myocyte | sclerocyte | sensocyte | gonocyte |
| --- | --- | --- | --- | --- | --- | --- |
| Group I | 2.2% | 5.1% | 10.0% | 3.3% | 7.9% | 6.5% |
| **Group J** | **18.9%** | **15.3%** | **15.5%** | **13.5%** | **17.8%** | **16.5%** |

⚠️ **The first row is not I1's row and must not be read as one.** I1 measured 2.5 / 0.9 / 2.1 / 3.4
/ 2.7 / 2.0 on the program *before* Group I; this is the same measurement on the program Group I
left behind, which is a different world after tick 38,000. The two rows above are the ones that can
be laid against each other.

The run over time, with the myocyte count at **every** checkpoint rather than only the last, because
single end-readings have misled twice in this phase:

| Tick | Alive | Mean cells | Max | At cap | Mean genes | Depth | Myocytes | Devorocytes | Named |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 25,000 | 1,600 | 1.97 | 8 | 0% | 1.42 | 287 | 1 | 0 | 1.4% |
| 50,000 | 1,909 | 1.99 | 15 | 0% | 1.90 | 368 | 1 | 3 | 4.3% |
| 75,000 | 2,078 | 2.00 | 5 | 0% | 2.20 | 408 | 3 | 2 | 6.6% |
| 100,000 | 2,060 | 2.07 | 15 | 0% | 2.43 | 414 | 0 | 0 | 8.7% |
| 125,000 | 2,005 | 2.20 | 13 | 0% | 2.65 | 428 | 2 | 1 | 10.8% |
| 150,000 | 1,825 | 2.56 | 11 | 0% | 3.13 | 445 | 1 | 2 | 11.7% |
| 175,000 | 1,534 | 3.04 | 32 | 0% | 3.53 | 436 | 0 | 3 | 12.6% |
| 200,000 | 1,335 | 3.73 | 17 | 0% | 4.35 | 441 | 0 | 1 | 13.3% |
| 225,000 | 1,069 | 4.74 | 18 | 0% | 5.04 | 462 | 2 | 0 | 13.8% |
| 250,000 | 947 | 5.21 | 17 | 0% | 5.61 | 481 | 2 | 3 | 14.9% |
| 275,000 | 862 | 6.14 | 17 | 0% | 6.19 | 497 | 0 | 4 | 16.3% |
| **300,000** | **848** | **6.09** | **17** | **0%** | **6.10** | **483** | **0** | **0** | **17.7%** |

*(The named column is cumulative over every sample taken so far, which is why it is still climbing
at the end. Measured per interval rather than cumulatively, the last 25,000 ticks alone are at
**27.1%**, and the trajectory runs 1.4, 5.8, 9.6, 13.1, 16.9, 14.7, 16.3, 16.1, 16.0, 21.0, 25.1,
27.1. Group I's run measured cumulatively the same way reaches 4.56%.)*

**The world survived and nothing degenerated.** No extinction; 848 alive against a `max_organisms`
of 4,000, and a peak nowhere near it; mean depth 483 in water 1,152 deep rather than a mat at the
surface; the ledger checked itself every thousand ticks of all 313,000 and never disagreed.

#### ⚠️⚠️ And the blob did not happen — the *opposite* did

The failure this change was watched for is a world in which every cell is developmentally live,
bodies slam into `limits.max_cells_per_organism` and every organism is a 64-cell blob. **It is not
what happened, and the numbers point the other way**: not one body at the cap at any checkpoint of
either run, and the largest body in the world fell from **32 to 17**. Group I's run touched 64 cells
at two checkpoints; this one never exceeded 32.

That is worth knowing before anyone reaches for the dial, because it says the dial has room in the
direction nobody expected to need it.

#### ⭐⭐ So the dial was turned, to find out where the blob actually is

A third 300,000-tick run at the same seed with the two proportions moved to **1.00 and 0.50** —
`trigger_state` re-drawn onto an occupied state *always*, and `child_state` half the time. Nothing
else changed. It is the only way to know whether the shipped pair is timid or is the right side of
something.

| | **Group I** | **shipped, 0.75 / 0.25** | **hard, 1.00 / 0.50** |
| --- | --- | --- | --- |
| Grown cells named, over the run | 4.56% | **17.68%** | **25.41%** |
| Grown cells named, last 25,000 ticks | — | 27.1% | **36.7%** |
| Alive | 797 | 848 | 870 |
| Mean cells | 6.62 | 6.09 | 5.81 |
| **Largest body** | 32 | **17** | **64** |
| **Bodies at the cap** | 0.00% | **0.00%** | **0.11%** |
| Mean genes | 6.17 | 6.10 | 5.24 |
| Biomass | 33,468 | 32,687 | 31,453 |
| Sclerocyte cell-observations | 4,973 | 6,342 | **7,524** |
| Sensocyte cell-observations | 2,125 | 2,988 | **4,437** |
| Myocytes at the end | 4 | 0 | 2 |
| Mean displacement per lifetime | 4.023 | 3.966 | **3.551** |

**Three things fall out of it, and the third is the one that settles the shipped value.**

The proportion is a **real dial**: the addressing goes 4.6 → 17.7 → 25.4 as it is turned, in the
direction and roughly the amount the arithmetic predicts.

It buys **cell-kind diversity** rather than size. Sensocyte observations double against Group I and
sclerocytes rise by half, because a differentiating gene that names a state something is in is a
gene that gets to differentiate something. Myocytes still do not persist — the run holds 9 at tick
275,000 and 2 at the end, which is the largest reading in this phase and is still one reading.

⚠️ **And the blob starts here.** At 1.00 the largest body in the world is 64 — the cap — from tick
275,000 onward, and 0.11% of bodies are sitting on it, where the shipped pair never exceeded 17
cells and never touched it. One body in a thousand is not a degenerate world, but it is the first
sign of the failure mode this change was watched for, and it appears exactly where removing the
uniform tail from `trigger_state` predicted it would: with no tail, no gene can ever be switched off
by being pointed at nothing. **0.75 and 0.25 are the last setting at which that does not happen at
all**, which is a better reason for them than the arithmetic they were chosen by.

⚠️ **Mean body size falls as the dial is turned up** — 6.62, 6.09, 5.81 — which is the clearest
statement of Q32 there is. More addressing does not make bodies larger. It makes a few bodies very
large and the rest slightly smaller, because the biomass is fixed.

#### ⚠️⚠️ Mean body size did not move, and that is the honest result

**6.09 cells against 6.62.** Down, not up, and by a margin no larger than the difference between any
two runs in this phase. If the 2.2% figure were the reason bodies were small, this is the number
that should have moved first and most, and it did not.

What *did* move is the middle of the run. Laid side by side:

| Tick | 100,000 | 125,000 | 150,000 | 175,000 | 200,000 | 225,000 | 250,000 | 300,000 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Group I | 2.05 | 2.13 | 2.29 | 2.69 | 3.57 | 4.23 | 5.26 | **6.62** |
| Group J | 2.07 | 2.20 | 2.56 | 3.04 | 3.73 | 4.74 | 5.21 | **6.09** |

Bodies are 12% to 13% larger between 150,000 and 225,000 ticks and then the two curves meet again.
**The addressing changed how fast bodies grow and not how large they end up**, and the reason is
visible in the accounts rather than in the genome: living cells are 5,164 against 5,276 and biomass
is 32,687 against 33,468, both inside 2.4%. **The same energy is arranged in the same number of
cells.** Body size in this world is a quotient — cells over bodies — and the light decides the
numerator.

⚠️ **So the two-cell plateau is not caused by the addressing miss**, or not only by it. Mean cells
at 100,000 ticks is 2.07 against 2.05, with four times as much of the genome addressing its own
body. A body of two cells is what pays while the population is still filling the world; what ends
the plateau is the population falling, and the population falls because the light runs out. That is
a **finding about where to look next** and it contradicts the sentence in SPEC section 7 that this
group was written against.

#### ⚠️ No myocyte signal, no devorocyte signal, and nothing travels

**Nought myocytes at the end against four**, and the per-checkpoint counts are 1, 1, 3, 0, 2, 1, 0,
0, 2, 2, 0, 0 against Group I's 0, 0, 1, 1, 1, 1, 1, 0, 1, 1, 0, 4. Both are noise in a population
of five thousand cells and neither is a signal. Devorocytes the same: 0, 3, 2, 0, 1, 2, 3, 1, 0, 3,
4, 0 against 3, 1, 1, 0, 3, 0, 1, 3, 5, 0, 4, 0.

**Mean displacement per lifetime is 3.966 world units against 4.023**, over a quarter of a million
lifetimes. It went *down*, by a fortieth. ⚠️ **Nothing in this world swims**, which is the same
answer Groups F, G, H and I each gave, and this change was not an answer to it either. The figure is
dominated by bodies with no muscle at all — a cell drifting on its own springs and its own
buoyancy — and the furthest any single body's seed cell travelled in its whole life was 38 units.

*(⚠️ The instrument here is the displacement of a body's **seed cell** between the tick it was born
and the tick it died, measured on both runs identically. SPEC section 15's 2.028 for the Group I run
was taken with a different one and the two are not comparable; 4.023 is what this instrument reports
for that same run. Both say the same thing about swimming.)*

#### What Group J decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐⭐ `trigger_state` re-draws onto an occupied state three times in four** | `mutation.rs`, `TRIGGER_ONTO_AN_OCCUPIED_STATE` | A trigger that names a state no cell is in is a gene that fires nowhere, and at a uniform draw that was the outcome 97.8% of the time. Large, because at or below a half the miss stays the ordinary case; not one, because a gene that can never be switched off is neutral material an operator has deleted — **and because at one, 0.11% of bodies sit on `max_cells_per_organism` and at three quarters nothing in the run exceeds 17 cells.** |
| **⭐⭐ `child_state` and `new_state` re-draw onto an answered state only one time in four** | `mutation.rs`, `CHILD_ONTO_AN_ANSWERED_STATE` | The opposite way round, deliberately. This field is the only way the set of addressable identities ever grows; bias it towards what the genome already answers to and no new body part can ever be invented. |
| **⚠️ Neither is a configuration key** | `mutation.rs` | `behaviour.rs`'s `LIGHT_REFERENCE` precedent. A key in `[mutation]` is a thing a person turns while watching a world and goes into the document a run is replayed from; these are a property of the operator's distribution, and a run whose archived settings carried them would be one where what a `state` *addresses* had been redefined by a slider. `point_rate` already turns the whole operator down. |
| **⭐ The alphabet is read off the gene list, not off a developed body** | `mutation.rs`, `Alphabet` | Two 64-bit masks, one pass, no allocation, against a whole development pass per reproduction. It is a provable superset of what a pass would report, and the error points the safe way: it can name a state that is written down and never reached, and can never miss one a cell is in. |
| **⚠️ The occupied mask always carries state 0** | `mutation.rs`, `Alphabet::of` | The load-bearing half. Development puts the seed cell in state 0 without any gene naming it, so leave it out and a genome handing out only state 5 draws every trigger onto 5, nothing answers to the seed, and every body in that lineage is one cell. |
| **The alphabet is read once, before any gene is mutated** | `mutation.rs`, `mutate` | The same rule `physics.rs` follows about reading every body axis before any cell moves. Recomputed as the loop went, the mutations of the genes at the back of a genome would depend on what had happened to the genes at the front — order dependence with nothing to justify it. |
| **The three state fields are still *re-drawn* rather than nudged** | `mutation.rs` | Unchanged, and it is what SPEC's sentence is actually about: state 5 and state 6 have nothing to do with one another. `genome.rs`'s genetic distance is untouched for the same reason — which distribution a name is drawn from says nothing about how far apart two names are, so no species boundary moves. |

#### ⚠️ What moved

- **`a_run_produces_what_it_produced_before_group_a`** — re-recorded, for the third time in this
  project. Both previous sets are kept beside the new one. It had to move: the operator draws a
  different number of times as well as landing somewhere different, so every stream parts company
  with its old self at the first point mutation of the first birth. **A vector that survived would
  have meant the operator was not being reached**, which is exactly the reading Groups H and I had
  to give. The world it describes is very nearly the same one — one more organism born, one more
  alive, every account inside 4% — because at four thousand ticks the genomes hold one or two genes
  and almost nothing has yet been addressed differently.
- **Nothing else was re-recorded.** Every other golden vector and pinned figure in the suite is
  unchanged, `a_headless_run_reaches_a_living_equilibrium` included.
- **Four tests added: 262 → 266.**

### Group K — the price of a muscle — **done, and the result is half negative**

⚠️ **Out of the phase's plan for the sixth time.** Groups F, G, H, I and J each removed one
reason a lineage could not swim and each ended with the same sentence: *nothing swims*. What was
left was an economic reading of the gap, and SPEC section 6 invites exactly this test —
*"if in play-testing one kind dominates every lineage, the costs are wrong — tune before adding
kinds"*, and photocytes and gonocytes are **99.7%** of every cell in every run recorded here.

- [x] **K1. The sweep** ⭐⭐ — six 300,000-tick runs of the shipped world, one per price, with
  the myocyte-count-per-body, the ≥2-myocyte fraction and Group J's displacement instrument on
  each. The 0.014 column reproduces Group J's run **bit for bit**, which is what makes the other
  five comparable.
- [x] **K2. `CellKind::Myocyte`'s upkeep 0.014 → 0.005** — SPEC section 6's table, with the
  measurement and the reasoning beside it.
- [x] **K3. `a_myocyte_costs_more_to_own_than_the_cell_that_earns`** ⭐⭐ — the two claims the
  sweep establishes, as a test rather than as a paragraph. Both would have been red at 0.014:
  the first was true and the second was not.

#### The hypothesis, which was a fitness valley with a lethal floor

One myocyte oscillating one spring is a **reciprocal** stroke, and SPEC section 8's water gives
a reciprocal stroke exactly nought net displacement — the scallop theorem, measured in this
project rather than assumed. So locomotion needs **two muscles at different phases on a bent
body** before it produces anything. Meanwhile a myocyte cost 0.014 against a photocyte's 0.004
and earned nothing, so the first one was selected away before the second could arrive beside it.
And 0.014 was **never measured**: it was written before anything ran, when `movement_cost` was
0.15 and using a muscle was arithmetically impossible, so upkeep was the only thing pricing one.
Phase 7 moved `movement_cost` by a thousandfold and never came back to it.

#### ⭐⭐ The sweep

300,000 ticks after the dawn, shipped configuration, seed 42, eight founders, release build.

| Myocyte upkeep | 0.014 | 0.010 | 0.007 | **0.005** | 0.004 | 0.002 |
| --- | --- | --- | --- | --- | --- | --- |
| Against a photocyte's 0.004 | ×3.5 | ×2.5 | ×1.75 | **×1.25** | ×1.0 | ×0.5 |
| **Myocytes per body**, over the run | 0.00097 | 0.00094 | 0.00135 | **0.00205** | 0.00186 | **0.00797** |
| **Bodies carrying ≥2 myocytes** | 0.0073% | 0.0064% | 0.0085% | **0.0345%** | 0.0147% | **0.0861%** |
| *Births* carrying ≥2 | 95 | 54 | 62 | **112** | 67 | 187 |
| Lifetimes closed | 252,522 | 251,224 | 261,516 | **252,189** | 270,437 | 261,628 |
| Mean life of a ≥2 body, ticks | 325 | 557 | 631 | **1,380** | 1,053 | 2,050 |
| **Mean displacement per lifetime** | **3.966** | 4.030 | 3.750 | **3.742** | 3.766 | **3.627** |
| Myocyte cell-observations | 861 | 823 | 1,234 | **1,822** | 1,756 | **7,306** |
| Myocytes, of all living cells | 0.033% | 0.031% | 0.047% | **0.069%** | 0.068% | **0.278%** |
| Photocytes / gonocytes | 66.0 / 33.7% | 66.5 / 33.3% | 65.0 / 34.8% | **66.4 / 33.3%** | 63.7 / 36.1% | 64.8 / 34.6% |
| Devorocyte cell-observations | 1,535 | 1,185 | 1,219 | **1,445** | 1,031 | 1,035 |
| Sclerocyte / sensocyte observations | 2,748 / 1,375 | 2,857 / 1,300 | 2,469 / 1,447 | **3,627 / 1,421** | 2,804 / 991 | 5,900 / 1,295 |
| Population at 300,000 | 848 | 659 | 844 | **826** | 744 | 832 |
| Mean cells | 6.09 | 7.99 | 6.00 | **6.21** | 6.94 | 6.28 |
| Biomass | 32,687 | 32,855 | 32,201 | **32,276** | 32,962 | 32,125 |
| Field | 68,663 | 63,387 | 69,470 | **65,618** | 66,399 | 69,036 |
| Largest myocyte count in one body | 22 | 11 | 12 | **14** | 21 | **46** |

Myocytes alive at each of the twelve checkpoints, because single end-readings have misled three
times in this phase:

| Upkeep | 25k | 50k | 75k | 100k | 125k | 150k | 175k | 200k | 225k | 250k | 275k | 300k |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0.014 | 1 | 1 | 3 | 0 | 2 | 1 | 0 | 0 | 2 | 2 | 0 | 0 |
| 0.010 | 0 | 0 | 3 | 3 | 1 | 1 | 1 | 2 | 1 | 0 | 7 | 1 |
| 0.007 | 1 | 2 | 3 | 2 | 0 | 1 | 2 | 0 | 2 | 5 | 1 | 1 |
| **0.005** | **1** | **1** | **3** | **1** | **1** | **1** | **8** | **9** | **7** | **5** | **1** | **1** |
| 0.004 | 1 | 3 | 3 | 2 | 3 | 4 | 2 | 1 | 0 | 4 | 5 | 10 |
| **0.002** | **1** | **1** | **5** | **2** | **2** | **9** | **15** | **12** | **7** | **22** | **51** | **42** |

*(The 0.014 row is Group J's published series, digit for digit — 1, 1, 3, 0, 2, 1, 0, 0, 2, 2, 0,
0 — which is the instrument agreeing with itself across two sessions.)*

#### ⭐⭐ What the price buys is persistence, not supply

**Births carrying two or more myocytes do not respond to price at all** — 95, 54, 62, 112, 67,
187 out of a quarter of a million births apiece, with no trend across a sevenfold range. How
often a mutation makes a second myocyte is a fact about `mutation.rs`, and the ledger cannot
reach it.

What price changes is **how long such a body lasts**, and it does so by construction rather than
by selection: `metabolism.rs` allows an organism `LIFETIME_UPKEEP × cells ÷ what it costs a
tick`, so cheap tissue lives proportionally longer. 325 ticks at 0.014, 1,380 at 0.005 — a factor
of four. The standing population of muscle is supply times persistence, which is why it rises
4.7-fold while nothing about the supply moved.

**The valley floor was real and it has come up. The near side of it is empty.**

#### ⚠️ Nothing swims at any price, and the sweep contains its own control

Mean displacement per lifetime is 3.74 at the shipped price against 3.97 — *down*, and every
column is inside the spread of every run since Group I.

Splitting the lifetimes by how many myocytes development gave the body looks at first like a
signal, because a body's composition is fixed at birth and so the split is exact:

| Myocytes the body was built with | 0.014 | 0.010 | 0.007 | 0.005 | 0.004 | 0.002 |
| --- | --- | --- | --- | --- | --- | --- |
| **none** | 3.966 | 4.030 | 3.750 | 3.741 | 3.764 | 3.612 |
| **one** — the control | 2.958 | 3.638 | 3.913 | 3.659 | 4.729 | **7.188** |
| **two or more** | 5.856 | 7.101 | 6.020 | 6.278 | 6.394 | 6.067 |

⚠️ **The middle row is what settles it.** A single muscle is a reciprocal stroke and *cannot*
produce net displacement; at 0.002, where that bucket is finally large enough to read — 997
lifetimes — it travels **further than the two-muscle bucket does**. So the excess in the bottom
row is not locomotion. It is that a body with any myocyte in it is a bigger and longer-lived body
than the two-celled median, and bigger bodies drift further. **Nothing in this world swims**,
which is the same answer Groups F, G, H, I and J each gave.

#### ⚠️⚠️ The neutral-bloat boundary is between 0.004 and 0.002

CLAUDE.md names the failure this change had to be watched for, and it is findable. **At 0.002 —
a sclerocyte's price, below the cell that earns the world's whole income — it begins.** The
myocyte count *rises through the run* rather than fluctuating about nothing (7, 22, 51, 42 over
the last four checkpoints against the shipped world's 2, 2, 0, 0), reaches **2.4% of bodies over
the last 25,000 ticks** against the shipped world's 0.10%, is 0.28% of every living cell over the
whole run, and one body carries **46 myocytes against a cap of 64** — while mean displacement is
the lowest reading in the sweep. A world accumulating a cell that does nothing is the definition
of the failure.

At 0.004, which is exactly a photocyte, it is barely visible and pointing the same way: 0, 4, 5,
10 across the last four checkpoints, and a body carrying 21. At 0.005 there is no trend at all
and the largest body holds fourteen, against **twenty-two at the 0.014 this replaces**.

**The line is the photocyte.** A muscle dearer than the cell paying for it is never free to own;
a muscle cheaper than it is.

#### ⚠️ And the confound the sweep was asked to check: photocytes are not displaced

Photocytes are 63.7% to 66.5% of all cell-observations across the whole sweep and the variation
has no order in it. Myocytes at their most numerous are **0.278%**, and at the shipped price
0.069%. There is nothing here for a myocyte to displace. What moves the photocyte share by a
point or two is the photocyte-to-gonocyte ratio, which is a statement about how many bodies are
still two cells.

Devorocytes show no response either — 1,535 / 1,185 / 1,219 / 1,445 / 1,031 / 1,035
cell-observations, which is noise at every price.

#### What Group K decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **A myocyte's upkeep is 0.005** | `cell.rs`, `CellKind::upkeep` | The largest measured effect among the prices that do not accumulate: myocytes per body double, the ≥2 fraction rises 4.7-fold, a two-muscle body lives four times as long, and the ecology is inside 2% on every account. |
| **⚠️ It stops above a photocyte** | `cell.rs`, `a_myocyte_costs_more_to_own_than_the_cell_that_earns` | Measured, not assumed. At 0.002 myocytes accumulate through the run and displacement *falls*; the cell that earns the world's income is the only meaningful floor for a cell that earns nothing. |
| **The claim shipped as a relationship, not only as a number** | `cell.rs` | Two assertions — dearer than a photocyte, and less than twice one. Both would have been red at 0.014, the first true and the second false, so the test states what changed rather than transcribing it. |
| **`movement_cost` and upkeep are one decision** | SPEC section 3 | The standing cost was set when using a muscle was impossible. After the thousandfold cut to `movement_cost`, swimming flat out cost a ninth of standing still; it is now about a third. `physics.rs`'s affordability assertion moved from a quarter to a half **because that is the change**, not as a concession to it. |
| **Six prices, one seed each, rather than one price and six seeds** | — | ⚠️ The weakness of the result and it is worth naming. Everything below a factor of two here is within run-to-run variation, so the readings that carry weight are the monotone ones — persistence, and the 0.002 accumulation — and not the ordering of 0.004 against 0.005. |

#### ⚠️ What moved

- **`a_run_produces_what_it_produced_before_group_a`** — re-recorded, for the fourth time. Every
  previous set is kept beside the new one. ⭐ **Exactly two of its eight numbers moved, by exactly
  the same amount in opposite directions**: 1.458 units out of `dissipated` and back into
  `biomass`. The tick count, the 584 born, the 542 alive, the field's total and the light that
  fell are unchanged to the last bit — which is the whole content of a pure upkeep change, since
  upkeep is the one movement in SPEC section 5 that goes `biomass → dissipated` and touches
  nothing else. At 0.009 saved per myocyte a tick, 1.458 units is about **162 myocyte-ticks** in
  four thousand ticks of a 542-body world.
- **`an_organism_dies_of_old_age`** — 571 ticks → **1,600**, and the ratio it asserts against a
  sclerocyte from seven to one to two and a half to one. Both previous figures kept. That the
  allowance nearly trebled is most of what the change *did*.
- **`every_cell_pays_upkeep_every_tick`** — the muscle figure re-recorded and the muscle-to-armour
  ratio from `> 6` to `> 2`. A myocyte is no longer the most expensive tissue in the world; a
  devorocyte at 0.009 is.
- **`a_freed_slot_is_reusable_and_reaping_is_deterministic`** — 600 ticks → 1,700, because the
  bodies of muscle it kills off now take 1,600 to die of old age. The claim is unchanged.
- **`a_travelling_wave_carries_a_body_through_the_water`** — the affordability bound from a
  quarter to a half, with the old figure kept and the reason recorded: it is the change.
- **Nothing else was re-recorded.** `a_headless_run_reaches_a_living_equilibrium`,
  `energy_is_conserved_over_100k_ticks` and every other pinned figure are unchanged.
- **One test added: 266 → 267.**

#### ⚠️⚠️ Where this leaves the muscle question — and it is not a price

The sweep's null half is the useful half. **The standing cost has been cut to a quarter of the
premium it carried and the number of bodies born with the configuration locomotion needs did not
move at all.** Two candidates follow, and neither is built:

1. ⭐ **Give a single muscle something useful to do that is not locomotion.** The scallop theorem
   forbids net displacement from one reciprocal stroke; it says nothing about *shape*. A body
   that contracts is a body whose cells occlude one another differently, and SPEC section 6 makes
   a photocyte's harvest depend on exactly that. If the first muscle paid in self-shading, there
   would be no valley to cross — every intermediate would be worth something, which is the
   argument SPEC section 6 already makes for buoyancy-by-`CellKind`.
2. **Make the second muscle reachable in one mutation rather than two.** Gene duplication already
   copies a gene adjacent to itself; a duplicated myocyte-making gene whose `osc_phase` then
   diverges is precisely the two-muscles-at-different-phases configuration. What is missing is
   any reason for the first copy to survive long enough to be duplicated — which is candidate 1
   again, from the other end.

**Q33** below carries this forward.

### Group L — the instrument, one bug, and the weather — **done**

⚠️ **Out of the phase's plan for the seventh time, and this time the round did not start with a
design.** Groups F, G, H, I, J and K each removed one reason a lineage could not swim and each
ended with the same sentence: *nothing swims*. The reason six rounds could each be necessary and
none sufficient is that **nobody could measure what a configuration was worth without waiting for
evolution to produce one** — so every change to the payoff was argued rather than priced, and each
argument cost a 300,000-tick run to refute.

- [x] **L1. The competition assay** ⭐⭐ — `crates/coacervate-app/src/assay.rs`. Public API only:
  `World::seed`, `World::organisms`, `Organism::serial` and `Organism::parent`. No change to the
  simulation whatever. Three tests: attribution, the noise floor, and the economy's calibration.
- [x] **L2. A myocyte pays for the shape its sensor moved it to** ⭐⭐ — a genuine pre-existing
  defect in `behaviour.rs`, independent of any payoff design.
- [x] **L3. `light.season_period` and `light.season_amplitude`** — a triangle wave on
  `light.influx`, shipped **inert**. `config/seasonal.toml` is the same world at 0.25.

---

#### ⭐⭐⭐ L1 — the selection coefficients, which are the most important thing in this document

**Twenty-six assay runs. This table is what the instrument was built to produce, and every future
payoff proposal in this project should be priced against it before a line of it is written.**

⚠️ **It was taken on a prototype of the instrument and only two of its rows have been re-measured
on the shipped one.** The noise floor and the myocyte row reproduce; the photocyte row does not,
and the section headed *One row does not reproduce* below is the correction. Read the table as
the best available price list and the two re-measured rows as the calibration — and **re-run the
arm before quoting a row of it as a fact**, which now costs four minutes.

Two founder sets that differ by **exactly one mutation** are seeded alternately into the shipped
world after the dawn — 32 founders, 16 per arm, at alternating positions so neither arm gets
systematically better water. Every organism born afterwards is attributed to the arm its parent
belonged to. After 42,000 ticks (23.9 generations at the measured 1,753.9-tick generation) the
ratio of living descendants **is** the selection coefficient.

| Arm B, against an identical arm A | upkeep added | descendant ratio | **coefficient** |
| --- | --- | --- | --- |
| a third **photocyte** | +0.004/tick | 1.076 (s42), 1.022 (s7) | **+0.04 %/gen** — exactly neutral |
| the longest **`rest_length`** the genome can ask for (8.0 → 13.6) | none | 1.297 / 1.063 / 1.289 | **+0.71 %/gen** flat, +0.88 seasoned |
| a third **sclerocyte** — the cheapest inert cell there is | +0.002/tick | 0.855 (s42), 0.755 (s7) | **−1.07 %/gen** |
| a third **myocyte**, holding still | +0.005/tick | 0.593 (s42) | **−2.46 %/gen** |
| a third **myocyte**, beating at 2.5 rad/s | +0.005/tick | 0.516 (s42), 0.355 (s7) | **−2.7 to −4.4 %/gen** |
| a third **devorocyte** | +0.009/tick | 0.126 (s42), 0.236 (s7) | **−6.1 to −9.0 %/gen** |
| a **myocyte with an adhered sensocyte** | +0.011/tick | **0.097** (s42) | **−8.6 %/gen** |

Every coefficient is quoted as an **excess over its own same-seed control**, which is what the
noise-floor arm is for.

| Magnitude | Value |
| --- | --- |
| Window | 42,000 ticks = **23.9 generations** |
| Noise floor | **±0.16 %/generation** (1 s.d.; log-ratios +0.064 / +0.009 / −0.008 on three seeds) |
| Resolution | about **0.3 %/generation** with three seeds |
| Attribution loss | **0 to 4 births in ~40,000** |
| Cost | about **four minutes per run** |

**And the reading that answers Q32 and Q33 at once: which arms *keep* their extra cell.** After
24 generations the surviving bodies hold, on average, 3.28 cells in the extra-photocyte arm, 2.22
with a sclerocyte, **2.03 with a myocyte**, 2.04 with a devorocyte and 2.04 with the muscle-plus-
sensor pair. The world keeps extra photocytes and sheds every other kind of cell inside two dozen
generations — and it keeps the photocyte at a coefficient of *exactly zero*.

> **Nothing in this world has an increasing return to being more than one thing.** A photocyte's
> income scales linearly with photocyte count, upkeep scales linearly with cells, the reproduction
> threshold is `reproduction_threshold × Σ construction` and is linear in cells, and lifespan is
> `LIFETIME_UPKEEP × cells ÷ cost` and is linear again. Occlusion is actively *sub*linear. So
> growth is a random walk and specialisation is a pure loss, at about **−0.5 %/generation for
> every 0.001/tick of upkeep, whatever the cell does**.

⚠️⚠️ **What that closes.** A muscle must earn **+2.5 %/generation** merely to break even. The
*entire* measured value of shape in this world — the largest free shape change the genome can
express, taken in full and for nothing — is **+0.85 %/generation**. A beating muscle shifts its
body's mean geometry by 0.8% against `rest_length`'s 70%, so its share of that channel is about
1%: **+0.01 %/gen against a −2.7 %/gen bill, a ratio of 1 : 270.** Even a muscle that could
*hold* a shape perfectly captures at most the whole channel — **+0.85 against −2.5** — and
`rest_length` already collects it for nothing, one point mutation away. **The self-shading muscle
payoff was refuted before a line of it was written**, on the measured coefficient of the exact
configuration it proposed to seed.

##### ⚠️ Two things every coefficient here must be quoted with

**It measures the filling regime** — two-celled bodies, a population rising to about 2,100. Group
K's world at 300,000 ticks holds 6.21 cells per body and 826 organisms. Nobody should quote an
assay coefficient as a fact about the mature world without seeding the arms into one.

**And a ratio near 1.0 in the first 40,000 ticks can mean *still filling* rather than *no
effect*.** The control's population is rising throughout that window, which is why every number
above is an excess over its own control.

##### ⚠️ One row does not reproduce, and it is the row the whole argument above rests on

Rebuilt from the public API, with arm B being the founder plus **one appended gene** that buds a
third cell off its photocyte at the next developmental step (angle π, so the three cells lie in a
line with the photocyte in the middle), the instrument was re-run on this machine:

| Arm, seed 42, 42,000 ticks | alive | ratio | log-ratio | **excess over its control** | cells/body |
| --- | --- | --- | --- | --- | --- |
| noise floor — the same genome both sides | 1031 / 1098 | 1.0650 | +0.0630 | — | 2.02 / 1.99 |
| noise floor, seed 7 | 1034 / 1061 | 1.0261 | +0.0258 | — | 1.99 / 2.00 |
| a third **photocyte** | 636 / 974 | 1.5314 | +0.4262 | **+1.52 %/gen** | 3.41 / 2.03 |
| a third **myocyte**, holding still | 1382 / 706 | 0.5109 | −0.6717 | **−3.07 %/gen** | 2.14 / 1.99 |

**Attribution was complete in every run — nought unattributable births out of 38,000 to 47,000 —
and both arms' founders received exactly 32.0 units of energy, so neither was seeded into better
water.** The instrument itself therefore reproduces: the noise floor at seed 42 comes back at
**+0.063** against the recorded **+0.064**, and the myocyte arm at **−3.07 %/gen** against the
recorded **−2.46**, which is the same sign and the same order and well outside the floor either
way.

⚠️ **The extra-photocyte arm does not.** It comes back at **1.531** — `+1.52 %/generation` against
a noise floor of ±0.16, and nearly twice the largest coefficient the table above records for
anything — where the design measured **1.076**, *"exactly neutral"*. What differs is **what a
third cell was made of**: doubling a body's photocytes while adding 44% to its bill is not
neutral, and *a third photocyte is worth precisely its own cost* does not survive this
construction of one.

**What survives, and it is the load-bearing half:** the two arms above are the *same* three-celled
body differing in one `child_kind`, and the earning cell is kept at **three to one** against the
silent one. A third cell that earns pays; a third cell that does not is shed — 2.14 cells a body
in the myocyte arm, where every organism in it was **born with three**. The muscle arithmetic is
unaffected either way, because it was never the photocyte's coefficient that priced a muscle. It
was the muscle's own, and that one reproduces.

---

#### ⭐⭐ L2 — a myocyte whose sensor changed moved its spring for free

**A genuine pre-existing defect, and it has nothing to do with any payoff design.**
`Behaviour::contract` worked out how far a spring had moved by re-evaluating SPEC section 9's
controller one tick back — **using this tick's sensor reading**. The sensor's whole contribution
therefore cancelled out of the subtraction and was never charged. With `osc_freq = 0`, which Group
I measured as **87% of all myocyte spring-ticks**, the sine cancelled too, and the charge was
**exactly nought** while the rest length had genuinely moved by up to `base × stroke` — eight world
units in one tick on a base-eight spring.

In SPEC section 8's anisotropic water a free shape change is free displacement. That section
refused this exact pattern by name when it declined to give a loose cell a drag axis: *"that is
thrust with no muscle behind it, and it would look exactly like life."*

| Magnitude | Value |
| --- | --- |
| Free movement available per tick | up to `base × stroke × \|sin(osc_phase)\|`, ≤ 8 units at base 8 |
| Typical size today | mean \|Δ\| **0.0003** per tick |
| Tail | reaches **0.824**, on 0.18–0.35% of body-ticks — the light gradient is quantised on 8-unit tiles and steps at every boundary crossing |
| How often a myocyte has a sensocyte adhered to it | order **one body per 300,000-tick run** |

That last row is why the fix is safe: it closes a real hole in the physics and changes essentially
nothing in the shipped world.

**The distance is now remembered rather than derived.** `Cell::contraction` holds what a myocyte's
controller last multiplied its springs by — the same route `energy_flow` already takes through
`World::scatter`. It is an **absence** until the controller has run once, because a body is
*developed* at the length its controller asks for and has not travelled anywhere on its first
tick.

⚠️⚠️ **Remembering it on the cell has a trap in it, and it was found by writing the test rather
than by reading the code.** A contraction is a fact about the *cell*; the charge is worked out per
*spring*. So a myocyte in the middle of a chain is visited twice in one tick — and the obvious
implementation, which reads last tick's value off the cell and writes this tick's back in the same
visit, finds on the second spring that *last tick* has already become *this tick*. The second
spring moves exactly as far and is charged **nothing**: every adhesion after the first, free,
which is the same free-shape-change defect this group exists to close, arrived at from the other
side. Measured red at exactly 1.0000× instead of 2×. The reading is therefore one pass over the
cells, the spring loop sits between, and the writing is a second pass — and the controller is now
evaluated once per **cell** rather than once per spring, which is also cheaper.

⚠️⚠️ **And the tension is taken at the rest length the spring was already at, not the one this tick
moved it to.** Taken after the jump, the tension contains the jump as well and the charge goes as
its **square**. The property test found the worst case immediately: **0.319 energy in a single
tick**, at a stiffness of 122 and a base of 8, with the spring standing exactly at rest and
carrying no force at all — **more than thirty times** a two-celled body's entire per-tick upkeep
of 0.009, produced by
the resolution of the grid rather than by anything biological. Force through distance means the
force that was opposing the movement when it began.

##### ⚠️ What moved

- **`a_run_produces_what_it_produced_before_group_a`** — ⭐ **unchanged, to the last bit**, and
  that is the result rather than a relief. No muscle in those four thousand ticks has a sensocyte
  adhered to it and none has a non-zero `osc_freq`, so neither half of L2 can reach the run. It is
  the cheapest available statement that the fix touches only the case it was written for.
- **`a_myocyte_oscillates_its_springs_and_pays_for_the_work`** — re-recorded for the third time,
  and all three earlier figures are kept: **0.00186** at `movement_cost = 0.15`, **0.0827** after
  Group H, **0.08233** now. Half a per cent, and it is the whole visible effect of L2 on a muscle
  with no sensor: over a smooth 3 rad/s oscillation the two readings of the tension differ only by
  the one-tick lag between the force and the distance it acts through.
- **`a_myocyte_that_does_nothing_pays_nothing`** and
  **`a_myocyte_takes_its_rhythm_from_the_gene_that_built_it`** — unchanged.
- **`the_arenas_are_allocated_once_at_the_size_the_config_asks_for`** — 28 bytes a cell → **36**,
  and 13,216,000 → **15,264,000** for the two arenas. Two megabytes against CLAUDE.md's
  two-gigabyte resident target. `Cell::contraction` is eight bytes because it is written as an
  absence rather than as a number standing for one.

---

#### ⭐⭐ L3 — the season

A scalar multiplier on **`light.influx` alone**, applied at the one line in `Grid::regrow` that
reads the per-row regrowth. The full argument is now SPEC section 4; what belongs here is what it
measured.

⚠️ **It ships at `season_amplitude = 0.0`** — `config/default.toml` is bit-for-bit the world every
figure in this document was measured on. `config/seasonal.toml` is the same world at 0.25.

##### The three things that were most likely to go wrong

**1. The off switch that does not switch off.** All three reviewers of the design found the same
defect, and it is the most dangerous thing the feature could have contained. The multiplier is
computed **unconditionally**: `1 + amplitude × triangle(phase)` is exactly 1.0 at an amplitude of
nought, so no branch is needed and the bit-identity still holds. An early return that skips the
recompute leaves the world permanently lit at up to 1.25× its stated influx, with
`season_amplitude = 0` showing in the config file, the panel and the replay log, while the ledger
balances perfectly and says nothing. `a_world_with_no_season_is_the_world_that_was_there_before`
is the guard, and its retune-to-zero half is the assertion that matters — **measured red at
25.235 units of light against a flat world's 23.040** when the recompute was skipped.

**2. A quadratic work charge.** See L2 above: measured red at **0.319 in a single tick**.

**3. A season that moved a ceiling.** `influx` enters no ceiling, so a season needs no retarget and
sheds no spill. `a_season_moves_no_ceiling` asserts the whole target array is bit-identical after a
seasoned period. ⚠️ It deliberately does **not** also assert that `dissipated` is unchanged:
measured over three periods on an empty world, dissipated per tick runs **15.184 flat, 14.992 at
±25%, 14.480 at ±50%**, because diffusion knocks tiles off their patchy ceilings every tick
whatever the light is doing. That assertion would be false and would end up weakened rather than
fixed.

⚠️ **`a_season_moves_no_ceiling` passes identically when the season is never applied at all**,
which is exactly the Group H failure mode. `the_light_a_row_is_offered_rises_and_falls_with_the_season`
is what catches that, and it was verified red — **1.0000003× instead of 1.25×** — with the
multiplication removed.

##### ⭐⭐ The measurement: three runs, fifteen whole periods, scored on integrals only

Shipped world, seed 42, eight founders, release build, `--ticks 330000`. The dawn ends at tick
**13,000**, so the window scored is world tick **15,000 → 330,000**: 315,000 ticks, which is
**fifteen whole periods** and begins and ends at the same phase. ⚠️ Fifteen rather than the
fourteen the design asked for, because the progress report is on a 5,000-tick grid and
`gcd(21,000, 5,000) = 1,000` — so a whole number of periods lands on a report only every **five**
of them, and 14 does not.

**Predictions were written down first**, and three of the four hold.

| Over fifteen whole periods | flat | ±25% | ±50% |
| --- | --- | --- | --- |
| `influx_total` | 6,978,241 | 6,951,729 (**−0.38%**) | 6,926,165 (**−0.75%**) |
| `dissipated` | 7,053,272 | 7,025,634 (**−0.39%**) | 6,996,081 (**−0.81%**) |
| `born` | 199,857 | 251,678 (**+25.9%**) | 265,800 (**+33.0%**) |
| mean alive over the window | 1,099.2 | 1,369.1 (**+24.6%**) | 1,454.0 (**+32.3%**) |
| mean cells per body | 5.65 | 4.11 | 3.32 |
| corr(detrended cells/body, season phase) | **−0.066** | +0.075 | **−0.259** |

- ✅ **`influx_total` falls, and by a fraction of a per cent.** Predicted 99.8% of the control;
  measured 99.62% at ±25%. `influx` enters no ceiling, so a season sheds no spill; what it loses
  is the little that a dim half-cycle fails to deliver into tiles that were not full.
- ✅ **`dissipated` falls too**, which is the sign that says the season is on the income and not on
  a ceiling. A season on `cap` would push it *up*.
- ❌ **The seasoned/flat population ratio is not between 0.9 and 1.1.** It is **1.25** and
  **1.32** on whole-window means, and the seasoned worlds carry *more* organisms rather than
  fewer. Biomass is nearly unchanged — 33,347 / 31,473 / 31,006 at the end — so what a season
  actually does is put **the same energy into more and smaller bodies**: 5.65 cells a body flat
  against 3.32 at ±50%.
- ⚠️ **And the falsifiable prediction is refuted at ±50% and unresolved at ±25%.** The
  quasi-static reading — bodies larger at the bright peak, because SPEC section 3's sustained
  influx sweep has mean cells rising 4.07 → 10.62 with the light — predicts a **positive**
  correlation. Q32 predicts a negative one, because body size is a quotient and the population is
  its denominator. At ±50% the correlation is **−0.26**, which is Q32's sign; at ±25% it is
  +0.075, the same size as the flat control's −0.066 and therefore nothing.

⚠️⚠️ **Three single runs at one seed, and the caveat is Group K's own.** Group I measured that a
perturbation anywhere in this world diverges from its own baseline by about tick 50,000 and ends
with a different world, so a 25% difference between three single runs is **not** established to be
outside run-to-run variation. What carries weight is the monotone ordering across three
amplitudes on four separate accounts, and the two sub-per-cent readings on the light, which are
integrals and not end-of-run readings.

⚠️ **The correlation statistic needs a detrend that this sampling rate barely supports.** Mean
cells per body climbs from 2.0 to 9.9 over the run, so a raw correlation against phase comes back
at **−0.44 on the flat control**, which must be zero. Subtracting a straight line fitted *within
each cycle* brings the control to −0.066, and that is the reading above. Anybody re-running this
should sample at 500 ticks rather than 5,000 and report the control's figure beside every other.

##### ⚠️ The flat control does not reproduce Group K bit for bit, and the reason is L2

The design required it to. It does not, and the honest reading is that **L2 changed the
simulation, so it could not**: any myocyte with a non-zero `osc_freq` is now charged at the
tension its spring was already carrying rather than at the post-jump one. Group I established
that the first such muscle appears at around tick 50,000, and that a perturbation there ends with
a different world.

What the flat run *does* do is land on Group K's aggregates:

| At world tick 313,000 | Group K | here | difference |
| --- | --- | --- | --- |
| `influx_total` | 6,873,766.5 | ~6,879,700 | **+0.09%** |
| alive at the 25,000-tick checkpoint | 1,619 | ~1,585 | −2.1% |

⚠️ **And it ends somewhere else entirely: 555 organisms of 9.85 cells against 826 of 6.21, at
33,796 units of biomass against 32,276.** The same energy, in fewer and larger bodies. That is a
single reading of a chaotic quantity and it should not be quoted as an effect of anything; the
light budget agreeing to a thousandth is what says the instrument is the same instrument.

##### What Group L decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **The assay lives in `coacervate-app` and is `#[cfg(test)]`** | `assay.rs` | It uses the public API only and must not perturb what it measures. It borrows `founding.rs`'s dawn and founder grid rather than growing a second copy of either — an assay whose arms stood somewhere other than a run's founders would predict a different experiment from the one it is for. |
| **The two arms must be one mutation apart, asserted** | `assay.rs`, `one_mutation_apart` | Identical, one gene appended, or one field of one gene changed — which is `mutation.rs`'s own insertion, duplication and point operators. An arm that had quietly picked up a second change would return a coefficient for the pair with nothing in the reading to say so. |
| **`Cell::contraction` is an `Option`, not a number** | `cell.rs` | A body is *developed* at the length its controller asks for, so a muscle has not travelled anywhere on its first tick. Started at 1.0 the charge would be measured from a rest length the spring was never at — which costs a newborn nothing today only because development lays an adhered daughter down at exactly its gene's `rest_length`. |
| **The contraction is read in one pass and written in another** | `behaviour.rs`, `Behaviour::contracted` | ⚠️ A contraction is a fact about a cell and a charge is worked out per spring, so a muscle in a chain is visited twice. Read and written in one visit, its second spring is charged nothing. |
| **The season is a triangle** | `grid.rs`, `Grid::season` | One scalar multiplies 36,864 tiles every tick, which is 36,864 times `behaviour.rs`'s single-spring `sin` exposure. Exact in f64, no libm, mean-preserving by exact antisymmetry, exactly 1.0 at amplitude nought. |
| **The phase is accumulated and starts at the first seeding** | `grid.rs`, `world.rs` | Group G's `drifted` decision applied to the second clock, plus the dawn's own stopping rule being light-dependent. |
| **Two new `ConfigError` variants, not one** | `config.rs` | ⚠️ The design asked for one, on the grounds that the amplitude could reuse `OutsideRange`. It cannot: that variant's sentence is `physics.drag_anisotropy`'s own — *"at 1.0 the water resists a cell equally in every direction"* — and in this file the sentence **is** the value of the refusal. |
| **The chronicle says both ends of the season and nothing else** | `chronicle.rs` | Two lines a period, no direction and no judgement. Without them the log would report fourteen mass extinctions per seasonal run and record nothing about why, and CLAUDE.md's *"a lineage that thrives and then dies when the light dims was never worse"* would be unwritable by anybody reading it. |
| **The `season_period` slider's far end is not a gate constant** | `settings.rs` | The gate deliberately has no ceiling. A slider still has to stop somewhere; 210,000 is ten times the shipped period and four times the median species lifetime. |

##### ⚠️ What moved

- **`the_live_settings_have_dials_and_the_locked_ones_do_not`** — `[light]` 6 → **8** dials, 25 →
  **27** in all.
- **`every_dial_is_a_condition_the_chronicle_reports`** — `CONDITIONS` 24 → **26**, and
  `Condition::read` widened from `f32` to `f64`, because a period is a count of ticks rather than
  a fraction of anything.
- **`every_spec_default_literal_narrows`** — 28 → **29** decimal settings.
- **`Kind::ALL`** — 10 → **11**, with `Kind::Season` tagged `"season"`.
- **Twenty tests added: 267 → 287.**

⚠️ **The check suite got slower, and the figure is worth recording because it was the thing this
group was most likely to get wrong.** The assay's two long tests are `#[ignore]`d and
`scripts/check.ps1` runs them through `--include-ignored` — five 42,000-tick runs — and
`energy_is_conserved_across_three_whole_seasons` adds a 63,000-tick living world with the ledger
checked on **every** tick. Measured end to end on the machine CLAUDE.md describes, with five other
runs of this simulation competing for the cores: **6 minutes 52 seconds**, of which the release
pass's app suite is 218 seconds and the two assay tests are nearly all of that. The 63,000-tick
season costs **5.7 s**, because that world is a sixteenth of the shipped one's area at the shipped
light.

CLAUDE.md warns that a suite somebody stops running is worth nothing. If seven minutes becomes
that, the answer is a filter on the release pass rather than a weaker test: the noise floor is
what every coefficient in this document is quoted against, and a noise floor nobody ever
re-measures is a number rather than a measurement.

### Group E — looking at one organism

- [ ] **E1. `a_click_finds_the_organism_under_it`** — ⚠️ `camera.rs` maps world→screen only;
  there is no inverse.
- [ ] **E2. `the_inspector_shows_a_body_and_its_genome`**
- [ ] **E3. `an_archived_genome_can_be_rebuilt_exactly`** — the museum. Development is a pure
  function of the genome (Phase 3's B9), which is what makes this possible at all.
- [ ] **E4. `the_museum_samples_without_changing_the_run`**

---

## Carried in from Phase 6

`Sample.species` was **deliberately omitted** from the time-series record: Phase 7 is what
makes a species exist, and it lands before Phase 8 writes anything to disk, so the field can
be added by the phase that has something to put in it. **Added in Group A**, filled from
`Taxonomy::species_count`.

⚠️ **The record grew from 64 bytes to 72**, and four of those are tail padding: eight for the
tick plus fifteen four-byte scalars is 68, which the tick's eight-byte alignment rounds up.
Phase 8 writes these as a flat array and **must write those four bytes as zeroes**, or two runs
that did the same thing produce files that differ. The whole series is now 288 KiB rather than
256 KiB, which changes nothing about the bound.

The count moves on the clustering's 500-tick grid rather than the chart's 100-tick one, so five
consecutive records carry the same figure — a reading of the last sample, not an interpolation.
It is nought for the first ten thousand ticks of every run, because that is how long twenty
consecutive samples takes; `the_series_records_how_many_species_there_are` is ignored in the
debug suite for exactly that reason and run by the release pass.

## Open questions carried forward

**Q31** (Group I) — ⭐⭐ **answered in Group J, and the answer moved the addressing without moving
the bodies.** The recommendation was taken: development's rule is untouched, and the *distribution*
a re-drawn state comes from is biased — three quarters of `trigger_state` re-draws onto a state some
cell is in, one quarter of `child_state` and `new_state` re-draws onto a state some gene answers to.
Grown cells in a state their own genome names went from **4.6% to 17.7%** over 300,000 ticks.
**Mean body size did not follow**: 6.09 cells against 6.62, with living cells and biomass inside
2.4% of one another. See Group J, and **Q32** below for what that leaves open.

**Q32** (new, Group J) — ⚠️⚠️ **if the addressing was not what held bodies at two cells, what
does?** Group J quadrupled the fraction of its own body a genome addresses and mean cells at
100,000 ticks went from 2.05 to 2.07. What it did move is the *rate*: bodies are 12% larger between
150,000 and 225,000 ticks and the two curves then meet again at 6.1 against 6.6. The accounts say
why — living cells 5,164 against 5,276, biomass 32,687 against 33,468 — **the same energy in the
same number of cells, arranged in slightly more bodies.** Body size is a quotient and the light
decides its numerator, so the plateau looks like an economic fact rather than a developmental one:
two cells is what pays while the population is still filling the world, and what ends the plateau is
the population falling. That is a claim nobody has tested directly, and the cheapest test of it is
`light.influx`, not the genome.

**Q33** (new, Group K) — ⚠️⚠️ **the price of a muscle is not what has been stopping one, and the
sweep says where the obstacle actually is.** Six 300,000-tick runs across a sevenfold range of
`CellKind::Myocyte`'s upkeep moved the *standing* population of muscle 4.7-fold and moved the
number of bodies **born** with two or more myocytes not at all: 95, 54, 62, 112, 67, 187 out of a
quarter of a million births apiece, with no trend. Mean displacement per lifetime is 3.74 against
3.97, and the one-myocyte control travels as far as the two-myocyte bucket, so nothing swims at any
price. **The valley floor came up and the near side is empty**, which makes the question a
supply question rather than an economic one. Group K names the two candidates and builds neither:
give a single muscle a payoff that is not locomotion — changing a body's shape and therefore its
self-shading, which SPEC section 6 already makes a photocyte's harvest depend on — or make the
second muscle reachable in one mutation instead of two. The first is the one that removes the
valley rather than shallowing it.

**Q34** (new, Group L) — ⭐⭐ **the return to size, which is the only question the assay leaves
open and the one it says is binding.** The instrument answered Q33 by pricing every specialisation
in the world and finding none of them worth owning: **nothing here has an increasing return to
being more than one thing**, and that is an economy-wide fact rather than a muscle fact. Every
scaling in the model is linear or worse — income linear in photocytes, upkeep linear in cells, the
reproduction threshold linear in cells, lifespan linear in cells — and occlusion is actively
*sub*linear, since a bigger body self-shades more. So growth is a random walk. The next round
should point at that and not at a cell kind: **what would have to be true for a five-celled body
to out-earn five one-celled ones?** The assay can price any answer in forty minutes, which is the
whole reason it exists.

**Q35** (new, Group L) — ⚠️ **does body size track the light or the population?** Q32 says the
population, with a half-generation lag; SPEC section 3's sustained influx sweep says the light,
because mean cells per body rose 4.07 → 10.62 across a fourfold range of it. Those are rivals and
they were untestable in a constant world. `config/seasonal.toml` separates them, because the
population now rises and falls on a **known clock** while the light does the same thing a quarter
of a period earlier. The statistic is the correlation of detrended mean cells per body against
season phase, with the flat control — which must come out at zero — beside it. ⚠️ **And a genome
statistic must be reported next to it**: genes per genome, mean `rest_length`, the fraction of
genes that divide and adhere. Mean cells per body is an accounting quotient whose numerator and
denominator the light both moves, and it will track the phase in a world where selection is doing
nothing whatever.

**Q3**, **Q5**, **Q6**, **Q8**, **Q9**, **Q12**, **Q16** (`reseed_on_extinction` still does
nothing), **Q18**, **Q19**, **Q21**, **Q23**, **Q25**, **Q28** (`egui-wgpu` still requires
`wgpu ^29`), **Q30** (the charts begin part-way up their boxes and nothing says why).
