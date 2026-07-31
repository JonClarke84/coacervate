# Phase 5 — the renderer, and the frame dump

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *a frame renders; Claude can see it*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 5** | in progress |
| **Current group** | D — what makes it worth looking at |
| **Suite** | green — 170 tests, 108s |

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

### Group C — the window — **done**

`winit` **0.30.13**, added to `coacervate-render` with default features off and `rwh_06` alone —
the defaults are Wayland, X11 and Android, a dozen crates this Windows-only executable can never
call. The window is **opt-in**: `--window`. Everything the group added is reachable from the
headless path, which is what let the one real bug in it be found.

- [x] **C1. The window opens and shows the world.** Opens at 1280 × 720, resizable, minimum
  320 × 180, `with_active(false)`. **Verified on the machine**: the window opens on the 4070 Ti,
  draws, and closes by itself when the run ends. The focus claim was measured rather than
  asserted — `GetForegroundWindow` before and during: unchanged.
- [x] **C2. `the_camera_pans_and_zooms_and_only_when_asked`**, plus
  `panning_past_the_seam_comes_back_round`, `zooming_is_anchored_on_the_pointer`,
  `the_camera_stays_where_there_is_a_world_to_see`,
  `a_window_of_any_shape_shows_the_world_unstretched`, and - on the card -
  `the_camera_can_be_dragged_across_the_seam`. Drag to pan, wheel to zoom.
- [x] **C3. `f12_asks_for_a_frame_once_per_press`** — to `runs/<id>/frames/tick-<n>.png`,
  through `Renderer::render_through`, which is `--dump-frame`'s own code with the window's
  camera handed to it. **Verified end to end** by driving the real window: `tick-0000014392.png`.
- [x] **C4. `the_simulation_and_the_display_run_at_their_own_speeds`** — `advance` takes as many
  ticks as fit in 8 ms of each frame, none at all if `max_ticks_per_second` says the next one is
  not due, and stops the moment the run ends. `a_run_says_whether_a_tick_is_due_and_why_it_stopped`
  is the other half, in `run.rs`.
- [x] **C5. `closing_the_window_stops_the_run_gracefully`** — the close button sets Phase 4's
  `Interrupt` and nothing else. The tick in progress finishes, `Run::step` answers `Asked`, the
  loop stops, the final report is printed. **Q17 is answered for the window** and still open for
  `Ctrl-C`.
- [x] **C6. Looked at.** `docs/frames/phase5-groupc.png`, from the windowed path's own camera.
  Written up below.

