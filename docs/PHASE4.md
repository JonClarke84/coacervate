# Phase 4 — reproduction, death, detritus

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *a headless run reaches equilibrium without
extinction or explosion*, and `.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 4** | **done** |
| **Current group** | — |
| **Suite** | green — 134 tests, 97s |

⚠️ The suite went from 54s to 97s and all of the difference is **D4**, which runs the shipped
world for thirty thousand ticks and costs 43 of those seconds. It is the phase's
done-criterion and CLAUDE.md's most-likely-to-fail test; it is marked `#[ignore]` and
`scripts/check.ps1` runs it once, in the release pass, exactly as the two long conservation
tests are run. Everything else in Group D costs about a second.

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

### Group C — renewal — **done**

Almost all of it lives in a new module, `reproduction.rs`, which the tick runs **last of all**,
after the reaping. Nothing else in the project can call it: `World::seed` is the door from
outside and this is the only other way the `organisms` array ever gains an entry.

The tests are in `world.rs` rather than beside the code, and that is a departure from Group B.
Every claim here is about a whole world — a slot coming off the free list, a body grown from a
mutated genome and written into the arena, an arena that did not grow when there was nowhere to
put a child — and only a `World` has those. `metabolism.rs` builds its scenes by hand because
its claims are about *amounts*, and an amount can be measured without a world around it.

- [x] **C1. `an_organism_reproduces_above_the_threshold`** — and the sum it multiplies is
  `metabolism.rs`'s `construction_energy`, asserted to be the same one a corpse is shared out
  by. SPEC uses the phrase "construction energy" twice and defines it nowhere; two sums would
  have been free to drift apart with nothing reporting it. The test watches every tick, so it
  asserts the birth did not happen *early* as well as that it happened.
- [x] **C2. `an_organism_with_no_gonocyte_cannot_reproduce`** — both bodies in one world, so the
  only thing differing between them is the one SPEC section 6 names. What says the barren one
  *could* have bred is that it ends holding twice its own bar and never once falls by so much
  as a unit in a tick, which is what handing over `offspring_share` would look like.
- [x] **C3. `an_offspring_is_a_mutated_copy_placed_next_to_a_gonocyte`** — the child's genome is
  **recomputed in the test**, from the parent's genome and a fresh stream of the parent's own
  numbers, and compared gene for gene. The parent is given two gonocytes so that "next to a
  gonocyte" has more than one answer, and a second seed is run to show the side it lands on was
  drawn rather than fixed. It also checks nothing exploded: a newborn is laid down *touching*
  its parent, which is a collision force from the first tick of its life.
- [x] **C4. `a_birth_transfers_offspring_share_of_the_parents_energy`** — ⚠️ **checked by
  breaking it.** The parent was made not to pay, so the child's energy came from nowhere, and
  **two tests failed: this one and C5.** Everything else went green — including
  `energy_is_still_conserved_with_organisms_present`, which asserts the invariant across
  120,000 ticks. That is not a gap in the suite; it is SPEC section 5's warning being exactly
  right. Both ends of a birth are `biomass`, so a transfer nobody declared moves *no account at
  all*, and the books balance perfectly around a body holding energy that was never counted. The
  parent is a lone gonocyte, which earns nothing, so every figure in the test is written out
  from SPEC's own numbers rather than measured and compared with itself.
- [x] **C5. `births_fail_silently_at_the_population_cap`** — ⚠️ **half of this test passes
  against a world in which nothing is ever born**, and it did: run before the code existed,
  every assertion about the cap went green and meant nothing. Its red was the **positive
  control** — the same three bodies with one slot free, which came back `[0, 1, 2]` against the
  `[0, 1, 2, 3]` expected. Capacity is compared rather than length, and so are the addresses of
  the two largest arenas.
- [x] **C6. `a_lineage_is_still_deterministic_across_generations`** — two halves, and the second
  is the load-bearing one. Two runs of one seed agreeing would pass against a single world-wide
  generator. So the same founder is bred in two worlds — alone, and with three bodies sitting on
  top of it shading it — and its first child has to be the *same child*. It is born on a
  different tick in the two, and the assertion that the ticks differ is what makes the genome
  assertion mean anything.

