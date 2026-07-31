# Phase 3 — genome, development, mutation

**Working ledger.** Same contract as the earlier phases: work can stop at any point and
resume from the next unticked box without re-deriving anything.

**Done when** (CLAUDE.md's phase table): *a genome grows a deterministic body;
duplication/divergence tested; caps hold under fuzzing*, and `.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 3** | **complete** — `.\scripts\check.ps1` exits 0 |
| **Current group** | D — done |
| **Suite** | green — **100 tests** (92 sim, 8 app), 45s |
| **Invariant** | relative error **5.88e-9** over 120,000 ticks of a world with eight bodies in it, non-trending. Tolerance is 1e-3. |
| **Next** | Phase 4 — reproduction, death, detritus |

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
| Bounds for `rest_length`, `stiffness`, `osc_freq`, `sensor_gain`? | Not in SPEC. Declared as constants in `genome.rs` with the reasoning beside each; they bound the random *draw*, not the type. **Group C decided: point mutation clamps to them** — see the Group C table below. |
| Where is the cell cap checked? | *Before* the daughter is made. SPEC's pseudo-code appends and then compares, which overshoots when the cap is 1. |

### Group C — mutation, and the operator the project rests on

- [x] **C1. `a_point_mutation_perturbs_a_numeric_field`** — `N(0, point_sigma)`. Needs a
  Gaussian; `rand_distr` pulls `num-traits` and `libm` and would break the sim's dependency
  rule, so this is ~10 lines of Marsaglia polar instead.
- [x] **C2. `gene_duplication_copies_a_gene_next_to_itself`** ⭐ — **the operator the whole
  genome design exists for.** The copy goes *behind* its original, where first-match-wins can
  never reach it, so a duplication changes the genome and changes nothing about the body.
  That is what makes it free, and free is what makes it able to wander.
- [x] **C3. `a_duplicated_gene_that_diverges_is_a_new_body_part`** ⭐⭐ — the headline, and it
  **works**. A four-cell stalk with one myocyte budded sideways off the tip; duplicate the tip
  gene (body unchanged); point the copy at the state of the *first* segment; the organism
  grows a second myocyte out of the middle of its trunk. Six cells instead of five, two
  muscles instead of one, and a cell with three springs on it where every cell of the parent
  had at most two — the parent is a chain and the mutant has a **fork**, which is not a longer
  chain but a body plan a chain cannot be.
- [x] **C4. `gene_deletion_removes_one`**
- [x] **C5. `gene_insertion_adds_a_random_one`**
- [x] **C6. `reordering_swaps_two_adjacent_genes`** — and `reordering_can_be_switched_off`
  beside it, which is what Q10's `reorder_rate` key bought.
- [x] **C7. `whole_genome_duplication_appends_a_full_copy`**
- [x] **C8. `the_genome_cap_holds_under_ten_thousand_mutations`** ⭐ *(property test)* —
  **critical.** CLAUDE.md marks `max_genes` as the one cap that must never be raised without
  a per-gene metabolic cost, because duplication is exponential without it. A lineage that
  duplicates faster than selection punishes it grows a genome into the megabytes and takes
  the process down.
- [x] **C9. `mutation_is_deterministic_from_the_organisms_own_stream`** — same organism, same
  mutations, regardless of what any other organism did or when.

**What SPEC left open in section 7's mutation list, and what was decided.** All of it is
argued at length in `mutation.rs`; this is the index, on the same terms as Group B's above.

| Question | Answer |
| --- | --- |
| "Each gene, with `point_rate`: perturb numeric fields; discrete fields re-draw uniformly" — every field of a hit gene, or one of them? | **One**, chosen uniformly from the sixteen. The other reading makes a hit gene a brand-new random gene wearing nudged numbers, which erases a duplicated copy before it can diverge and collapses duplicate-then-diverge into "insert a random gene". It also agrees with `default.toml`'s "per gene, per reproduction" and with `genome.rs`'s argument that a gene carries all three actions' parameters so an action change is a *step*. |
| Does point mutation clamp drifted numbers to the bounds `Gene::random` draws from? | **Yes**, and the two that are angles wrap instead. `MAX_STIFFNESS` is where the physics' explicit integrator diverges at a sixtieth of a second and `MAX_REST_LENGTH` is what keeps a body from reaching round the world into SPEC section 8's seam — those are edges of the arithmetic, not preferences. Clamping does pile probability on the wall; rejecting instead would *freeze the field* for a gene sitting on it, which is worse. `angle` and `osc_phase` are circles and have no ends to clamp to. |
| Where does an inserted gene go? SPEC says "insert a fully random gene" and not where. | **Anywhere, uniformly.** Appending would make every inserted gene the last one consulted, so it could only fire on a state no existing gene claims — insertion would be an operator that mostly inserts silence. |
| Is the duplicated copy inserted before or after its original? | **After.** In front, the *copy* would be the expressed gene and the original the silent one, so the gene under selection would be the one made a moment ago rather than the one that has been working. |
| What rate does reordering fire at? SPEC gave five rates for six operators. | **`mutation.reorder_rate = 0.02`**, now a key in SPEC section 3 like the other five, matching duplication and deletion — the other two operators that rearrange rather than rewrite. It was a `REORDER_RATE` constant in `mutation.rs` until Jonathan decided Q10; **see Q10, resolved.** |
| Is `point_sigma` absolute or scaled to each field's range? | **Absolute**, one sigma for all six numbers, which is the literal reading and what Phase 1's Q5 assumed. The consequence is that fields drift at very different speeds *relative to their ranges* — see Q11. |

### Group D — organisms in the world

- [x] **D1. `an_organism_occupies_one_fixed_slot`** — see decision 1 above. Most of the test
  is about the *gap*: a three-celled organism in a slot of eight leaves five cells untouched
  and the next organism starts at cell 8 rather than at cell 3. A packed arena passes every
  other assertion in it.
- [x] **D2. `a_birth_fails_at_the_cap_rather_than_allocating`** — CLAUDE.md: "a full world
  means nowhere to reproduce into", and it is what makes the memory guarantee hold. Compares
  the **capacity** of every arena and the addresses of the two largest, because a test that
  only checked the refusal would pass against an implementation that grew the arena and
  returned the error from somewhere else.
- [x] **D3. `seeding_an_organism_takes_its_energy_out_of_the_field`** — SPEC section 5. ⚠️
  **This is the one where the expected failure mode turned out to be wrong; see below.**
- [x] **D4. `energy_is_still_conserved_with_organisms_present`** — Phase 2's headline,
  re-run over a world that now has bodies in it. **5.88e-9 after 120,000 ticks**, non-trending
  (6.12e-9 worst over the whole run against 2.94e-9 over its first tenth, where an
  accumulating leak would give ten times). `#[ignore]`d and run by `check.ps1`'s release pass,
  the same trade Phase 2's D1 makes: two seconds in release against half a minute in debug.

**⚠️ The energy invariant does not catch a seeding that conjures its energy.** Phase 2 and
SPEC section 5 both say it would — *"it will show up as an invariant failure with no obvious
cause"* — and `world.rs`'s module documentation said so too. It does not, and the reason
matters: an organism whose energy was never *told* to the ledger leaves all five accounts
exactly as they were, so the books balance perfectly while a body stands in the world holding
energy nobody counted. Nothing announces itself until Phase 4 kills that organism and moves
its energy out of a `biomass` account that never received it.

Measured, by making seeding conjure its energy and running the suite: **two tests fail**, D3
and D4, and the world's own tick-by-tick check stays silent throughout. What stands between
this project and the failure is therefore D3's assertion that *the field went down*, not the
invariant. `world.rs`'s module documentation now says so.

**What SPEC left open, and what was decided.**

| Question | Answer |
| --- | --- |
| How much energy does a seeded organism start with? SPEC section 10 gives reproduction a formula (`offspring_share` of the parent) and gives seeding nothing. | **The caller names it**, as an amount. Deriving it would need a per-cell construction cost, which SPEC mentions twice (`reproduction_threshold × body_construction_cost`, and death "carrying that cell's construction energy") and never gives a number for. That number belongs with Phase 4's metabolism, which is the first thing that needs it. |
| Which tiles pay for it, when a body stands on several? | The tiles under the body, each counted once however many cells stand on it, visited in the order the cells were grown, each giving up what is still wanted or what it has. Nothing is spread evenly: a body on rich water pays out of the first tile it is standing on. |
| What if they cannot pay? | **The seeding fails and the field is untouched** — not part-paid, and not a body standing there holding less than it was meant to. The same answer the population cap gives. |
| What type is an organism's stored energy? | **`f64`**, which is SPEC section 5's exception for the ledger accounts rather than a second one: an organism's energy *is* a share of the `biomass` account, and Phase 4's death moves what it holds out of that account. A rounded copy would invent or destroy the difference on every death. |
| A world starts dark. When can the first organisms be seeded? | Not on tick zero, unless they are seeded holding nothing: the field is empty until the light has been falling for some hundreds of ticks (`cap / influx` ≈ 667 at SPEC's defaults). `grid.rs` raised this at the end of Phase 2 and it is now a refusal a caller can see rather than a note. |
| The physics wants a dense array of cells and the slots leave gaps in one. | The tick **gathers** the living cells into a dense crowd, in slot order, with the springs' endpoints shifted into the crowd's numbering, and writes the moved cells home afterwards. Two copies per living cell per tick and nothing at all per empty slot. The alternative — handing the physics the whole arena — would make every empty slot an invisible obstacle that living bodies bump into, and would make a nearly-empty world cost what a full one does. |

---

## Carried into Phase 4

**Freeing a slot on death is three lines, and one of them is easy to forget.** Set
`organisms[slot] = None`, push the slot onto the free list, and **move the organism's energy
out of `biomass` before dropping it** — `Ledger::die` takes an amount and the organism is the
only thing that knows it. Nothing else has to happen: the cells and springs lying in the slot
are never read again and are overwritten by whoever is born there next. There is no index to
fix anywhere, which is the whole point of decision 1.

**The free list is a stack, and its ordering is deterministic but visible.** It is built
holding every slot in reverse, so an empty world fills from slot 0 upwards; a freed slot is
pushed on the end and is therefore the *next* one handed out, ahead of any never-used slot
with a higher number. That is deterministic — the same run gives the same slots in the same
order — but it is only deterministic *given* that deaths are processed in a fixed order, and
Phase 4 owns that. Kill organisms in whatever order a parallel pass happens to finish in and
the free list comes out permuted, which permutes the slots, which permutes the crowd, which
changes the order forces are summed in, which changes the last bit of the physics. Nothing
would announce it: the run would simply stop matching its own recording. **Sweep the slots in
index order when reaping.**

**Slot number is not identity.** A slot is reused; a serial is not. Species clustering, the
event log and the museum all want the serial. Two organisms that lived in the same slot at
different times must never be treated as one, and they will look identical to anything
keying on the slot.

**The organism's energy and the ledger's `biomass` are two records of one quantity**, and
they are only allowed to move together: every change to `organism.energy` needs the matching
`Ledger` call with the same `f64` amount. `energy_is_still_conserved_with_organisms_present`
checks the two against each other at the end of its run, which is what would catch a
metabolism that charged an organism without telling the books.

**Solvency is still nobody's job.** SPEC section 5 is explicit that the ledger does not check
it: spending more than an organism holds drives `biomass` negative while the books balance
perfectly. Upkeep is the first thing that can do it, so Phase 4 is where "energy reaches zero"
has to become death rather than a negative number.

**The crowd is rebuilt every tick and is where liveness will bite.** It costs one copy per
living cell and a walk over every slot, which at four thousand slots is nothing. What is worth
knowing is that it is the only place the arena's gaps are dealt with: anything Phase 4 adds
that walks cells — harvesting, upkeep, detritus — has to walk the slots the way `gather` in
`world.rs` does, not the arena, or it will charge upkeep to the empty half of every slot.

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
cheap answer if it ever matters. *Group C adds one to that list: the natural logarithm, which
the hand-rolled Gaussian in `mutation.rs` needs. Sines, cosines and logarithms are now the
whole of it.*

**Q9, raised by Group B, and looked at by Group D without being closed.**
`MAX_REST_LENGTH` keeps the widest body a genome can grow to under 900 world units, which is
inside half of SPEC's default world width and therefore inside the distance at which the
horizontal wrap starts resolving a spring the wrong way round (SPEC section 8). Nothing
enforces that relationship: a configuration with a world narrower than ~1,800 units breaks it.

Group D placed the bodies and did **not** add the check, and the reason is worth recording
before somebody adds it. The 900 units is the worst case over every genome there could ever
be — sixty-four cells in a straight line, each at the longest rest length a random gene can
draw. Real bodies are nothing like it, and a rule written against the worst case would refuse
world sizes that are perfectly sound: three of Group D's own tests run in worlds 256 and 512
units across, with bodies sixteen and twenty-four units wide. A check that fired on those
would be a check people turn off.

What would actually be wanted is a check on the *body*, at the moment it is grown, rather
than on the configuration: a body wider than half the world is the thing that breaks, and
development is where that becomes knowable. It is cheap — the offsets are already in hand —
and it is Phase 4's to decide, because Phase 4 is the first thing that grows bodies nobody
wrote by hand.

**~~Q10. SPEC section 7 listed six mutation operators and gave five of them a rate.~~
RESOLVED 2026-07-31.** Reordering was written as *"Reordering — swap two adjacent genes"*
with no rate beside it and `[mutation]` had no key for one, so `mutation.rs` declared
`REORDER_RATE = 0.02` — and an operator with no rate cannot be switched off, so it fired on
one reproduction in fifty during every other operator's test. Jonathan decided the key goes
in: `reorder_rate = 0.02` is now in SPEC sections 3 and 7, `config/default.toml`,
`RawMutation`, `MutationConfig` and the validation gate, the constant is gone, and
`reordering_can_be_switched_off` is the test that says so. The claims about a genome at its
cap are now about the genome being *identical* rather than merely holding the same genes.

**Q12, raised by Group D, and it corrects a Phase 2 claim.** `grid.rs` says of the energy
diffusion leaves in transit: *"what is in transit is at most half a rounding step per tile,
which for the default world is around two hundredths of a unit in total, and does not grow
however long the run goes on."* Measuring a world for nine hundred thousand ticks says
otherwise. The books run **short**, and the shortfall grows in a straight line and does not
level off — 4.8e-4 units by a hundred thousand ticks, 2.3e-3 by half a million, 4.2e-3 by
nine hundred thousand, which is about five billionths of a unit per tick. The in-transit
ceiling for that world is 7e-4, and the measurement passes straight through it, so some of
this is genuinely leaving rather than waiting.

**Nothing is at risk and that is why it was not chased.** The light puts 0.78 units a tick
into the same world, so the *relative* error — which is what SPEC section 5's invariant is
stated in, and the reason that wording matters — converges: 4.9e-9 at fifty thousand ticks,
6.6e-9 at half a million, 6.71e-9 at nine hundred thousand and flat thereafter. That is a
hundred and fifty thousand times inside the tolerance, and it is a *converged* number rather
than a growing one, so an overnight run of tens of millions of ticks sits in the same place.
The loss is also six parts in a billion of the world's income, which is nothing beside
anything Phase 4 will tune.

What is open is only the sentence in `grid.rs`, which claims a bound the arithmetic does not
respect. Somebody should find out which part of the field's arithmetic is dropping it —
`spill` cutting a tile back to its ceiling while a residue for that tile is still sitting in
the flux array is the first place to look. Phase 8's archive is the phase that would care,
because a replay has to reproduce the loss exactly, and it will.

**Q11, raised by Group C.** `point_sigma` is a single absolute number applied to all six of a
gene's real-valued fields, which is the literal reading of SPEC section 7 and what Phase 1's
Q5 assumed. The consequence was not obvious until the ranges were written down: at the shipped
0.12, one mutation moves `sensor_gain` by a *twelfth* of its whole range and `stiffness` by
one part in twelve hundred of its. Stiffness and rest length therefore drift far more slowly
than angle and gain do, in a way nobody chose. Nothing is broken; it is a tuning fact that
belongs with Phase 4's balancing, and the alternative — a sigma scaled per field — means five
more constants that SPEC does not have either.
