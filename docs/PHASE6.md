# Phase 6 — panels, sliders, live charts

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *initial conditions settable; run controllable*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 6** | in progress |
| **Current group** | C — the charts (A and B are done) |
| **Suite** | green — 201 tests, ~120s |

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

### Group B — the sliders — **done**

Twenty-one settings a person can turn while a run is going, ten they cannot, a stop button, and
the ten numbers that decide what the picture looks like taken out of three files and put in one
record on the card. **Q26 and Q27 are both answered.**

- [x] **B1. `the_live_settings_can_be_changed_while_a_run_is_going`** — everything SPEC
  section 3 does *not* lock: `[light]`, `[physics]`, `[metabolism]`, `[mutation]`, twenty
  settings. `World::retune` walks the **five** subsystems that hold their own copy of a number
  rather than replacing the configuration the panel reads back — see the decision table, and the
  mutation check that names the fault exactly.
- [x] **B2. `the_locked_settings_cannot_be_changed_while_a_run_is_going`** — `[world]`,
  `[limits]` and `seed`. Shown, in a fold called `locked`; not editable, and asserted twice —
  `settings.rs` never builds a dial for one and `World::retune` **panics** if it is handed a
  configuration whose arenas are a different size, because *"the interface does not offer it"* is
  a promise about a screen and the arenas are a promise about memory.
- [x] **B3. `a_value_the_gate_refuses_is_not_applied`** ⭐⭐ — **both**, and the two are
  different claims. Every dial's range is inside what validation accepts, and
  `every_dial_reaches_both_of_its_ends` drives all twenty-one to both ends through the real gate;
  *and* every change goes through `RawConfig::validate` whole, so the bound is a convenience and
  the gate is the guarantee. `light.diffusion`'s far end is **`config::DIFFUSION_STABILITY_LIMIT`
  itself**, imported, not a copy of `0.25`.
- [x] **B4. `the_run_can_be_paused_and_stepped`** — `Space`, `→`, and two buttons. `Pace::allows`
  is asked **once per tick and not once per frame**, which is the whole of what makes a step one
  tick rather than eleven. `run.max_ticks_per_second` has a slider and reaches `Run::retune`.
- [x] **B5. `the_look_is_the_only_thing_that_decides_what_a_frame_looks_like`** ⭐⭐ — **Q26**,
  done once. Ten numbers from three files into `camera::Look`, a second uniform at 64 bytes.
  `PEAK` and `TONE_KNEE`'s `const` assertion survived **twice over**: unchanged over the
  defaults, and as `Look::sane`, asserted on every look that reaches the card.
- [x] **B6. Looked at.** `docs/frames/phase6-sliders.png`, five dump-look-adjust rounds, and one
  of them was a *window* rather than a dump. Written up below.

#### ⭐⭐ B5 — the frame moved by six bytes, and this is the line that moved it

The regression guard `B5` deserves is byte-identity, and it was taken: the shipped 30,000-tick
frame was dumped before the move and after it.

**Six bytes of 8,294,400 differ, each by exactly one, in six pixels of 2,073,600.** Then the cause
was pinned rather than guessed — the exponent in `water.wgsl`'s depth falloff was put back to a
literal, leaving every other one of the ten reading from the uniform, and the frame came back
**byte-for-byte identical** (`4d405853…`, the same md5 as before the move).

```wgsl
let daylight = pow(1.0 - depth, look.deepens);
```

Written `pow(1.0 - depth, 3.0)` the compiler folds a literal exponent of three into `x * x * x`;
written against a uniform it has to stay `exp2(deepens × log2(x))`, which is the same function to
within a unit in the last place and is not the same arithmetic. **Every other number in the move —
the peak, the glow, the bloom, the knee, the water's three colours, the shafts' lean, the trail's
fade — moved for nothing at all.** The literal was not kept: a `deepens` that cannot be moved is
not what Q26 asked for.

