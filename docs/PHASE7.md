# Phase 7 — species, names, and the event log

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *speciation is visible and named*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 7** | in progress |
| **Current group** | D — Darwin in the margin (A, B, C and F are done) |
| **Suite** | green — **252 tests, 111s** |

⚠️ **Group F is out of order and it had to be.** It is the swimming work, taken out of turn
because Jonathan's live run had reached tick 2.8 million with one myocyte in it and the
diagnostic found that **nothing in this world could move, and nothing ever could have**. It sits
in the step ledger below between Groups D and E, which are the two still to do.

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

**Q3**, **Q5**, **Q6**, **Q8**, **Q9**, **Q12**, **Q16** (`reseed_on_extinction` still does
nothing), **Q18**, **Q19**, **Q21**, **Q23**, **Q25**, **Q28** (`egui-wgpu` still requires
`wgpu ^29`), **Q30** (the charts begin part-way up their boxes and nothing says why).
