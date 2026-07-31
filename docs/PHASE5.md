# Phase 5 — the renderer, and the frame dump

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *a frame renders; Claude can see it*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 5** | in progress |
| **Current group** | B — one frame, on disk |
| **Suite** | green — 147 tests, 110s |

---

## The order is the point

CLAUDE.md is unusually specific about this phase, and about the order:

> There is no browser here, so the usual approach of driving a page and screenshotting it
> does not apply. `--dump-frame <path>` renders one frame to a PNG and exits… Claude reads
> those PNGs directly to see what it has built. **Without this, every visual change becomes
> Jonathan describing what's wrong in prose.**

So **the PNG dump comes before the window**, not after. A headless render to an offscreen
texture is simpler than a windowed one, fully testable, and it is the thing that lets the
rest of the phase be verified at all. Building the window first would mean every visual
decision after it is made blind.

> **A UI change is not complete until a frame has been dumped and looked at.**

That is the rule for the whole phase. Every group below ends with a frame on disk that has
been looked at, and what was seen is written into this ledger.

---

## Step ledger

### Group A — the seams — **done**

Phase 4 listed what the renderer needs from the simulation that does not exist. None of it
is GPU work; all of it is testable now.

- [x] **A1. `a_run_can_be_stepped_one_tick_at_a_time`** — `Run::step() -> Option<Stop>`, with
  `go` rewritten as a loop around it, so there is one loop and not two that can drift apart.
  Asserted the strong way: the stepped run and the run left to itself are compared **tile for
  tile and account for account**, and a step is *counted* to be exactly one tick.
- [x] **A2. `the_living_cells_can_be_read_without_the_empty_slots`** — `World::living_cells`
  and `World::living_cell_owners`, handing out the crowd `gather` already builds every tick.
- [x] **A3. `the_drift_can_be_read`** — `World::drift`. One grain per cell, left where the
  body was, holding exactly what the ledger's `detritus` account says.
- [x] **A4. `a_body_has_a_position_that_wraps`** — `organism::body_centre`: a circular mean
  over `x`, a plain mean over the depth.
- [x] **A5. `an_organism_knows_its_parent`** — `Organism::parent` (nothing at all for a
  founder) and `Organism::genome_hash`, from a hand-written FNV-1a in `genome.rs` with the
  gene count written in before the genes.
- [x] **A6. `the_binary_takes_arguments`** — `args.rs`: `--config`, `--seed`, `--ticks`,
  `--dump-frame`, `--help`. Hand-rolled. `--dump-frame` parses and then says it is not built
  yet, with a failing exit code.