#### What Group C decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **Where a birth sits in the tick** | `world.rs`, `tick` | **Last**, after the reaping. A slot this tick's deaths freed can be born into on the same tick, so a world at its cap turns over rather than idling a tick between every death and its replacement; a body that did not survive the tick does not breed on its way out; and a newborn is laid down beside a parent the physics has finished moving. The price is that a newborn gets one free tick — it is not aged, fed or charged until the next one. |
| **A body does not breed on the tick it was born** | `reproduction.rs`, the `age() == 0` skip | The pass walks the slots from the front and a newborn can land in a slot the walk has not reached. Without this, a rich body has a child holding enough to have a child, and a world goes from one organism to its cap between two frames. Written as a rule about organisms — *a body that has not lived a tick has not had a tick in which to accumulate anything* — rather than as a guard over a loop. |
| **Which gonocyte** | `reproduction.rs`, `nursery` | The **first in the body**, which is the order development made them and therefore an order the genome decides. Rejected: a random one, which spends a draw from the parent's stream to no visible end, and the nearest or richest, neither of which means anything — a gonocyte holds no energy of its own and there is nothing for a nursery to be near. A lineage that wants a different one grows it first. |
| **What "adjacent" is** | `reproduction.rs`, `beside` | **Exactly touching**: the two radii added, from SPEC section 6's table. It is the one distance SPEC section 8's collision does not immediately act on, and the same reading `development.rs` already takes when it buds a daughter that does not adhere. Placing a newborn *on top of* its parent works — the physics shoves them apart — but every birth in the world would start with a spike of collision force, and a spike is what an explicit integrator does not want. |
| **What "a small random offset" is** | `reproduction.rs`, `beside` | **Which way.** A fixed direction would stack a body's successive children in the same spot, because a gonocyte does not move between one birth and the next — and cells at exactly one point are the case `physics.rs` has to break a tie for. One number from the parent's stream scatters them. |
| **A birth is a named ledger movement** | `ledger.rs`, `Ledger::inherit` | `biomass → biomass`, so no total in the world changes and the check at the end of the tick can never see it. Named anyway, for the reason `predate` already gives: the alternative is somebody adding to one organism and subtracting from another by hand, which is the same arithmetic with nothing watching it. |
| **Where a body's cells go** | `organism.rs`, `lay_out` | One definition, shared by a seeding and a birth, replacing `World::placed`. It became worth extracting because a newborn can be budded **across the seam** — a child of a body at the world's right-hand edge belongs at the left-hand one — where a seeded body's position was always chosen by a caller. |
| **`CONSTRUCTION_TICKS` stays at 1,000** | `cell.rs` | Group B invited Group C to move it. Measured instead: a photocyte-and-gonocyte body seeded into a full default world has its first child on **tick 458**, against the 1,963 ticks `metabolism.rs` allows it to live — three or four generations a body, which is the ratio `LIFETIME_UPKEEP` was chosen against. Moving the thousand moves both numbers together and changes nothing about that ratio. |

⚠️ **The loop now turns, and Group C ran it.** The first ecology reading is under Q15 below,
and it is not reassuring. Read it before Group D touches a single number.

### Group D — the runner, and the question the project exists to ask — **done**

The runner is three new files in `coacervate-app`, and it is there rather than in
`coacervate-sim` because a run's bounds are wall-clock bounds and the simulation is forbidden a
clock. `run.rs` is the loop and the bounds; `founding.rs` is the light falling and the first
bodies going in; `census.rs` reads a population from outside, because the world deliberately
keeps no summary of itself.

- [x] **D1. `a_run_stops_on_whichever_bound_comes_first`** — four worlds, each given the full
  set of bounds with exactly one brought within reach, so "whichever comes first" is actually
  being asked. Graceful is asserted as *the books balance where it stopped*, which is the
  observable form of "the last thing it did was finish a tick".
- [x] **D2. `max_ticks_per_second_actually_slows_a_run`** — and the load-bearing half is the
  other one: the capped run and the uncapped run are compared **tile for tile and account for
  account**, so the `slow` profile cannot quietly perturb a result. SPEC section 2's
  "real-time speed is decoupled" as a test.
- [x] **D3. `energy_is_conserved_across_a_whole_living_run`** — the first test in the project
  where every movement `ledger.rs` knows about happens at once. Most of it is the half a
  conservation check cannot see: the field went **down**, `dissipated` went **up**, there were
  grains in the water, and the biomass at the end is held by bodies that were *born* rather
  than seeded.
- [x] **D4. `a_headless_run_reaches_a_living_equilibrium`** ⭐⭐ — **the phase's done-criterion,
  and the test CLAUDE.md says is most likely to fail.** It passes, and it did not before
  Group D retuned `light.influx`. Checked both ways: put the old light back and it fails on
  the arena assertion; raise `limits.max_organisms` tenfold and the run comes out *identical
  to the digit*.

