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
| **Current group** | B — the resource field |
| **Suite** | green — **43 tests** (35 sim, 8 app) |

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

- [ ] **B1. `the_grid_is_allocated_once_at_the_size_the_config_asks_for`**
- [ ] **B2. `light_falls_off_with_depth`** — `light_profile(y) = 1 - gradient × (y/height)`;
  brightest at the surface, dimmest at the floor. *SPEC section 4: the gradient is what
  gives movement a reason to exist.*
- [ ] **B3. `patchiness_is_stable_across_runs`** — the spatial noise is a pure function of
  the world seed, computed once at construction, never per tick.
- [ ] **B4. `a_tile_regrows_towards_its_target_and_stops_there`**
- [ ] **B5. `regrowth_credits_influx_with_the_realised_change`** — see design decision 2.
- [ ] **B6. `diffusion_moves_energy_sideways`**
- [ ] **B7. `diffusion_does_not_leak_at_the_edges`** — the world wraps horizontally and is
  closed vertically. *A closed vertical axis that quietly loses energy at the floor is the
  most likely conservation bug in this phase.*
- [ ] **B8. `the_field_reaches_a_ceiling`** — total influx is fixed, so the field is bounded.
  This is SPEC section 4's carrying capacity, and the pressure everything later grows from.

### Group C — cells and physics

- [ ] **C1. `a_cell_has_the_fields_spec_section_6_gives_it`**
- [ ] **C2. `the_spatial_hash_finds_the_same_neighbours_as_checking_everything`** *(property
  test)* — the hash is an optimisation, and the only way to trust an optimisation is to
  check it against the slow, obviously-correct version. *SPEC section 8 calls this the
  single most important performance decision on the CPU side.*
- [ ] **C3. `overlapping_cells_push_apart`**
- [ ] **C4. `a_spring_pulls_towards_its_rest_length`**
- [ ] **C5. `motion_is_viscous_not_ballistic`** — with drag at 0.92 a cell coasts to a halt
  rather than sailing on. SPEC section 8: at cell scale inertia is nearly irrelevant, which
  is both physically right and numerically far more stable.
- [ ] **C6. `the_world_wraps_sideways_and_is_closed_top_and_bottom`**
- [ ] **C7. `physics_is_stable_under_a_pile_up`** — a crowd of overlapping cells must not
  explode. *The classic failure of a spring-and-collision system, and it fails loudly rather
  than subtly, which is why it is worth a test rather than a hope.*

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
