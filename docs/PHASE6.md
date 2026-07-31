# Phase 6 — panels, sliders, live charts

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *initial conditions settable; run controllable*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 6** | in progress |
| **Current group** | B — the sliders (A is done) |
| **Suite** | green — 186 tests, ~110s |

---

## What this phase is for

Everything so far has been settable only by editing a TOML file and starting again. SPEC
section 3 says *"every one of these is exposed as a slider in the UI"*, and that
`[world]`, `[limits]` and `seed` lock at run start while **the rest can be changed live —
which is how environmental events work**. That last clause is the point of the phase: raising
`upkeep_scale` mid-run is not a settings change, it is the weather turning.

CLAUDE.md is equally clear about the register:

> `egui` panels sit over the world: translucent dark, thin borders, monospace numerics,
> recessive. **The simulation is the subject; the chrome should nearly disappear.**

---

## Step ledger

### Group A — the first panel, and the mode that hides it — **done**

- [x] **A1. `egui_draws_over_the_world_without_clearing_it`** — the chrome is a sixth pass on
  the same target with `LoadOp::Load`, between the composite and the present. Asserted the
  strong way: **every pixel outside the panel's own rectangle is byte-identical** to the same
  frame drawn with no chrome at all.
- [x] **A2. `the_panel_reports_what_the_world_is_doing`** — population, the five ledger
  accounts, mean body size, mean genome length, and the tick in **millions of years**. Every
  figure is `Census::of(world)`'s or the ledger's own, because `census.rs` **moved into
  `coacervate-render`** rather than being copied. See the decision table.
- [x] **A3. `screensaver_mode_hides_every_panel`** — **`S`**. ⭐ Stated as a measurement:
  a frame taken in screensaver mode is **byte-for-byte identical** to one rendered by a
  program with no `Chrome` in it. **Q24 is answered.**
- [x] **A4. Looked at.** `docs/frames/phase6-panel.png`, three dump-look-adjust rounds.
  Written up below.

#### ⚠️ `egui-wgpu` cannot be used, and this is the version matrix

Checked before anything was built, and it is the finding of the group:

| Crate | Newest published | Requires |
| --- | --- | --- |
| `egui` | **0.35.0** | nothing about a graphics card |
| `egui-winit` | 0.35.0 | `winit 0.30.13` — **exactly what this workspace pins** |
| `egui-wgpu` | 0.35.0 | **`wgpu ^29.0`** |
| `wgpu` | — | **30.0.0**, which is what this renderer is written against |

`^29.0` excludes 30. Cargo would resolve *both*, and the types do not unify:
`egui_wgpu::Renderer::render` takes a wgpu-29 `RenderPass` and every pass in `frame.rs` is a
wgpu-30 one. A type error, not a warning.

Moving `wgpu` back to 29 under a working renderer was refused: `CurrentSurfaceTexture`,
`Queue::present`, `SurfaceColorSpace`, `multiview_mask`, `depth_slice` and
`PollType::wait_indefinitely` are all wgpu 30, so it is a rewrite of Phase 5 rather than a
version bump. **So only `egui` was taken**, and `panel.rs` paints its output with the `wgpu`
this project already has — about two hundred lines, because what egui asks a backend for is a
triangle list, a scissor rectangle per batch and one texture atlas. **Delete it and take the
dependency the day `egui-wgpu` names `wgpu 30`.**

`egui-winit` was *not* taken either, and that is a separate decision — see the table below.