#### What Group D decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **`light.influx` is 0.001, not 0.012** | `SPEC.md` section 3 | The one number in SPEC's table that has now been measured. Twelvefold down. Carrying capacity is very nearly proportional to it — about 2.2 million times it — and at 0.012 the world hits `max_organisms` four to eight times before it hits the energy budget. The sweep and both control experiments are in SPEC beside the value. |
| **`upkeep_scale` is the wrong lever, and Q15's arithmetic was wrong to name it** | measured; recorded in SPEC section 3 | It is *also* the lifespan slider, so raising it shortens a life while lengthening the time needed to earn a child — it closes the breeding window from both ends. **At 3 and at 4 the founder dies before a single birth.** Q15 asked for 4. |
| **The runner lives in the app, and the clock allowance is one line at the crate root** | `main.rs` | Not a `[lints]` table in the package manifest, which would replace the workspace one and silently drop the five cast lints. `clippy.toml` said so; this is it being done. |
| **A run's bound is on the *world's* tick count** | `run.rs` | Including the ticks the dawn spent. One clock, not two - and it is the number SPEC section 2's deep-time display reads off. |
| **Bounds are examined between ticks and never inside one** | `run.rs`, `Run::over` | That is the whole of the graceful-shutdown promise, and it is structural rather than careful: there is no state a stop can catch halfway. |
| **A run is handed a world that is already alive** | `founding.rs` | So extinction can be the plain question "is anything alive", with no history to keep. An empty world is already over and says so before it takes a tick. |
| **Founders go on an even grid over the whole world** | `founding.rs` | A lineage stays where its founder was for a long time, so one founder digs one hole in one corner. It measurably does not change where the population ends up — one founder and eight reach the same level — only how long it takes. |
| **⚠️ `Ctrl-C` cannot be caught, and Enter is what stops a run** | `run.rs` | `SetConsoleCtrlHandler` is `unsafe` and `ctrlc` is a new dependency; both are forbidden. What exists is the *seam* — an `Interrupt` flag the loop reads between ticks — with `main` setting it from a thread waiting on standard input. Phase 5 brings an event loop and can wire a signal to the same flag. |
| **Tests that only need *a lit world* now set their own light** | `world.rs`, `behaviour.rs`, `metabolism.rs`, `grid.rs` | `light.cap / light.influx` is how long a field takes to fill, so retuning the ecology broke fifteen tests that had nothing to do with it. They pin their own influx now. The two long conservation tests deliberately do **not**: they are the ones that read the shipped world. |
| **`run.reseed_on_extinction` is still read by nothing** | `run.rs` | Deliberately. Putting a second founding population into a world whose first one died is a statement about what a run *is*, and it wants deciding on purpose rather than as a side-effect of the loop that stops. Carried forward below. |

---

### ⭐⭐ What the balance work actually found

**The measured sweep is in SPEC section 3 and it is the shipped justification.** Three things
came out of it that were not in the calculation Q15 wrote.

**1. Carrying capacity is proportional to `influx`, cleanly, over more than an order of
magnitude** — about 2.2 million times it. Section 4's claim holds exactly.

**2. The field is drawn down to about half, at *every* influx where energy is what binds.**
That is not a coincidence: where a tile settles is decided by the income a body needs to
replace itself before it dies, which comes from `[metabolism]` and not from `[light]`. So
`influx` decides how *many* bodies there are and the metabolism decides how *hard* each works.
The pair make a diagnostic — **a field that is barely below full is a world where something
other than energy is limiting the population**, which at 0.012 was `max_organisms`.

**3. Low light does not cause extinction.** At a tenth of the shipped influx the world settles
at about 210 bodies and turns over indefinitely. The `famine` profile's stated purpose in SPEC
section 3 was wrong and has been corrected: what ends a run is `upkeep_scale`, not darkness.

### ⭐⭐ And the half-million-tick run, which is the interesting part

SPEC section 15 asks for 500,000 ticks. Run once, at the shipped configuration, from eight
founders. The equilibrium D4 asserts holds from tick 50,000 to tick 200,000 — a hundred and
fifty thousand ticks of a flat population at about 2,100. **Then it moves, and what moves it is
the bodies**: mean cell count goes 2.00 → 6.73 and mean genome length 2.10 → 10.80, while the
population falls to 794 and living biomass stays at about 33,000 throughout.

The same energy, held by a quarter as many bodies, each three times the size. Nothing in the
model rewards being larger. The full table is in SPEC section 15.