#### What Group C decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⚠️ `winit` went into `coacervate-render`, and CLAUDE.md's architecture sketch says `coacervate-app`** | `window.rs` | Recorded as a deviation rather than done quietly. The window, the surface, the camera and the event mapping are all *drawing*, and a surface is built from the window handle - splitting them would put `winit` in both crates. What crosses the gap instead is a four-method trait, `Watched`: is a tick due, take one, read the world, ask it to stop. `coacervate-app` still owns the loop, the bounds and the clock, and `coacervate-sim` still has never heard of either. The one rule CLAUDE.md is emphatic about - the simulation crate must not learn that rendering exists - is untouched. |
| **⭐⭐ A resized window keeps the piece of world on the screen, not the magnification** | `camera.rs` | The other rule was written first and is what most map views do. What it does on Windows is this: a window asked for at 1280 × 720 is created and winit reports **`Resized(2858, 1481)` and then `Resized(1280, 720)`**, a frame apart, before anybody has touched anything. Keeping the magnification meant the wide one clamped the scale and the narrow one kept it, so **every window opened showing 917 units of a 2048-unit world** and said nothing. See the mutation checks. |
| **The window is opt-in** | `args.rs` | A run is twelve hours long by default and is started by a scheduled task as often as by a person. A window that opened by itself would cost a display's worth of drawing for nobody and would fail outright on a machine with no graphics card that is otherwise perfectly able to run the simulation. |
| **⭐ There is no easing, and that is the design rather than a shortcut** | `camera.rs` | SPEC section 12 says the camera *"must never move on its own"*, and an eased camera by definition carries on moving after the input that started it has stopped. So a wheel notch moves the view by a tenth **and then it is still**. The smoothness comes from the step being small. `the_camera_pans_and_zooms_and_only_when_asked` draws a thousand frames from an untouched lens and compares them all. |
| **Zooming out stops at the world's width** | `camera.rs` | `cells.wgsl` draws each cell exactly twice - itself and its copy across the seam - and that covers any view up to one world wide and no more. A view wider than the world would need a third copy, which would cost 50% more vertices on every cell in the world for a view of empty margins. Fully zoomed out is therefore `Lens::at_rest`, which is where a window opens and what `--dump-frame` draws. |
| **The camera's left edge is *wrapped*, and the value exactly at the width is caught** | `camera.rs` | `f32::rem_euclid` of a value a hair below nought returns the modulus itself once the subtraction rounds. Group A measured exactly such a value - `x = -2.8e-16` out of `body_centre` without its wrap - and an origin of exactly 2048 in a 2048-wide world is the position this codebase is written on the assumption that nothing can hold. |
| **The pointer only moves the camera while a button is held** | `controls.rs` | The way "user-driven only" actually gets violated is not an animation somebody added on purpose - it is a window that pans whenever the pointer crosses it, so reaching across the screen for something else drags the view. A pointer that *leaves* the window while held is treated as having let go, for the same reason: otherwise the view jumps by however far the pointer travelled while it was away. |
| **⭐ The simulation gets 8 ms of each frame, and a watched run is therefore slower** | `window.rs` | Measured: 30,000 ticks headless is about 23 s and through a window about 45 s. Stated rather than hidden, because the alternative arrangements are worse - giving the simulation the whole frame makes the window answer a drag a quarter of a second late, and giving it a thread of its own means a mutex around the world and a snapshot copy per frame, which is Group D's problem if it ever becomes one. `--dump-frame` exists so that looking at what a run produced does not require watching it happen. |
| **⚠️ The event loop asks `Run::due()` instead of sleeping** | `run.rs` | `Run::wait` holds a capped run back by sleeping, which is right for a headless run with nothing else to do and wrong for a windowed one: at `max_ticks_per_second = 10` the event loop would be asleep for a tenth of a second at a time and the window would not answer the mouse. `due` reports the answer `wait` would have waited for, so the pacing is still decided in exactly one place. |
| **Present mode is `Fifo`** | `window.rs` | Two reasons and both are CLAUDE.md's *"visually calm"*. An unsynchronised present tears, and a tear is a line that flickers across the picture and pulls the eye; and without it the program would draw four hundred frames a second nobody sees, which on a second screen is a fan that never stops. |
| **The surface is asked for `Rgba8UnormSrgb` rather than for what it prefers** | `window.rs` | One pipeline, one format, one `cells.wgsl`. The alternative is a second pipeline built for the surface's format and kept in step with the first for ever, in exchange for nothing: `F12` has to read back exactly what was presented. If a machine ever refuses the format the window says so and the headless path is untouched. |
| **⭐ `--window --dump-frame` writes the frame the window was showing** | `main.rs`, `window.rs` | Not a picture of the whole world drawn afterwards - the view the window was left at, at the size it was left at, through the same call `F12` makes. This is the whole reason the resize bug above was found rather than shipped: it made the windowed path's own camera checkable from a session with no screen in it, and comparing that frame against the same tick dumped headlessly is what showed a quarter of the world where the whole of it should have been. |
| **The title bar is the only interface, and it reads in millions of years** | `window.rs` | CLAUDE.md's deep time: *"tick counts are displayed as millions of years"*. `Coacervate - 30.0 Ma, 1713 alive`, written only when the words change - a title rewritten sixty times a second is a taskbar entry that flickers. Everything else a person might want on the screen is Phase 6's, and this group deliberately binds one key so that Phase 6 has all the others. |
| **A windowed run prints the same closing report a headless one does** | `main.rs` | A window that closed leaving nothing behind but a PNG would be the one way of running this program that produced no record of what happened. |
| **`runs/<seconds-since-1970>-<seed>/frames/`, and nothing is created until `F12` is pressed** | `main.rs` | SPEC section 13 wants a readable timestamp and turning an instant into a date is civil-calendar arithmetic or a dependency. Phase 8 owns the run directory because it is the phase that has to write a replay log into it; the number sorts correctly in the meantime, which is the one property a directory listing needs. A run nobody photographs leaves nothing behind. |
| **⚠️ Screensaver mode is deferred, and the reason is that there is nothing to hide** | — | CLAUDE.md lists it under *Character of the thing* and the phase table puts it in Phase 10. A toggle added now would hide the window's entire interface, which is a title bar. The thing it exists to hide is Phase 6's panels. Carried below as **Q24**. |
| **⚠️ Opening a window is not tested, and no test pretends to** | `window.rs` | `EventLoop::new` wants a display and `run_app` does not return until the window closes, so a test that opened one would either hang the suite or put a window on somebody's screen in the middle of a check run. What is tested instead is everything either side of it: the camera arithmetic, the event mapping against winit's own `WindowEvent`s, and the tick loop against a stand-in. The window itself was checked by running the program and reading what it printed. |
| **A keyboard event cannot be built outside winit, so the key mapping is split** | `controls.rs` | `KeyEvent` carries a private platform-specific field. `gesture` handles mouse events - which *can* be built, and are, in the tests - and hands keys to `key(physical, state, repeat)`, which is fully reachable. What is left untested is one line that takes three fields out of a struct. |