The permanent guard is `the_look_is_the_only_thing_that_decides_what_a_frame_looks_like`, which is
the same claim stated as a **round trip**: four of the ten are moved one at a time and each has to
change the frame, then the defaults are put back and the frame has to be the same bytes. A value
latched anywhere — in a pipeline, in the trail, in a shader's constant folding — survives the
journey back and shows up.

#### ⭐⭐ Q27 — how the input arrived without the dumped frame and the window parting company

Group A's condition, in its own words: *"an input path that only existed in the window would mean
the panel on a dumped frame and the panel on the screen were composed by two different routes, and
the frame would stop being evidence about the window."*

**The events go into the `RawInput` that `Chrome::compose` was already building.** `Chrome::feels`
pushes onto a queue; `compose` drains it into the `events` field of the input it has constructed
since Group A. There is no second path and no branch anywhere on whether there is a window. A
headless composition finds the queue empty and is otherwise the same call.

`controls.rs`'s `Controls::felt` is the forty lines that stand in for `egui-winit`, and **that
dependency is still not taken — now as a settled decision rather than a deferral.** Its entry
point is `State::on_window_event(&Window, …)`, and a `&winit::Window` is the one object a headless
dump has not got, so taking it would create exactly the two routes the paragraph above forbids.
What it does that this does not is IME, clipboard, accessibility and touch, none of which a panel
of sliders over a simulation has any use for.

⭐ **The evidence is a test that could not otherwise be written.**
`a_slider_answers_a_pointer_with_no_window_anywhere_near_it` drags `light.influx` from end to end
by pushing three `egui::Event`s and composing between them — the same three a window pushes,
through the same call — **with no window anywhere in it**, and asserts the setting moved and that
what came out is a `Config`, a type with no constructor but `RawConfig::validate`. The day
somebody moves the input into `window.rs`, that test stops compiling rather than stopping being
true.

**And the pointer knows the panel is there.** `Chrome::wants_pointer` is egui's own answer read off
the last composition; `Controls::apply` takes it and refuses a *grab* that starts over the chrome.
⚠️ It is the grab and not every event, and the difference is one a hand notices: refusing while
the pointer is over the panel makes a pan begun on open water stop dead half way across and start
again on the far side, which is the sudden camera move CLAUDE.md forbids.
`a_pointer_over_a_panel_does_not_move_the_camera` states all three claims.