#### What Group A decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐ `census.rs` moved from `coacervate-app` into `coacervate-render`** | `census.rs` | The panel wants the same six numbers the progress line prints, and a panel cannot reach into the binary that draws it. The alternative was to compute them twice, which is the arrangement that module's own opening paragraph argues against: *"keeping two copies of one quantity in step is the bookkeeping that goes wrong."* A panel reading 1,712 while the progress line read 1,713 is a bug nobody could see and everybody would half-believe. Nothing about the numbers changed; `coacervate-app` imports them. |
| **⭐ `millions_of_years` is one function, and there were already two copies of it** | `census.rs` | Three things show a person how far a run has got — the progress line, the window's title bar and now the panel — and the first two each did `ticks × years_per_tick / 1e6` themselves. A third copy is how they end up disagreeing about what year it is. |
| **⚠️ `egui-winit` is deliberately absent until Group B** | `panel.rs` | Two reasons. Nothing in Group A answers a pointer or a key that egui knows about, so there is no input to translate; and **a headless dump has no winit window**, so an input path that existed only in the window would compose the panel by a different route from the one a dumped frame goes through — and the frame would stop being evidence about the window, which is the property `frame.rs` spends its whole design on. `Chrome::compose` builds its own `RawInput`. The honest cost: dragging across the panel pans the camera underneath it. Carried below as **Q27**. |
| **⚠️ A dumped frame has *no* panel on it unless `--panel` asks** | `lib.rs`, `args.rs` | CLAUDE.md gives frame dumping one job — *"Claude reads those PNGs directly to see what it has built"* — and every measurement in `frame.rs` is taken off a frame drawn that way. A panel over the top-left corner of all of them would be a permanent blind spot in the one instrument this project has for looking at itself. A frame is a picture of the world. |
| **A frame dumped from a *window* does have the panel on it** | `window.rs` | The other way round, and for `docs/PHASE5.md`'s C6 reason: a windowed dump is a photograph of the window, at the view and the size it was left at. The chrome is part of what was on the screen. A frame taken in screensaver mode therefore has none of it, which is how somebody keeps a picture rather than a screenshot. |
| **⭐ The switch is checked in exactly one place, above every widget** | `panel.rs` | This is the whole of Q24's argument made real. `Chrome::compose` returns before it builds anything at all, so **a panel added in Group B or C is hidden by a line written in Group A** and nothing downstream has an `if` in it to forget. |
| **`S`, not `F11`** | `controls.rs` | `S` for screensaver is the only mnemonic there is. `F11` means *full screen* everywhere else on Windows and this mode is not that: the window stays the size it was and only the chrome goes. |
| **The title bar stays in screensaver mode** | `window.rs` | The mode hides the *program's* chrome. A window's title bar belongs to the desktop, and a window with no name in the taskbar is harder to find rather than calmer. |
| **⭐ `animation_time` is nought** | `panel.rs` | Two constraints at once. CLAUDE.md's *"nothing that pulls the eye"* — a panel whose contents fade in is a panel that moves — and reproducibility: an animating widget draws differently on its second frame than its first, and A3 compares two frames byte for byte. `Area::fade_in(false)` is the same decision at the container level. |
| **The chrome is the one non-additive blend in the crate** | `panel.rs` | Everything else here draws light, which adds. A panel is an object in front of the picture, which covers. |
| **The atlas is `Rgba8Unorm` and the shader undoes sRGB** | `panel.wgsl` | egui works in sRGB values throughout and `frame.rs`'s target is `Rgba8UnormSrgb`, so a colour handed straight through would be encoded twice and every panel would come out washed pale. |

##### ⭐ What the mutation checks found

**A1 was written with `LoadOp::Clear` first, deliberately.** One test failed, and it failed on
the pixel that names the fault: *"the chrome changed the frame outside its own panel at (0, 0):
`[45, 68, 83, 255]` became `[0, 0, 0, 255]`… so the egui pass is not loading the picture it is
drawing over, and the world it is supposed to be sitting on top of is being erased."* That is
the water at the corner of the frame turning black. `screensaver_mode_hides_every_panel`
**passed** under that mutation, which is the point of the two being separate: with the chrome
hidden no pass is submitted at all, so there is nothing to clear.

**Then screensaver mode was written without its early return.** Two tests failed, and the
second gives the number: *"screensaver mode left **146,284 bytes of 589,824** different from a
frame drawn with no chrome in the program at all, so the mode dims or ghosts the interface
rather than removing it."*

##### ⭐⭐ And the one that was real, found before it could ship

**egui's very first pass over a fresh `Context` draws nothing at all.** Measured on the first
version of `Chrome::compose` written:

| Pass | Area rectangle | Tessellated | Atlas changes |
| --- | --- | --- | --- |
| 0 | `[0, 0] – [230, 47]` | **0 indices** | 1 |
| 1 | `[12, 12] – [242, 59]` | 474 indices | 0 |
| 2 | `[12, 12] – [242, 59]` | 474 indices | 0 |

The font atlas is *created* during pass 0, so there are no glyphs to lay the text out with and
the area comes back at the wrong place and half the right height. A `compose` that ran once and
painted therefore drew nothing, and `egui_draws_over_the_world_without_clearing_it` failed with
*"the panel drew on 0 of the 38,870 pixels of its own rectangle."*