##### ⭐ What the mutation checks found

Four, and the fourth was not a mutation at all - it was a real defect, found by the one check
this phase exists to make possible.

**`Lens::settle` was written without the wrap first, deliberately**, in the same spirit as A4's
naive mean. `panning_past_the_seam_comes_back_round` failed on the sentence that names the
fault: *"dragging the camera the whole width of the world did not bring it back to where it
started, so the seam is a wall"* — **left: -1501.7, right: 546.3**. A camera at x = -1501 in a
2048-wide world is not at a place.

**The zoom was then anchored on the middle of the frame instead of the pointer.** One test
failed, `zooming_is_anchored_on_the_pointer`: *"zooming in moved the world sideways under the
pointer: measured 1241.1 against 1313.0"* — 72 world units, about forty pixels, of the thing
being looked at sliding away per notch. The anchor in that test is deliberately **not** the
middle of the frame, because a camera zooming about its own centre keeps the centre fixed and
would have passed.

**`Controls::apply` was written panning on every pointer movement**, with no button state at
all. `moving_the_pointer_without_holding_anything_moves_nothing` failed on the first movement
it was given: *"the pointer moving to (0, 0) with no button held moved the camera"*. Nothing
else in the suite noticed, which is why that test is separate from the one that checks a drag
works.

**⭐⭐ And the one that was real.** The first frame ever dumped from the window was compared
against the same tick dumped headlessly - the same seed, the same tick, the same ledger to the
last digit - and they were different pictures. The headless frame had `founding.rs`'s eight
colonies in a four-by-two grid; the windowed one had two of them, half as far apart again and
twice the size. The cause was the resize rule in the decision table above, and the sequence that
triggers it is one Windows produces every single time a window is created. It had no symptom
anybody could have noticed from inside the program: the window drew a perfectly plausible
picture of a quarter of a world. `a_window_of_any_shape_shows_the_world_unstretched` now feeds
`Resized(2858, 1481)` and `Resized(1280, 720)` in that order and requires the view to come back
to the one it opened with; under the old rule it fails with *"resizing the window changed how
much world is on the screen: measured 1907.5 against 1271.6"*.

