# Phase 7 — species, names, and the event log

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *speciation is visible and named*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 7** | in progress |
| **Current group** | A — telling one lineage from another |
| **Suite** | green — 207 tests, ~115s |

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

### Group A — telling one lineage from another

- [ ] **A1. `two_genomes_have_a_distance`** — SPEC section 11: a normalised alignment cost
  over the gene lists; matched genes contribute their scaled numeric difference, unmatched
  genes a fixed penalty. ⚠️ `Organism::genome_hash` is **no use here** — it jumps rather than
  drifts, which is precisely why Phase 5 stopped using it for hue.
- [ ] **A2. `distance_is_a_metric`** *(property test)* — zero to itself, symmetric, and the
  triangle inequality. Clustering rests on all three and none of them is obvious for an
  alignment cost.
- [ ] **A3. `the_living_population_clusters_by_distance`** — every 500 ticks, per SPEC.
- [ ] **A4. `a_cluster_that_persists_is_promoted_to_a_species`** — 20 consecutive samples.
  ⚠️ This needs a **second periodic observer** beside `Series`, with its own memory of
  cluster identity across samples.
- [ ] **A5. `an_organism_knows_its_species`** — organisms carry a drifting `marker` and no
  cluster id, so nothing can count members or say when a lineage ended.
- [ ] **A6. `clustering_does_not_change_what_the_world_does`** — ⚠️ Phase 5's golden vector
  exists for exactly this. An observer that draws one random number, or reorders one arena,
  makes a run deterministic and *different*.

### Group B — names

- [ ] **B1. `a_species_gets_a_binomial_name`** — generated from Latin-ish syllables.
- [ ] **B2. `a_new_species_inherits_its_genus_and_gets_a_new_epithet`**
- [ ] **B3. `a_large_enough_jump_mints_a_new_genus`**
- [ ] **B4. `a_name_is_never_reused_in_one_run`** — two lineages sharing a name is a
  chronicle nobody can read.
- [ ] **B5. `names_are_reproducible_from_the_seed`** — same run, same names. Without this the
  chronicle of a replayed run disagrees with the original.

### Group C — the event log ⭐

The highest-value item in the phase. Append-only, human-readable, in a naturalist's register.

- [ ] **C1. `an_event_is_recorded_with_its_tick_and_its_deep_time`** — *"Tick 41,208 — 41.2
  Ma."*
- [ ] **C2. `first_adhesion_is_noticed`** — the origin of multicellularity in this run.
- [ ] **C3. `the_first_appearance_of_each_cell_kind_is_noticed`**
- [ ] **C4. `the_first_predation_is_noticed`**
- [ ] **C5. `speciation_and_extinction_are_recorded_by_name`**
- [ ] **C6. `new_records_are_noticed`** — body size, cell count, genome length, population.
- [ ] **C7. `a_mass_extinction_is_noticed`** — population falling by >50% within 5,000 ticks.
  ⚠️ `Series` has population history but thins to a 25,600-tick stride late in a run, so this
  needs its own short high-resolution window.
- [ ] **C8. `a_change_the_user_made_is_recorded`** — Phase 6's sliders are environmental
  events and the log should say so.
- [ ] **C9. `no_event_text_uses_the_banned_vocabulary`** ⭐ — the register test described
  above, run over every string the phase can generate.
- [ ] **C10. `serial_repetition_is_noticed`** — **candidate, decide deliberately.** Not in
  SPEC's list. In its favour: it is what Jonathan actually spotted, it is the visible
  signature of the operator the whole project rests on, and it is cheap — a body whose
  structure repeats is knowable at development time. Against: SPEC's list is deliberate and
  this is scope creep. **If it goes in, it must be phrased as what changed** — *"a structure
  is being built more than once in one body"* — never as an achievement.

### Group D — Darwin in the margin

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
be added by the phase that has something to put in it. **Add it here.**

## Open questions carried forward

**Q3**, **Q5**, **Q6**, **Q8**, **Q9**, **Q12**, **Q16** (`reseed_on_extinction` still does
nothing), **Q18**, **Q19**, **Q21**, **Q23**, **Q25**, **Q28** (`egui-wgpu` still requires
`wgpu ^29`), **Q30** (the charts begin part-way up their boxes and nothing says why).