⚠️ **A window would never have shown this.** A window composes sixty times a second and only the
first of those is empty, so the panel would have appeared instantly and correctly on the screen
while **every frame this project dumps to disk came out with no chrome on it** — silently, and
in the one instrument CLAUDE.md provides for judging visual work. `Chrome::compose` now asks
again when the tessellator hands back nothing, bounded at `SETTLES = 2`, and
`a_panel_appears_on_the_first_frame_it_is_asked_for` holds it there.

⚠️ The condition is on the **tessellated** output and not on `output.shapes`. egui hands back
shapes on that first pass and they tessellate to nothing, which is a difference worth an hour.

##### ⚠️ And one thing this group broke that it did not write

`window.rs`'s `a_run_that_ends_on_its_own_closes_the_window` is Phase 5's, and it started
failing intermittently on the first check run after this group landed: *"a run with three ticks
left in it did not report itself over inside one frame."* Nothing about it changed. What changed
is that `coacervate-render` gained five tests, three of which build render pipelines and block
on the card from other threads — and that test was quietly asserting that three cost-free ticks
fit inside eight milliseconds of **wall clock**, which they do on an idle machine and did not on
a loaded one. Its claim was never about a clock, so it now gets a budget nothing can exhaust.
`the_simulation_and_the_display_run_at_their_own_speeds`, where the budget *is* the claim, keeps
the real one.

#### ⭐ A4 — what the frame with the panel on it actually looks like

`docs/frames/phase6-panel.png` — 1920 × 1080, world tick 30,000, seed 42, **1,713 alive, mean
genome 1.70 ± 0.85**. The same figures Groups B, C and D of Phase 5 measured, to the last digit:
the chrome moved nothing.

**Three dump-look-adjust rounds**, and two of the three found something the source could not
show:

| Round | What the frame said | What changed |
| --- | --- | --- |
| 1 | The panel lands on the shallowest, brightest colony in this world, and at two-thirds opacity **the magenta bodies behind it are the dominant thing inside its own rectangle**. A label in the middle of it is unreadable and the eye goes to the chrome, because the chrome is where two pictures are fighting | Fill from `α = 170` to `α = 217` |
| 2 | Legible — but the numerals do not line up. `30.0 Ma` sits three characters right of every other number | The unit column is a *reserved width*, not a padded string: epaint trims the trailing spaces off `"Ma   "` and lays it out two characters wide, while `"     "` keeps all five |
| 3 | Every numeral in the panel ends on the same pixel | Shipped |

**The honest report.**

- ⭐ **It is recessive, and the third round is what made it so.** At a glance the frame is the
  world: eight colonies of coloured light on near-black water, and a small dark rectangle in the
  top-left corner that reads as a shadow until you look at it. It occupies 230 × 214 pixels of
  2,073,600 — **about 2% of the frame** — and nothing on it is brighter than a colony. The
  brightest thing in the picture is still something that is alive.
- **The world passes under it, and that is the best thing about it.** The magenta colony behind
  the panel is clearly *there* and clearly *behind*: a soft ghost at about a sixth strength, with
  the bloom's halos still legible as halos. It reads as smoked glass rather than as a hole cut in
  the picture, which is exactly what SPEC's *translucent dark* is asking for.
- **The border is at the edge of visible**, which is what *thin borders* should mean. On the full
  frame it is a slightly-lighter hairline; magnified three times it is a clean one-pixel edge.
  The rule between the population and the ledger is the same colour and reads as part of the
  frame rather than as a divider.
- **The typography works better than expected.** Everything is monospace at 11 points: dim labels
  left, bright numerals right-aligned, dim units in a fixed column beyond them. The eye goes to
  the numerals because they are the only bright thing, and the five ledger accounts read as a
  column of figures rather than as a list of sentences. This is the one part of the panel that is
  genuinely *nice* rather than merely quiet.
- ⚠️ **The honest criticism is the corner it is in.** Top-left is conventional and it is also,
  in this world at this tick, where the densest colony is. There is no arrangement that avoids
  that — the world is different every run and the panel cannot follow it around, and a panel that
  moved would be far worse than one that overlaps. But it is worth saying that the frame would be
  calmer with the panel over empty water, and that this frame does not get to have that.
- **Nothing moves.** `animation_time` is nought and `fade_in` is off, so two dumps of the same
  tick are the same file. A second of watching changes nothing on the panel except the numbers
  that changed in the world.

### Group B — the sliders

- [ ] **B1. `the_live_settings_can_be_changed_while_a_run_is_going`** — everything SPEC
  section 3 does *not* lock: `[light]`, `[physics]`, `[metabolism]`, `[mutation]`.
