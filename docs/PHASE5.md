# Phase 5 — the renderer, and the frame dump

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *a frame renders; Claude can see it*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 5** | in progress |
| **Current group** | C — the window |
| **Suite** | green — 156 tests, 107s |

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

### Group B — one frame, on disk ⭐ — **done**

**The phase's done-criterion.** Headless: no window, no event loop. A new crate,
`coacervate-render`, against `wgpu` 30.0.0, `pollster` 1.0.1, `bytemuck` 1.25.2 and
`png` 0.18.1. `coacervate-sim` gained nothing at all.

- [x] **B1. `a_headless_gpu_device_can_be_created`** — an adapter and a device with no
  surface, no display handle and no event loop. **Two ways of failing, told apart**: no
  adapter is a *skip*, an adapter that refuses a device is a *failure*. See the decision
  table.
- [x] **B2. `a_frame_renders_to_a_png`** — an offscreen texture, a copy back, a PNG. The
  frame in the test is **300 pixels across on purpose**, which is 1,200 bytes a row and not
  a multiple of 256, so the row padding is exercised rather than avoided. `--dump-frame
  <path>` runs the world and then draws it.
- [x] **B3. `cells_are_drawn_in_one_instanced_call`** — one draw for one cell, for five
  hundred, and for none. `Frame::draws` is what makes that a count rather than a promise.
  The five attributes' offsets are checked against Rust's own field offsets.
- [x] **B4. `neighbouring_cells_merge_into_one_silhouette`** — ⭐ **three measurements**, not
  an impression. See below.
- [x] **B5. `the_camera_maps_world_coordinates_to_the_frame`** — the middle, the surface, the
  floor, and the seam drawn on both edges at once.
- [x] **B6. Looked at.** `docs/frames/phase5-groupb.png`. Written up below.

#### ⭐ How B4 was stated as a measurement

SPEC's sentence is about how something looks, so the test turns it into three numbers taken
off the frame. All three are measured in **linear light** — `Frame::light_at` undoes the
sRGB curve first — and all three subtract the water, because additive blending adds to the
background as much as to anything and leaving it in drags every ratio towards one.

1. **The light adds up.** Two cells six units apart, and then the left-hand one of that pair
   *alone*. The midpoint between them is measured in both. The second cell is the first's
   mirror image about that point, so the answer has to be exactly **two**, and it is:
   `2.00`. Anything near one means the cells are being drawn *over* one another.
2. **There is no valley between them.** Walking the row from one centre to the other, the
   dimmest point is at least three-quarters of the brightest. It measures ~0.86 — and the
   *brightest* point on that walk is the midpoint, not either centre, which is the metaball
   bulge that makes a pair read as one swollen shape rather than two circles touching.
3. **A pair genuinely far apart does have a valley.** Twenty units apart, the middle is
   empty water — under 1% of the peaks. Without this the first two would also pass on a
   renderer that flooded the frame with light.

Six units is not arbitrary: `founding.rs` springs the two cells of the plainest body in this
world **eight** units apart.