#### What Group B decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐⭐ A change is made to a *copy*, validated whole, and kept only if the gate accepted** | `settings.rs` | `B3` in eleven lines. `config.rs` calls itself *"the only gate… there is no other way to obtain a `Config`"*, and a slider is a new way for a number to arrive that **looks like a widget rather than like a document being edited** — which is what makes it easy to write one that assigns straight into the running configuration. `Dials` holds the unchecked `RawConfig` and the only way to change it is `Dials::set`. Nothing anywhere can produce a `Config` by another route, so nothing can go round it. |
| **⭐⭐ `light.diffusion`'s slider stops at the gate's own constant, imported** | `settings.rs`, `config.rs` | `DIFFUSION_STABILITY_LIMIT` was private and is now public for this. It is the one bound in SPEC section 3 where being wrong is **silent** — above a quarter the stencil overshoots, the field grows without limit, and the energy ledger goes on reporting a healthy world the whole way down, because overshoot moves energy rather than inventing it. A `0.25` written out a second time in a file `config.rs` knows nothing about is one edit from being wrong. |
| **⭐ Both a bound *and* the gate, and they are different claims** | `settings.rs` | The bounds are a **convenience**: a slider whose far end the gate refuses is a slider that fights the hand on it. The gate is the **guarantee**. `every_dial_reaches_both_of_its_ends` is what keeps them in step, and it is a test about two files agreeing, which is the only way that claim can be stated. |
| **⭐⭐ `World::retune` walks five subsystems, and the tick reads none of them from `Config`** | `world.rs` | `grid.rs` precomputes every tile's ceiling and every row's regrowth; `physics.rs` and `metabolism.rs` hold their numbers widened; `reproduction.rs` keeps the whole `[mutation]` table — because reading a configuration inside a loop over a quarter of a million cells is not what a configuration is for. A retune that only replaced `World::config` moves **the number the panel reads back** and leaves every number the simulation charges where it was: a world reporting weather it is not having. |
| **⚠️ A retune leaves the tiles holding whatever they were holding** | `grid.rs` | A ceiling lowered under a full field puts every tile above its target, and SPEC section 4 already has an answer: `Grid::spill` moves the excess `field → dissipated` at the end of the tick it is noticed on. Clamping the tiles instead would destroy that energy with no account to put it in, which is the exact failure section 4's opening paragraphs are about. |
| **⚠️ The blotch *pattern* does not move when the light does** | `grid.rs` | `PatchNoise` is seeded from `world.seed`, which is locked, so turning `patchiness` up changes how deep the blotches are and not where they are. A retune that re-drew the pattern would rearrange the whole field under a living population. |
| **⭐ Refusing a locked setting is a panic, not a returned error** | `world.rs` | CLAUDE.md: *"invariants are asserted at runtime, not just in tests"*, and the arenas are the memory guarantee. `panel.rs` never offers one, so reaching `World::retune` with a different `[limits]` is a program that has gone wrong rather than a person who typed something. |
| **⭐ `Look` is a record *beside* `View`, and `View`'s two look-ish fields came out of it** | `camera.rs` | Q26 asked for the constants to go *into* `View`. What they went into is a second uniform, and `glow` and `peak` left `View` to join them — so the split is now the honest one. `View` says where the world is; `Look` says how it is drawn. A slider belongs to exactly one of them and there is no third place for a number about the picture to live. `View` is still 48 bytes; `Look` is 64. |
| **⭐ The `PEAK`/`TONE_KNEE` guard is a `const` block **and** a runtime assert** | `camera.rs` | The `const` block still holds, unchanged, over the two **defaults**. `Look::sane` is the form a slider left it in, and `Renderer::looks` calls it on every look that reaches the card — so there is no route that does not pass it. The panel goes further and never *offers* the failure: the peak's slider stops at half the knee, and there is no knee slider at all. |
| **⭐ A change of look throws the motion trail away** | `frame.rs` | The same rule `frame.rs` already keeps about a camera that moved, applied to the other reason a frame's history stops being about the same picture: an accumulation buffer holding several frames drawn the old way would leave the old look decaying on the screen for a second and a half. It is also what makes B5's round trip a *byte* comparison rather than a nearly-the-same one. |
| **⚠️ `min_binding_size` is stated on both uniform layouts** | `frame.rs` | Left unstated the two are the same layout — one buffer, binding nought, both stages — and wgpu is entitled to hold the *view*'s buffer to the size the *look*'s shader asked for. It does, and the message is a good one: *"the buffer bound at binding index 0 is bound with size 48 where the shader expects 64"*. Naming the size makes them different things and gets each buffer checked against the struct it is the bytes of. |
| **⚠️ `View`'s padding is three scalars in WGSL and not a `vec3<f32>`** | `cells.wgsl` | WGSL gives a `vec3` an alignment of sixteen, so one placed after four `vec2`s and an `f32` starts at byte 48 rather than 36 and takes the record to 64 — while `camera.rs`'s `[f32; 3]` leaves it at 48. Two different records with the same name, caught at pipeline creation by the line above rather than in the picture. |
| **⭐ Only the *first* settle pass is given the input** | `panel.rs` | A settle pass is the same composition asked for again, so an event handed to two of them happens twice — and both things on this panel that answer a click break in ways nobody would look for. A fold would open and close inside one frame, so clicking `light` would appear to do nothing; a button would ask twice, so the pause button would toggle the run back to where it was. Neither shows up in the source. |
| **⭐ A composition is done when the chrome comes out where it came out *last frame*** | `panel.rs` | Group A's loop stopped as soon as anything tessellated, which was enough for a list of numbers. An `egui::ScrollArea` sizes itself from what it held last time, so a panel with one in it is nine points shorter on its first frame than on its second — one frame in a window and nobody would ever see it, and **the whole picture on a dumped frame**. `SETTLES` is a bound of eight and the steady case pays one pass. |
| **⭐ Every fold is closed but `[light]`** | `panel.rs` | Thirty-one rows of widgets over a picture of water is a control surface with a simulation behind it, and no amount of quietness per row fixes that. The default is the run's controls and six closed folds — eight rows — and the labels are SPEC section 3's own table names, so anybody who has read the configuration file knows which to open. `[light]` opens by itself for the reason SPEC gives about it in as many words: *"`influx` is the single most consequential slider"*. |
| **⭐ The controls take at most two-fifths of the frame's height** | `panel.rs` | Measured on a **window**, not on a dump. The panel is a fixed size in egui's *points* and a point is 1.5 pixels on this machine's display, so the panel that is 4.9% of a 1920 × 1080 dumped frame is a quarter of a 1280 × 720 window. Bounded by the room left over it would grow until it ran out. |
| **The run's controls are not behind a fold** | `panel.rs` | Pausing is not a setting. `B4` is the pair of things somebody reaches for while *watching*, and something that answers a question about the moment cannot be two clicks away. |
| **⚠️ No key reaches egui** | `controls.rs` | egui is perfectly willing to swallow a keystroke that lands on a focused widget, and `S` or `Space` quietly not working while the pointer happened to be over the panel is exactly the sort of fault nobody reports and everybody works around. Only the pointer and the wheel are translated. |
| **⭐ A key and a button produce the same `Ask`** | `controls.rs`, `panel.rs` | So `Space` and the panel's *pause* button cannot become two implementations of pausing that disagree about whether the run is stopped. `Watcher::asked` is the one place either of them is acted on. |
| **`[run]`'s other three settings have no slider** | `settings.rs` | `max_ticks_per_second` is what `B4` asks for by name and is the only thing in `[run]` about how a run is *watched*. `max_wall_clock_hours` and `max_ticks` are the terms this run was **started** under — `--ticks` overrides one on the command line and `--dump-frame` supplies the other — and a deadline that could be dragged forward mid-run is a run whose closing report cannot say what bounded it. `reseed_on_extinction` still does nothing (Q16). |
| **⭐ `args::Settings` keeps the parsed `RawConfig`** | `args.rs` | The panel edits a transcript, so it has to start from one. The alternative is to widen a `Config` back into a `RawConfig` field by field — but narrowing is lossy by design, so a document rebuilt that way reads `0.0010000000474974513` where the file said `0.001`, and the panel would open showing numbers nobody typed. |

