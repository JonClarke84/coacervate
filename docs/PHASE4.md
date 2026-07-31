# Phase 4 — reproduction, death, detritus

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *a headless run reaches equilibrium without
extinction or explosion*, and `.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 4** | in progress |
| **Current group** | C — renewal |
| **Suite** | green — 116 tests, 55s |

---

## This is the phase where it either works or it doesn't

Phases 1–3 built a world, a genome, and bodies. None of it is alive yet: nothing eats,
nothing pays for itself, nothing dies, nothing reproduces. This phase closes the loop, and
the moment it closes, the balance question arrives with it.

CLAUDE.md is honest about the odds: *"Balance is genuinely hard. Too much light and
everything blooms and stagnates; too little and everything dies. Expect to spend real time
on the energy economy."* The ecology smoke test in Group D is the one CLAUDE.md says is
**most likely to fail**, and failing it is information rather than defeat — it tells us the
economy is wrong, which is exactly what this phase exists to find out.

---

## Order matters: income before expense

Groups are sequenced so the world is never trivially doomed. Build harvesting first, then
the costs, then reproduction. Building upkeep before harvesting would mean every
intermediate state is a world where everything starves, and no test in between could tell a
balance problem from a missing feature.

---

## Step ledger

### Group A — income: what cells actually do — **done**

SPEC section 6's table gives every cell kind a function. This group implements them. All of it
lives in `behaviour.rs`, which the tick runs between the water and the physics.

- [x] **A1. `a_photocyte_harvests_from_the_tile_it_occupies`** — rate ∝ local energy ×
  exposure, via `Grid::harvest` so the ledger cannot be bypassed.
- [x] **A2. `a_photocyte_is_occluded_by_cells_above_it`** — ⭐ *this is what rewards
  spread-out, branching body plans over compact blobs.* Without occlusion there is no reason
  for a body to be any shape at all, and SPEC section 6 says so explicitly. The consequence has
  its own test, `a_spread_out_body_earns_more_than_a_compact_one`, so that the sentence SPEC
  actually makes fails under its own name.
- [x] **A3. `a_devorocyte_drains_detritus_on_contact`**
- [x] **A4. `a_devorocyte_drains_a_foreign_cell_at_a_rate_its_toughness_reduces`** —
  predation, **emergent not scripted**. A body is a denser package of energy than the soup,
  so eating one is simply a better strategy under some conditions. Whether a
  herbivore/predator split appears is one of the genuinely interesting outcomes and must
  never be coded in.
- [x] **A5. `a_myocyte_oscillates_its_springs_and_pays_for_the_work`** — SPEC section 9's
  reactive controller. Costs `movement_cost` × work done.
- [x] **A6. `a_sensocyte_reports_a_gradient_towards_its_target`** — and because
  `sensor_gain` is signed, *both attraction and avoidance are reachable by mutation*, so
  phototaxis, detritus-seeking and predator-avoidance are all discoverable rather than coded.

#### What Group A decided that SPEC does not say

Each is argued in full where it lives; this is the index.

| Decision | Where | Short version |
| --- | --- | --- |
| **The occlusion model** | `behaviour.rs`, `shade` | Optical depth adds up and `1/(1+depth)` gets through. A shadow is as wide as the cell casting it, tapers to nothing at its edge, and fades to nothing 27.2 units down — twice the longest limb a genome can grow. Anything above shades, not only the body's own cells. |
| **Harvest rate** | `behaviour.rs`, `HARVEST_RATE` | 0.01 of the tile per tick. Not a config key: `influx` and `upkeep_scale` already span the balance space between them, and SPEC section 3's table is transcribed and tested. Group D may disagree. |
| **Toughness** | `cell.rs`, `CellKind::toughness` | Six numbers, sclerocyte at 0.9. Chosen so that biting a sclerocyte returns less than a devorocyte's own upkeep. |
| **Devour rate** | `behaviour.rs`, `DEVOUR_RATE` | 0.05 per contact per tick, one rate for both scavenging and predation — dead tissue simply has no toughness. |
| **Where a cell's behaviour comes from** | `behaviour.rs`, `Behaviour::contract` | The **first gene whose `trigger_state` matches the cell's state**, which is development's own rule with the step window taken off. SPEC never says. |
| **Whose `sensor_gain`** | `behaviour.rs`, `contraction` | The **myocyte's**, per SPEC section 9's pseudo-code (`genome.rs`'s field doc reads the other way). It is also the more evolvable reading: one sensor can excite one side of a body and inhibit the other. |
| **What "connected" means** | `behaviour.rs`, `Behaviour::contract` | Adhered, one spring away. Anything wider gives every muscle in a body the same number, and a body whose muscles all hear the same thing can pulse but cannot turn. |
| **What "work done" is** | `behaviour.rs`, `Behaviour::contract` | Force through distance: the tension already in the spring times how far the contraction moved its rest length. A muscle that does not move pays exactly nothing. |
| **Detritus** | `behaviour.rs`, `Detritus` | A position and an energy, and nothing else. Nothing makes one yet; the tick hands the pass an empty drift. Group B sizes the arena. |

⚠️ **Two things Group B has to pick up.** The devorocyte's reach for a grain and the
detritus-sensing gradient are both **linear scans over the whole drift** — free while it is
empty, quadratic the moment it is not. And an organism can end a tick holding *less than
nothing*: `Organism::lose` deliberately does not refuse it, because SPEC section 5 says
insolvency is not a bookkeeping question. B2 has to clamp before moving anything into
`detritus`. *(Both done — see below.)*

### Group B — expense and exit — **done**

Almost all of it lives in a new module, `metabolism.rs`, which the tick runs after the
physics: the costs, the two ways of dying, and what a corpse becomes. `world.rs` owns the
drift the way it owns the cells, because `behaviour.rs` needs it in the same tick.

- [x] **B1. `every_cell_pays_upkeep_every_tick`** — `upkeep × upkeep_scale`, biomass →
  dissipated.
- [x] **B2. `an_organism_that_runs_out_of_energy_dies`** — SPEC section 5 is explicit that
  the ledger will *not* catch insolvency; this is where it becomes death. Group A's warning
  turned out to have a second half, which has its own test: clamping at nothing leaves the
  overspend sitting in `biomass` for the rest of the run. See
  `a_death_does_not_leave_its_debts_in_the_books`.
- [x] **B3. `an_organism_dies_of_old_age`**
- [x] **B4. `a_dead_body_becomes_detritus_carrying_its_construction_energy`**
- [x] **B5. `detritus_sinks_and_decays_into_the_tile_beneath_it`** — the marine snow, which
  is both atmospheric and functional.
- [x] **B6. `a_freed_slot_is_reusable_and_reaping_is_deterministic`** — ⚠️ Phase 3 flagged
  this as the one nasty one, and it was right. **Checked by breaking it:** the sweep was
  reversed and the whole suite run, and *one* test failed — this one. Every conservation
  claim, every determinism claim and 120,000 ticks of a world with bodies living and dying in
  it all went green against a reaping order that would silently stop a run replaying.
- [x] **B7. `a_per_gene_metabolic_cost_keeps_duplication_alive`** — `metabolism.gene_cost`,
  a new key in `[metabolism]`, defaulting to **0.0001**.

Also done, because Group A handed it over: the two linear scans over the drift are now
searched through a bucket index — `a_crowded_drift_is_searched_rather_than_scanned`.

#### What Group B decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **Construction energy** | `cell.rs`, `CellKind::construction` | A cell is worth **1,000 ticks of its own upkeep**. Derived from upkeep because SPEC gives no third column to derive it from, and because "expensive tissue is expensive to build" is defensible. Not scaled by `upkeep_scale`: a body's cost is a property of the body, not of the weather. |
| **What a corpse carries** | `metabolism.rs`, `Metabolism::scatter` | SPEC's "carrying that cell's construction energy" cannot be read literally without printing energy, so construction energy decides **how a corpse is shared out** and what there is to share is what the organism was holding. A body that starved leaves nothing. |
| **Maximum age** | `metabolism.rs`, `LIFETIME_UPKEEP` | A cell is allowed **8 units of its own upkeep** before its organism dies: `max_age = 8 × cells ÷ cost per tick`. A photocyte body gets 2,000 ticks, a myocyte body 571, a sclerocyte body 4,000. Genome-derived through the *body*, which SPEC section 7 makes a pure function of the genome. Rejected: a lifespan proportional to genome length, which would pull directly against B7. |
| **A dead organism's debts** | `ledger.rs`, `Ledger::write_off` | The one movement that runs backwards out of `dissipated`. An organism that died owing had that energy recorded as heat that was never produced; left alone it accumulates, one death at a time, into a `biomass` account that is large and negative while every organism in the world holds something positive. |
| **Sink speed and decay rate** | `metabolism.rs` | 12 units a second, and 0.4% of what a grain holds per tick with a floor of 0.001 under it. The decay rate is anchored against `DEVOUR_RATE`: rot has to be **slower than a mouth** or scavenging never pays and half of SPEC section 6's devorocyte is a function nothing would ever use. |
| **The last crumb of a grain** | `metabolism.rs`, `Metabolism::rot` | A residue too small for an `f32` tile to express leaves the world, written as the two movements it actually is. Without it grains are immortal: measured, eight corpses left eight grains still in the water 118,000 ticks later. |
| **The drift's size** | `world.rs`, `drift` | One grain per cell the world can hold — 256,000 grains, **4 MB**. That is the whole world dead at once. A corpse that will not fit is dissipated rather than allocated for. |
| **`gene_cost`'s magnitude** | `config.rs`, `MetabolismConfig::gene_cost` | 0.0001. Twenty genes cost what one sclerocyte costs; a genome at the 128 cap costs 0.0128 a tick, which is 3.2 photocytes; a ten-gene genome is 6% of a four-celled body's upkeep. |

### Group C — renewal

- [ ] **C1. `an_organism_reproduces_above_the_threshold`** — `reproduction_threshold ×
  body construction cost`.
- [ ] **C2. `an_organism_with_no_gonocyte_cannot_reproduce`** — reproduction has a real
  structural cost.
- [ ] **C3. `an_offspring_is_a_mutated_copy_placed_next_to_a_gonocyte`**
- [ ] **C4. `a_birth_transfers_offspring_share_of_the_parents_energy`**
- [ ] **C5. `births_fail_silently_at_the_population_cap`**
- [ ] **C6. `a_lineage_is_still_deterministic_across_generations`**

### Group D — the runner, and the question the project exists to ask

- [ ] **D1. `a_run_stops_on_whichever_bound_comes_first`** — wall clock, tick count, or
  extinction. Graceful: finish the tick, then exit.
- [ ] **D2. `max_ticks_per_second_actually_slows_a_run`** — the `slow` profile's only lever.
- [ ] **D3. `energy_is_conserved_across_a_whole_living_run`** — every account in motion at
  once. *Remember Phase 3's lesson: a conservation check cannot see energy that was never
  declared, only energy declared wrongly. Assert the accounts moved.*
- [ ] **D4. `a_headless_run_reaches_a_living_equilibrium`** ⭐⭐ — **the phase's
  done-criterion, and the test CLAUDE.md says is most likely to fail.** A default-config run
  ends with a living, non-degenerate population: neither extinct nor a single clone filling
  the world.

---

⚠️ **Two things Group C and Group D have to pick up.**

**Group C reuses `construction_energy`.** It is in `metabolism.rs`, takes the body's own cells
(`World::cells_of`), and is the same sum C1's `reproduction_threshold × body construction
cost` needs. Do not write a second one. At the shipped multiplier a four-celled photocyte body
is worth 16 units, so C1's threshold is 35.2 and such a body reaches it in about 370 ticks
from empty — against a lifespan of 1,963. Group C may reasonably want to move
`CONSTRUCTION_TICKS`; nothing in Group B depends on its size, only on the ratios.

**⭐⭐ Group D's balance question has moved, and not in the direction Group A expected.** See
Q15 below.

## The decision this phase has to take: a metabolic cost per gene

Phase 3 found the problem and CLAUDE.md already anticipated the answer.

With the shipped rates, duplication and insertion together (0.03) exceed deletion (0.02),
so genomes drift **upward** and will spend most of their time pressed against `max_genes`.
And at the cap, a lengthening mutation *fails* — by design, because truncating from the end
would eat the neutral tail that duplication feeds on.

The consequence is bad: **the project's central operator switches off exactly when a lineage
is most complex.** A saturated lineage can only duplicate in a generation where a deletion
has already freed a slot.

CLAUDE.md's caps table says it plainly: *"Never remove or raise this cap without also adding
a metabolic cost per gene."* So the cost goes in — but note the reasoning is the opposite of
what that sentence implies. It is not there to *stop* bloat. It is there to keep genomes
away from the ceiling so that **duplication stays available**, which is what keeps the
simulation open-ended. A lineage should be pushed back from the cap by selection long before
it arrives there.

This needs a config key. It is a real evolutionary parameter and hardcoding it would repeat
the `reorder_rate` mistake.

---

## Open questions carried forward

**Q15** — ⭐⭐ **the population cap binds about four times before the energy budget does, so
SPEC section 4's carrying capacity never actually applies at the shipped defaults.** Group A
predicted a bloom on the strength of a photocyte earning ~7× upkeep while shaded, and asked
for that to be re-measured once the costs existed. It has been, in
`energy_is_still_conserved_with_organisms_present`, and it stands: **0.0277 a tick earned
against 0.00408 to keep, a margin of 6.75×**, with the bodies deliberately shading one
another.

The number that matters more is what that implies. A four-celled body costs 0.0163 a tick.
The default world is offered 36,864 tiles × 0.012 × a mean light profile of 0.625, which is
**276 units a tick**, so light alone would support about **17,000 such bodies**.
`limits.max_organisms` is **4,000**. The population therefore hits the arena long before it
hits the energy budget — so the pressure the whole ecology is supposed to grow out of never
switches on, and D4 would be judging a world where nothing is scarce. That is CLAUDE.md's
"too much light and everything blooms and stagnates" arriving by a slightly different route
than expected.

`upkeep_scale` is the lever with the right shape: it is live, it scales the cost side without
touching the light, and it is now also the lifespan slider (a hotter world is one where
everything dies younger). The arithmetic above suggests about **4**. It is a calculation, not
an experiment, and Group D is where it gets tried against a running ecology.

**Q13** — **the light gradient is nearly invisible at body scale.** SPEC's `gradient` of 0.75
spread over 1,152 world units is a change of about half a per cent per tile, so a sensocyte
tuned to `Light` in still, full water reads roughly 0.005 — and at `MAX_SENSOR_GAIN` of 1.0
that moves a myocyte's amplitude by half a per cent off a base of 0.3. Phototaxis on the
*background* gradient is therefore reachable in principle and almost invisible in practice.
What a light sensor can actually see is a tile something has been eating out of, which reads
ten to a hundred times higher — so light-sensing lineages would find *feeding grounds* long
before they found the surface. Either `MAX_SENSOR_GAIN` wants raising (`genome.rs` says Phase 4
owns it), or the signal wants amplifying, or the finding is simply accepted. It cannot be
settled without a running ecology, so it belongs to Group D.

**Q14** — **income at equilibrium does not depend on the harvest rate, or on exposure.** A
photocyte draws its tile down until what it takes equals what the light puts back, so its
steady income is the influx and nothing else. `HARVEST_RATE` and occlusion decide how far the
tile is pulled down to get there, and therefore who wins when two organisms share one — but a
*lone* organism earns the same whatever they are. The consequence for Group D is that occlusion
is a pressure that only switches on once the world is crowded, which is exactly when D4's
equilibrium is being judged, and that a sparse world is a world where shape does not matter
much.

**Q3** (no config key for resident-memory or replay-log budgets), **Q5** (`point_sigma`'s
bound is not derived from SPEC — this phase is where mutation strength starts to matter),
**Q6** (`spring_damping`'s meaning was chosen, not specified), **Q8** (trig and logarithm are
the operations IEEE 754 does not pin, so a body's shape is reproducible on *this* toolchain —
Phase 8's archive and Phase 9's GPU port both need to verify rather than assume), **Q9** (a
body wider than half the world would have a spring hauled the wrong way through the seam;
development is where that becomes knowable), **Q12** (the absolute conservation shortfall
grows linearly in diffusion; harmless because the relative error converges, but `spill`
cutting a tile while a residue is still in the flux array is the first place to look).