#### What Group B decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐ The seam is a second *quad*, not a second draw** | `cells.wgsl` | Twelve vertices a cell instead of six: the same cell, drawn again a world's width away. For a cell that is not near an edge that copy lands entirely outside the frame and is thrown away before a fragment of it is shaded. This is what keeps SPEC section 12's **one instanced draw call** literally true while a body sitting on the join is still whole. The alternative — three copies, or a second draw for the cells near an edge — costs either 50% more vertices on every cell in the world or the one property SPEC actually asks for. |
| **⭐ A cell's light reaches 2.6× its own radius** | `camera.rs` | A glow that stopped at the cell's edge would leave two units of black water between the two halves of every founder — `founding.rs` springs them **eight** apart and a photocyte is **three** wide — and SPEC's "silhouette rather than a string of beads" would be false of the plainest body in the world. At 2.6 the pair overlaps over almost its whole separation. |
| **⭐ A cell's peak brightness is a half, not one** | `camera.rs` | The technique is *additive*. At one, every overlap clips to white and the sum that makes the silhouette exists and cannot be seen. Held as a `const { assert!(…) }`, so tuning it past a half stops the build rather than one test. Group D's `D1` HDR target is what makes this a choice rather than a ceiling. |
| **No depth buffer, and that is the point** | `frame.rs` | Additive light has no nearer and no further. A depth test would make cells occlude one another, which is precisely the string of beads the falloff exists to avoid. |
| **The target is sRGB, and every measurement undoes it** | `frame.rs` | Blending on an sRGB target happens in **linear** light and is encoded on the way out, which is what makes "two cells are twice one" true of the light and false of the bytes. `Frame::light_at` decodes before anything is measured. A test written against raw bytes would have had to accept a fudge factor, and a fudge factor is where a real error hides. |
| **The camera fits the world's *width*, not its height** | `camera.rs` | The seam only means anything if the left and right edges of the frame are the same place. The depth is scaled by the same factor and centred, so nothing is stretched; at SPEC's exactly-sixteen-by-nine world a sixteen-by-nine frame shows all of it and no water that is not there. |
| **⚠️ No adapter is a skip; an adapter that refuses is a failure** | `gpu.rs` | Two quite different events. `docs/PHASE5.md` asks the suite to skip on a machine with no GPU, and the over-applied version of that swallows a `RequestDeviceError` too — under which the entire renderer could stop working on the one machine this project is built for and the suite would still report green. |
| **⚠️ A skipped test prints a line nobody sees unless they ask** | `gpu.rs` | There is no standard way to report a skip in Rust and no dependency worth adding for one. The message goes to stdout, which cargo captures for a test that passes, so it is visible under `cargo test -- --nocapture` and not otherwise. Carried below as **Q22**. |
| **One device is shared by the whole suite** | `gpu.rs` | Opening a device costs a couple of hundred milliseconds and six tests need one. A `OnceLock` makes the nine tests in this crate run in **0.67 s** total, which is what keeps `docs/PHASE5.md`'s "fast or `#[ignore]`d" on the fast side. |
| **`Frame::draws` exists so that "one instanced call" is a count** | `frame.rs` | The obvious way to write a renderer — a draw per organism, or per cell — works perfectly at the scale of a test and falls over at four thousand organisms. Nothing about the *output* can tell the two apart, so the renderer reports what it did. |
| **`--dump-frame` reuses `--ticks`, and defaults to tick 30,000** | `main.rs` | A world is not worth looking at for some time after it starts. Measured on the shipped settings: tick 10,000 is 8 alive (the dawn, 3 s); 20,000 is 873 (12 s); **30,000 is 1,713, mean genome 1.70 ± 0.85 genes (23 s)**; 60,000 is 2,260 (65 s). Thirty thousand is a population well past its founding with mutation visibly having done something, for a wait somebody checking a shader will sit through. A second flag was not invented: `--ticks` already means "the world's own tick count, dawn included", so `--dump-frame late.png --ticks 200000` draws one four times as old. |
| **The frame is drawn from whatever the run left** | `main.rs` | A run that went extinct before its bound is drawn extinct. A frame dump is a picture of what happened rather than of what was hoped for, and empty water is a perfectly good answer to what a world looks like when nothing is in it. |
| **All five per-instance fields are used, none decoratively** | `cells.wgsl` | Position and radius are the geometry; hue is the colour; **kind** sets saturation (a sclerocyte is structure with no metabolic function and is drawn nearly colourless; a gonocyte, which carries the lineage, is the most saturated thing in the world); **energy flow** shifts brightness, clamped to a third down and just over a half up. A field carried in the vertex layout and never read would be a lie in a struct the tests check the offsets of. `D6` is where the feeding glow becomes worth looking at. |

##### ⭐ What the mutation checks found

Three mutations, chosen as the three ways this group could be quietly wrong.

**The blend was changed from `One + One` to `One + Zero`** — cells drawn *over* one another
instead of added. One test failed, and it failed on the number that names the fault:
*"the midpoint between two cells has 0.25057006 of light with both of them there and
0.25057006 with one, a ratio of 1"*. Exactly one, which is what "the nearer one wins" gives.

**The draw was shortened from twelve vertices to six** — the second quad, and with it the
seam, simply gone. One test failed: *"a cell sitting on the seam lights the left edge of the
frame with 0.41760978 and the right with 0, so half of every body crossing the join simply
vanishes"*. Nothing else in the suite noticed, which is the point of B5 existing separately.

**The row padding was left in the copy back** — `row * real` instead of `row * padded`,
which is the single most likely typo in this crate. Three tests failed, and the one that
*names* the fault is B2's: *"row 148's brightest pixel is at column 299 rather than at the
middle, so the copy back from the texture is not taking the row padding out."* The other two
failed with everything at nought, which on its own would read as a shader that draws
nothing — and that is exactly why B2's frame is 300 pixels wide rather than a convenient
multiple of 64.

#### ⭐ B6 — what the frame actually looks like

`docs/frames/phase5-groupb.png` — 1920 × 1080, world tick 30,000, seed 42, 1,713 organisms.
This is the first time anybody has looked at this simulation.