#### What Group A decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **The dense cell list is the population of the tick just taken, not of this instant** | `world.rs` | The crowd is gathered at the *start* of a tick and a tick ends by reaping and then breeding, so a body that died during the tick is still in the list and one born during it is not. Both are one tick — a sixtieth of a simulated second — out of date. The alternative is a second `gather` at the end of every tick: one copy per living cell, paid by every headless run, for a reader that may not exist. **Pinned as an assertion** rather than left to be discovered by somebody wondering why a corpse flickered. |
| **A body's centre is a circular mean, and it may never be read back into a tick** | `organism.rs` | `docs/PHASE4.md`'s Q8 records that trigonometry is what IEEE 754 does not pin, so a sine may differ in its last bits between toolchains. That is harmless for a camera, an inspector or species clustering — all *readings* — and would not be harmless anywhere else. |
| **⚠️ `atan2` answers `-π..π`, so the wrap at the end is load-bearing** | `organism.rs` | Measured by removing it: a body sitting exactly on the seam then comes out at **x = -2.8e-16**, a position outside the world, which the spatial hash and Group B's camera wrap both assume cannot exist. `physics.rs` warns about the same value from the other side. |
| **The genome fingerprint is a hand-written FNV-1a, pinned by a golden vector** | `genome.rs` | `std`'s `DefaultHasher` explicitly does not promise stability across Rust releases, so a hue taken from it would shift the colour of every lineage on the next `rustup update` and silently orphan every frame kept and every run archived. Same decision `grid.rs` takes about the blotchiness of the light, for the same reason. |
| **A founder's parent is nothing at all, not a zero** | `organism.rs` | Serials start at nought, so a zero meaning "founder" is the same value as "child of the first organism in the run" — and a lineage tree then comes out with every root joined to one arbitrary body. |
| **`--ticks` sets the config key, so it counts the dawn too** | `args.rs` | `run.max_ticks` is a bound on the *world's* tick count, the dawn included — Phase 4's decision. A second meaning for one word is the arrangement this project keeps refusing, so the flag sets the key and `--help` says what the key means. At the shipped light the dawn is about ten thousand ticks. |
| **A flag given twice is refused rather than resolved** | `args.rs` | `--seed 1 --seed 2` could be either, and both answers are guesses about which of two contradictory instructions was meant. |
| **⚠️ `--seed` and `--ticks` do not appear in the kept document** | `args.rs` | SPEC section 13 wants `config.toml` verbatim, so the document is kept exactly as it arrived and the overrides are applied to the *values*. A run started with `--seed 7` therefore has a configuration document that is wrong about its own seed. `main` says so out loud; the fix is Phase 8's. Carried below as **Q18**. |
| **A golden vector pins the whole group as having changed nothing** | `run.rs` | `a_run_produces_what_it_produced_before_group_a` was recorded from the code as it stood at the end of Phase 4, before a line of Group A was written — because every figure in `docs/PHASE4.md` was measured on that program, and an accessor that quietly drew one extra random number would leave a run perfectly deterministic and deterministically *different*. |

##### ⭐ What the mutation checks found

**A4 was written against a naive mean first, deliberately.** With `body_centre` returning the
plain average of the cell positions, `a_body_has_a_position_that_wraps` fails with SPEC section
8's warning stated as a number: *"a body with one cell two units inside the left edge and one
two units inside the right has its centre at x = 1024. The plain average says 1024 — the middle
of the world, a thousand units from either cell — and this answer is no better."*

**Then the shipped version was mutated by dropping the `wrapped` call at the end.** One test in
the workspace failed — this one — and it failed on the assertion that the answer is inside the
world, reporting **x = -0.00000000000000028**. That mutation was chosen over the obvious ones
(swapping the sine and the cosine, averaging the depth circularly) because it is the one a
*correct-looking* circular mean most plausibly omits: `atan2` returns a signed angle and the
conversion back to a coordinate looks finished without it. It is also the only one whose damage
is invisible at a glance — a hair below nought reads as zero in anything printed to two decimal
places, and it is a position the rest of the codebase has been written on the assumption that
nothing can hold.

### Group B — one frame, on disk ⭐

**The phase's done-criterion.** Headless: no window, no event loop.

- [ ] **B1. `a_headless_gpu_device_can_be_created`** — and skips cleanly rather than failing
  if no adapter is available, so the suite still runs on a machine without a GPU.
- [ ] **B2. `a_frame_renders_to_a_png`** — `--dump-frame <path>` renders once and exits.
- [ ] **B3. `cells_are_drawn_in_one_instanced_call`** — SPEC section 12. Per-instance:
  position, radius, hue, energy flow, kind.
- [ ] **B4. `neighbouring_cells_merge_into_one_silhouette`** — ⭐ soft radial falloff in the
  fragment shader, drawn additively. *SPEC section 12: "This one technique is most of the
  difference between 'creature' and 'physics demo'."*
- [ ] **B5. `the_camera_maps_world_coordinates_to_the_frame`** — including the horizontal
  wrap, so a body on the seam is drawn on both sides rather than half-vanishing.
- [ ] **B6. Look at the frame.** Write down what it actually looks like.

### Group C — the window

