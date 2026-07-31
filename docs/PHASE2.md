# Phase 2 — resource grid, physics, energy ledger

**Working ledger.** Same contract as `PHASE1.md`: work can stop at any point and resume
from the next unticked box without re-deriving anything.

**Done when** (CLAUDE.md's phase table): *energy conservation holds over 100,000 ticks as a
property test*, and `.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 2** | in progress |
| **Current group** | D — the done-criterion |
| **Suite** | green — **68 tests** (60 sim, 8 app) |
| **Invariant** | relative error **1.74e-10** over 100,000 ticks, default config, non-trending. Tolerance is 1e-3, so about six orders of magnitude of headroom. |

## What this phase changed in the spec

Three corrections, all found by writing the tests before the code. Each is now written into
`SPEC.md` with its reasoning.

**The regrowth formula destroyed energy.** SPEC wrote `tile += min(regrowth, target - tile)`
with the comment "never exceeds target". Read literally that is a *subtraction* whenever a
tile sits above its target — light running backwards, with no account to put the difference
in. Not hypothetical: ceilings fall with depth, so diffusion permanently pushes energy into
dimmer rows. It produced a *false* equilibrium propped up by a leak.

**Clamping light to a source was not enough either, and this is the one that mattered.**
With light unable to remove energy, diffusion still carries it downward and deep tiles sit
above their targets forever. The field fills until it is **level** — every tile at the
deepest ceiling in the world, no depth structure left. Swimming upward would pay nothing,
and section 4's claim that "the gradient is what gives movement a reason to exist" would
quietly stop being true. So a tile genuinely cannot hold more than its target now, and the
excess moves `field → dissipated`: energy sinking below the light is energy the system
loses, which is what the biological pump does in a real ocean.

**`diffusion` is bounded at 0.25, not 1.0, and the ledger cannot catch it.** An explicit
five-point stencil overshoots above a quarter and compounds until the numbers stop being
finite — but overshoot *moves* energy rather than inventing it, so the invariant reports a
healthy world right up until the field is nonsense. Now a validation error that says it is a
stability limit rather than a preference.

## Two bugs worth remembering

**Diffusion leaked 70% of an evenly-filled world in 100 ticks.** The naive per-tile gather —
add four neighbours, subtract four times me, treat a missing neighbour as nothing — passes a
test that drops a spike in the middle of an empty world, because the spike never reaches an
edge. Rewritten as a list of neighbour *pairs*, one flow per pair applied to both ends
together. Edges then need no special case: the last row simply appears in fewer pairs, so
there is no route out of the world to write down and therefore none to write down wrongly.

**Rounding lost energy steadily, and inside tolerance.** The obvious implementation gives
2.8e-4 after 100,000 ticks — well within the 1e-3 allowed, and *growing*, crossing the line
around a million ticks. An overnight run is tens of millions, so it would have failed with
no bug to find. The cause is the shape of the field, not arithmetic bias: a few tiles lose a
lot each while many gain a millionth each, and a millionth added to a tile holding eight
units rounds to nothing. Fixed by leaving the part of a movement that will not fit in the
flux array rather than discarding it — energy is then never lost, only ever *in transit*.

**Ecological consequence for Phase 4:** the standing field now settles at ~184,030 at the
default config rather than creeping past 238,600 towards ~313,000. Carrying capacity is
roughly a quarter lower than the old numbers implied, and now genuinely stable.

**Carried into Phase 3:** SPEC section 5 now says explicitly that seeding an organism must
take its starting energy out of `field`. A seeded organism feels like it comes from outside
the world, so conjuring its body is an easy leak to write — and it shows up as an invariant
failure on tick zero with no obvious cause. Solvency (an organism spending more than it
holds) is deliberately *not* the ledger's problem; it belongs with the organism.

---

## What this phase is for

Phase 1 built the things a run is *described* by. This one builds the world it happens in:
a field of energy that light replenishes, a ledger that proves no energy is invented or
lost, and the physics that will later push bodies around. There are still no organisms —
they arrive in Phase 3 — so `biomass` and `detritus` stay at zero throughout, and that is
fine. The point of doing the ledger before there is anything to eat is that a conservation
bug found now is a five-line fix, and the same bug found in Phase 4 looks like an ecology
that mysteriously blooms or starves.

---

## The three design decisions that shape everything else

### 1. `field` is not stored. It is measured.

Four accounts are stored as running `f64` totals: `biomass`, `detritus`, `dissipated`,
`influx_total`. The fifth, `field`, is **computed by summing the grid tiles in `f64`** every
time the invariant is checked.

This is deliberate. A stored `field` account and an array of tiles are two representations
of one quantity, and keeping them in step is exactly the bookkeeping that goes wrong. If
`field` is measured, it cannot disagree with the grid, because it *is* the grid.

### 2. Record what actually happened, not what was intended.

Tiles are `f32`. Adding `0.012` to a tile does not increase it by exactly `0.012` — it
increases it by whatever the nearest representable value allows. So light regrowth is
written as:

```
let before = tile;
tile = (tile + regrowth).min(target);
influx_total += f64::from(tile - before);      // the realised change, not the intended one
```

`influx_total` then matches the grid exactly rather than approximately, and the only
remaining source of drift in the whole ledger is diffusion.

### 3. Diffusion's drift is real, bounded, and what the tolerance is for.

SPEC section 5 states the invariant with a `± 1e-3 relative` tolerance, which is an
admission that float arithmetic drifts. Lateral diffusion moves energy between `f32` tiles
and each move rounds. That drift is the phase's headline risk and the headline test:
100,000 ticks, and the books still balance.

---

## Step ledger

### Group A — the energy ledger

- [x] **A1. `a_new_ledger_balances`** — five accounts, an empty world, the books balance.
- [x] **A2. `energy_can_only_move_between_accounts`** — every operation is a *transfer*.
  There is no public way to add or remove energy except light, so conservation is
  structural rather than merely checked. *This is the single most valuable decision in the
  group: a bug that cannot be expressed does not need a test.* Delivered as a private
  `transfer` taking one amount and applying it to both ends, so a credit and its debit
  cannot disagree, plus two `should_panic` guards — a negative amount would otherwise run a
  transfer backwards and turn heat into biomass, which was the last remaining way to say
  "destroy 5 units".
- [x] **A3. `light_is_the_only_source`** — `influx_total` is the one account that grows from
  nothing, and it grows only via the grid's realised change.
- [x] **A4. `a_leak_is_caught`** — deliberately destroy energy and confirm the invariant
  notices. *Without this, every other ledger test could pass against a check that always
  returns true* — which is precisely what it caught: at the moment A4 was written, `check`
  had the body `let _ = field;` and every other test in the group was green against it.
- [x] **A5. `the_invariant_panics_in_release`** — CLAUDE.md requires the check to hold in the
  profile the overnight runs actually use.
- [x] **A6. `an_f32_account_would_have_stopped_counting`** — ⚠️ **this test corrected the
  spec.** SPEC's original claim (mine) was that an `f32` `influx_total` stalls after ~38,000
  ticks, "under a minute". That conflated a *ratio* with an absolute total and was wrong by
  a factor of ~470: the account actually freezes at tick **17,780,259**, at 2³³, where the
  gap between neighbouring `f32` values is 1,024 against 442.4 being added. The f64 decision
  survives via a different and earlier number — the invariant breaks at tick **121,128**,
  because it does not need the account to freeze, only for the two sides to drift more than
  `1e-3` apart. Both numbers are now measured, pinned, and written into SPEC section 5. The
  frozen total sits ~9% *above* the truth, so nothing announces itself.

### Group B — the resource field

- [x] **B1. `the_grid_is_allocated_once_at_the_size_the_config_asks_for`**
- [x] **B2. `light_falls_off_with_depth`** — `light_profile(y) = 1 - gradient × (y/height)`;
  brightest at the surface, dimmest at the floor. *SPEC section 4: the gradient is what
  gives movement a reason to exist.* Sampled at the **middle** of each row's band of depth:
  sampling the upper edge would put the whole grid half a tile too shallow, and the middle
  is the only choice under which the average over rows equals the average over the world's
  depth. Note the world's `height` in world units cancels entirely — a field of a given
  shape is lit identically whether the world is a micrometre or a mile deep.
- [x] **B3. `patchiness_is_stable_across_runs`** — value noise on a 16-tile lattice, hashed
  from the world seed, bilinear with smoothstep easing. Pure function of seed and position,
  computed once at construction, never touching the world RNG stream. 16 tiles is 128 world
  units — about twenty cell widths, so crossing a patch is a real journey, and it divides
  both default grid dimensions exactly so the horizontal wrap has no seam (measured: 0.005
  across the join against 0.032 between ordinary neighbours). Pinned by a golden test.
  **Open question Q7 is answered.**
- [x] **B4. `a_tile_regrows_towards_its_target_and_stops_there`** — ⚠️ **corrected SPEC.**
  See "What this phase changed in the spec" below.
- [x] **B5. `regrowth_credits_influx_with_the_realised_change`** — see design decision 2.
  Each side is widened to `f64` *before* subtracting, not after: the difference of two `f32`
  values is not always an `f32`, and rounding it would defeat the one number this design
  exists to keep exact.
- [x] **B6. `diffusion_moves_energy_sideways`**
- [x] **B7. `diffusion_does_not_leak_at_the_edges`** — ⚠️ **caught a real 70% leak.** See
  below.
- [x] **B8. `the_field_reaches_a_ceiling`** — settles with the depth gradient intact: rows
  fall monotonically from surface to floor, top row **3.26×** the bottom. The floor sits
  exactly on its own ceiling and sheds everything that reaches it.
  *Note the world no longer freezes bitwise, and cannot* — a field with a gradient has light
  falling and the same amount leaving every tick forever, so "at rest" means no tile moves by
  more than a millionth of what it holds. Verified not to be a slack bar hiding a world still
  filling: the tick before it is met, some tile is still moving seven times further.

### Group C — cells and physics

- [x] **C1. `a_cell_has_the_fields_spec_section_6_gives_it`** — `Vec2`, `CellKind` (radius
  and upkeep only), `Cell`. No cell *function* — harvesting, predation, oscillation and
  sensing are Phases 3 and 4.
- [x] **C2. `the_spatial_hash_finds_the_same_neighbours_as_checking_everything`** *(property
  test)* — the hash is an optimisation, and the only way to trust an optimisation is to
  check it against the slow, obviously-correct version. *SPEC section 8 calls this the
  single most important performance decision on the CPU side.* Proptest shrank its red
  straight to **seam crossings**, which is exactly where it would have been wrong.
  ⚠️ **This test had a gap and it was closed:** it measured every candidate with its own
  independently-written separation helper, so it only ever tested candidate *enumeration* —
  breaking the wrap in the distance calculation was invisible to it. It now also checks the
  wrapped offset against the slow version and requires it never to exceed half a world.
- [x] **C3. `overlapping_cells_push_apart`** — ⚠️ **my assertion was wrong and the test
  caught it.** An overlapping pair does *not* settle at exactly touching; it coasts about
  four units past contact, because the water is viscous rather than infinitely thick. That
  is correct for SPEC's model, so the claim now requires the pair to come to *rest* and the
  measured overshoot is documented.
- [x] **C4. `a_spring_pulls_towards_its_rest_length`**
- [x] **C5. `motion_is_viscous_not_ballistic`** — measured: a cell shoved at 60 units/second
  travels **11.5 units** and stops, under two body-widths.
- [x] **C6. `the_world_wraps_sideways_and_is_closed_top_and_bottom`**
- [x] **C7. `physics_is_stable_under_a_pile_up`** — an 8×8 pile stops settling between
  `collision_stiffness` **3,240 and 3,280**. The shipped default of 40.0 sits about **eighty
  times below** that — comfortable, not a cliff. Two things worth knowing: it is a figure for
  a *crowd* (a cell in the middle feels eight contacts, and eight forces of stiffness `k`
  behave like one of `8k`; a lone pair has no ceiling at all), and instability here does
  **not** produce infinities, because overlap is bounded by the cells' own width — a runaway
  world jitters forever instead. *A stability check written as "are the numbers still finite"
  would therefore pass at every value there is.*

### Group D — the done-criterion

- [ ] **D1. `energy_is_conserved_over_100k_ticks`** ⭐ — the phase's headline. A default
  config, 100,000 ticks, invariant checked throughout.
- [ ] **D2. `energy_is_conserved_for_any_config`** *(property test)* — SPEC section 15 asks
  for "any config", not just the default one.
- [ ] **D3. `a_run_is_still_reproducible`** — Phase 1's determinism test, extended over a
  world that now actually does something.

---

## Open questions

Carried forward from Phase 1 and still unanswered: **Q3** (no config key for the resident
memory or replay log budgets), **Q5** (`point_sigma`'s upper bound is not derived from
SPEC), **Q6** (`spring_damping` has no stated semantics at all).

New in this phase:

- **Q7. SPEC section 4 says `noise(x, y)` and does not say which noise.** The choice is
  load-bearing in one narrow sense — it becomes part of a run's identity the moment a run is
  archived — but it is not otherwise consequential, so it is being chosen rather than asked
  about, and pinned by a golden test like the RNG mapping was.