**The renderer.** It works, and the merging works better than expected. A two-celled body
does not read as two beads: it is one soft rounded lozenge with a faint waist where the two
cells join, which is exactly what SPEC section 12 asks for. Where several bodies are pressed
against one another they fuse into a single continuous blob — at this density a cluster of
four organisms of similar hue is genuinely indistinguishable from one larger animal, which
is arguably *too* merged and is a thing to watch when Group D adds bloom on top. The
background is flat, very dark, slightly blue. The one pale, near-white cell visible in a
crowd of coloured ones is a sclerocyte, so the kind-to-saturation mapping is doing something
legible. The seam is continuous: bodies straddling the join are whole, with no cut edges and
no dark stripe.

The honest criticism of the drawing is that the **interiors are flat**. The falloff's plateau
plus an 8-bit target means a body is a solid slab of colour with a soft rim and no internal
structure at all. It looks like a paper cut-out lit from behind rather than like something
with substance. `D1`'s HDR target and bloom are what that is waiting for.

**The world.** Four things, and three of them were surprises.

- ⭐ **It is eight separate colonies, not a population.** The eight founders are still eight
  distinct patches on `founding.rs`'s four-by-two grid, twenty thousand ticks after they were
  seeded, with wide black water between them. `founding.rs` argues at length that founders
  are spread so the population meets the whole world from the first generation — and what the
  frame shows is that at this age it has not met most of it. Nothing is wrong; the note simply
  described the intent and not the timescale.
- ⭐ **The depth gradient is enormously visible and nobody predicted how much.** The four
  shallow colonies are **several times** the area of the four deep ones. `light.gradient` is
  0.75 and this is what it buys: the top row has grown up until it is pressed against the
  surface and sideways until the four patches nearly touch, and the bottom row has barely
  spread past where it was seeded. Between the two rows is a band of water the full width of
  the world with **nothing in it at all**. Depth is not a minor axis of this world; it is the
  dominant one.
- ⭐ **The hue is confetti, and it should not be.** Adjacent bodies inside one colony are
  cyan, magenta, orange and green in no pattern at all. The hue is the top sixteen bits of the
  genome fingerprint, so *any* mutation — a change to one gene's angle in the fourth decimal —
  rerolls the colour completely. SPEC section 12 wants hue to *drift* as the lineage drifts,
  and a hash does not drift, it jumps. The result is that speciation is currently **less**
  visible than it would be with no colour at all: there are no clusters to see. It is also
  loud, which CLAUDE.md's visually-calm constraint is entitled to object to. This is `D5`'s
  problem and it is now a measured one rather than a predicted one. Carried below as **Q20**.
- Every body is two cells. Mean 1.98 with a spread of 0.14 — twenty thousand ticks of
  mutation have changed the genome (1.70 genes on average, from 1.00) without changing the
  body once. Nothing in the frame contradicts `docs/PHASE4.md`; it is simply the first time
  it has been possible to *see* that the world is entirely made of one shape.

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

**Q20** (new, Group B) — **hue from a genome fingerprint does not drift, it jumps.** SPEC section
12 wants *"hue from species; hue drifts as the lineage drifts genetically"*, and the frame shows
what a hash gives instead: neighbouring bodies inside one colony are unrelated colours, because
any mutation at all rerolls sixteen bits. The effect is that colour currently *hides* speciation
rather than showing it. Two ways out and both are `D5`'s: a hue carried as a **value that mutates
by a small step**, inherited from the parent alongside the genome — which drifts by construction
and needs a field on `Organism` — or a hue derived from Phase 7's species clustering, which does
not exist yet and would make the colour of a body depend on the whole population. The first is
cheap and honest; the second is what SPEC's word *"species"* actually says. Do not decide it here.

**Q21** (new, Group B) — **the merging works so well that a crowd is one animal.** B4's whole
point is that neighbouring cells fuse, and in the frame four organisms of similar hue pressed
together are indistinguishable from one larger organism. That is correct for a *body* and wrong
for a *population*, and there is no information in the frame that separates them. Group D's bloom
will make it more pronounced, not less. Whether anything should — a faint rim, a slightly
different falloff at the boundary between two organisms, or simply leaving it to Phase 7's
inspector — is undecided. It may be that nothing should: a coral reef looks like this too.

**Q22** (new, Group B) — **a test that skips because there is no graphics card says so where
nobody looks.** Rust has no notion of a skipped test and cargo captures the output of a test that
passes, so the line `gpu.rs` prints is visible under `cargo test -- --nocapture` and invisible
otherwise. On a machine with no adapter the six GPU tests therefore report *passing*, which is a
mild lie. A dependency exists for this and is not worth taking for one line; the honest fix if it
ever matters is for `scripts/check.ps1` to open a device itself and say out loud what it found.

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