- [ ] **B2. `the_locked_settings_are_shown_but_cannot_be_changed`** — `[world]`, `[limits]`
  and `seed`. Shown, because a run should be able to say what it is; not editable, because
  every arena in the world was sized from them.
- [ ] **B3. `a_changed_setting_is_still_validated`** — the config gate is the only thing
  standing between a typed number and a broken world, and a slider must not go round it.
  ⚠️ `light.diffusion` above 0.25 makes the field diverge *while the energy ledger reports
  a perfectly healthy world*.
- [ ] **B4. `the_run_can_be_paused_and_stepped`** — and `max_ticks_per_second` is reachable,
  which is what the `slow` profile is made of.
- [ ] **B5. `the_render_constants_are_uniforms_not_constants`** — ⚠️ **Q26**: bloom strength
  and radius, trail fade, `PEAK`, `TONE_KNEE`, the water colour and gradient, shaft strength
  are all compiled in. Move them **once, together**, rather than one at a time as each gets a
  slider. The `const` assertion tying `PEAK` and `TONE_KNEE` (that a two-celled body does not
  compress) must survive the move.

### Group C — the charts

- [ ] **C1. `a_time_series_is_recorded_as_the_run_goes`** — ⚠️ nothing like this exists;
  `Census::of(world)` is a snapshot and `stats.bin` is Phase 8's. SPEC section 13 says these
  records are small and fixed-size *"so the whole run's time-series always fits in memory for
  charting"* — build the in-memory half now, in a shape Phase 8 can write to disk unchanged.
- [ ] **C2. `the_charts_show_population_biomass_and_the_ledger_over_time`**
- [ ] **C3. `the_series_is_bounded`** — an overnight run is tens of millions of ticks. A
  record every 100 ticks unbounded is not a chart, it is a leak. Decide the bound and say so.
- [ ] **C4. Look at a frame with the charts in it.**

---

## Constraints that outrank features

- **Visually calm, and recessive.** CLAUDE.md: the chrome should nearly disappear. If a
  choice is between informative and quiet, quiet wins — the log is where drama belongs.
- **Never steals focus.** Phase 5 measured this; do not regress it.
- **`coacervate-sim` must not learn that a UI exists.** Panels read; they do not get hooks.
- **A slider must not bypass validation.** See B3.

---

## Open questions carried forward

**Q28** (new, Group A) — **`egui-wgpu` is a dependency this project wants and cannot have.**
See the version matrix above: the newest `egui-wgpu` names `wgpu ^29` and this renderer is
`wgpu 30`. `panel.rs`'s `Painter` is the two hundred lines that stand in for it, and it is
deliberately shaped so that it is the *only* thing that would be deleted: nothing else in the
crate touches egui's tessellated output. The check to make before Group B is simply whether
`egui-wgpu` has published a version naming `wgpu 30` — and if it has, taking it is a strictly
better position than keeping a hand-written backend, because glyph rasterisation and atlas
management are exactly the sort of thing that is fine until it is not.

**Q27** (new, Group A) — **the pointer does not know the panel is there.** Dragging across the
panel pans the camera underneath it, because Group A has no input path: `egui-winit` was not
taken, for the reasons in the decision table. Nothing in Group A is interactive so nothing is
*wrong* today, but a person who tries to drag the panel gets the world instead, which is a
surprise. Group B is where it is fixed, because a slider is the first thing that has to answer a
pointer at all — and the fix has to keep `Chrome::compose` as the single composition route that
both the window and `--dump-frame` go through, which means the input goes *into* `RawInput`
rather than a second path being added beside it.

**Q24** (Group C) — **answered.** Screensaver mode is `S`, it arrived with the first panel
exactly as that question asked, and the mechanism is the thing that was actually at stake:
`Chrome::compose` checks the switch above every widget in the program, so no later panel has to
remember. Stated as a measurement rather than an inspection — a frame in screensaver mode is
byte-for-byte identical to one drawn by a build with no chrome in it.

**Q3**, **Q5**, **Q6**, **Q8**, **Q9**, **Q12**, **Q16** (`reseed_on_extinction` still does
nothing), **Q18** (`--seed`/`--ticks` are not reflected in the kept config document),
**Q19** (the dense cell list is one tick behind), **Q20** *(resolved in Phase 5 Group D)*,
**Q21** (a crowd of similar bodies reads as one animal), **Q23** (a watched run is about half
speed), **Q25** (motion trails read as a softening, not a tail), **Q26** (the render constants
are compiled in — Group B's `B5`).