##### ⭐ What the mutation checks found

**`World::retune` was written to replace only `self.config` first** — the field the panel reads
back — which is the shape a retune naturally takes and is wrong. The test failed on the number
that names it: *"two hundred ticks cost the world at upkeep_scale 1.0 **2.440000112983398** and the
same world at 4.0 **2.440000112983398**, so raising the temperature mid-run changed what the panel
says and not what living costs — which means `metabolism.rs` is still charging the old number."*
Identical to the last digit, which is what makes it unambiguous.

**Then `grid.relight` was left out and the other four kept.** A different test line failed, and it
is the one about the tables a tick never recomputes: *"the ceiling was lowered from 8.0 to 1.0 and
the field went from **184030.34989988804** to **184030.34990680218**, so `light.cap` did not reach
the tile targets it decides."* Twelve thousand ticks of light later, the field is where it was.

**And `Dials::set` was written to assign before validating.** *"`light.diffusion` was refused and
the document kept the refused value: it reads 0.5 and was 0.04."* That is `B3` failing in the exact
way `B3` exists to prevent — the gate said no, the sentence came back, and the world took the
number anyway.

##### ⭐⭐ And one that was real, found by a test rather than by a mutation

**A panel with a scroll area in it is a different size on its second frame than on its first.**
`a_panel_appears_on_the_first_frame_it_is_asked_for` is Group A's and it failed the moment the
controls landed: *"the panel was [12, 12, 230, 267] on its first frame and moved on frame 1"*.
Measured, on the first composition of a run:

| Pass | Where the chrome came out | Tessellated |
| --- | --- | --- |
| 0 | `[0, 0] – [230, 169]` | **nothing** — the font atlas is created during this pass |
| 1 | `[12, 12] – [242, 279]` | laid out, but the scroll area has not measured its own contents |
| 2 | `[12, 12] – [242, 288]` | it has now |
| 3 | the same | settled |

⚠️ **This is Group A's finding happening a second time for a second reason**, and it fails the same
way: in a window it is one frame at sixty a second and nobody would ever see it, and on a *dumped*
frame — the one instrument this project has for judging visual work — it is the whole picture. The
loop's condition is now that the chrome came out **where it came out last time** rather than that
something came out at all.

#### ⭐⭐ B6 — what the panel with sliders on it actually looks like

`docs/frames/phase6-sliders.png` — 1920 × 1080, world tick 30,000, seed 42, **1,713 alive, mean
genome 1.70 ± 0.85**. The same figures Groups B, C and D of Phase 5 and Group A of this phase
measured: the sliders moved nothing, and neither did `B5`.

**Five dump-look-adjust rounds, and the fourth was a window rather than a dump.**

| Round | What the frame said | What changed |
| --- | --- | --- |
| 1 | Every slider's own number sits in a **filled box**, and twelve of those stacked down the panel are twelve pale chips — easily the loudest thing in the picture, brighter than the readings panel above them | `weak_bg_fill` down. And the `pause` button was read as bright at 3× and turned out at 8× to be a dark box with light text — a reminder that a magnified crop is the instrument, not the thumbnail |
| 2 | The chips are *still* there and are now **darker than the panel**: `FILL` is 85% opaque and painting it over a panel of the same colour makes a hole rather than a tile | Transparent. And the handles are the brightest things left, at the readings' own colour, marking nothing anybody is reading |
| 3 | Legible and quiet — but the numerals do not line up. `0` and `0.0010` end four pixels apart, because egui **centres** a slider's value in a box it sizes to the contents | A fixed six-character column, padded with *leading* spaces. ⚠️ Group A's note is about trailing whitespace, which epaint trims; leading it keeps |
| 4 | ⭐ **A window, at 1280 × 720 on a 150% display.** The panel is a quarter of the frame — the same panel that is 4.9% of a dumped frame, because a point is 1.5 pixels | A ceiling: the controls take at most two-fifths of the frame's height, whatever is left over |
| 4b | With every fold open, `offspring_share0.45` came out as one word with a number stuck to it, and the scroll clip cut a row in half with nothing to say why | Nine labels shortened to twelve characters, dropping the word the fold already says; a solid four-point scroll bar instead of egui's floating one |
| 5 | Shipped | |

**The honest report.**

- ⭐ **It is still recessive, and the register survived twenty-one sliders.** At a glance the frame
  is the world: eight colonies of coloured light on near-black water, and a narrow dark column in
  the top-left corner. The two panels are 230 × 443 pixels of 2,073,600 — **4.9% of the frame**,
  against Group A's 2% — and nothing on either of them is brighter than a colony. The brightest
  thing in the picture is still something that is alive.
- ⭐ **The two panels read as one column rather than as two objects**, which is what the six-point
  gap and the shared width are for. The eye goes to the readings, which is right: they are what
  changes.
- **A slider is a dim groove with a dim tick on it.** The filled part of each rail says at a glance
  where a setting sits in its range — `gradient` is visibly three-quarters along, `influx` visibly
  at the bottom of its — which is the one thing a slider is better at than a number, and it costs
  nothing brighter than the panel's own border.
- ⭐ **The typography is the best part, again, and it is Group A's extended.** Dim labels left,
  bright numerals in one column, and every numeral on the controls panel ends on the same pixel —
  including the integer `ticks/s`, which was the round-three fault. It reads as a column of figures
  rather than as a list of widgets.
- **The folds do their job.** Five closed lines — `physics`, `metabolism`, `mutation`, `view`,
  `locked` — under five open ones. Sixteen settings and ten readings are one click away and take
  five rows until somebody wants them.