State it as what changed rather than as an improvement, per CLAUDE.md: the population is now
4,537 photocytes, 788 gonocytes, 11 sclerocytes, 5 sensocytes, 1 myocyte and 1 devorocyte. No
feeding-strategy split appeared. Nothing swims.

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

**Q16** (new, Group D) — **`run.reseed_on_extinction` is a configuration key that does
nothing.** See the decision table above. It wants a decision about what a run *is*, not a line
in the loop.

**Q17** (new, Group D) — **`Ctrl-C` does not shut a run down gracefully and cannot be made to
without `unsafe` or a new dependency.** Enter does. Harmless today because nothing is written to
disk; the moment Phase 8's replay log exists it stops being harmless. Phase 5 brings `winit`
and an event loop, which is the natural place to fix it.

**Q13 is answered, negatively, and by accident.** The light gradient is still nearly invisible
at body scale — but the half-million-tick run produced five sensocytes in a population of 794,
one myocyte, and no swimming at all. Nothing has yet found a use for either, so raising
`MAX_SENSOR_GAIN` would be tuning a mechanism nothing is using. Left alone.

**Q15 — closed.** ⭐⭐ **The diagnosis was right and the lever it named was wrong**, and both
halves are recorded above and in SPEC section 3. `light.influx` is now 0.001 and the energy
budget binds first; `upkeep_scale` at the 4 this asked for kills every world it is applied to
before a single birth. It is kept below, unedited, because the reasoning is worth being able to
re-read against what actually happened — a static energy-budget calculation is a perfectly good
way to find out that something is wrong and a bad way to decide what to do about it.

#### The original Q15, kept as written

**The population cap binds about four times before the energy budget does, so
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
everything dies younger). The arithmetic above suggests about **4**.

### ⭐⭐ The first ecology reading — the whole loop turning, once, for real

Group C's last act was to run it, because the calculation above was only ever a calculation.
**One** photocyte-and-gonocyte body, holding two units, seeded into a default-config world
that had been lit for 1,500 ticks first. Nothing else changed. Measured 31 July 2026,
Windows 11 x86-64, release build.

| Tick | Population | What is happening |
| --- | --- | --- |
| **458** | 1 → 2 | first birth |
| 1,000 | 7 | |
| 2,500 | 42 | first deaths; the founder's generation reaches old age |
| 5,000 | 177 | |
| 10,000 | 762 | |
| 15,000 | 2,059 | |
| **19,565** | **4,000** | **`limits.max_organisms`. Births start failing.** |
| 115,000 | 4,000 | still 4,000, and still nowhere near running out of food |

**It does not crash and it does not stabilise. It fills.** Growth is clean exponential —
population roughly doubles every 2,700 ticks — from one body to the arena cap in **under
twenty thousand ticks**, which is about five minutes of simulated time and a few seconds of
wall clock. It then sits at exactly 4,000 for as long as it was watched, with the population
turning over underneath (14,000 grains of detritus in the water at tick 115,000, against a
drift built for 256,000, so the arena is not close to being the binding constraint either).

**Q15 is confirmed, and it is worse than the calculation suggested.** The world is not short of
anything. Over the 95,000 ticks after the cap was reached:

- the **field falls only 12%**, from 184,000 units to 163,000 — the water is still nearly full
  while every slot in the world is taken;
- **biomass climbs four-fold**, from 61,000 to 235,000, because the bodies at the cap keep
  getting richer with nothing to spend it on;
- **mean cell count climbs from 2.00 to 5.06** and **mean genome length from 1.97 to 4.24**.

That last line is the interesting one and it cuts both ways. Bodies are growing, which is what
this project was built to see — but they are growing under *no selection pressure at all*,
because the only thing stopping a lineage reproducing is that there is nowhere to put a child.
When every birth fails for the same reason regardless of how well the parent is doing, being
better at anything buys nothing. That is a world where drift is the only force acting, and it
is exactly the "blooms and stagnates" failure CLAUDE.md warns about, arriving by the route
Group A predicted.

**Group D therefore has a real, measured problem rather than an arithmetic worry**, and the
lever is the one already identified: `upkeep_scale`, which is live, scales the cost side
without touching the light, and doubles as the lifespan slider. The calculation said about 4.
The reading above is what it now has to be tried against — and the thing to watch is not
whether the population survives but whether the **cap stops being what limits it**: D4's
"living, non-degenerate population" is only a meaningful test in a world where the energy
budget binds first.

*(End of the original Q15. What Group D actually did is at the top of this file and in SPEC
section 3. The last sentence above turned out to be exactly the right instruction and the
paragraph before it turned out to name the wrong slider.)*

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