- [ ] **C1. `the_window_opens_and_shows_the_world`** — `winit`, resizable, well-behaved at
  any aspect ratio, **never steals focus**.
- [ ] **C2. `the_camera_pans_and_zooms_and_only_when_asked`** — ⚠️ CLAUDE.md's second-screen
  constraint: *it must never move on its own*.
- [ ] **C3. `f12_dumps_the_current_frame`** — to `runs/<id>/frames/`.
- [ ] **C4. `the_simulation_and_the_display_run_at_their_own_speeds`** — the tick rate is
  decoupled from the frame rate; `max_ticks_per_second` still governs the former.
- [ ] **C5. `closing_the_window_stops_the_run_gracefully`** — Phase 4 left `Interrupt` as the
  seam for exactly this.
- [ ] **C6. Look at a frame from the window.**

### Group D — what makes it worth looking at

- [ ] **D1. `bodies_render_into_an_hdr_target_and_bloom`** — separable Gaussian, composited
  with tone mapping.
- [ ] **D2. `an_accumulation_buffer_leaves_motion_trails`** — which make swimming legible.
- [ ] **D3. `the_background_is_a_depth_gradient_with_light_shafts`** — bright at the surface,
  near-black at depth.
- [ ] **D4. `marine_snow_is_the_actual_detritus`** — not decoration. It is already in the
  simulation.
- [ ] **D5. `hue_comes_from_lineage_and_drifts_with_it`** — so speciation is visible as it
  happens.
- [ ] **D6. `a_well_fed_cell_visibly_glows`** — saturation and brightness from `energy_flow`.
- [ ] **D7. Look at the frames, and keep looking until it is worth looking at.**

---

## Constraints that are easy to forget

- **`coacervate-sim` must never learn that rendering exists.** No `wgpu`, no `winit`, no
  `egui` in that crate — CLAUDE.md calls this out and it is what keeps a browser front-end
  possible later. Group A adds *accessors*, not hooks.
- **Visually calm.** CLAUDE.md: no flashing, no sudden camera moves, nothing that pulls the
  eye. When something dramatic happens the *log* says so; the screen does not shout. This is
  easy to violate accidentally and it is a real design constraint, not flavour.
- **No portability budget.** SPEC section 12: target the 4070 Ti and use what it does best.
  There is no requirement to stay inside mobile or embedded feature levels.
- **The suite must still run on a machine with no GPU.** Skip, do not fail.

---

## Open questions carried forward

**Q18** (new, Group A) — **a configuration overridden from the command line is a run whose own
verbatim document disagrees with it.** SPEC section 13 wants `config.toml` copied into the run's
directory exactly as written, and `--seed` and `--ticks` are applied to the parsed values rather
than to the text, so a run started with `--seed 7` archives a document that says `seed = 42`.
The two ways out are both worse than the problem today: rewriting the document loses the
comments, which is the thing the verbatim copy exists for, and writing the overrides down beside
it invents a file format. Phase 8 owns the replay log and should own this. `main` prints a line
when an override is in force in the meantime.

**Q19** (new, Group A) — **`World::living_cells` is one tick behind on births and deaths**, and
`the_living_cells_can_be_read_without_the_empty_slots` pins that rather than fixing it. See the
decision table above for the trade. If Group D's motion trails or Phase 7's inspector ever make
the lag visible, the fix is a second `gather` at the end of the tick and the cost is one copy per
living cell per tick on every run, watched or not.

**Q3**, **Q5** (`point_sigma`'s bound is not derived from SPEC), **Q6** (`spring_damping`'s
meaning was chosen), **Q8** (trig and log are what IEEE 754 does not pin — `body_centre` is now
the first thing in the project that depends on it, and it is a reading rather than a cause),
**Q9**, **Q12**, **Q16** (`reseed_on_extinction` is a config key that deliberately does nothing
yet), **Q17** (`Ctrl-C` does not stop a run gracefully — `Run::step` is now the seam a window's
event loop will drive, so Group C is where this gets fixed).