- ⚠️ **The honest criticism is what a window looks like at 150% scaling.** On the dumped frame the
  chrome is 4.9%; in the 1280 × 720 window this program opens, on a display at 150%, it is about
  **22%** — because the panel is a fixed size in points and the window is small. The ceiling stops
  it being worse and the scroll bar says there is more, but a person who leaves the window at the
  size it opens at is looking at a fifth of their picture being chrome. Dragging the window bigger
  fixes it, and `S` takes all of it away, but it is worth writing down that this panel is sized for
  a frame rather than for a window.
- ⚠️ **The second honest criticism is the same one Group A made**: the panel is in the corner where
  this world's densest colony is, and it is now twice as tall. There is no arrangement that avoids
  it, and a panel that moved would be far worse than one that overlaps.
- **Nothing moves.** `animation_time` is nought, `fade_in` is off, and the settle loop now stops on
  a *stable* rectangle rather than on a non-empty one — so two dumps of the same tick are the same
  file, and a panel does not change size on its second frame.

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

**Q29** (new, Group B) — ⚠️ **the panel is sized for a frame and this program opens a window.**
See `B6`. The chrome is 4.9% of a 1920 × 1080 dumped frame and about **22%** of the 1280 × 720
window this program opens, on a display at 150% — because everything on it is a fixed size in
egui's *points* and a point is 1.5 pixels there. `CEILING` bounds it at two-fifths of the frame's
height so it can never be worse than that, and `S` takes all of it away, but a person who leaves
the window at the size it opens at is looking at a fifth of their picture being chrome. Three ways
out and none of them is Group B's: open the window larger; scale the chrome down when the frame is
small, which means a second font size and a second set of measurements; or make the readings panel
fold too, which Group C's charts will make more pressing rather than less. **Whatever Group C adds
goes below both of these**, so this is the question to answer before it does.

**Q28** (Group A) — **still open, and re-checked at the top of Group B as that question asked.**
`egui-wgpu`'s newest published version is **still 0.35.0**, still naming `wgpu ^29`, so the
version matrix above is unchanged and `panel.rs`'s `Painter` stays. It cost Group B nothing: the
sliders needed a texture atlas and a triangle list, which is what that backend already draws, and
**not one line of it changed**. Ask again before Group C.

**Q27** (Group A) — **answered.** The events go into the `egui::RawInput` that `Chrome::compose`
was already building, so there is still exactly one composition route and a headless dump simply
finds the queue empty; `egui-winit` is still absent, now settled rather than deferred, because its
entry point takes a `&winit::Window` and a dump has not got one. `Controls::felt` is the forty
lines that stand in for it. The property is stated as a test that could not otherwise exist —
`a_slider_answers_a_pointer_with_no_window_anywhere_near_it` drives a slider end to end with no
window in the program at all. And the pointer knows the panel is there: a **grab** that starts over
the chrome never reaches the camera, while a drag begun on open water goes on panning across the
panel rather than stopping dead half way. See the write-up above.

**Q24** (Group C) — **answered.** Screensaver mode is `S`, it arrived with the first panel
exactly as that question asked, and the mechanism is the thing that was actually at stake:
`Chrome::compose` checks the switch above every widget in the program, so no later panel has to
remember. Stated as a measurement rather than an inspection — a frame in screensaver mode is
byte-for-byte identical to one drawn by a build with no chrome in it.

**Q3**, **Q5**, **Q6**, **Q8**, **Q9**, **Q12**, **Q16** (`reseed_on_extinction` still does
nothing), **Q18** (`--seed`/`--ticks` are not reflected in the kept config document),
**Q19** (the dense cell list is one tick behind), **Q20** *(resolved in Phase 5 Group D)*,
**Q21** (a crowd of similar bodies reads as one animal), **Q23** (a watched run is about half
speed), **Q25** (motion trails read as a softening, not a tail — ⭐ **and the fade is now on a
slider**, in the `view` fold, so the two ends that note describes can be looked at rather than
imagined), **Q26** *(resolved in Group B's `B5` — see the six bytes above)*.
