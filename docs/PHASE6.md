# Phase 6 — panels, sliders, live charts

**Working ledger.** Same contract as the earlier phases.

**Done when** (CLAUDE.md's phase table): *initial conditions settable; run controllable*, and
`.\scripts\check.ps1` exits 0.

---

## Status

| | |
| --- | --- |
| **Phase 6** | **done** |
| **Current group** | — (A, B and C are done; `Q29` answered) |
| **Suite** | green — **207 tests, 113s** |

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

### Group C — the charts, and `Q29` — **done**

Three sparklines under the sliders, a bounded in-memory `stats.bin` behind them, and the panel
resized so that the chrome is a fraction of *whatever it is drawn into* rather than a fixed number
of pixels. **`Q29` is answered.**

- [x] **C0. `the_chrome_is_a_small_part_of_whatever_it_is_drawn_into`** — ⭐⭐ **`Q29`, and it went
  first because everything below it makes the panel taller.** A point is now a pixel of the
  1920 × 1080 frame this project judges itself on, scaled with the surface and capped at the
  display's own point. **22.1% → 8.3%** on the window this program opens. Measured at four sizes,
  and the 30,000-tick dumped frame came back **byte-for-byte identical** to Group B's.
- [x] **C1. `a_time_series_is_recorded_as_the_run_goes`** — `series.rs`. SPEC section 13's record,
  **64 bytes, asserted at compile time**, taken every 100 ticks of the world's own clock by
  `Run::step` — the one place in the program a tick happens. Every figure is `Census::of`'s or the
  ledger's own. ⚠️ SPEC's `species counts` field is **deliberately absent**; see the decision table.
- [x] **C2. `the_charts_show_population_biomass_and_the_ledger_over_time`** — `alive`, `biomass`
  and `energy`, as three rows of the same grammar the readings panel uses: a dim name on the left
  and a faint trace on the right. The ledger is the four accounts as **shares of a conserved
  whole**, which is SPEC section 5 drawn.
- [x] **C3. `the_series_is_bounded`** — ⭐⭐ **4,096 records, 256 KiB, allocated once and never
  resized**, thinned by halving the resolution each time it fills. The arithmetic and what a
  viewer loses are below.
- [x] **C4. Looked at.** `docs/frames/phase6-charts.png` and `docs/frames/phase6-charts-window.png`,
  seven dump-look-adjust rounds, three of them windows. Written up below.

#### ⭐⭐ `Q29`, answered: a point is a pixel of the frame this project judges itself on

Group B handed egui the **display's** scale factor, which is what a normal application does and is
wrong for this one. A point was 1.5 pixels whatever it was drawn into, so a panel of a fixed number
of points was a fixed number of *pixels* — and a fixed number of pixels is a small part of a large
frame and a large part of a small one.

```rust
fn chrome_scale(frame: (u32, u32), display: f32) -> f32 {
    let across = points(frame.0) / points(crate::DUMP_WIDTH);
    let down = points(frame.1) / points(crate::DUMP_HEIGHT);

    across.min(down).min(display).max(SMALLEST)
}
```

The lesser of the two ratios, so a window dragged tall and thin shrinks the chrome by its width
rather than growing it by its height. Two bounds, and a bare ratio is wrong at both ends: it never
goes **below `SMALLEST` = 0.8**, because eleven points at less than that is a numeral nobody can
read and a panel too small to read is a worse answer than one too large; and it never goes **above
the display's own scale**, because a point larger than the desktop's point is chrome that is
physically bigger than every other window on the screen — which is the fault.

The second half is that `CEILING` moved **off the controls and onto the whole column**. Group B
bounded the scroll area and nothing else, which was right while the controls were the last thing on
the frame and is wrong now the charts are under them: three bounds that each hold say nothing about
their sum. The readings, the controls and the charts now share one ceiling and the controls get
what is left of it, so the chrome's share of a frame is something that can be *stated*.

| Frame | Display | Before | **After** | A point is |
| --- | --- | --- | --- | --- |
| 1920 × 1080 — the dump | 1.0 | 4.9% | **6.0%** | 1.0 px |
| **1280 × 720 — the window this program opens** | **1.5** | **22.1%** | **8.3%** | 0.8 px |
| 1280 × 720 | 1.0 | 14.7% | **8.3%** | 0.8 px |
| 2560 × 1440 | 1.5 | 12.4% | **5.8%** | 1.33 px |

The 6.0% is Group B's 4.9% *plus the charts*, so the frame this project measures itself on grew by
a fifth and the window it opens shrank by nearly two thirds. ⭐ **And the shipped 30,000-tick frame
is unchanged**: dumped before and after the change, `md5 acc44185…` both times, byte for byte —
because `chrome_scale((1920, 1080), 1.0)` is exactly 1.0 and the new ceiling binds on nothing there.

⚠️ **What is left.** A window dragged down to the 320 × 180 floor `window.rs` allows is still mostly
chrome, because `SMALLEST` stops the shrinking before the panel does. That is a window nobody uses
and `S` empties it; the honest statement is that the chrome is bounded at a tenth of the frame for
every size a person would actually work at, and is not bounded below that.

#### ⭐⭐ `C3` — the bound is 256 KiB, and this is the arithmetic

A headless run manages about 1,300 ticks a second on this machine (Phase 5 Group C measured a
*watched* run at about 650, and **Q23** records that watching costs about half the speed). A record
every hundred ticks, kept for ever:

| Run | Ticks | Records | Bytes |
| --- | --- | --- | --- |
| 1 hour | 4.7 M | 47,000 | 3 MB |
| 12 hours — CLAUDE.md's default wall-clock bound | 56 M | 562,000 | **36 MB** |
| A week — which SPEC section 13 explicitly plans for | 786 M | 7.9 M | **503 MB** |

Half a gigabyte of chart is half a gigabyte that is not a simulation, and it is the exact leak
CLAUDE.md's *"allocate once, never grow — a simulation that cannot allocate cannot leak"* exists to
prevent. So `Series` is an arena like the organisms and the grid: **`CAPACITY` = 4,096 records at
`Sample`'s 64 bytes = 256 KiB, allocated in `Series::new` and never resized** — 0.0125% of
CLAUDE.md's 2 GB resident target, for a week-long run as much as for a one-minute one.
`the_series_is_bounded` drives a hundred times the capacity through it and asserts on
`Vec::capacity` at every step, which is the claim stated as *the memory does not grow* rather than
as *the length is bounded*.

**Thinning, not a ring buffer**, and the two lose completely different things. A ring buffer of
4,096 records at one per hundred ticks holds the last **409,600 ticks — about five minutes** of an
overnight run. CLAUDE.md: *"You come back hours later and read what happened."* A chart that can
only show the last five minutes has thrown away everything the person came back for. Thinning
doubles the stride each time the arena fills and keeps every reading on the new grid, so the series
always spans **the whole run**, at a resolution that falls as the run gets longer — which is the
right trade for deep time.

| Run | Ticks | Halvings | A reading every | Records held |
| --- | --- | --- | --- | --- |
| 7 minutes | 409,600 | 0 | 100 ticks | 4,096 |
| 1 hour | 4.7 M | 4 | 1,600 ticks | 2,900 |
| 12 hours | 56 M | 8 | 25,600 ticks | 2,195 |
| A week | 786 M | 11 | 204,800 ticks | 3,840 |

⚠️ **What a viewer loses, stated rather than buried.** After eight halvings a reading is one tick in
25,600 — about twenty seconds of wall clock — and **anything that happened and was over inside that
window is not in the series at all**. SPEC section 11's mass extinction (*"population falls by >50%
within 5,000 ticks"*) shows on a 12-hour run's chart as a step between two adjacent readings rather
than as a cliff with a shape. The chart says *that* it happened and roughly when; Phase 7's event
log is what says the tick. A chart is for shape.

⚠️ **A dropped reading is dropped, not averaged.** Averaging two readings makes a third that no tick
of the world ever produced — a population of 1,712.5, a ledger that does not balance — and a chart
drawn from those is a picture of a world that did not happen. Every point on every chart in this
program is a reading the world actually gave.

#### What Group C decided that SPEC does not say

| Decision | Where | Short version |
| --- | --- | --- |
| **⭐⭐ SPEC section 13's `species counts` field is omitted, deliberately** | `series.rs` | Phase 7 is what makes a species exist — clustering every 500 ticks, promotion after twenty consecutive samples, naming — and **Phase 7 lands before Phase 8**, so the field can be added by the phase that has something to put in it and nothing will have been written to disk in the meantime for it to be incompatible with. The alternative is a `species: u32` that is nought on every record of every run: a column of zeroes on a chart, and a number in a file that reads as *"this world has no species"*, which is false. CLAUDE.md: *"Don't over-build. No speculative abstractions."* An absent field cannot be believed; a wrong one can. |
| **⭐⭐ The series lives on the `Run`, and the type lives in `coacervate-render`** | `run.rs`, `series.rs` | Two halves of one argument. It is **recorded** by `Run::step` because a reading is taken every hundred ticks *of the world's own clock* and the only thing that knows when a tick happened is the thing that took it — a window draws about every eleventh tick and a headless run draws none at all, so a series sampled from `draw` would be a different series in the two builds and neither would be SPEC's grid. And the **type** is in `coacervate-render` for `census.rs`'s reason, unchanged from Group A: a sample is made of a `Census` and the ledger, the panel that draws it is in that crate, and computing the same eleven numbers twice is what that module's opening paragraph argues against. It is also where Phase 8 wants it: `stats.bin` is a file in the run's own directory. |
| **⭐ The grid is the world's tick count, not a count of calls** | `series.rs` | Which is what makes a series reproducible. Two runs of one seed record the same ticks; a run watched through a window records the same ticks as the same run headless; a run resumed from a Phase 8 snapshot lands on the same grid as the run it was taken from. A counter starting at nought whenever the series was constructed would have none of those. |
| **⭐⭐ Thinning filters on the **tick** and not on the position in the list** | `series.rs` | And this is what keeps the spacing exactly even for ever. Dropping every other *record* leaves the survivors evenly spaced only until the next reading arrives, because `observe` records on multiples of the stride and the survivors are on multiples of it offset by wherever the run happened to start. Filtered on the tick, the survivors are *exactly* the readings `observe` would have taken at this stride all along, and the run carries on on the same grid. It is idempotent, which is the plainest statement of the same thing. The mutation check below is what measured it. |
| **⚠️ `Series::record` is public and the bound does not depend on it behaving** | `series.rs` | Phase 8 reads `stats.bin` back through it, and a caller handing over ticks that are not on the grid could have every one of them survive the filter — which would push the vector past its capacity and reallocate. That is the one thing this type promises cannot happen, so `thin` holds it with a second pass by position rather than by the contract being kept. |
| **⭐ `Sample` is 64 bytes and the size is a `const` assertion** | `series.rs` | *"Small and fixed-size"* is what `CAPACITY`'s whole arithmetic rests on, and a field added in the wrong place breaks it silently — a `vec3`-shaped mistake, and `camera.rs` carries the same note about the same class of fault. Sixty-four is also **no padding at all**: eight for the tick, then fourteen four-byte scalars. |
| **⚠️ The energies are 32-bit and the tick is not** | `series.rs` | Nothing reads a `Sample` as a *figure* — the panel's own numbers come from the world through `panel::readings` — and these are read as a chart, which is a box twenty points tall. Seven significant digits is five more than a chart can show. The tick is not narrowed because it is not a magnitude but an **identity**: `thin` filters on it, and 16.7 million ticks (four hours in) is exactly where a 32-bit float stops being able to tell one tick from the next. |
| **⚠️ Per-kind biomass is apportioned by cell count** | `series.rs` | An organism holds one pool of energy and its cells hold none, so the only division of it that invents nothing is an equal share per cell. The consequence is a property worth having and worth testing: **the six figures sum to the ledger's `biomass` account**, so they are a decomposition of it rather than a second opinion about it. |
| **⭐ A chart is a *row*, in the readings panel's own grammar** | `panel.rs` | A dim name in a reserved column on the left and something to look at on the right — which is what every row of Group A's panel already is, with a shape where the numeral goes. A chart with an axis on it has tick marks, numbers along the bottom and a legend, which between them are half a dozen bright small things in a corner that is supposed to nearly disappear. The cost: no chart here can be read as a *quantity*. That is deliberate — the readings panel directly above prints every one of these numbers as a figure, and a second copy of a number is the thing `census.rs` exists to argue against. |
| **⭐ Each chart is scaled to its own greatest reading** | `panel.rs` | A population of two thousand and a biomass of a hundred and forty thousand do not go on one axis, and a shared scale would draw one of them as a line along the floor. What a sparkline can say is *how this went*; what it cannot say is how big it got. |
| **⭐⭐ The ledger chart is the four accounts as **shares of a conserved whole**** | `panel.rs` | Over the shipped run the field holds 139,886, `detritus` holds 3,713 and `dissipated` holds 270,506, so a chart scaled to the largest of them draws two of the four as the same line along the bottom. As shares they are four bands that fill the box, and what the chart says is **where the world's energy is** — which is the question SPEC section 5 exists to answer. ⚠️ Four bands and not five: `light` is not a place energy *is*, it is where the energy in the other four came from and is already inside them. A fifth band would count every joule twice. It shows instead as the whole stack's total rising. |
| **⭐ What is *filled* is what the world still has** | `panel.rs` | `field` carries the picture and `dissipated` is drawn as the absence it is, so the shaded part of the box is the energy still in the water — a region that starts nearly full and drains. The other way round is identical arithmetic and reads backwards: a growing grey block that means *gone*. And the brightest band is the smallest one: `biomass` is a twentieth of the total and gets `LEVEL`, which is `frame.rs`'s decision about the picture underneath applied to the chrome — **the brightest thing is the thing that is alive**. |
| **⚠️ The readings are decimated to the width of the box before anything is drawn** | `panel.rs` | Up to 4,096 readings behind a chart 150 points wide is twenty-seven readings a column. The reading **nearest** the column is taken rather than the mean of those behind it, which is `series.rs`'s rule about thinning applied to drawing, for the same reason. |
| **⚠️ A band of a stack that holds anything is drawn at least 1.5 points tall** | `panel.rs` | A *window* found this; see the round table. The cost is stated rather than hidden: a band forced up to a pixel is drawn larger than it is, and in the worst case three points of a twenty-point box go on saying *there is something here*. That is the right trade, because a sliver that rounds away to nothing is the one reading this chart must not give. |
| **⭐ `pixels_of` rounds outward, and `Q29` is what found it** | `panel.rs` | Its doc comment has said *"rounded outwards — down at the near edge, up at the far one"* since Group A and the code called `f32::round`. Until Group C every position was a whole number of points at a scale of one, so the two were the same conversion. At a scale of 0.8 the panel's corner lands on pixel 9.6, rounding puts it at 10, and the row of pixels the panel's own border is drawn across is outside the rectangle the chrome claims — which `egui_draws_over_the_world_without_clearing_it` failed on the moment the scale stopped being one. It also grows by one pixel for epaint's feathering, which is ink outside the geometry. |
| **The charts panel's height is a constant, not a measurement** | `panel.rs` | The controls above it are bounded by what is left of the column once the charts have had their share, and a height that could only be known after the fact would mean the scroll area being sized by *last frame's* charts — which is exactly the one-frame lag Group B's settle loop exists to remove. |

##### ⭐ What the mutation checks found

**`Series::thin` was written to drop every other record by position first**, which is the shape the
operation naturally takes and is subtly wrong. The test failed on the number that names it: *"the
samples at ticks 409,600 and 411,000 are **1,400** apart and the rest are **1,600** apart, so the
thinning left a seam."* One short interval, at the join between what survived the halving and what
was recorded after it — invisible on a chart, and a lie about when things happened.

**`chrome_scale` began as the display's own factor**, which is Group B shipped: *"the chrome takes
**22.1%** of a 1280 by 720 frame at a display scale of 1.5, which is [18, 18, 345, 591] of 921,600
pixels."* That is `Q29` restated by a test to the decimal place `B6` reported it at.

##### ⭐⭐ And the one that was real, found by a *window* and not by a test

**A stacked band a fifth of a pixel tall does not draw, and the band above it then paints over the
one below.** Two faults, and the second is the one that took an hour.

epaint tessellates lines and rectangles with a pixel of feathering; a `Mesh` is handed to the card
as triangles and gets none, so a band 0.87 pixels tall is filled only in the columns where it
happens to cross a pixel centre. On the 1920 × 1080 dump `biomass` came out as a line. **In the
1280 × 720 window, where a point is 0.8 pixels, it came out as a row of dashes** — which reads as a
rendering fault rather than as the thing that is alive.

The floor that fixes it broke the stack, and the frame did not say so plainly:

| Band | Share | Unfloored top | With a floor of 4 points |
| --- | --- | --- | --- |
| `field` | 0.319 | 531.6 | 531.6 |
| `biomass` | 0.374 | 530.5 | **527.6** |
| `detritus` | 0.382 | 530.4 | 530.4 — **below the band beneath it** |
| `dissipated` | 1.000 | 518.0 | 518.0, drawn from there **down to 530.4** |

Once `biomass` was lifted, `detritus`'s level was still computed from the true cumulative share and
came out *underneath* the band beneath it, so its quad was inside out — and `dissipated`, whose top
is always the top of the box, then painted from there down over everything the floor had just made
room for. The visible symptom was **the same dashes**, which is why raising the floor from 1.5 to 4
changed nothing on the frame and sent the search in the wrong direction for a while. Whether a band
holds anything is a question about the **readings**; where it is drawn is a question about the box,
and the two need separate lists.

⚠️ **No test catches this and none is added that pretends to.** It is a claim about pixel centres
under a particular scale, and the instrument for it is the one CLAUDE.md provides: dump a frame and
look at it — in a window as well as headless.

#### ⭐⭐ `C4` — what the charts actually look like

`docs/frames/phase6-charts.png` — 1920 × 1080, world tick 30,000, seed 42, **1,713 alive, mean
genome 1.70 ± 0.85**. The same figures every group since Phase 5 Group B has measured: the charts
moved nothing. `docs/frames/phase6-charts-window.png` — the same run in the 1280 × 720 window this
program opens, on a display at 150%, which is `Q29`'s own case.

**Seven dump-look-adjust rounds, three of them windows**, and the windows found what no dump could.

| Round | What the frame said | What changed |
| --- | --- | --- |
| 1 | The `alive` and `biomass` traces read exactly as wanted. The `energy` chart is **an empty box with a dotted line in it** — four bands at values a hair apart, and no eye can find the composition | One band carries the picture: `field` filled, `dissipated` drawn as absence |
| 2 | Legible. But at 4× the trace fills are **solid blocks**: a rail's colour over eight times a rail's area is not a rail's weight | `CHART_FILL` down below `TRACK`; the edge line does the work |
| 3 | ⭐ **A window.** `biomass` is a row of **dashes** along the energy chart | A floor of 1.5 points on any band that holds anything |
| 4 | Nothing changed at all | — |
| 5 | Raised the floor to 4 points. **Still nothing changed**, which is the useful kind of wrong answer: the floor was working and something downstream was undoing it | Instrumented, and found the inverted quad above |
| 6 | Four bands, every column, at both scales | Floor restored to 1.5 |
| 7 | ⭐ A window again. Shipped | |

**The honest report.**

- ⭐ **It is still recessive, and `Q29` is why.** At a glance the frame is the world: eight colonies
  of coloured light on near-black water and a narrow dark column down the left. The three panels
  are 232 × 537 pixels of 2,073,600 — **6.0% of the dumped frame**, against Group B's 4.9% for two
  of them — and in the window they are **8.3%**, against the 22.1% Group B shipped. The chrome grew
  by a third and got nearly three times smaller where it matters. Nothing on it is brighter than a
  colony; the brightest thing in the picture is still something that is alive.
- ⭐ **A sparkline was the right shape.** Three rows of *name, then thing* under two panels of
  *name, then number* — the eye reads the third block as more readings rather than as a dashboard,
  which is the whole of how a chart stays inside SPEC section 12's register. There are no
  gridlines, no axis, no legend and no numbers on any of them.
- ⭐ **The `energy` chart is the best thing in this group and it is the one that nearly did not
  work.** At tick 30,000 it reads, bottom to top: a grey band a third of the box high (the water),
  a bright hairline (what is alive), a fainter one (what was), and then nothing to the top (what
  has been spent). Over the run the bright line **descends** — you can watch the world convert its
  water into having-lived. That is SPEC section 5's conservation law as a picture, and it says
  something the five numbers above it cannot: not *how much*, but *where it is going*.
- **`alive` and `biomass` are two nearly parallel wedges**, which is honest and slightly redundant:
  in this world the population and the energy in it track each other closely. A run where they came
  apart would be the interesting one, and the pair of charts is what would show it.
- ⚠️ **The traces begin part-way up their boxes and there is no mark saying why.** The series starts
  at the first hundred-tick mark after `Run::new`, which is after `founding.rs`'s nine-thousand-tick
  dawn — so the left edge of `alive` is the population at tick 9,900, already 55% of what it
  reaches. It reads as though the run began mid-climb, which it did. A chart with an axis would say
  so; this one cannot, and that is the price of the register.
- ⚠️ **The second honest criticism is the window's controls.** With the column ceiling shared three
  ways, the `locked` fold is now below the fold in a 1280 × 720 window and the scroll bar is what
  says so. Group B's window scrolled sooner and further, so this is not a regression — but it is
  worth writing down that the settings panel is a scrolling panel on a small window and a complete
  one on a large.
- **Nothing moves.** `animation_time` is still nought, the settle loop still stops on a stable
  rectangle, and the charts are drawn from a series that changes once every hundred ticks — so a
  chart is the one thing on the chrome that is *deliberately* a second and a half behind the world,
  and two dumps of the same tick are the same file.

---

## Constraints that outrank features

- **Visually calm, and recessive.** CLAUDE.md: the chrome should nearly disappear. If a
  choice is between informative and quiet, quiet wins — the log is where drama belongs.
- **Never steals focus.** Phase 5 measured this; do not regress it.
- **`coacervate-sim` must not learn that a UI exists.** Panels read; they do not get hooks.
- **A slider must not bypass validation.** See B3.

---

## Open questions carried forward

**Q29** (Group B) — **answered, and it was Group C's first step rather than its last.** The chrome
scales with the **surface** now and not with the display: a point is a pixel of the 1920 × 1080
frame this project judges itself on, taken as the lesser of the two ratios, floored at 0.8 pixels so
the numerals stay readable and capped at the display's own point so the chrome is never physically
larger than every other window on the screen. `CEILING` moved off the controls and onto the whole
column at the same time, so the chrome's share of a frame is a stated bound rather than a measured
one. **22.1% → 8.3%** on the window this program opens; the dumped frame is byte-for-byte
unchanged. See the write-up above, and
`the_chrome_is_a_small_part_of_whatever_it_is_drawn_into`, which holds it at a tenth of the frame
over four sizes and two display scales.

⚠️ What is *not* fixed: a window dragged down to `window.rs`'s 320 × 180 floor is still mostly
chrome, because `SMALLEST` stops the shrinking before the panel does. That is a window nobody uses
and `S` empties it, but the bound is a tenth of the frame for every size a person would work at and
is not a bound below that.

**Q28** (Group A) — **still open, and re-checked at the top of Group C as that question asked.**
`egui-wgpu`'s newest published version is **still 0.35.0**, still naming `wgpu ^29`, so the version
matrix above is unchanged and `panel.rs`'s `Painter` stays. It cost Group C nothing either: the
charts needed a triangle list with a colour per vertex, which is exactly what that backend already
draws, and **not one line of it changed across two groups**. Ask again in Phase 7.

**Q27** (Group A) — **answered.** The events go into the `egui::RawInput` that `Chrome::compose`
was already building, so there is still exactly one composition route and a headless dump simply
finds the queue empty; `egui-winit` is still absent, now settled rather than deferred, because its
entry point takes a `&winit::Window` and a dump has not got one. `Controls::felt` is the forty
lines that stand in for it. The property is stated as a test that could not otherwise exist —
`a_slider_answers_a_pointer_with_no_window_anywhere_near_it` drives a slider end to end with no
window in the program at all. And the pointer knows the panel is there: a **grab** that starts over
the chrome never reaches the camera, while a drag begun on open water goes on panning across the
panel rather than stopping dead half way. See the write-up above.

**Q30** (new, Group C) — ⚠️ **the charts show a run that has already begun.** The series starts at
the first hundred-tick mark after `Run::new`, which is after `founding.rs`'s nine-thousand-tick
dawn, so the left edge of every chart is the world at tick ~9,900 rather than at tick nought — and
with no axis there is nothing on the panel that says so. It is the honest picture of what the run
recorded and it is not the honest picture of what the *world* did. Two ways out and neither belongs
to this phase: record the dawn as well, which is nine thousand ticks of a world with nothing alive
in it and ninety records saying so; or say what span the chart covers, which is a number on a chart
and is exactly what `C2` decided against. Phase 8's scrubbing will have to answer the same question
about a much longer series, so it is the phase to answer it in.

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