That is what *"a UI change is not complete until a frame has been dumped and looked at"* is
worth, stated as a number: without the dump, Group D would have been tuned against a camera that
was silently 2.25× too close.

#### ⭐ C6 — what a frame from the window looks like

`docs/frames/phase5-groupc.png` — 1280 × 720, world tick 30,000, seed 42, 1,713 organisms,
written by `--window --dump-frame` through the window's own camera at the size the window was.

**It is the same world Group B looked at, and that is the finding.** Sampled on a 64 × 36 grid
against the headless dump of the same tick, the two agree on **2,304 of 2,304 samples**. The
eight colonies are where `founding.rs` put them, the band of empty water between the shallow row
and the deep row is the full width of the world, the shallow colonies are several times the area
of the deep ones, and the hue is still confetti (**Q20**). Everything in B6's write-up holds and
none of it is repeated here.

What is *new* to say about it is about the window rather than the world.

- **The frame is smaller and nothing else changed.** At 1280 × 720 a world unit is 0.625 of a
  pixel against Group B's 0.94, so a two-celled body is about eight pixels across rather than
  twelve. Bodies are still legible as shapes, colonies still read as colonies, and the merging
  still works — but this is close to the floor. Below about 1000 pixels across, an organism is a
  coloured smudge, which is worth knowing before Group D tunes a bloom radius in pixels.
- **The interiors are still flat**, exactly as B6 said, and at this size it is *less* obvious
  rather than more, because a body is small enough to read as a dot. `D1`'s HDR target and bloom
  are still what that is waiting for.
- **A frame taken mid-run looks like a frame taken at the end**, which is the point. An actual
  `F12` press into an actual window produced `runs/<id>/frames/tick-0000014392.png` — not kept,
  because `/runs/` is ignored, which is the arrangement SPEC section 13 and `.gitignore` already
  agreed on. It shows the same eight colonies at a quarter of the population: the four shallow
  ones already several times the area of the four deep ones at tick 14,392, fifteen thousand
  ticks before the frame above. The depth gradient's dominance is visible almost from the
  beginning.
- **Watching it, the honest report is that very little happens.** At 60 frames a second and about
  650 ticks a second, a body crosses its own width in a few seconds and a colony changes shape
  over minutes. That is correct for what this is - CLAUDE.md's *"you come back hours later and
  read what happened"* - and it is also the strongest argument for `D2`'s motion trails, which
  are the one thing in Group D that would make the *movement* legible rather than the shapes.

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

**Q23** (new, Group C) — **a watched run is about half the speed of a headless one.** The event
loop gives the simulation 8 ms of each 16.7 ms frame and the display takes the rest, so 30,000
ticks costs about 45 s through a window against about 23 s headless. Nothing is wrong with that
for a program meant to be left running for twelve hours, and it is the reason `--dump-frame`
exists. The arrangement that would remove it is a simulation thread with the world behind a lock
and a `Scene` copied out of it once a frame - which is a copy per living cell per frame, a lock a
tick has to take, and a second answer to *"what tick is on the screen?"*. Not worth it until
something needs it. If Group D's motion trails make the frame rate matter more than the tick
rate, the cheaper move is to lower the budget rather than to add a thread.

**Q24** (new, Group C) — **screensaver mode was deferred and there is nothing to hide yet.**
CLAUDE.md lists it under *Character of the thing* and the phase table puts it in Phase 10. Group
C's entire interface is a title bar; a toggle that hid it would be a toggle for the window
decorations, which is not what the mode is for. It belongs with Phase 6's panels, which are the
thing it exists to take away — and Phase 6 should add it at the same time as the panels rather
than after, because a mode that hides chrome is much easier to keep working if it exists from
the first piece of chrome onwards.

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
yet), **Q17** (**half answered**: closing the window now stops a run gracefully, through the same
`Interrupt` Enter sets. `Ctrl-C` still kills the process where it stands, and still cannot be
caught without `unsafe` or a new dependency. It stops mattering the moment Phase 8 writes
anything to disk).
