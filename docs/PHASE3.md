# Phase 3 — genome, development, mutation

**Working ledger.** Same contract as the earlier phases: work can stop at any point and
resume from the next unticked box without re-deriving anything.

**Done when** (CLAUDE.md's phase table): *a genome grows a deterministic body;
duplication/divergence tested; caps hold under fuzzing*, and `.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 3** | in progress |
| **Current group** | C — mutation (A and B are done) |
| **Suite** | green — 85 tests, 43s |

---

## This is the most important phase in the project

CLAUDE.md's decision log opens by saying so: *"This is the single most important decision in
the project."* A fixed list of numbers can only ever evolve a better single cell, because
there is no slot for a thing that does not exist yet. A variable-length list of rules keyed
on developmental **state** can gain structure — duplicate a rule, change its `trigger_state`,
and you have a **new body part**.

Everything here serves that one operator. If gene duplication and divergence do not work,
the project is a screensaver.

---

## Two architectural decisions, taken before any code

### 1. Organisms live in fixed slots. Nothing is ever compacted.

Group D of Phase 2 flagged the real problem: a body is a *range* of the cell arena, and
removing a dead body from the middle of a flat array invalidates every spring index above
it. The tempting answers — swap-remove with index fixups, generational handles, a free list
over variable ranges — are all fragmentation problems, and all of them are bugs waiting to
be written.

So: **organism `n` owns cells `[n × max_cells_per_organism, …)` and nothing else ever does.**
A slot is either alive or free. Death marks it free; birth takes a free one from a free
list. No range ever moves, so no index ever needs fixing, and the awkward problem simply
does not exist.

This costs nothing we were not already paying — Phase 2 already allocates
`max_organisms × max_cells_per_organism` cells at startup, and CLAUDE.md requires exactly
that. It is also the layout the GPU port wants: SPEC section 14 proposes a compaction pass
for births and deaths, and fixed slots mean there is nothing to compact.

Springs are slotted the same way, at `max_cells_per_organism - 1` per organism: development
creates a spring only when a gene divides a cell into an *adhered* daughter, and a daughter
is created once, so a body of `n` cells has at most `n - 1` springs. Spring endpoints are
stored **local to the organism**, because a spring never crosses between organisms — and
SPEC section 8 warns that a spring created across the world would haul two cells together
through the seam.

### 2. Staying with `rand`. The organism's generator is rebuilt, not stored.

Phase 1 decision A1 left this open, to be settled before the arena was written. `rand`'s
`ChaCha8Rng` is not `Clone`, not `Default`, and cannot be serialised; `rand_chacha`'s is.

**Staying with `rand`.** The workaround is not a workaround so much as the better design: an
organism persists `(serial, word_position)` and its generator is rebuilt from the world seed
on demand. That is 16 bytes instead of a 136-byte cipher state in every snapshot, it cannot
drift out of step with the seed, and it keeps `coacervate-sim`'s direct dependencies at
exactly `rand` and `serde` as CLAUDE.md requires — which is also what keeps `thread_rng` a
compile error rather than a rule.

The two implementations are bit-identical for the same seed, so this decision reverses
cleanly if it ever proves wrong. No golden vector and no archived run depends on it.

---

## Step ledger

### Group A — the gene and the genome

- [x] **A1. `a_gene_has_the_fields_spec_section_7_gives_it`** — condition (`trigger_state`,
  `min_step`, `max_step`), action (`Divide` / `Differentiate` / `Terminate`), and the
  parameters each action needs. Fixed-size records so they pack into flat arrays.
- [x] **A2. `a_genome_is_a_list_of_genes_and_is_capped`** — `max_genes` from the config.
- [x] **A3. `a_random_gene_is_always_within_bounds`** *(property test)* — every field in
  range, whatever the generator produces. The mutation operators lean on this.

### Group B — development

- [x] **B1. `a_genome_with_no_genes_grows_a_single_cell`** — the seed cell is a photocyte at
  state 0, per SPEC section 7.
- [x] **B2. `divide_appends_a_daughter_at_the_angle_the_gene_asks_for`**
- [x] **B3. `an_adhered_daughter_is_sprung_to_its_parent_and_a_free_one_is_not`** — *this is
  the origin of multicellularity in the model, and it is one boolean.*
- [x] **B4. `differentiate_changes_a_cell_in_place`**
- [x] **B5. `terminate_makes_a_cell_fire_no_further_genes`**
- [x] **B6. `the_first_matching_gene_wins_so_gene_order_carries_information`** — *reordering
  is therefore a meaningful mutation, and non-firing genes accumulate as the raw material
  duplication diverges from. That is a feature, not waste.*
- [x] **B7. `development_stops_at_the_cell_cap`** — and stops *entirely*, per SPEC section 7.
- [x] **B8. `development_stops_at_the_step_cap`**
- [x] **B9. `a_body_is_a_pure_function_of_its_genome`** ⭐ *(property test)* — same genome,
  same body, every time. This is what makes the museum able to rebuild any archived organism
  exactly, and it is what makes every other test in this phase trustworthy.
- [x] **B10. `development_always_terminates`** *(property test)* — over arbitrary genomes,
  including adversarial ones. SPEC section 15 asks for this by name.

**What SPEC left open, and what was decided.** All of it is argued at length in
`genome.rs` and `development.rs`; this is the index so Groups C and D do not re-derive it.

| Question | Answer |
| --- | --- |
| What is "the parent's body axis"? | The direction a cell was budded in, fixed at birth and never changed. The seed cell has no parent, so it faces `+x` — the same tie-break `physics.rs` already uses. Angles therefore *compound* down a chain of cells, which is what lets one gene draw a curve rather than only a straight line. |
| Does a daughter made during a step act in that same step? | No. A step visits the cells present when it began, so a step is a **generation** and a dividing genome doubles per step. |
| How far from its parent is a daughter placed? | An adhered one at its spring's `rest_length`, so a body is grown already relaxed; a free one just touching, which is what budding looks like and the one distance collision will not immediately undo. |
| Is `state` really `0..=63`? | Yes, and it is now a six-bit type rather than a `u8` with a rule attached. Any byte constructs one, keeping the low six bits — which is exactly uniform, so Group C's "re-draw discrete fields uniformly" needs no retry loop. |
| Bounds for `rest_length`, `stiffness`, `osc_freq`, `sensor_gain`? | Not in SPEC. Declared as constants in `genome.rs` with the reasoning beside each; they bound the random *draw*, not the type. **Group C must decide whether point mutation clamps to them.** |
| Where is the cell cap checked? | *Before* the daughter is made. SPEC's pseudo-code appends and then compares, which overshoots when the cap is 1. |

### Group C — mutation, and the operator the project rests on

- [ ] **C1. `a_point_mutation_perturbs_a_numeric_field`** — `N(0, point_sigma)`. Needs a
  Gaussian; `rand_distr` pulls `num-traits` and `libm` and would break the sim's dependency
  rule, so this is ~10 lines of Marsaglia polar instead.
- [ ] **C2. `gene_duplication_copies_a_gene_next_to_itself`** ⭐ — **the operator the whole
  genome design exists for.**
- [ ] **C3. `a_duplicated_gene_that_diverges_is_a_new_body_part`** ⭐⭐ — the headline. Take a
  genome, duplicate a gene, change the copy's `trigger_state`, and show the body gains a
  structure it did not have. *If this test cannot be written, the project's central bet has
  failed and we need to know now rather than in Phase 7.*
- [ ] **C4. `gene_deletion_removes_one`**
- [ ] **C5. `gene_insertion_adds_a_random_one`**
- [ ] **C6. `reordering_swaps_two_adjacent_genes`**
- [ ] **C7. `whole_genome_duplication_appends_a_full_copy`**
- [ ] **C8. `the_genome_cap_holds_under_ten_thousand_mutations`** ⭐ *(property test)* —
  **critical.** CLAUDE.md marks `max_genes` as the one cap that must never be raised without
  a per-gene metabolic cost, because duplication is exponential without it. A lineage that
  duplicates faster than selection punishes it grows a genome into the megabytes and takes
  the process down.
- [ ] **C9. `mutation_is_deterministic_from_the_organisms_own_stream`** — same organism, same
  mutations, regardless of what any other organism did or when.

### Group D — organisms in the world

- [ ] **D1. `an_organism_occupies_one_fixed_slot`** — see decision 1 above.
- [ ] **D2. `a_birth_fails_at_the_cap_rather_than_allocating`** — CLAUDE.md: "a full world
  means nowhere to reproduce into", and it is what makes the memory guarantee hold.
- [ ] **D3. `seeding_an_organism_takes_its_energy_out_of_the_field`** — SPEC section 5. *A
  seeded organism feels like it comes from outside the world, so conjuring its body is an
  easy leak to write — and it shows up as an invariant failure on tick zero with no obvious
  cause.*
- [ ] **D4. `energy_is_still_conserved_with_organisms_present`** — Phase 2's headline,
  re-run over a world that now has bodies in it.

---

## Open questions carried forward

**Q3** (no config key for resident-memory or replay-log budgets), **Q5** (`point_sigma`'s
upper bound of 1 is not derived from SPEC — it is a standard deviation, not a probability,
and this phase is where it starts to matter), **Q6** (`spring_damping` has no stated
semantics; a meaning has been chosen and documented in `physics.rs`).

**Q8, raised by Group B.** Turning an angle into a direction needs a sine and a cosine, and
those are the only arithmetic in this simulation that IEEE 754 does *not* pin to one answer:
two versions of a maths library may legitimately differ in the last bit. Everything else
would replay identically on any machine. A body's shape is reproducible here, with this
toolchain; Phase 8's archive and Phase 9's GPU port both have to check that rather than
assume it. A golden-vector test on a grown body — the same shape as `rng.rs`'s — is the
cheap answer if it ever matters.

**Q9, raised by Group B.** `MAX_REST_LENGTH` keeps the widest body a genome can grow to
under 900 world units, which is inside half of SPEC's default world width and therefore
inside the distance at which the horizontal wrap starts resolving a spring the wrong way
round (SPEC section 8). Nothing enforces that relationship: a configuration with a world
narrower than ~1,800 units breaks it, and `config.rs` has no reason to know. Group D places
bodies in the world and is where a check belongs, if one is wanted.
