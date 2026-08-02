# Coacervate — Simulation Specification

The implementable detail. For project conventions, safety limits, Windows setup and the
decision log, see [CLAUDE.md](CLAUDE.md).

Everything below is a starting point with reasoning attached, not scripture. Disagree on an
informed basis — but read the reasoning first, and check the Decision Log in `CLAUDE.md`
before changing anything marked **load-bearing**.

---

## 1. Scope

The simulation is a closed 2D world containing:

- a **resource field** that gains energy from light at a slow constant rate,
- **organisms** built of one or more physically-connected cells,
- **detritus** — dead biomass returning energy to the field.

Organisms harvest energy, pay metabolic costs, move, reproduce with mutation, and die. That
is the entire ruleset. Everything interesting is meant to be a *consequence*, not a feature.

**Explicit non-goals:** no player agency beyond initial conditions and environmental
events; no scoring; no designed progression; no scripted behaviours; no hand-authored
species.

---

## 2. Units, coordinates, determinism

- World space is metric-ish and abstract: 1 world unit ≈ 1 µm. Default world is
  2048 × 1152 units (16:9).
- Origin is top-left. **+Y is down**, matching screen space, so light comes from `y = 0`.
- Fixed timestep, `dt = 1/60` simulated seconds per tick. Real-time speed is decoupled: the
  runner executes as many ticks per wall-clock second as it can, up to a configured cap.
- **Deep time display:** 1 tick = 1,000 years by default (configurable). One million ticks
  therefore reads as one billion years. This is presentation only and never enters the
  physics.

### Determinism — load-bearing

A run must reproduce **exactly** from `(seed, config)`. This is what lets you go back and
watch an interesting event again, and it is what makes the GPU port testable.

- One `ChaCha8Rng` seeded from the config. No `thread_rng`, ever.
- Each organism carries its own RNG stream, seeded deterministically from
  `(world_seed, organism_serial)`. This keeps organism-level randomness independent of
  evaluation order, which is what allows `rayon` parallelism without breaking reproducibility.
- **No `HashMap` iteration** anywhere that affects simulation state — use `BTreeMap`, or
  index into dense arrays.
- No wall-clock time in simulation logic.
- `f32` throughout for state (GPU compatibility). Accept that this constrains
  reproducibility to a fixed instruction set; document it rather than chasing bit-exactness
  across architectures. **One exception, and it is not optional:** the energy ledger's
  accounts are `f64`, because they are running totals over the whole run rather than state.
  See section 5 for the measured arithmetic — at `f32` the invariant breaks at tick 90,996.

**Test:** run 100,000 ticks twice from the same seed, hash the world state, assert equal.

---

## 3. Configuration

TOML, loaded at startup, embedded verbatim into the replay log so a run is always
self-describing.

```toml
[world]
seed = 42
width = 2048.0
height = 1152.0
grid_cols = 256          # resource field resolution
grid_rows = 144
years_per_tick = 1000.0

[light]
influx = 0.001           # energy per grid tile per tick — measured, see below
cap = 8.0                # max energy a tile can hold
gradient = 0.75          # 0 = uniform, 1 = fully top-weighted
patchiness = 0.5         # spatial noise amplitude
patch_drift = 0.0006     # world units per tick the patches slide sideways — see section 4
diffusion = 0.04         # lateral spread per tick
season_period = 21000    # ticks for one whole rise and fall of `influx` — measured window,
                         # see section 4
season_amplitude = 0.0   # how deep that rise and fall goes, as a fraction of `influx`.
                         # ⚠️ Ships at nought: `config/seasonal.toml` is the same world at 0.25

[physics]
drag = 0.92              # velocity retained per tick (high — viscous regime)
drag_anisotropy = 2.0    # power `drag` is raised to across a cell's body axis — measured,
                         # see section 8. 1.0 is isotropic water, in which nothing can swim
collision_stiffness = 40.0
spring_damping = 0.35

[behaviour]
resting_amplitude = 0.8  # how hard a myocyte contracts with nothing telling it otherwise —
                         # measured, see section 9
stroke = 1.0             # how much of its rest length a myocyte at full amplitude works
                         # through, either way — measured, see section 9. Bounded at 1.0,
                         # where a spring's shortest rest length reaches nought

[metabolism]
upkeep_scale = 1.0       # global multiplier on all cell upkeep ("temperature")
gene_cost = 0.0001       # per gene, per tick — see section 7
movement_cost = 0.0001   # energy per unit of work done by contraction — measured, see below
reproduction_threshold = 2.2   # × body construction cost
offspring_share = 0.45   # fraction of parent energy passed to offspring

[mutation]
point_rate = 0.06        # per gene, per reproduction
point_sigma = 0.12       # gaussian magnitude on numeric fields
duplication_rate = 0.02  # per genome
deletion_rate = 0.02
insertion_rate = 0.01
reorder_rate = 0.02      # swap two adjacent genes; order carries information
genome_duplication_rate = 0.0008

[limits]
max_organisms = 4000
max_cells_per_organism = 64
max_genes = 128
max_dev_steps = 16

[run]
max_wall_clock_hours = 12.0
max_ticks = 0            # 0 = unbounded
max_ticks_per_second = 0 # 0 = uncapped. The `slow` profile's only lever.
reseed_on_extinction = false
```

Every one of these is exposed as a slider in the UI. `[world]`, `[limits]` and `seed` lock
at run start; the rest can be changed live, which is how environmental events work.

### ⭐⭐ `metabolism.movement_cost` is 0.0001, not 0.15 — measured in Phase 7

**A thousandfold down, and it had to be**, because at the value this document was first
written with, moving could never pay for itself under any circumstances whatever.

The measurement is in `behaviour.rs`'s `a_myocyte_oscillates_its_springs_and_pays_for_the_work`.
One spring of stiffness 10, worked at the resting amplitude — a myocyte with nothing to sense,
which is what every body in the world starts as — cost **0.0215 a tick** at `0.15`. A myocyte's
whole upkeep was 0.014 at the time. So a single muscle at rest was charged **one and a half
times its own cost of living** for the privilege of twitching, before it had gone anywhere.

Set that against what going somewhere is worth. A tile of water at the shipped `cap` and
`influx` yields on the order of `3e-4` to a photocyte standing on it, so crossing one tile in
search of a better one has to cost less than that to be worth doing. `0.15` is about **a
thousand times** break-even. No stroke, no body plan and no amount of anisotropy in section 8's
water would have made swimming a living: the ledger closed the question before the physics was
asked. `0.0001` puts a resting muscle at a thousandth of its own upkeep and a one-tile hop just
inside what a tile is worth.

⚠️ It is deliberately **not** zero. Free movement is a coherent world to run and the gate
accepts it, but a myocyte that costs nothing to work is a cell with no downside at all, and
a body would accumulate them as neutral bloat.

⭐ **The other half of that trade was not looked at until much later, and it was wrong in the
same direction.** With `movement_cost` at 0.15 a muscle's upkeep was the only thing pricing it,
because using one was impossible; after the thousandfold cut, a muscle was priced almost
entirely by being *owned* — a body swimming flat out paid about a ninth of what its muscles
cost it to stand still. Section 6 records the sweep that brought a myocyte's upkeep from 0.014
to **0.005**, at which working a muscle is about a third of owning one. The two numbers are one
decision and they are now set together.

### ⭐⭐ `physics.drag_anisotropy` is 2.0, and one is the number that made this world impossible

The whole argument is section 8's, because it is a fact about the integrator rather than about
the economy, and it is the single most consequential thing measured in this project so far:
**with one drag for every direction, nothing in this world could ever have moved.** Not slowly
— at all, under any parameters. Two is the factor by which a long thin body in water resists
being pushed sideways rather than lengthways, and it is what makes the shipped world one a
lineage can swim in. Read section 8 before changing it.

### ⭐⭐ `light.patch_drift` is what makes swimming worth anything, and `patchiness` is what makes it worth following

Section 8's anisotropic water made locomotion *possible*. It did not make it *pay*: a
310,000-tick run at the shipped configuration, with a working undulator mechanism in the
physics, still ended with **one myocyte**. The reason is not in the physics and not in the
economy. It is that the resource field was **worked out once from the seed and never moved
again**, so the best tile in the world was the best tile in the world for ever, and a lineage
that could swim had nowhere better to go than where it already was.

`patch_drift` slides the whole pattern of blotches sideways, for ever, at a speed chosen so
that following it is possible for one kind of body and not another. The full argument, the
three measured speeds it sits between, and what the drift costs in energy are in section 4.
**0.0006 world units per tick**, bounded at 0.005.

`patchiness` moved from **0.15 to 0.5** in the same change, and the two go together: a field
that drifts is worth following only if there is somewhere better to arrive at. It enters
section 4's formula exactly linearly, so 0.5 puts a tile's ceiling within ±67% of what its
depth alone would give it. One is the arithmetic bound — above it some tiles' targets go
negative — and 0.5 was chosen from what a population does rather than from the arithmetic.

### ⭐⭐ `light.influx` is the one number in this table that was measured rather than chosen

Every value above began as a guess written before anything ran. This one has since been run,
and it moved by a factor of twelve — from `0.012` to `0.001`. The reasoning is the whole of
section 4's carrying-capacity argument, so it is worth setting out here beside the value.

At `0.012`, a single seeded body grows to `limits.max_organisms` — four thousand — in under
twenty thousand ticks and stays there for as long as anybody watches, **with the field only
1.6% below what the light alone leaves it holding.** Every slot in the world is taken and the
water is still essentially full. That is a world where the *arena* is the binding constraint
and nothing whatever is scarce, so every birth that fails does so for a reason unconnected to
how well its parent was doing, and drift is the only force acting on the population. Section 4
calls carrying capacity "the pressure that drives everything else in the simulation"; at
`0.012` it never switches on.

Measured, at the shipped world and `upkeep_scale = 1.0`, over runs of 150,000 ticks:

| `influx` | Population | Field, against what the light alone leaves | Verdict |
| --- | --- | --- | --- |
| 0.012 | **4,000 — the arena cap**, reached by tick 20,000 | 98% | nothing is scarce |
| 0.003 | 4,000 by tick 30,000, then slowly down to 3,374 | 46% | pinned at the cap for tens of thousands of ticks |
| 0.002 | 4,000 by tick 45,000, then down to 3,017 | 43% | as above |
| 0.0015 | 4,000 by tick 4,500 when founded with a thousand bodies | 44% | still reaches the arena |
| **0.001** | **~2,200, and it stays there** | **47%** | the energy budget binds first |
| 0.0008 | ~1,600 | 49% | as above, with less headroom |
| 0.0002 | ~440 | 51% | as above; a small world, not a dying one |
| 0.0001 | ~210 | 53% | as above |

`0.001` settles at the same level from **both directions** — grown up to it from a single
founder over ninety thousand ticks, and cut back down to it from a thousand founders in
thirteen thousand — which is what makes it an equilibrium rather than a coincidence.

Two things in that table are worth reading off it, because neither was expected and both are
now the clearest diagnostic this project has.

**Population is very nearly proportional to `influx`** — about 2.2 million times it, across
more than an order of magnitude — which is section 4's carrying-capacity claim holding exactly.

**And the field is drawn down to about half whatever the light is, at every influx where the
energy budget is what binds.** That is not a coincidence either: what a tile settles at is
decided by the income a body needs in order to replace itself before it dies, and that figure
comes from `[metabolism]` and not from `[light]`. So `influx` decides *how many* bodies there
are and the metabolism decides *how hard* each one has to work. The pair make a test: a world
whose field is barely below full is a world where something other than energy is limiting the
population, which at `0.012` is `limits.max_organisms`.

**`upkeep_scale` was tried first and is the wrong lever, which is worth recording because the
arithmetic says otherwise.** Scaling the cost side by four is what a static energy-budget
calculation asks for, and it kills every world it is applied to *immediately*. Section 10's
lifespan is derived from what a body costs to run, so raising `upkeep_scale` shortens a life in
proportion while *lengthening* the time it takes to earn section 10's reproduction threshold —
it closes the window between "old enough to breed" and "dead" from both ends at once. Measured:
`upkeep_scale = 2` still breeds but grows very slowly; **`3` and `4` both go extinct with the
founder's death, before a single birth.** The temperature slider is a real environmental
pressure and it is not a carrying-capacity control.

### ⭐⭐ `light.season_amplitude` ships at nought, and shipping it inert is the decision

The season is the first mechanism this project has landed **switched off**, and that is a
decision rather than caution. Phase 7's Group H shipped a sevenfold change to every muscle in
the world and could not afterwards separate *did I break anything* from *the world is now
different*; the two questions had been asked in one commit. With the amplitude at nought the
multiplier `1 + amplitude × triangle(phase)` is exactly 1.0, `f64::from(x) * 1.0` narrows back to
`x` for every `x` there is, and `config/default.toml` is **bit-for-bit** the world every figure
in `docs/PHASE7.md` was measured on. Six golden vectors do not have to be re-recorded, every
prior reading stays comparable, and a varying world is one profile away.

⚠️ **The multiplier is computed unconditionally and there is no branch for the amplitude being
nought.** An early return there is the most dangerous thing this feature could contain: it leaves
the multiplier frozen wherever the season was when the dial reached zero, and the world then runs
permanently at up to 1.25× its stated influx with `season_amplitude = 0` written in the config
file, in the panel and in the replay log, while section 5's ledger balances to the last digit and
says nothing. That is section 4's named failure — *"a world that was poorer than its `influx`
said, with a carrying capacity nobody could explain"* — reached through the settings panel and in
the other direction.

**Profiles.** Ship named presets rather than expecting anyone to tune 26 sliders from cold:

| Profile | Intent |
| --- | --- |
| `default` | Balanced. The starting point for experiments on the PC. Settles near 2,200 organisms with the field about half eaten. |
| `dense` | ⭐ **The same total energy in a quarter of the water** — `world.height` cut to 288 with `light.influx` raised fourfold to match, so `tiles × influx` is the `default` profile's exactly. It exists to ask section 6's open question: does a feeding-strategy split appear once bodies actually *meet* one another? At the shipped density only **13.5%** of cells are in contact with a foreign body; at four times it, **58.8%** are, and predation measured **eight times** higher. ⚠️ **The height is what shrinks, never the width.** Section 8 warns that springs have no length limit and are not found by the spatial hash, and a 64-cell body at `MAX_REST_LENGTH` reaches 870 units — so in a world narrower than about 1,200 a single chain reaches more than half way round it and that warning goes live. ⚠️ And the population *falls* here rather than holding: raising `influx` further to prop it back up is the `bloom` failure below, arrived at by a different road. |
| `slow` | `max_ticks_per_second` reduced so meaningful change happens over hours rather than minutes. For leaving it up on a second screen and noticing it rather than watching it. |
| `bloom` | High light influx — the old `0.012` is exactly this. **The population fills `max_organisms` and stops, with the water still full**, which is stagnation by way of the arena rather than by way of abundance. Worth shipping precisely because it is what a too-bright world actually looks like from the outside: a healthy-looking constant population with no selection acting on it. |
| `seasonal` | ⭐⭐ **The shipped world with the light rising and falling** — `light.season_amplitude` 0.0 → 0.25 and nothing else changed. It exists because every other profile in this project is a *constant* environment, in which being adapted is a fixed fact about a lineage. ⚠️ **Do not expect a muscle from it**, and that is measured rather than hoped: the competition assay run flat and seasoned, three seeds, two whole periods, moved the coefficient on the largest free shape change from **+0.71 to +0.88 %/generation** against a seed-to-seed spread of ±0.5 — no detectable difference. The fastest the standing field can change is its own 8,000-tick filling time, which is 4.6 lifetimes, so **no body ever lives through a change in its own conditions.** What it is for is ecology and chronicle: a population that rises and falls on a known clock is the first thing in this project that makes section 15's Q32 — does body size track the light or the population? — testable at all. |
| `famine` | Low influx — a world of a few hundred bodies rather than a few thousand. ⚠️ **It does not produce extinction, and that is a measured finding rather than an oversight.** At a tenth of the shipped light the population settles at about 210 and goes on turning over indefinitely, because how hard a body has to work to replace itself does not depend on how much light there is — only how many bodies the world can carry does. What *does* end a run is `upkeep_scale`: at 3 or above, a founder dies of old age before it has earned the reproduction threshold, and nothing is ever born. If a preset is wanted that demonstrates extinction, that is the slider it has to move. |

---

## 4. The resource field

A coarse grid, `grid_cols × grid_rows`, each tile holding a scalar energy value.

Per tick, for each tile:

```
offset     += patch_drift                                // section 4's drifting field, below
target      = cap × light_profile(y) × (1 + patchiness × noise(x + offset, y))
regrowth    = influx × light_profile(y)
tile        = max(tile, min(tile + regrowth, target))   // light only ever adds
```

then, after diffusion has run:

```
if tile > target:
    overflow  = tile - target
    tile      = target
    dissipated += overflow                       // field → dissipated
```

**Both halves of that are corrections to an earlier draft, and both matter.**

The original was `tile += min(regrowth, target - tile)`, with a comment reading "never
exceeds target". Written literally that is a *subtraction* whenever `tile > target` — light
running backwards, dragging the tile down and destroying the difference with no account to
put it in. That state is not hypothetical: ceilings fall with depth, so diffusion
permanently pushes energy into dimmer rows, and from Phase 4 detritus will decay into
already-full tiles.

But clamping light to a source and stopping there is not enough either, and the reason is
the whole point of the gradient. Light refills the top, diffusion carries energy downward,
and deep tiles then sit *above* their targets with nothing to bring them back. The field
fills until it is **level** — every tile at the deepest ceiling in the world, no depth
structure left at all. A run reaches that state; it is slow, but overnight runs are tens of
millions of ticks. A level field is one where swimming upward pays nothing, and section 4's
own claim that "the gradient is what gives movement a reason to exist" quietly stops being
true.

So a tile genuinely cannot hold more than its target, and energy that arrives somewhere it
cannot be held is **dissipated** rather than deleted. This is also the ecologically
familiar answer: energy sinking below the light is energy the system loses, which is what
the biological pump does in a real ocean. Total influx still bounds total living biomass,
so the carrying-capacity argument is untouched.

where

```
light_profile(y) = 1 - gradient × (y / height)
```

so light is strongest at the surface and dimmest at depth. **The gradient is what gives
movement a reason to exist** — without it there is no spatial structure for phototaxis to
discover, and swimming has no payoff.

Lateral diffusion runs after regrowth: a simple 5-point stencil at rate `diffusion`. This
smooths harvest shadows and prevents organisms from permanently strip-mining a single tile.

**`diffusion` must not exceed 0.25, and the ledger will not catch it if it does.** An
explicit five-point stencil overshoots above a quarter — a tile sent past its neighbours'
value, then dragged back further the next tick — and the overshoot compounds until the
numbers stop being finite. Energy stays perfectly conserved the whole way down, because
overshoot moves energy rather than inventing it, so the invariant reports a healthy world
right up until the field is nonsense. The config bound is therefore `0..=0.25`, not
`0..=1`, and it is a stability limit rather than a taste one.

The stencil must also be written as a list of **neighbour pairs**, one flow per pair applied
to both ends together, rather than as a per-tile gather over four neighbours. The gather
form has to special-case the world's edges, and the natural way to write that — treat a
missing neighbour as a tile holding nothing — silently drains the whole world out through
the surface and the floor. Measured on an evenly-filled test world: 70% of its energy gone
in 100 ticks. With pairs there is no route out of the world to write down, so there is none
to write down wrongly.

### ⭐⭐ The patches drift, and the speed is the whole design

**Swimming is possible without being worth anything.** That is what a 310,000-tick run
measured after section 8's anisotropic water landed: the mechanism works, a full-stroke
undulator manages 1.9 world units per 1,000 ticks, and the run still ended with one myocyte in
a population of 650 bodies. Nothing was wrong with the physics or with the economy. There was
simply **nowhere better to go**, because the field of blotches above was worked out once from
the seed and then never moved again. In a static field the optimal strategy is to sit still,
and sitting still is free.

So the field moves. `PatchNoise::at` takes a **coordinate offset**, the offset advances every
tick by `patch_drift`, and the tiles' ceilings are re-read off the moved lattice periodically —
through the same routine a live change to `[light]` uses, because there must be exactly one
place a ceiling is computed.

**The speed is not a taste.** Three things in this world move, and all three have been
measured:

| What moves | How fast | |
| --- | --- | --- |
| A lineage dispersing by **budding** | **0.0003** units/tick | one ~6-unit bud per ~1,500-tick generation |
| A tile **refilling** from empty, over its own width | **0.001** units/tick | 8 units in 8,000 ticks |
| An anisotropic **swimmer** | **0.0005 – 0.0025** units/tick | section 8's table |

A field drifting between the first and the second is **followable by a body that can swim and
not by one that can only bud**, and it does not outrun the light that has to refill behind it.
That is the entire design, and 0.0006 sits twice the budding speed and inside the swimming
band.

#### ⚠️ It must be an offset and never a reseed

Re-drawing the lattice heights from a moving seed also produces a field that changes, and it
is a completely different world: every blotch would vanish and be replaced somewhere else in a
single tick, **under a living population**. What a lineage had found would stop existing rather
than move, so there would be nothing to follow and no advantage whatever in being able to
follow it. With an offset, a blotch keeps its shape, its size and its neighbours and simply
arrives somewhere else.

The drift is **sideways only**. The lattice wraps sideways because the world does, so a
horizontal drift is seamless at every offset. Downwards it does not wrap — the world has a
surface and a floor — so a vertical drift would push blotches through the floor and invent
replacements at the surface, which is the reseed above applied at the two edges.

#### ⚠️ A drifting ceiling destroys energy, continuously, and the ledger cannot see it

A tile cannot hold more than its target, and what will not fit is `dissipated`. A drifting
patch field is a field of ceilings sliding sideways, so **every full tile whose ceiling has
moved down under it sheds the difference out of the world, every tick, for ever.** The loss
scales with `patchiness × patch_drift`.

That is accounted for and therefore invisible: energy leaving through `dissipated` keeps
section 5's invariant balanced to the last digit, so nothing in the program would catch a
drift set too high. It would simply be a world that was poorer than its `influx` said, with a
carrying capacity nobody could explain.

**Measured** on a field held full with nothing living in it — the worst case, since a
population has already eaten most of the field down below the level a falling ceiling can
reach — at the shipped `patchiness` and `patch_drift`: **0.0179 per tick against the 23.04 the
light offers, which is 0.08%.** It is negligible, and it is negligible for a reason worth
recording: a ceiling moves by `patch_drift × d(target)/dx` per tick, which at the shipped
settings is about a thousandth of a unit per tile, against tiles holding several units.

The bound `light.patch_drift ≤ 0.005` is where that stops being true. `d(target)/dx` is at most
`cap × patchiness × 0.0234` per world unit — the 0.0234 being the steepest a smoothstep between
lattice points 128 units apart can be — so at `cap = 8` and `patchiness = 1` the drift costs
`0.1875` per unit and the light delivers `0.001`, and the two are equal at 0.0053. It is a
bound on the *shipped* light rather than on any light, which is a real difference from
`diffusion`'s stability limit, and it is written as one number anyway because the interesting
range is a ninth of it.

**How often the ceilings are recomputed does not change what the drift costs.** The energy shed
per recomputation is proportional to how far the field moved since the last one, and the
recomputations happen in inverse proportion to the same interval. What the interval buys is
smoothness; the implementation uses 100 ticks, which at the shipped drift is a hundred and
thirtieth of a tile's width.

### ⭐⭐ The light rises and falls, and it does so on `influx` and on nothing else

The patches drift, so the best water is somewhere new. That is a field moving in **space**. A
season is the same field moving in **time**, and it exists for a reason that is not a payoff: a
world whose statistics never change is one in which being adapted is a fixed fact about a
lineage, and there is no version of *conditions changing* in a constant environment.

Per tick, `regrowth` is scaled by one scalar before it is offered:

```
phase      = (phase + 1 / season_period) mod 1        // accumulated, never derived from a tick count
season     = 1 + season_amplitude × triangle(phase)
regrowth   = influx × light_profile(y) × season
```

where `triangle` is nought at the start, `+1` a quarter of the way through, nought at the half
and `−1` three quarters through — the corners being the brightest and dimmest moments of the
year.

**A triangle rather than a sine, and it is not a stylistic choice.** One scalar multiplies all
36,864 tiles every tick, which is **36,864 times** the golden-vector exposure that `behaviour.rs`'s
single-spring `sin` carries, and a transcendental that moves in the last bit across a toolchain
change would move every recording this project has ever made. A triangle is exact in 64-bit
arithmetic at every phase, needs no library function, is mean-preserving over a period by exact
antisymmetry rather than by cancellation, is **exactly 1.0** at an amplitude of nought, and reads
identically to a sine at ±25%.

**The phase is accumulated rather than worked out from the tick count**, exactly as the drift's
own offset is and for the same reason: `[light]` is live, so somebody can turn `season_period` up
mid-run, and a `ticks / period` phase would teleport a living world into a different part of its
year the instant they did. Turning a dial changes the *speed* of the season and never where in
one the world is.

**And it is held at nought until the first organism is ever seeded.** The dawn that fills the
field stops on a **light-dependent** test — `gained / after < DAWN_SETTLED` — so a season running
through the dawn would change the dawn's *length*, and a seasoned run and a flat run would then
start their clocks at different ticks against different fields. It would also land the founders
of every shipped run at 0.83× and falling towards the trough 1.6 generations later, when every
survival figure in `docs/PHASE7.md` was taken on level light at the founding.

#### ⚠️ It is on `influx` because `influx` enters no ceiling

`relight` builds `regrowth` from `influx` and `targets` from `cap × light_profile × (1 + patchiness
× noise)`. **`influx` appears in no target.** So a season needs no retarget, moves no ceiling down
under a full tile, and sheds no spill whatever — which is the whole energy argument, and it is
what makes a season a different kind of object from the drift above. Measured over **fifteen whole
periods** of a living world, world ticks 15,000 to 330,000:

| Over fifteen whole periods | flat | ±25% | ±50% |
| --- | --- | --- | --- |
| `influx_total` | 6,978,241 | 6,951,729 (**−0.38%**) | 6,926,165 (**−0.75%**) |
| `dissipated` | 7,053,272 | 7,025,634 (**−0.39%**) | 6,996,081 (**−0.81%**) |

**Both fall, and both by a fraction of a per cent.** That pair of signs is the detector: a season
on the *income* lowers `influx_total` slightly, because a dim half-cycle fails to deliver a little
into tiles that were not full; a season on a *ceiling* would raise `dissipated` instead, and would
raise it by the spill it created.

The same amplitude applied to `cap` instead adds **0.656 per tick** of ceiling spill — 4.3% of
throughput — swings the standing field by **90.7%**, and silently rescales a sensocyte's fixed
0.02 reference against a field that has moved by that much. Nothing in the program would catch
it: energy leaving through `dissipated` keeps section 5's invariant balanced to the last digit,
which is the same blindness this section already records about a drifting ceiling. **A season on
`cap`, `gradient`, `patchiness` or `metabolism.upkeep_scale` is refused for that reason and not
for a stylistic one**, and the last of those is the worst: section 3's sweep records that
`upkeep_scale` at ×3 and ×4 kills every world it is applied to before a single birth.

#### The window the period has to sit in, measured

| | Ticks | Why |
| --- | --- | --- |
| **Floor** | **8,000** | `cap / influx` — the time a tile takes to fill from empty. Below it the light changes and the water does not: measured, the standing field swings **2.04%** at a period of 2,000 against **6.74%** at 20,000 |
| **Shipped** | **21,000** | 12.0 generations, so **6.0 generations per half cycle**; 2.4 whole cycles inside a median species life; and `gcd(21,000, 25,000) = 1,000`, so this project's 25,000-tick checkpoints walk **21 distinct phases** instead of the four a period of 20,000 would give them |
| **Ceiling** | **~50,500** | the median species lifetime, above which a lineage lives entirely inside one half cycle and what it experiences is a trend rather than a season |

The gate refuses anything below the floor and **nothing** above: a million-tick climate is a
legitimate experiment, and an invented upper bound is one somebody argues with on the evening an
experiment is refused. Zero is refused along with everything else below the floor, because
`season_amplitude` is the off switch and a second one is two ways of saying the same thing.

#### What the amplitude costs the population, against the right control

The first reading of this compared the seasoned **trough** against the flat run's **mean**, which
overstates the depth by more than double. **The flat world already swings by a factor of about two
and a half on its own**, so its own second-half minimum is the only control worth comparing a
seasoned trough against. Measured over the second half of a 330,000-tick run at 5,000-tick
resolution:

| | flat | ±25% | ±50% |
| --- | --- | --- | --- |
| second-half **low** | **517** | 468 | 529 |
| second-half **high** | **1,234** | 2,109 | 2,202 |
| peak to trough | **2.39×** | 4.51× | 4.16× |
| mean alive over the run | **1,099** | 1,369 | 1,454 |

⚠️ **Read the first row as *the trough does not deepen much* and no further.** At 5,000-tick
sampling a trough is undersampled, and the ±50% run's low is above the ±25% run's — which is
noise, not a reversal. What the table does establish is the thing the bound rests on: a seasoned
world does not collapse, and it carries **more** organisms on average rather than fewer, because
the same energy goes into more and smaller bodies (5.65 cells a body flat against 3.32 at ±50%).

⚠️ **The bound of 0.5 is where the evidence stops and it is not a drift argument.** With a trough
of a few hundred bodies and the largest real selection coefficient in this world at 0.85
%/generation, `N·s` is comfortably above 1 at every amplitude the gate allows and for some way
past it — drift does not outrun selection anywhere in the range. A bound is still not optional:
above 1.0 the multiplier goes negative, which is light running backwards.

#### ⚠️ And the honest headline: the season does not change what shape is worth

Run under a ±25% season at period 21,000, three seeds, two whole periods, the competition assay
returns the coefficient on the largest free shape change the genome can express at **+0.88
%/generation**, against **+0.71** flat. The difference is 0.17 against a seed-to-seed spread of
±0.5. **There is no detectable effect.** The reason is in the floor above: the fastest the
standing field can change is its own 8,000-tick filling time, which is 4.6 lifetimes, so no body
ever lives through a change in its own conditions. Build a season because a world that varies is
the world this project is about; do not build one expecting a muscle.

**Carrying capacity.** Total influx per tick is fixed, so total living biomass is bounded by
it. This is the pressure that drives everything else in the simulation and is the reason
`influx` is the single most consequential slider.

---

## 5. The energy ledger — load-bearing

Energy is conserved and **asserted every tick**. Unbalanced energy is the most common way
these simulations quietly become nonsense.

Five accounts:

| Account | Contents |
| --- | --- |
| `field` | Energy in resource grid tiles |
| `biomass` | Energy stored in living organisms |
| `detritus` | Energy in dead biomass, not yet returned to the field |
| `dissipated` | Energy permanently gone: spent on metabolism and movement, or drained out of a field tile that could not hold it (section 4) |
| `influx_total` | Cumulative energy added by light since `t = 0` |

A sixth value, `initial_total`, is captured once at construction — the energy the world was
born holding. It is not an account, because nothing ever moves into or out of it, but the
invariant needs it and it has to be stored somewhere.

Invariant:

```
field + biomass + detritus + dissipated  ==  initial_total + influx_total   (± 1e-3 relative)
```

**Relative to `initial_total + influx_total`**, with a floor of one energy unit. The floor
matters: without it, a world holding nothing is being asked for bit-exactness, which no
float arithmetic can promise.

Note what the invariant does *not* claim. It says energy is conserved, not that any
particular account is solvent. Spending more than an organism holds drives `biomass`
negative while the books still balance perfectly — those are different claims, and
solvency belongs with the organism in section 10, not here.

Checked every tick in debug, every 1,000 ticks in release. **On violation, panic.** Eight
hours of quietly wrong output is worse than a crash.

### The five accounts are `f64`. This is an exception to section 2, and it is required.

Section 2 mandates `f32` for simulation *state*, for GPU compatibility. The ledger accounts
are not state in that sense — they are running totals over the whole life of a run, and at
`f32` they stop working long before an overnight run finishes.

The numbers below were measured, not estimated — see
`an_f32_account_would_have_stopped_counting` in `ledger.rs`, which pins them.

`f32` carries 24 bits of mantissa, so a running total loses its ability to absorb small
additions long before it overflows. At the default config, light offers `0.001 × 36,864 ≈
36.9` per tick to `influx_total`. Two things then happen, in this order:

- **At tick 90,996** — a couple of minutes of wall clock — accumulated rounding has pushed
  the two sides of the invariant more than `1e-3` apart, and the run panics. Nothing is
  wrong with the simulation; the accumulator simply cannot represent what it has been told.
  This is the number that forces the decision.
- **At tick 24,501,362** the total reaches 1,073,741,824 and freezes completely. The gap
  between neighbouring `f32` values there is 128, more than twice the 36.9 being added,
  so every subsequent addition rounds straight back to where it started.

The frozen total sits above the true figure, so there is no version of this that announces
itself as an obvious error.

Both numbers moved when Phase 4 lowered `light.influx`, and they moved in opposite
directions: the freeze came later, because a smaller amount per tick takes longer to reach a
total coarse enough to swallow it, and the failure that actually matters came *sooner*,
because how far a running total drifts from the truth depends on how many additions have been
made and barely at all on their size. At the original `0.012` the two figures were 121,128
and 17,780,259.

The same loss applies to summing 36,864 `f32` tiles to compute `field`, on every single
tick.

So: **tiles stay `f32`; the five accounts are `f64`; and the per-tick sum over tiles
accumulates in `f64`.** The GPU port in section 14 must do the same — a reduction over
tiles returns to the CPU as a `f64` total, or is performed in two stages to avoid the same
loss. This costs nothing on the GPU side, because the accounts are never read by a shader.

Flow:

- Light adds to `field` and `influx_total`. This is the only operation in the simulation
  that creates energy; everything below moves it.
- **Seeding an organism takes its starting energy out of `field`**, exactly as harvesting
  would. Easy to miss, because a seeded organism feels like it comes from outside the world.

  ⚠️ **And the invariant will not catch you.** An earlier draft of this section claimed a
  conjured seed shows up as an invariant failure. It was tested, and it does not. An organism
  whose energy was never *told* to the ledger leaves all five accounts exactly as they were,
  so the books balance perfectly while a body stands in the world holding energy nobody
  counted. It stays silent until something moves that energy out of a `biomass` account that
  never received it — hours into a run, with no cause to find.

  What guards this is not the invariant but a test asserting the **field went down**. The
  general lesson, which Phase 2 learned twice: a conservation check cannot see energy that
  was never declared, only energy that was declared wrongly.
- Photocytes move `field → biomass`.
- Devorocytes move `detritus → biomass`, or `biomass → biomass` when predating.
- Metabolism and movement move `biomass → dissipated`.
- A field tile holding more than its target moves the excess `field → dissipated` (section 4).
- Death moves `biomass → detritus`.
- Detritus decays at a fixed rate into the field tile beneath it, moving `detritus → field`,
  while sinking slowly (this is the marine snow, which is both atmospheric and functional).
- **An organism that dies owing moves `dissipated → biomass`, for exactly what it overspent.**
  Added in Phase 4. This is the one movement that runs backwards out of `dissipated` and the
  only one there will be. It follows from the paragraph above about solvency: while an
  organism is alive, spending more than it holds is a fact about the organism and not a
  bookkeeping error, so `spend` allows it and `biomass` goes red. Once the organism is gone
  that red figure belongs to nobody, and left alone it accumulates over a night's worth of
  deaths into a living-biomass account that is large and negative while every organism in the
  world holds something positive. So the last thing a dying organism does is fail to pay: the
  spending that was never affordable is undone, and the account goes back to saying what the
  living are holding.

---

## 6. Cells

```rust
struct Cell {
    pos: Vec2,
    vel: Vec2,
    radius: f32,        // derived from cell type
    kind: CellKind,
    state: u8,          // developmental identity, 0..=63
    gene: Option<u8>,   // which gene of its genome made it what it is; nothing for the
                        // seed cell. Added in Phase 7 — see section 7
    energy_flow: f32,   // last tick's net gain, for rendering brightness
}
```

### Cell kinds

The trade-offs matter more than the list. Each kind must be *worth* specialising into under
some circumstance and not others, or differentiation never evolves.

| Kind | Radius | Upkeep/tick | Toughness | Buoyancy | Function |
| --- | --- | --- | --- | --- | --- |
| `Photocyte` | 3.0 | 0.004 | 0.10 | **−0.50** | Harvests from the field tile it occupies, rate ∝ local energy × exposure. **Occluded by cells above it** — this is what rewards spread-out, branching body plans over compact blobs. |
| `Devorocyte` | 2.6 | 0.009 | 0.30 | **+0.80** | On contact, drains energy from detritus, or from another organism's cells at a rate reduced by that cell's toughness. |
| `Myocyte` | 2.8 | **0.005** ⭐⭐ | 0.30 | **0.00** | Oscillates the rest length of its springs. Costs `movement_cost` × work done. The only source of locomotion. **The one upkeep in this table that has been measured rather than guessed — it was 0.014; see below.** |
| `Sclerocyte` | 3.4 | 0.002 | 0.90 | **+1.00** | High spring stiffness, high toughness. No metabolic function — pure structure and defence. |
| `Sensocyte` | 2.0 | 0.006 | 0.00 | **−0.20** | Samples a local gradient (light, detritus, or foreign biomass — determined by its gene) and emits a scalar signal. |
| `Gonocyte` | 3.2 | 0.005 | 0.10 | **+0.50** | Accumulates energy toward reproduction. An organism with no gonocyte cannot reproduce. |

Toughness was decided in Phase 4 and buoyancy in Phase 7; SPEC gave neither. Both live beside
the radius and the upkeep because **a kind is a trade-off**, and a trade-off split across
columns nobody puts next to each other is one nobody can see.

**Design intent:** photocytes want surface area and light, which means being high up and
spread out. Devorocytes want contact, which means reaching things. Myocytes cost energy but
are the only way to reach either. Sclerocytes are the answer to predation but contribute
nothing. Requiring a gonocyte means reproduction has a real structural cost. If in play-
testing one kind dominates every lineage, the costs are wrong — tune before adding kinds.

### ⭐⭐ A myocyte's upkeep is 0.005 and was 0.014, and it is the only measured number in the table

The sentence immediately above is the one this change was made under, and the condition it
names is met: **photocytes and gonocytes are 99.7% of every cell in every run this project has
recorded.** So the price of the one kind that has never established itself was swept, and what
came back is a partial answer worth having in full, because the null half of it is the more
useful half.

#### Why the price was the thing to try

Four rounds of work had made swimming physically possible (section 8's anisotropic water),
energetically cheap (`movement_cost`, a thousandfold down), sensorily visible (section 9's
fixed light reference) and behaviourally wired (section 7's gene-that-built-it). Nothing swam.
The remaining obstacle had the shape of **a fitness valley with a lethal floor**:

- One myocyte oscillating one spring is a **reciprocal** stroke, and section 8's water gives a
  reciprocal stroke exactly no net displacement. That is the scallop theorem and it is measured
  in this project, not assumed.
- So locomotion needs **two muscles at different phases on a bent body** before it produces
  anything at all.
- Meanwhile a myocyte cost **0.014 a tick against a photocyte's 0.004** — three and a half
  times — and earned nothing.

The first muscle therefore paid nothing and cost a great deal, so it was removed before the
second could arrive beside it. And **0.014 was never measured**: it was written before anything
ran, at a time when `movement_cost` was 0.15 — a value section 3 records as making the *use* of
a muscle arithmetically impossible, so upkeep was the only thing pricing one. `movement_cost`
moved by a thousandfold in Phase 7 and the standing cost was never revisited.

#### The sweep

Six runs of 300,000 ticks after the dawn, shipped configuration, seed 42, eight founders, one
price each. The 0.014 column reproduces `docs/PHASE7.md`'s Group J run **bit for bit**, which
is what makes the other five comparable to everything else in this document.

| Myocyte upkeep | 0.014 | 0.010 | 0.007 | **0.005** | 0.004 | 0.002 |
| --- | --- | --- | --- | --- | --- | --- |
| Against a photocyte's 0.004 | ×3.5 | ×2.5 | ×1.75 | **×1.25** | ×1.0 | ×0.5 |
| **Myocytes per body**, over the run | 0.00097 | 0.00094 | 0.00135 | **0.00205** | 0.00186 | **0.00797** |
| **Bodies carrying ≥2 myocytes** | 0.0073% | 0.0064% | 0.0085% | **0.0345%** | 0.0147% | **0.0861%** |
| *Births* carrying ≥2, of ~255,000 | 95 | 54 | 62 | **112** | 67 | 187 |
| Mean life of a ≥2 body, ticks | 325 | 557 | 631 | **1,380** | 1,053 | 2,050 |
| **Mean displacement per lifetime** | **3.966** | 4.030 | 3.750 | **3.742** | 3.766 | **3.627** |
| Myocytes, of all living cells | 0.033% | 0.031% | 0.047% | **0.069%** | 0.068% | **0.278%** |
| Photocytes / gonocytes | 66.0 / 33.7% | 66.5 / 33.3% | 65.0 / 34.8% | **66.4 / 33.3%** | 63.7 / 36.1% | 64.8 / 34.6% |
| Devorocyte cell-observations | 1,535 | 1,185 | 1,219 | **1,445** | 1,031 | 1,035 |
| Population at 300,000 | 848 | 659 | 844 | **826** | 744 | 832 |
| Mean cells | 6.09 | 7.99 | 6.00 | **6.21** | 6.94 | 6.28 |
| Biomass | 32,687 | 32,855 | 32,201 | **32,276** | 32,962 | 32,125 |
| Myocytes alive at the end | 0 | 1 | 1 | **1** | 10 | **42** |
| Largest myocyte count in one body | 22 | 11 | 12 | **14** | 21 | **46** |

#### ⭐⭐ What the price buys is persistence, and not supply

**The share of births carrying two or more myocytes is 0.02% to 0.04% at every price and has no
trend in it whatever** — 95, 54, 62, 112, 67, 187 out of a quarter of a million births apiece.
Price cannot change that number, because how often a mutation makes a second myocyte is a fact
about `mutation.rs` and not about the ledger.

What price changes is **how long such a body lasts**: 325 ticks at 0.014 and 1,380 at 0.005, a
factor of four. That is not a subtlety of selection, it is section 10's lifespan — an
organism's allowance is `LIFETIME_UPKEEP × cells ÷ what it costs per tick`, so cheaper tissue
lives proportionally longer *by construction*. The standing population of muscle is those two
multiplied, and it rises 4.7-fold.

So the valley floor was real and it has come up. **The near side of it is empty**, and that is
where the next experiment belongs.

#### ⚠️ Nothing swims at any price, and the sweep contains its own control

Mean displacement per lifetime is **3.74 at the shipped price against 3.97 at the old one** —
down, not up, and every column is inside the spread of every run since Group I.

Splitting those quarter-million lifetimes by how many myocytes development gave the body looks
at first like a signal: bodies with two or more travel about 6.1 world units against 3.7 for
bodies with none, at every single price. **It is not one.** The one-myocyte bucket is the
built-in control — a single muscle is a reciprocal stroke and provably cannot produce net
displacement — and at 0.002, where that bucket is large enough to read (997 lifetimes), it
travels **7.19 units, further than the two-muscle bucket does.** What the excess measures is
that a body with any myocyte in it is a bigger and longer-lived body than the two-celled median,
which is a fact about body size and not about locomotion.

#### ⚠️ The bloat boundary is between 0.004 and 0.002, and it is why this stops above a photocyte

CLAUDE.md names the failure to watch for: a cell that becomes free accumulates without limit.
**At 0.002 — a sclerocyte's price, and below the cell that earns the world's entire income —
it starts.** Myocytes rise through the run rather than fluctuating (7, 22, 51, 42 over the last
four checkpoints, against the shipped world's 2, 2, 0, 0), reach **2.4% of bodies over the run's
last 25,000 ticks** against 0.10% in the shipped world, are 0.28% of every living cell over the
whole of it, and the largest body in the world carries **46 of them against a cap of 64** —
while mean displacement is the *lowest* reading in the sweep. That is a world accumulating a
cell that does nothing, which is the definition of neutral bloat.

Nothing of the kind happens at 0.005: a mean of one myocyte per five hundred bodies, no trend
across the run, and no body ever holding more than fourteen — against twenty-two at the price
this replaces. **The line is the photocyte**, and a muscle has to stay dearer than the cell
paying for it or owning one is free.

#### Why 0.005 rather than one of the others

- It is the **largest measured effect among the safe prices**: myocytes per body double, the
  ≥2 fraction goes up 4.7-fold, and a two-muscle body lives four times as long.
- It stays **above a photocyte**, which the 0.002 run shows is where accumulation begins. 0.004
  is exactly a photocyte and is the first price at which the end-of-run count drifts upward
  (0, 4, 5, 10 over the last four checkpoints).
- It restores the split the two settings were always meant to have. A body driven flat out
  does about 95 units of work a tick; at `movement_cost` of 1e-4 that is 0.0095 against the
  0.030 its six myocytes cost to own. **Working a muscle is now about a third of owning one,
  where it was a ninth** — *carrying the machinery is cheap and using it costs*, which is what
  section 3's `movement_cost` argument assumed and the standing cost had never been brought
  into line with.
- **The ecology does not notice.** Biomass 32,276 against 32,687, population 826 against 848,
  mean body 6.21 against 6.09, photocytes 66.4% against 66.0%, devorocytes unmoved. The
  carrying-capacity claim holds for the sixth change running.

⚠️ **It is exactly a gonocyte's upkeep, and nothing rests on that.** A muscle and a store of
reserves costing the same is a coincidence of two independently chosen numbers, not a claim.
What is load-bearing is only that it sits above the photocyte and below the devorocyte.

⚠️ **And this did not produce a lineage that swims**, which is the honest headline. It is the
fifth change in a row not to. What it establishes is that the *price* is not the remaining
obstacle: the standing cost has been cut to a quarter of a photocyte's premium and the number
of bodies born with the configuration locomotion needs did not move at all. Two candidates
follow from that and neither is a price — **a single muscle doing something useful that is not
locomotion**, such as changing a body's shape and therefore its self-shading, so that the first
one pays; or **making the second muscle reachable in one mutation rather than two**. Both are
argued in `docs/PHASE7.md`'s Group K and neither is built.

### ⭐⭐⭐ What each of these cells is actually worth, measured

**The first of the two candidates above was built as a design and refuted before a line of it was
written, by an instrument that prices a body plan in forty minutes instead of a day.** The
competition assay — `crates/coacervate-app/src/assay.rs` — seeds two founder sets that differ by
**exactly one mutation** alternately into the shipped world after the dawn, attributes every
organism born afterwards to the arm its parent belonged to, and reads the ratio of living
descendants after 42,000 ticks (23.9 generations). Its noise floor is **±0.16 %/generation** and
it resolves about **0.3**.

| Arm B, against an identical arm A | upkeep added | descendant ratio | **coefficient** |
| --- | --- | --- | --- |
| a third **photocyte** | +0.004/tick | 1.076 | **+0.04 %/gen** — neutral |
| the longest **`rest_length`** the genome can ask for (8.0 → 13.6) | none | 1.297 / 1.063 / 1.289 | **+0.71 %/gen** |
| a third **sclerocyte** | +0.002/tick | 0.855 / 0.755 | **−1.07 %/gen** |
| a third **myocyte**, holding still | +0.005/tick | 0.593 | **−2.46 %/gen** |
| a third **myocyte**, beating at 2.5 rad/s | +0.005/tick | 0.516 / 0.355 | **−2.7 to −4.4 %/gen** |
| a third **devorocyte** | +0.009/tick | 0.126 / 0.236 | **−6.1 to −9.0 %/gen** |
| a **myocyte with an adhered sensocyte** | +0.011/tick | **0.097** | **−8.6 %/gen** |

The world keeps extra photocytes — 3.28 cells a body after 24 generations against the control's
1.98 — and **sheds every other kind of cell inside two dozen generations**: 2.22 for a sclerocyte,
2.03 for a myocyte, 2.04 for a devorocyte, 2.04 for the muscle-and-sensor pair.

> ⚠️⚠️ **Nothing in this world has an increasing return to being more than one thing.** A
> photocyte's income scales linearly with photocyte count, upkeep scales linearly with cells,
> section 10's reproduction threshold is linear in cells and section 10's lifespan is linear in
> cells. Occlusion is actively *sub*linear, because a bigger body self-shades more. So growth is
> a random walk and specialisation is a pure loss, at about **−0.5 %/generation for every
> 0.001/tick of upkeep, whatever the cell does.**

**The arithmetic that closes the muscle question.** A muscle must earn **+2.5 %/generation** to
break even. The entire measured value of shape in this world — the largest free shape change the
genome can express, taken in full and for nothing — is **+0.85 %/generation**. A beating muscle
shifts its body's mean geometry by 0.8% against `rest_length`'s 70%, so its share of that channel
is about 1%: **+0.01 against −2.7, a ratio of 1 : 270.** Even a muscle that could hold a shape
perfectly captures at most the whole channel, **+0.85 against −2.5** — and `rest_length` already
collects it for nothing, one point mutation away. This is why a self-shading payoff is not built,
and it is a measurement rather than an opinion.

⚠️ **Two things every coefficient here must be quoted with.** It measures the **filling** regime —
two-celled bodies, a population rising towards 2,100 — and section 15's 300,000-tick world holds
6.21 cells per body and 826 organisms, where own-body shading is a much larger share of a
photocyte's income. And a ratio near 1.0 over the first 40,000 ticks can mean *still filling*
rather than *no effect*, which is why the noise-floor arm exists and why every figure above is an
excess over its own same-seed control.

⚠️ **The photocyte row does not reproduce and the honest note belongs here.** Rebuilt from the
public API with arm B being the founder plus one appended gene, that arm comes back at **1.531**
rather than 1.076 — `+1.52 %/generation` against a noise floor of ±0.16. The noise floor itself
reproduces (+0.063 against a recorded +0.064 at seed 42) and the myocyte row reproduces (−3.07
against −2.46 %/gen), so the instrument agrees with itself; what differs is what a *third cell*
was made of. Doubling a body's photocytes while adding 44% to its bill is not neutral. What
survives, and it is the load-bearing half, is the **pair**: the same three-celled body differing
in one `child_kind` keeps the earning cell over the silent one at three to one, and the arm that
was given a muscle is back to 2.14 cells a body having been born with three. `docs/PHASE7.md`'s
Group L has both readings.

### ⭐⭐ Buoyancy, and why depth is a property of composition

Section 8 rules out the obvious way to make depth cost something. A uniform gravity is an
*external* force, so a body does move — straight down, at a terminal velocity, and **there is
nothing a muscle can do about it**, because a muscle only ever makes internal forces and those
cancel in the sum however cleverly they are arranged. Measured: a body contracting hard against
gravity fell 0.1% slower than the same body holding still.

Buoyancy tied to composition is the version that works, and what changes is not the physics but
where the number comes from. A body's resting depth becomes a function of **what it is made
of**, which is a function of its genome; reaching a different depth takes *one* mutation to one
gene's `child_kind`, which is the smallest step the mutation operators have; and there is no
valley on the way, because every intermediate composition has an intermediate depth.

**The founder is exactly neutral, and that is a constraint rather than a coincidence.** Every
run begins with section 7's seed cell — a photocyte — and the one gonocyte without which
nothing can reproduce. `−0.50 + 0.50 = 0`, so that body sits precisely where it is put, no
world drowns the moment this column arrives, and every depth recorded before it still means
what it meant. **What moves a lineage is departing from that composition**, which is exactly
what selection is being asked to do.

Each row, and the argument for it:

- **A photocyte floats.** It is the cell whose whole function is to be where the light is, and
  floating is how real phytoplankton stay there — buoyant inclusions rather than work. This is
  the entry that makes the trade-off exist: **staying shallow means being mostly photocyte**,
  which competes directly with spending the same cells on muscle, armour or teeth.
- **A gonocyte sinks by exactly as much.** It is a store of packed reserves, which is dense —
  and the founder constraint requires it, because the alternatives are a weightless photocyte,
  which removes the whole effect, or a seventh cell to balance it, which no body has.
- **A sclerocyte is the densest thing in the world.** Structural, mineralised tissue. A lineage
  that answers predation with armour sinks away from the light while it does it, which is what
  stops armour being free to a body nothing was eating.
- **A devorocyte is dense, a little less so.** ⭐ This is the pairing the column is pointed at:
  a devorocyte-heavy body sinks into dim water, where photosynthesis pays badly and eating pays
  well. That is a *reason* for a feeding split to appear rather than a decree that one should,
  which is the difference the decision log draws about predation being emergent.
- **A myocyte is exactly nothing**, and it is the one row chosen against the others rather than
  from what the tissue is. Contractile tissue is close to the density of the water it works in,
  which is the physical answer; writing it as *zero* is the design one. A muscle that floated
  or sank would be a way of changing depth **without swimming**, which is precisely the thing
  section 4's drifting field exists to make worth doing. Muscle still costs a body its lift, by
  diluting the photocytes it is made of; it does not earn any.
- **A sensocyte is very slightly buoyant.** The smallest cell in the world, at a radius of 2.0
  against a sclerocyte's 3.4, with the least of anything in it — a fifth of a photocyte's lift,
  enough that a light-seeking body is not fighting its own sensors and far too little to be
  useful as a float.

#### ⚠️ The magnitude is the whole risk

Section 8's diagnostic measured a uniform sink of `g ≈ 5` putting a population on the floor in
**forty generations**, and `g ≈ 50` in three. Buoyancy is a *net* force from an unbalanced
composition, so what matters is the mean over a body rather than any figure in the column.
Measured through the real physics, as the drift of a body's centre over a full 2,000-tick
lifetime:

| Body | Mean buoyancy | 571 ticks | 2,000 ticks |
| --- | --- | --- | --- |
| **Founder** — 1 photocyte, 1 gonocyte | **0.000** | **0.000** | **0.000** |
| Photocyte-heavy — 7 photocytes, 1 gonocyte | −0.375 | −0.33 | **−1.20** |
| Devorocyte-heavy — 6 devorocytes, 1 photocyte, 1 gonocyte | +0.600 | +0.67 | **+2.33** |
| Sclerocyte-heavy — 6 sclerocytes, 1 photocyte, 1 gonocyte | +0.750 | +0.82 | **+2.87** |

Half a cell's width in a lifetime at the extreme, and nothing at all for the body every run
starts with. **A lineage crosses this world over hundreds of generations or not at all**, which
is the timescale selection acts on rather than the timescale a body falls on.

Two consequences of implementing it as a *force* rather than a velocity are worth knowing,
because neither needed writing down separately. It goes through the same drag everything else
does, so a cell in the middle of a chain — which has a body axis, and whose axis lies across
the pull when the body lies flat — sinks at about half the rate a loose cell would: **a long
flat body settles more slowly than a compact one**, which is what a long flat body in water
does. And a cell held against the surface or the floor stays there, because section 8's
boundary takes its vertical motion away every tick.

---

## 7. The genome — load-bearing

A genome is an **ordered, variable-length list of genes**, each a condition-action rule
fired during development. Fixed-size records so they pack into flat arrays.

```rust
struct Gene {
    // --- condition ---
    trigger_state: u8,     // fires on cells with this state
    min_step: u8,          // earliest development step this may fire
    max_step: u8,          // latest
    // --- action ---
    action: Action,        // Divide | Differentiate | Terminate
    // Divide
    angle: f32,            // radians, relative to the parent's body axis
    adhere: bool,          // does the daughter stay physically attached?
    child_state: u8,
    child_kind: CellKind,
    rest_length: f32,      // spring rest length if adhered
    stiffness: f32,
    // Differentiate
    new_kind: CellKind,
    new_state: u8,
    // Behaviour (used by Myocyte / Sensocyte)
    osc_freq: f32,
    osc_phase: f32,
    sensor_gain: f32,      // signed — sign determines attraction vs avoidance
    sensor_target: SensorTarget,  // Light | Detritus | ForeignBiomass
}
```

Why this shape: because conditions key on `state`, **duplicating a gene and changing its
`trigger_state` creates a new body part**. That single operator — duplicate, then diverge —
is the mechanism behind essentially all real biological complexity, and it is the entire
reason the genome is a variable-length rule list rather than a struct of parameters.

### ⭐⭐ The last four fields belong to the cells the gene *built* — decided in Phase 7

`osc_freq`, `osc_phase`, `sensor_gain` and `sensor_target` sit in the same fixed record as
`child_kind` and `new_kind`, and one record describes one thing: **a gene that divides a
parent into a myocyte says how that myocyte oscillates**, and a gene that differentiates a
cell into a sensocyte says what that sensocyte is tuned to. Development stamps the position of
the gene onto every cell it makes (`Divide`) or re-makes (`Differentiate`); section 9's
controller reads that back. `Terminate` does not stamp, because stopping a cell says nothing
about what it is.

**Phase 4 read it the other way and the measurement is why it changed.** Behaviour was looked
up by the first gene whose `trigger_state` matched the cell's `state` — development's own rule
with the step window taken off — on the grounds that a state is what a genome uses to say what
a cell *is*. Measured on the shipped world, seed 42, eight founders, over a 300,000-tick run
sampled every 200 ticks — **6.46 million cell-observations**:

| Cells in a state their own genome names | |
| --- | --- |
| Every living cell | **40.4%** |
| **Every cell except the seed cell it grew from** | **2.2%** |
| photocyte / devorocyte / myocyte / sclerocyte / sensocyte / gonocyte, grown | 2.5% / 0.9% / 2.1% / 3.4% / 2.7% / 2.0% |

The 40% is almost entirely the seed cell, which is in state 0 because the development loop
below puts it there and which the founder's own gene names. **Take the seed cells out and the genome answers 2% of
its own body.** A state is one of 64, a genome of that age holds three to five genes,
`trigger_state` is not where mutation spends its time, and `child_state` scatters daughters
across the whole range — so a cell grown into a state nothing is listening to is the ordinary
case rather than the exception.

What is given up is that a duplicated gene can no longer take a *standing* cell's behaviour
over by naming its state. What is kept is what duplication is actually for: duplicate a
dividing gene, point the copy at another state, and the new body part it grows arrives **with
its own rhythm**, because the rhythm travels with the gene rather than being looked up
afterwards. Gene order still decides which gene builds a cell, so order still carries
information.

**The seed cell has no gene, and it is the one cell that needs none.** Nothing put it there
but the model, so it is given *nothing* rather than gene zero or a default rhythm — the same
answer a founder's missing parent gets. Nothing is lost by that, and it is provable rather
than hoped for: the only two ways a cell can become a myocyte or a sensocyte are a gene's
`child_kind` and a gene's `new_kind`, and both stamp the gene on, so **a cell with no gene is
always a photocyte**. `development.rs`'s `a_cell_with_no_gene_is_the_seed_cell_and_needs_none`
is that argument as a property test over arbitrary genomes.

⚠️ **`trigger_state` still decides development, and the 2.2% above is a fact about development
too.** Development also stops at a cell whose state no gene names — which is why bodies in this
world sat at a mean of 2.0 cells for the first 140,000 ticks of every run ever measured. ⭐⭐ **That
half was answered differently and the answer is below, under Mutation**: development's rule is
untouched, because this section's whole justification for the genome design rests on it, and what
changed instead is the *distribution* a re-drawn state comes from.

### Development

Deterministic, bounded, and pure:

```
cells = [ Cell { state: 0, kind: Photocyte, pos: origin, gene: none } ]
for step in 0..max_dev_steps:
    for each cell in cells (in stable index order):
        find the FIRST gene where
            gene.trigger_state == cell.state
            && step within [gene.min_step, gene.max_step]
        if none: continue
        apply the action:
            Divide      → append a daughter at `angle` from the parent's axis,
                          with child_state / child_kind; if `adhere`, create a spring.
                          The daughter's `gene` is this gene
            Differentiate → change this cell's kind and state in place, and its `gene`
                          to this gene
            Terminate   → mark this cell inert; it fires no further genes. Its `gene`
                          is untouched — stopping a cell says nothing about what it is
        if there is no room for another cell: stop entirely
```

Four things that pseudo-code leaves open, decided in `development.rs` and recorded here so
they are not re-litigated:

- **⚠️ A cell whose state no gene names is a cell development can do nothing further with**,
  and this rule is unchanged and is not going to change: it is what makes a `state` an address,
  and this section's whole argument for a variable-length rule list rests on it. **What was
  changed is how likely a genome is to be addressing its own cells at all.** It used to be
  **2.2%** of grown cells, by the measurement above, which is why the founder is two cells — its
  one gene hands the daughter `child_state = 1` and nothing names 1 — and why mean body size in
  this world was 2.0 for the first 140,000 ticks of a run. The distribution a re-drawn state
  comes from is now biased; see **Mutation** below, and `docs/PHASE7.md`'s Group J for the run.

- **The cap is checked *before* a daughter is made, not after.** An earlier draft appended
  and then compared `cells.len() == max_cells_per_organism`, which is correct for every cap
  but one: a body allowed a single cell already starts at its cap, so the check arrives too
  late and the first division takes it to two.
- **"The parent's body axis" is the direction a cell was budded in**, fixed at birth. The
  seed cell has no parent and faces `+x`. Angles therefore *compound* down a chain, so one
  gene firing repeatedly draws a curve, a spiral or a ring rather than only a straight line
  — a strictly larger space of shapes from the same genome length, which is what the
  duplicate-and-diverge bet needs.
- **A daughter made during a step does not act until the next one.** A step visits the cells
  present when it began, so a step is a *generation* and a dividing genome doubles per step.
  Under the other reading a single self-perpetuating gene fills the whole body inside step 0
  and `min_step`/`max_step` on every other gene become nearly meaningless.

The body is a **pure function of the genome**. Same genome, same body, every time — which
makes it trivially testable and means the museum can rebuild any archived organism exactly.

First-match-wins ordering means gene *position* carries information, so reordering is a
meaningful mutation. Non-firing genes accumulate as neutral genetic material, which is
exactly where duplication finds raw material to diverge — this is a feature, not waste.

### Mutation

Applied at reproduction, in this order:

1. **Point mutation** — each gene, with `point_rate`: perturb numeric fields by
   `N(0, point_sigma)`; discrete fields re-draw uniformly. ⭐⭐ **Except the three that are
   states, two of which re-draw from a mixture — see immediately below.**
2. **Gene duplication** — with `duplication_rate`: copy a random gene, insert adjacent.
3. **Gene deletion** — with `deletion_rate`: remove a random gene.
4. **Gene insertion** — with `insertion_rate`: insert a fully random gene.
5. **Reordering** — with `reorder_rate`: swap two adjacent genes.
6. **Whole-genome duplication** — with `genome_duplication_rate`: append a full copy.

**Hard cap at `max_genes`.** Duplication is exponential without it.

**And a metabolic cost per gene exists, added in Phase 4 — but not for the reason this
paragraph originally gave.** The draft said to add one "if genome bloat still appears under
selection", as a brake. The measured problem is the other way round. With the rates above,
duplication and insertion together (0.03) exceed deletion (0.02), so genomes drift *upward*
and a lineage left alone ends up pressed against the cap — where, by the paragraph below, a
lengthening mutation fails. The project's central operator therefore switches itself off
exactly when a lineage is at its most elaborate. `metabolism.gene_cost` is there to keep
genomes *away* from the ceiling so that duplication stays available: a lineage should be
pushed back by selection long before it arrives, and never discover that the wall exists.

### ⭐⭐ A state does not re-draw uniformly, and that is what made the genome address itself

*"Discrete fields re-draw uniformly"* above is a sentence about a **field** — it says a state is
re-drawn rather than nudged, because state 5 and state 6 have nothing to do with one another. It
leaves the distribution the re-draw comes from open, and **that** is what was wrong.

The measurement is the one recorded at the top of this section, and it is the largest this project
has taken. Over 6.46 million cell-observations of the shipped world, **a genome contained a gene
naming its own cell's state for 2.2% of the cells it grew.** Development matches on
`trigger_state`, so it **stopped at 97.8% of the cells it visited** — which is why mean body size
sat at 1.98 cells for the first 140,000 ticks of every run ever measured here, and why the founder
is exactly two cells: its gene hands the daughter `child_state = 1` and nothing names 1.

**The rule was not the thing to change.** The paragraph opening this section rests the whole
justification for the genome design on it, and a rule has to say which cells a gene acts on.
Drawing a state uniformly over sixty-four when a genome mentions three is what makes
duplicate-and-diverge land on nothing.

#### ⚠️ The two state fields want opposite biases

They are not the same kind of thing, and treating them alike breaks the mechanism in one direction
or the other:

| Field | What it is | Where a re-draw should land | Shipped |
| --- | --- | --- | --- |
| `trigger_state` | which cells a gene **acts on** | a state that **already exists in bodies**, or a duplicated gene fires nowhere | **0.75** of re-draws |
| `child_state`, `new_state` | the identity a gene **hands out** | mostly **a state nothing yet names**, or the space of addressable identities can never grow | **0.25** of re-draws |

The second row is the one that is easy to get backwards, and getting it backwards is the worse
failure. If a gene could only ever hand out names its own genome already answers to, no state
nothing yet names could ever be minted, **no new body part could ever be invented**, and a lineage
would collapse onto the closed set of three or four states its founder happened to be given. Small
bodies are slow; a closed alphabet is the design not working.

So both are **mixtures rather than replacements**, and the proportions are opposites of each other:
*a rule reaches for a cell that exists; a name is mostly new.*

**Three quarters, and a quarter.** Large enough that the biased branch is the ordinary case — at or
below a half the miss would still be the ordinary case and the change would not be measurable — and
short of one, deliberately. A quarter of `trigger_state` re-draws still going anywhere is what
leaves a gene able to be switched *off*, which this section calls the raw material duplication feeds
on, and it is the dial against the failure the change was most likely to produce: if every cell in
every body became developmentally live, bodies would run straight into
`limits.max_cells_per_organism` and every organism in the world would be a 64-cell blob.

**They are constants in `mutation.rs`, not configuration keys**, for `behaviour.rs`'s
`LIGHT_REFERENCE` reason. A key in `[mutation]` is a thing a person turns while watching a world and
goes into the document a run is replayed from; these are a property of the operator's own
distribution, mean nothing to anybody setting up an experiment, and a run whose archived
configuration carried them would be one in which what a `state` *addresses* had been redefined by a
slider. `mutation.point_rate` already turns the whole operator down.

#### Where "the states that already exist" comes from, and what it costs

**From the genome, not from a body.** Developing the parent to read off the states its cells are
actually in is exact and costs a whole development pass per reproduction, on top of the one
reproduction already does for the child; it is also less stable than it looks, because which states
a body reaches depends on the step windows and on first-match-wins as well as on the states, so the
answer would move under mutations that have nothing to do with addressing.

What is used instead is read straight off the gene list, in one pass with no allocation — two 64-bit
masks, one bit per state:

- **occupied** — every gene's `child_state` and `new_state`, **plus state 0**, which the development
  loop above puts the seed cell in without any gene naming it. ⚠️ Leave state 0 out and a genome
  whose genes hand out only state 5 draws every trigger onto 5, nothing answers to the seed cell,
  and every body in that lineage is one cell.
- **answered** — every gene's `trigger_state`.

`occupied` is a **superset** of what a development pass would report, and provably so: a cell's
state is written in exactly two places, a gene's `child_state` when it is budded and a gene's
`new_state` when it is re-made, and the one cell neither touches is the seed. So the estimate can
over-report — by naming a state that is written down and never reached — and can never under-report,
which is the right direction for the error to point. `mutation.rs`'s
`every_state_a_body_reaches_is_one_its_genome_writes_down` is that as a test rather than an argument.

#### What it did, measured

Section 15 has the run. The headline, over 300,000 ticks of the shipped world: **the fraction of
grown cells sitting in a state their own genome names went from 4.6% to 17.7%** — the 4.6% being
that same measurement taken on the program *as it ships today*, where the 2.2% above was taken
before a cell was connected to the gene that built it.

⚠️ **And mean body size did not follow it: 6.09 cells against 6.62.** What moved is the rate rather
than the level — bodies are 12% larger through the middle of the run and the two curves meet again
— because living cells and biomass are inside 2.4% of each other in both. **The addressing miss was
real and it was not what held bodies at two cells.** Section 15 and `docs/PHASE7.md`'s Group J carry
the numbers and what they leave open.

**At the cap, a mutation that would lengthen the genome simply fails.** It does not truncate.
This matters more than it sounds: truncating from the end is a silent, *biased* operator, and
the end of the genome is exactly where the neutral, non-firing material accumulates — the raw
material this design says duplication feeds on. A lineage that saturated its genome would
begin eating its own raw material from the far end, quietly losing the thing that makes it
open-ended. Failing instead matches what the rest of the simulation already does when it runs
out of room: births fail at the population cap rather than allocating, and a full world means
nowhere to reproduce into. Deletion still works, so a saturated lineage can shrink and grow
again; it just cannot grow past the cap.

---

## 8. Physics

Deliberately simple, and viscous rather than ballistic. At cell scale, inertia is nearly
irrelevant (low Reynolds number) — which is both physically right and numerically far more
stable.

Per tick, semi-implicit Euler:

```
force = 0
force += spring forces from adhered neighbours (Hooke, with spring_damping)
force += collision repulsion from overlapping non-adhered cells

vel += force × dt
along = the direction between this cell's adhered neighbours, if it has two
vel  = along-component × drag  +  across-component × drag^drag_anisotropy
                                       // no axis: × drag in every direction, as before
pos += vel × dt
```

⭐⭐ **The two-drag line replaced a one-drag line in Phase 7, and the reason is the most
important measurement in this document.** It is set out immediately below, before anything
else in this section, because every other sentence here was written on the assumption that
locomotion was possible and none of it was true.

### ⭐⭐ Nothing could swim, and it was not a balance problem — it was a conservation law

Written as this section originally had it — one `drag`, applied identically to every cell —
**a free body's total velocity is a conserved quantity of the integrator, and it decays to
zero.** Not "swimming was slow", or "the parameters were wrong". Movement was arithmetically
impossible, for every body plan, at every setting of every slider in section 3.

Three facts about the model above, each of them deliberate and each of them fine on its own:

1. **Every internal force appears twice, with opposite signs.** A spring pulls `+f` on one
   cell and `−f` on the other; so does a collision. That is Newton's third law and it is what
   makes the forces honest.
2. **There is no mass.** This section says so in as many words. Force becomes velocity
   directly.
3. **`drag` is one scalar and every cell gets the same one.**

Put them together and sum the velocity update over the cells of one body:

```
Σvel ← (Σvel + Σforce × dt) × drag       and Σforce = 0, because every force cancels
Σvel ← Σvel × drag
```

The total velocity is multiplied by 0.92 every tick and touched by nothing else. Whatever the
muscles do, it goes to zero and stays there, and the body's centre never moves. **Measured:
`|Σvel|` of 5.96e-7 after 2,000 ticks, and a twelve-cell travelling-wave undulator moved
0.00015 world units per 1,000 ticks** — which is the noise floor of a 32-bit float, not slow
swimming.

**This is stronger than the scallop theorem, and that is why it was missed.** The scallop
theorem is the well-known result that at low Reynolds number a *reciprocal* stroke — one that
retraces its own shape — goes nowhere however fast it is performed. A travelling wave is the
textbook way out of it, and section 9's controller is built to produce one. It escapes the
scallop theorem and it did not escape this, because this is not a statement about strokes at
all. It is a statement about the integrator.

### What real swimming works on, and what the fix is

A body at this scale does not swim by pushing off anything. It swims because **the water
resists a slender body about twice as hard across its axis as along it** — that is the whole
of resistive-force theory, and it is why a flagellum works. A wave running down a body puts
each segment obliquely to its own motion, the sideways resistance exceeds the lengthways one,
and the imbalance is thrust.

The model above had no anisotropy at all, so there was nothing for a wave to push against.
`physics.drag_anisotropy` is the fix, and it is deliberately a *property of the water* rather
than a force added to a cell: the cell's velocity is split into the part running along its own
body and the part crossing it, and the two are damped by different amounts. Because the two
parts add back to exactly what was there, a cell moving straight along its own axis is left
with precisely `drag` — the physics this section always described — and only motion across the
body is treated differently.

**Where the axis comes from.** The line between a cell's two adhered neighbours. A cell with
**fewer than two adhesions has no axis and keeps the plain isotropic drag**, and that is
load-bearing rather than tidy: the tempting alternatives — use the one spring it does have, or
break the tie the way the collision code does and say sideways — would hold every loose cell
in the world harder in one direction than another, in a direction decided by the order the
body happens to be stored in. That is thrust with no muscle behind it, and it would look
exactly like life.

**Measured, at the shipped `drag` of 0.92 and `drag_anisotropy` of 2.0**, over 1,000 ticks:

| Body | Isotropic (`k = 1`) | Shipped (`k = 2`) |
| --- | --- | --- |
| One myocyte, one spring | 0 | 0 |
| Two springs at π/2, cells in a line | 0.0005 | 0.0005 |
| Six-cell travelling wave, cells in a line | 0.0003 | 0.0003 |
| **Eight-cell zig-zag, resting stroke** | 0.0005 | **0.154** |
| **Eight-cell zig-zag, driven to full amplitude** | 0.0004 | **1.896** |
| Eight-cell zig-zag, springs in antiphase | 0.0001 | 0.0002 |

Two things in that table matter as much as the headline.

**⚠️ A body whose cells lie in a straight line still cannot swim, and that is correct.** Its
springs run along the line, so all its motion is along the line, so every cell's velocity is
parallel to its own axis and the sideways drag never engages. A straight rod that only
lengthens and shortens along itself is doing one-dimensional motion, and nothing
one-dimensional swims in any fluid. **What a lineage has to find is a shape, not a rhythm** —
development buds every daughter at a gene's `angle`, so a bent body is the ordinary case, but a
lineage that grows a straight spine gets nothing at all from its muscles until something bends
it.

**And the scallop theorem still holds**, which is the check that the new water is water: the
last row is the same kinked body with its springs beating in antiphase, which is a reciprocal
stroke, and it goes nowhere in either column.

`drag_anisotropy` is bounded at `1.0..=3.0`. One is isotropic water — kept reachable because it
is the control experiment for every claim above. Three is where the arithmetic stopped: at a
`collision_stiffness` of 5,000, which is inside the range the explicit integrator otherwise
survives, a pile of cells produced not-a-number within a few hundred ticks. Splitting a
velocity and damping the halves unequally is a *rotation* of that velocity towards the body
axis, and a cell whose axis is itself turning under a stiff collision can be handed a velocity
pointing somewhere no force pushed it; past three the correction and the overshoot stop
cancelling.

### ⚠️ The same law says gravity is not the lever it looks like

The obvious next idea, once bodies can swim, is to make them sink — give every cell a small
constant downward pull, so that staying in the light costs work and depth becomes something a
lineage has to earn. **It does not work, and it fails for exactly the reason above.**

Gravity is an *external* force, so `Σforce` is no longer zero and the body does move — straight
down, at a terminal velocity of `g × dt × drag / (1 − drag)`, and there is nothing a muscle can
do about it. A muscle only ever produces internal forces, which still cancel in the sum. It can
change the body's *shape* while it falls; it cannot change the rate of the fall.

Measured: a body contracting hard against gravity fell **0.1% slower** than the same body
holding still. That is not a lineage swimming upwards against a current, it is rounding.

**Buoyancy as a property of `CellKind` is the version of the idea that works**, and it is what
Phase 7 shipped. Each of section 6's six kinds has its own weight — some heavier than water,
some lighter — and a body's depth follows from *what it is made of*. The consequences are all
the right shape: a body's resting depth becomes a function of its composition, which is a
function of its genome; changing it takes **one mutation** to one gene's `child_kind`, which is
the smallest step the mutation operators have; a lineage that wants to be shallower pays for it
in whatever the floaty cell is bad at rather than in continuous work; and there is a real
trade-off, because the kind that harvests and the kind that floats need not be the same kind.
Nothing in it asks a muscle to do something the integrator forbids.

The column, the argument for each row and the measured drift per lifetime are in **section 6**,
which is where a kind's trade-offs live. Two things about it belong here, because they are
facts about *this* section's arithmetic rather than about cells:

- **It is the one force in the module that does not cancel.** That is the entire point of it.
  Springs and collisions are `+f` on one cell and `−f` on another, which is what makes `Σforce`
  zero and the total velocity a conserved quantity; buoyancy is external, so the sum over a
  body is not zero and the body genuinely moves. A body still cannot *fight* it, and does not
  have to.
- **It is added to the forces and not to the velocity**, so it goes through the two drags above
  exactly as everything else does. A cell in the middle of a chain has an axis, and a chain
  lying flat has its axis across the pull, so it sinks at about half the rate a loose cell
  would. A long flat body settles more slowly than a compact one — which is correct, and which
  came out of the anisotropy rather than being written down.

Neighbour queries use a **uniform spatial hash** sized to **twice** the largest cell radius.
This is the single most important performance decision on the CPU side: it takes collision
detection from O(n²) to roughly O(n), which at a few thousand cells is a larger win than any
language or hardware choice.

*Twice*, not once — an earlier draft said the largest radius and that is half of what is
needed. Two cells touch when they are `r₁ + r₂` apart, so with buckets one radius wide a
3×3 neighbourhood misses genuinely overlapping pairs. Buckets should also divide the world
width a whole number of times, so the horizontal wrap falls exactly on a bucket edge.

### Things the physics does not have, and Phase 3 should know it

- **There is no mass.** Force translates directly into a velocity change, so a sclerocyte
  and a sensocyte accelerate identically despite one being nearly three times the area.
  Being big is free in the physics; it costs only upkeep.
- **Momentum is not a strategy.** Measured at the shipped drag of 0.92: a cell shoved at 60
  units per second travels 11.5 units and stops — under two body-widths. There is no
  gliding and no coasting. If a lineage ever appears to coast, something is wrong with the
  drag rather than clever about the lineage.
- **Springs are not found by the spatial hash** and have no length limit. They are resolved
  from the flat list, so a spring created between cells on opposite sides of the world will
  work — and will haul them together through the seam. Development should not create one.
- **A body straddling the seam has no single position.** Wrapping is handled per pair, so
  the physics is fine, but averaging cell positions to find a body's centre will put that
  centre in the middle of the world for a body sitting on the join. Species clustering,
  rendering and the inspector all need to know this.
- **Vertical closure is about cell centres, not bodies.** A cell resting on the floor has its
  centre at `y = height` and half of itself below. If bodies should sit fully inside the
  world, the clamp needs insetting by the radius; the literal reading is what is implemented.

Boundaries: the world wraps horizontally and is closed vertically (surface and floor).
Wrapping horizontally prevents edge-hugging strategies from dominating; a closed vertical
axis preserves the light gradient's meaning.

---

## 9. Behaviour

**Phase 1 — evolved parameters on a fixed reactive controller.** Cheap, and genuinely
evolvable.

Each `Myocyte` oscillates its springs' rest length:

```
signal    = mean of connected Sensocyte outputs, or 0 if none
amplitude = clamp(resting_amplitude + sensor_gain × signal, 0.0, 1.0)
rest_len  = base_rest × (1 + amplitude × stroke × sin(t × osc_freq + osc_phase))
```

⭐⭐ **`osc_freq`, `osc_phase`, `sensor_gain` and `sensor_target` are read off the gene that
*built* the cell** — the `Divide` whose `child_kind` made it, or the `Differentiate` whose
`new_kind` last re-made it. Section 7 has the argument and the measurement that changed it; a
cell no gene built has no behaviour at all, which is only ever the seed cell, which is only
ever a photocyte.

⭐⭐ **The two coefficients were written here as `0.3` and `0.4` and are now section 3's
`[behaviour]` table, shipping at 0.8 and 1.0.** The measurement that moved them, and the
measurement that says it was not enough, are immediately below.

Each `Sensocyte` outputs a normalised gradient magnitude toward its `sensor_target`,
sampled from the resource grid or from nearby foreign biomass.

Because `sensor_gain` is signed and evolvable, both attraction and avoidance are reachable
by mutation, and phototaxis, detritus-seeking and predator-avoidance are all *discoverable*
rather than coded. Undulating locomotion falls out of neighbouring myocytes evolving
compatible `osc_phase` values — and, section 8 adds, out of the body being **bent**, which is
the part that is not obvious.

### ⭐⭐ The stroke is a setting, and it is the only lever that makes swimming worth doing

Section 8 records that until Phase 7 **nothing in this world could move at all** — a free
body's total velocity was a conserved quantity of the integrator — and that anisotropic water
fixed it. Section 4 records that the light then began to drift, so that there was somewhere
better to be. Neither produced a muscle. The reading at the end of both was that the payoff was
too thin to climb: **a perfect undulator driven flat out for a whole 2,000-tick lifetime covered
about four world units**, against a body eight units long, a lattice of light 128 units across,
and a field that slid 1.2 in the same time.

So every lever was measured against the same nine hand-built bodies — three lengths by three
kinks, meaned, because a single undulator is strongly resonant and the first reading taken on
one shape had a factor of five of noise in it. World units covered per 2,000-tick lifetime:

| Lever | Walked from → to | Distance | Work per tick |
| --- | --- | --- | --- |
| **the stroke, unsensed** (`resting_amplitude × stroke`) | 0.12 → 0.8 | **0.3 → 11.7** | ×24 |
| **the stroke, sensor-driven** (`stroke`) | 0.4 → 1.0 | **3.7 → 41.1** | ×2.8 |
| `physics.drag_anisotropy` | 2.0 → 3.0 | 41.1 → 46.2 | ×1.0 |
| `osc_freq` | 1 → 5 rad/s | 6.0 → 15.5, peaking near 3 | ×12 |
| segment length (`MAX_REST_LENGTH`) | 8 → 13.6 | ×1.7 | ×2.9 |
| `physics.drag` | 0.92 → 0.99 | ×9 | ×1.7 |
| spring stiffness | 10 → 144 | ×0.65 | ×2.8 |

**Distance goes as roughly the cube of the stroke and as the square root of everything else.**
That is what makes it the lever rather than one of seven, and it is why `[behaviour]` exists.

Three of the others were rejected on their own terms rather than on their size. **`physics.drag`
at 0.99 lets a cell coast a hundred units off one shove**, and section 8 is explicit that
momentum is not a strategy here — that is a different world, not a faster one. **`osc_freq`'s
range already contains its own optimum**: a body is fastest between two and three and a half
radians a second and falls away either side, so `MAX_OSC_FREQ` of 5 covers it and widening the
range would only add draws that are worse. And **`drag_anisotropy` is nearly spent**: 2.0 is
where slender-body theory puts a real slender body, and the whole of the range above it is worth
12%.

⚠️ **`stroke` stops at one, and that is arithmetic rather than tidiness.** The amplitude above is
clamped into `0..=1`, so the shortest a spring ever asks to be is `base_rest × (1 − stroke)` —
and one is exactly where that reaches nought. Past it the rest length is *negative*: the spring
pulls at every phase of its cycle instead of oscillating about anything, and the body hauls
itself through its own cells. It does not fail loudly. Measured, a body at `stroke = 1.5`
travels **twenty-four times further** than one at 1.0, which is what a broken model looks like
from the outside.

`resting_amplitude` is bounded for the plainer reason: the clamp on the line above would
silently undo anything outside `0..=1`. Raising it from 0.3 to 0.8 leaves a sensor two tenths of
room upwards and eight tenths downwards, and that asymmetry is deliberate — `sensor_gain` is
signed, so inhibition was always half of what a sensor is for, and a body with one side
inhibited and the other not is a **turn**, which is the thing a swimmer needs and a pulse is not.

### ⚠️ And it did not produce myocytes, because the controller above almost never runs

The honest result, and it is worth more than the table. A 310,000-tick run at the shipped
settings ended with **one myocyte and no devorocytes**, against the previous run's two and six.
No signal, and the world was otherwise unchanged: 812 organisms against 777, 6.28 cells against
6.66, mean depth 534 of 1,152.

What the diagnostic found is that the stroke was never the binding constraint, and neither was
the water or the field. **Counted over 120,000 ticks of the shipped world, every spring in it
with a myocyte on one end:**

| | Spring-ticks |
| --- | --- |
| A myocyte on a spring, and **no gene in its genome names its state** | **56,903** |
| A gene answers, but its `osc_freq` and `osc_phase` are both still exactly nought | 874 |
| **A muscle that actually moved a spring** | **0** |

A cell's state is one of **64**; a genome at that age holds about **three genes**, and nearly
every one of them triggers on state 0, because that is the founder's and `trigger_state` is not
where mutation spends its time. Meanwhile development scatters daughters across the whole state
space through `child_state`. So a myocyte is grown into a state nothing in its own genome is
listening to, and it is **anatomically present and behaviourally disconnected** — a muscle with
no nerve to it. The 1.5% that do find a gene then meet the second gate: that gene is a copy of
the founder's, whose `osc_freq` and `osc_phase` are both zero, so `sin(0)` is zero and the rest
length is multiplied by exactly one.

**This is why three separate changes to the payoff have all come back null.** Group F made
locomotion possible, Group G gave it somewhere to go, and Group H made it eleven times faster,
and all three acted on a code path the world takes about once in every two hundred thousand
spring-ticks. Every one of them was necessary and none of them could have been sufficient.

Where to look next is therefore **the wiring rather than the reward**, and the measurement above
names three candidates in the order of how much they cost: a `trigger_state` mutation operator
that moves a gene onto a state some cell actually occupies; a much smaller state space, since 64
against three genes is what makes the miss near-certain; or a rule that a myocyte with no gene
answering it falls back to the gene that built it. The first is the smallest and the third
changes what a state *means*, so it should be argued before it is written.

### ⭐⭐ The wiring, taken: a cell's behaviour comes from the gene that built it

*(The third candidate above, argued and then written. The argument is in section 7; this is what
it did.)*

Re-counted on the same world, the same seed and the same eight founders, over the same 120,000
ticks — every spring with a myocyte on one end:

| | Before | After |
| --- | --- | --- |
| A myocyte on a spring, and **no gene speaks for it** | **70,352** | **0** |
| A gene answers, but its `osc_freq` is still exactly nought | 874 | 68,571 |
| **A muscle whose spring's rest length is a moving function of time** | **0** | **9,901** |
| Total myocyte spring-ticks | 71,226 | 78,472 |

*(The 70,352 is the same count as the 56,903 above taken over a longer window: 120,000 ticks
after the founding rather than 120,000 of the world's, which includes a 13,000-tick dawn. The
874 is identical in both, which is what says the two instruments agree.)*

**The first row is the whole change.** Every myocyte in the world is now attached to a gene,
because a myocyte can only exist by a gene having asked for one. Nothing is looked up and
nothing can miss.

⚠️ **The second row is what is left, and it is a different problem with the same shape.** A gene
that has just become a myocyte-maker still carries the founder's `osc_freq` of nought, and
`sin(0)` is nought, so the muscle holds still. Section 7's point mutation perturbs **one field
of a hit gene**, so a moving muscle needs *two* mutations. What the change did was put both of
them on **one gene out of three or four** instead of requiring one of them to land on a state in
a space of **sixty-four** — and 9,901 spring-ticks of genuine movement, against nought before,
is that difference.

⚠️ **And it is not yet a myocyte signal, which is the honest half of the result.** The
313,000-tick run in section 15 ends with **four** myocytes in 5,277 living cells, against the
previous run's one — and the same run passed through checkpoints holding nought, one and nought
on the way there, while the run it replaced passed through ten. Both are single readings of
single figures. Muscles fire; **what a lineage gets for owning one is a separate question**, and
nothing in this change was an answer to it.

### ⭐ What a light sensor is normalised against — measured in Phase 7

"Normalised" was left undefined here, and the first reading of it was the natural one: divide
the gradient by **the energy of the tile the sensocyte is standing in**, so the signal is a
*relative* gradient. It reads well — the same slope is worth more in dim water, so a cell deep
down is more sensitive, which is where a lineage would want the sensitivity — and it made a
light sensor worth nothing whatever.

The arithmetic is one line. Section 4's field falls by `cap × gradient` over `grid_rows`, which
in the shipped world is `8 × 0.75 / 144 = 0.042` a row, and the field settles at about half its
ceiling once a population is eating out of it — so the gradient a sensocyte can see is about
**0.02**. The divisor was the tile's own energy, about **4**. Every reading came back at a few
thousandths, which moved a myocyte's amplitude by less than the rounding on it. Measured:
**0.0025** for the background gradient, and 0.05 to 0.31 beside a tile something had been
grazing.

So the reference is now **fixed** — the 0.02 above, which is the gradient of ordinary open
water — and the signal is an *absolute* gradient rather than a relative one. A reading of a half
means "the ordinary slope of open water", less means flatter, and a grazed tile runs up towards
one without reaching it. The price is the property that was nice about the first reading: light
is now read the same way at every depth, and a lineage wanting to be more sensitive in the dark
has to evolve the gain for it.

`MAX_SENSOR_GAIN` moved with it, from **1.0 to 8.0**. It is the range a gain is *drawn* from
rather than a bound on the field, and at one a gain spent most of its life pressed against its
own clamp, where a point mutation changes nothing at all.

⚠️ **The genetic distance of section 11 does not follow that number, and must not.** It scaled
a difference in `sensor_gain` by `2 × MAX_SENSOR_GAIN`, which is the obvious thing to write and
is wrong for one reason: genetic distance is the unit every species boundary in every chronicle
is measured in, and moving it would silently redraw them all. The two are now separate
constants — one is a sensor's range, the other is a unit of measurement, and only the first is
free to move as the model is tuned.

### ⭐⭐ Locomotion is not reachable in this model, and here is the arithmetic

Measured with the competition assay over fourteen world configurations and three seeds each.
A hand-built five-cell zig-zag with two phased muscles — verified to travel ten times as far
as its own held-still twin, and verified to breed — comes back at **−10.3 % per generation**
in the shipped world, and is **extinct in three seeds of three** in every world made fine
enough for patch-following to matter. Finer light makes it monotonically *worse*, which is
the opposite of the hypothesis.

**The first wall. A body travels about two thirds of its own length in a whole lifetime.**
Measured three ways and invariant: the assay swimmer spans 25.6 units and covers 16.6
(×0.65); the same body at `MAX_REST_LENGTH` spans 34.8 and covers 21.7 (×0.62); section 9's
nine hand-built undulators average 61 units of span and cover 41 (×0.67).

For a patch to be worth crossing, a body must cover about half of one:
`blotch ≤ 2 × travel ≈ 1.3 × body length`. For a patch to have any contrast *across* a body,
`blotch ≥ body length`. So the window is `1.0 × length ≤ blotch ≤ 1.3 × length` — and a body
filling its own patch reads no gradient at all. **Both bounds scale with the body**, so the
window does not open at any value of `width`, `grid_cols`, `patchiness`, `patch_drift`,
`season_amplitude` or `rest_length`. As shipped, a blotch is five body-lengths and a body
covers 0.13 of one.

Nor is diffusion the bound, which was the obvious suspect. The lattice is 16 *tiles* and the
stencil is per *tile*, so there are 16 diffusion steps across a blotch whatever it is worth in
world units: the standing field's row-wise variation is 0.1522 at blotch 128, 64, 32 and 16
alike. The body is the bound.

**The second wall is independent and permanent. There is no directional information anywhere
in the controller.** `light_gradient` returns `.length()` — a magnitude, with the direction
discarded — and `sensor_gain` scales a myocyte's *amplitude*. Measured: the income available
by moving 16.6 units in the *best* direction rises ×1.05 → ×1.10 → ×1.19 → ×1.34 as the
blotch falls 128 → 64 → 32 → 16, while a direction nothing chose stays at ×1.00 to ×1.04
throughout. Nothing in the model can move a body from the second column to the first.

**What this means.** Swimming is physically possible here (section 8) and it is affordable
(section 3). It is not *reachable*, because the distance a body can cover in a lifetime is a
fixed small fraction of its own size, and the resource has no structure at that scale that a
body could detect the direction of. Closing it needs a change to the physics or to the
controller, not to the economy or the environment — and that is a decision, not a tuning
exercise. Recorded here so nobody spends another week on the reward side.

**Phase 2 — evolved neural networks**, whose inputs and outputs wire to whatever sensory
and contractile cells the body actually grew, so anatomy and behaviour co-evolve. Far more
interesting, far more expensive per tick. The architecture leaves room; do not build it in
phase 1.

---

## 10. Life cycle

**Metabolism.** Each tick, every cell pays `upkeep × upkeep_scale`, and every organism pays a
further `gene_cost × upkeep_scale` for each gene in its genome — a fixed overhead for carrying
the program rather than a charge per cell, and the reason for it is in section 7.
`upkeep_scale` is the "temperature" slider — raising it is a live environmental pressure.
Organisms whose energy reaches zero die.

**Reproduction.** When an organism's stored energy exceeds
`reproduction_threshold × body_construction_cost` *and* it has at least one gonocyte, it
reproduces: copy the genome, mutate, develop the new body, place the seed cell adjacent to a
gonocyte with a small random offset, transfer `offspring_share` of parent energy. Asexual
only.

Births fail silently when `max_organisms` is reached. Deliberate: a full world means nowhere
to reproduce into, which is both correct behaviour and the thing that makes the memory
guarantee hold.

**Death.** Energy reaches zero, or age exceeds a genome-derived maximum. The body becomes
detritus particles at each cell's position, carrying that cell's construction energy.
Detritus sinks slowly and decays into the field tile beneath it.

**Predation is emergent.** A living body is a denser package of energy than the surrounding
soup, so devorocytes contacting foreign cells is simply a better strategy under some
conditions. Whether a herbivore/predator split appears is one of the genuinely interesting
outcomes and must never be scripted.

---

## 11. Species, naming, and the chronicle

**Clustering.** Genetic distance between two genomes is a normalised alignment cost over
their gene lists: matched genes contribute their scaled numeric difference, unmatched genes
contribute a fixed penalty. Every 500 ticks, cluster the living population by distance with
a threshold; a cluster that persists for 20 consecutive samples is promoted to a named
species.

**Naming.** Binomial, generated from Latin-ish syllables. A new species inherits its genus
from its parent species and receives a new epithet; a sufficiently large jump mints a new
genus. Colour is **inherited, not computed**: an offspring takes its parent's hue and shifts it by
a small amount, larger when the genome changed more. Lineages are therefore visually
distinct and **drift in hue as they drift genetically** — you can literally watch speciation
happen, because a splitting lineage comes apart on screen as a gradient rather than as a
jump.

⚠️ **An earlier draft said "colour derives from a hash of the genome", and that is
self-contradictory.** A hash does not drift; it jumps. Any mutation at all reseeds it, so
every child is a completely unrelated colour from its parent. It was built that way and
looked at, and the result is confetti: adjacent bodies within one colony come out cyan,
magenta and orange at random, and colour makes speciation **less** visible than no colour
would. See `docs/frames/phase5-groupb.png`, which is what that looks like.

The two clauses cannot both hold. "Drifts as they drift" is the one worth keeping, because
it is the one that does the work the paragraph claims.

**Events.** Append-only, written in a naturalist's register. Detect and record:

- first adhesion (the origin of multicellularity in this run)
- first appearance of each cell kind
- first predation event
- speciation and extinction, by name
- new records: body size, cell count, genome length, population
- mass extinction (population falls by >50% within 5,000 ticks)
- environmental changes made by the user

> *Tick 41,208 — 41.2 Ma.* A cell has not separated from its daughter. **Coacervus
> primus** is the first lineage to persist as more than one body.
>
> *Tick 96,540 — 96.5 Ma.* **Coacervus vorax** has begun consuming its neighbours.
>
> *Tick 210,880 — 210.9 Ma.* Mass extinction. 94% of lineages lost.

**The chronicle** is a generated natural history of the whole run, written on shutdown as
Markdown alongside the replay log. This is the payoff for leaving it running overnight, and
it is mostly presentation over data already being collected.

### Darwin marginalia

Everything Darwin published is public domain, as are his letters. Surfacing his own words
alongside the events they describe gives the simulation a voice without inventing one.

**The rule that makes this work: quotes are captions on events, not decoration.** A quote
fires because something happened that it actually describes, which makes Darwin read as a
commentator on your world. A rotating quote box in the corner would be a fortune cookie —
disconnected, and tiresome within an hour.

| Trigger | Theme he actually wrote about |
| --- | --- |
| Population reaches carrying capacity | The struggle for existence |
| First predation event | The struggle for existence |
| Speciation | Divergence of character |
| Mass extinction | Extinction, and the rarity of its observation |
| A lineage *loses* a cell kind | **Rudimentary and atrophied organs** — he wrote at length on structures reduced or abandoned. The perfect pairing for the non-teleological rule. |
| Deep-time milestone | The immensity of geological time; the imperfection of the record |
| World seeded | His 1871 letter to Hooker speculating that life began in a "warm little pond" |
| Chronicle header | The closing line of *On the Origin of Species* |

⚠️ **The example above was written as "a cell has *failed* to separate", and that does not
pass this section's own rule.** Failing is not something a cell can do — it implies the cell
was trying to separate, which is intent, which is the teleology this section spends four
paragraphs banning. It was caught by the banned-vocabulary test rather than by anybody
reading carefully, which is the argument for having the test. Two further sentences were
caught the same way while being written with the rule open in front of the author.

⚠️ **And "the first lineage to persist as more than one body" cannot happen in this world.**
Section 6 requires a gonocyte for reproduction and section 10 requires a body to feed itself,
so the smallest lineage that can both eat and breed is already two adhered cells — every
viable founder is multicellular from tick one, and the log honestly says so on its first
tick rather than pretending otherwise. Multicellularity here is a *precondition*, not an
outcome. Making it an outcome would mean a founder that cannot reproduce until a mutation
adheres its daughter, which is a different and interesting world; it is not this one, and
the difference should be a decision rather than a surprise.

**Anachronism discipline.** Only quote him where he genuinely spoke. Darwin knew nothing of
genes, mutation or molecular heredity, and he deliberately avoided the origin of life in
*Origin* — the warm little pond was a private aside in a letter, not a published claim.
Quoting him beside a mutation event would be a category error, and exactly the kind of thing
a biologist would notice. Selection, struggle, divergence, extinction, rudimentary organs and
deep time are all safely his.

**Presentation.** A data file (`darwin.toml`) of `{ text, work, year, trigger }` records.
Each trigger fires at most once per run — a handful of quotes across several hours, not a
stream. Typeset as marginalia: serif face, generous leading, low contrast, slow fade in and
out. It must obey the visually-calm constraint; this is a note in the margin of a book, not a
notification. Always attribute the work and year. Disableable in config.

---

## 12. Rendering

The visual quality comes from shaders, not from the widget library.

- **One instanced draw call** for all cells. Per-instance: position, radius, hue, energy
  flow, kind.
- Fragment shader does a soft radial falloff. Neighbouring cells drawn additively **merge
  into a single organic silhouette** rather than reading as a string of beads. This one
  technique is most of the difference between "creature" and "physics demo".
- Render bodies into an **HDR offscreen target**, then a separable-Gaussian bloom pass, then
  composite with tone mapping.
- An **accumulation buffer with a slow fade** gives motion trails, which make swimming
  legible and look excellent.
- Background: vertical gradient (bright at the surface, near-black at depth), slowly
  drifting light shafts, and marine snow — which is the actual detritus, not decoration.
- Colour: hue from species; saturation and brightness modulated by cell kind and
  `energy_flow`, so a well-fed organism visibly glows.
- Camera: smooth pan and zoom, user-driven only. **It must never move on its own** — see
  the second-screen constraint in `CLAUDE.md`.

`egui` panels sit over the world: translucent dark, thin borders, monospace numerics,
recessive. The simulation is the subject; the chrome should nearly disappear.

### No portability budget

Target the 4070 Ti and use what it does best. There is **no** requirement to stay within
mobile or embedded GPU feature levels — the eventual Raspberry Pi version will be a separate
rewrite, not a port of this renderer, so constraining the visuals for hardware this codebase
will never run on would be pure waste.

### Frame dumping — build this in phase 5

- `--dump-frame <path>` renders one frame and exits.
- `F12` dumps the current frame to `runs/<id>/frames/`.

This is how visual work gets verified without a browser. Without it, every visual change
becomes a prose description of what looks wrong.

---

## 13. Replay log

Directory per run: `runs/<timestamp>-<seed>/`.

```
config.toml        verbatim copy — the run is self-describing
events.jsonl       append-only, human-readable, keep everything
stats.bin          fixed-size records every 100 ticks, keep everything
snapshots.bin      full world state every 10,000 ticks, zstd, rotated to fit budget
museum/            top genomes sampled periodically, postcard-encoded
chronicle.md       written on shutdown
frames/            PNG dumps
```

`stats.bin` records are small and fixed-size — population, per-kind biomass, mean genome
length, mean cell count, species counts, the five ledger accounts — so the whole run's
time-series always fits in memory for charting.

`snapshots.bin` is chunked with a magic header and a version byte, length-prefixed,
`postcard` + `zstd`. When the 8 GB budget is reached, drop the oldest snapshots rather than
stopping — a week-long run keeps a bounded footprint. **Log what was dropped**; silent
truncation reads as complete history when it isn't.

Use `postcard` and `zstd`. Do not invent a clever format.

---

## 14. GPU port (phase 9)

Only after the CPU implementation is stable and tested.

**Moves to the GPU** — uniform, per-tick, embarrassingly parallel:

- resource field regrowth and diffusion
- spring forces
- collision resolution (sort-based spatial hash with prefix sums)
- position integration
- behaviour evaluation

**Stays on the CPU** — irregular, rare, branchy:

- development (growing a body from a genome)
- mutation
- birth and death bookkeeping
- species clustering and event detection

Births and deaths are batched into a compaction pass rather than mutating buffers mid-tick.

**Validation is the whole point of the phase order:** run CPU and GPU from the same seed and
assert the states match within float tolerance. That turns "is my shader correct?" from a
debugging exercise into a passing or failing test.

---

## 15. Testing

TDD throughout — red, then green. The tests are the part Jonathan can actually read, so
they carry the guarantees.

**Property tests** (`proptest`) are the strongest tool here:

- For any genome and any 10,000 random mutations: genome length ≤ `max_genes`, every field
  in range, development always terminates, cell count ≤ `max_cells_per_organism`.
- For any config and 100,000 ticks: the energy ledger balances.
- For any seed: two runs produce identical state hashes.
- For any organism: development is a pure function of the genome.

**Resource-guarantee tests:**

- Peak RSS stays under target across a long headless run.
- No allocation occurs after warm-up.
- Births fail cleanly at the population cap rather than growing any arena.

**Differential test (phase 9):** CPU and GPU agree from the same seed.

**Ecology smoke test:** a default-config headless run of 500,000 ticks ends with a living,
non-degenerate population — neither extinct nor a single clone filling the world. This is
the test that tells you the *balance* is right, and it is the one most likely to fail.

⭐⭐ **Run, in Phase 4, and it passes — but only after `light.influx` was retuned, and the
half-million-tick version says something the shorter one cannot.** The test that ships is
`a_headless_run_reaches_a_living_equilibrium` in `coacervate-app`, and it runs thirty
thousand ticks rather than five hundred thousand, because that is what a check suite can
afford. What it asserts is the equilibrium: the population settles at about 2,100, well below
`limits.max_organisms`, with the field drawn down 45% and about 1.2 births and 1.2 deaths a
tick.

The full five hundred thousand was measured separately, once, at the shipped configuration
from eight founders. The equilibrium above holds from tick 50,000 to tick 200,000 — a hundred
and fifty thousand ticks of a flat population. Then it **moves**, and what moves it is the
bodies:

⚠️ **The table below was measured on the pre-Phase-7 program** — isotropic water, a
`movement_cost` of 0.15, a `MAX_SENSOR_GAIN` of 1 — and every one of those three moved in Phase
7. The run is not reproducible from today's binary and its last line explains why the change
was made: one myocyte in a population of 794, in a world where a myocyte could not have moved
anything if it had tried. The Phase 7 re-measurement is beneath it.

| Tick | Population | Mean cells | Mean genes | Field |
| --- | --- | --- | --- | --- |
| 50,000 | 2,138 | 1.99 ± 0.12 | 2.10 | 55% |
| 150,000 | 2,144 | 2.00 ± 0.28 | 2.93 | 49% |
| 250,000 | 1,729 | 2.81 ± 1.88 | 4.27 | 44% |
| 350,000 | 943 | 5.46 ± 2.98 | 7.93 | 39% |
| 500,000 | 794 | 6.73 ± 3.01 | 10.80 ± 5.66 | 37% |

Living biomass barely moves across the whole of that — about 33,000 units throughout — which
is the carrying-capacity claim holding while everything else changes. **What changed is that
the same energy is being held by a quarter as many bodies, each three times the size.** The
population at the end is 4,537 photocytes, 788 gonocytes, 11 sclerocytes, 5 sensocytes, 1
myocyte and 1 devorocyte.

That is the outcome CLAUDE.md's decision log calls a realistic overnight one, arriving
unprompted: multicellularity, from a founder of two cells, with no term anywhere in the model
that rewards being larger.

### ⭐⭐ Re-measured in Phase 7, after the water was made anisotropic

Three hundred thousand ticks at the shipped configuration, eight founders, seed 42, with
`drag_anisotropy` at 2.0, `movement_cost` at 0.0001 and `MAX_SENSOR_GAIN` at 8. Ticks are the
world's, so the first ten thousand are the dawn.

| Tick | Population | Mean cells | Mean genes | Field | Myocytes |
| --- | --- | --- | --- | --- | --- |
| 50,000 | 2,070 | 1.98 | 1.90 | 60% | 2 |
| 100,000 | 2,159 | 2.03 | 2.45 | 48% | 0 |
| 150,000 | 2,007 | 2.22 | 2.89 | 47% | 1 |
| 200,000 | 1,498 | 3.21 | 3.86 | 43% | 0 |
| 250,000 | 844 | 6.07 | 6.28 | 38% | 0 |
| 310,000 | 650 | 8.39 | 8.75 | 36% | 1 |

**It is the same world, arriving sooner.** Living biomass sits between 30,000 and 34,000
throughout, exactly as before; the population falls by two thirds while bodies quadruple; and the
end state — 4,797 photocytes, 652 gonocytes, 11 sclerocytes, 6 sensocytes, 1 myocyte, 0
devorocytes — is the same shape of population the half-million-tick run reached, at a little over
half the ticks. Neither of the failure modes the changes were watched for appeared: myocytes did
not accumulate as neutral bloat now that moving is nearly free (a myocyte cost 0.014 a tick to own
at the time against a photocyte's 0.004, which is what was ever pricing it), and mean depth drifted
only from 296 to 483 in a world 1,152 deep rather than to the surface.

*(⭐ That parenthesis was right and it was also the whole problem. Section 6's sweep has since
taken the upkeep to **0.005** and measured what happens below it: at 0.002 the neutral bloat this
paragraph reports the absence of does appear, and nothing travels any further for it.)*

⚠️ **And it is still one myocyte, which is worth being exact about.** Swimming is now possible and
measurable — section 8's table — and it is far too slow to be worth anything: a body that lives 571
to 2,000 ticks and swims 0.154 world units per 1,000 has moved a fortieth of its own width by the
time it dies. **The mechanism exists; the payoff does not yet.** That is a different situation from
the one before, where the payoff was exactly zero and provably so, and it says where to look next —
at the speed, which means the beat, the swing, or section 8's buoyancy-by-`CellKind`, and not at the
water.

*(That paragraph was written before section 9's spring-tick count existed. What it says about the
payoff may well be true and the measurement could not have shown it either way, because in the
world it describes **no muscle was ever executed at all**.)*

### ⭐⭐ Re-measured again, after a cell was connected to the gene that built it

Section 7's change, on the same world, the same seed and the same eight founders. The run is
313,000 of the world's ticks, of which the first 13,000 are the dawn.

| Tick | Population | Mean cells | Mean genes | Field | Depth | Myocytes |
| --- | --- | --- | --- | --- | --- | --- |
| 38,000 | 1,578 | 1.98 | 1.59 | 73% | 295 | 0 |
| 88,000 | 2,123 | 1.99 | 2.32 | 52% | 418 | 1 |
| 138,000 | 2,083 | 2.13 | 2.91 | 49% | 445 | 1 |
| 188,000 | 1,703 | 2.69 | 3.41 | 46% | 460 | 1 |
| 238,000 | 1,164 | 4.23 | 4.77 | 42% | 508 | 1 |
| **313,000** | **797** | **6.62** | **6.17** | **36%** | **488** | **4** |

Against the same run measured on the program before the change: 846 alive, 6.26 cells, 5.54
genes, 37.7% field, depth 553, **0** myocytes, 1 devorocyte. And against the figure this
document already carried for a 310,000-tick run: 812, 6.28, 5.46, depth 534, **1** myocyte, 0
devorocytes.

**It is the same world.** Living biomass is 33,468 against 33,970 and 32,548 — inside 2% of
every run since Phase 4, which is the carrying-capacity claim holding while everything else
moves. No extinction, a peak population of 2,131 against a `max_organisms` of 4,000, and a mean
depth of 488 in water 1,152 deep rather than a mat at the surface.

⚠️ **The first 38,000 ticks are bit-identical to the run before the change and then they part.**
That is the change landing: identical while no muscle in the world has yet drawn an `osc_freq`,
different from the moment one does.

⚠️ **There is still no myocyte signal and no devorocyte signal.** Four against nought and one
are single readings of single figures in a population of five thousand cells, and the run
passed through checkpoints holding nought and one on the way to them while the run it replaced
passed through ten. **Mean displacement over a lifetime moved from 1.938 to 2.028 world units**
— a twentieth of one cell's width, across 263,000 lifetimes, and dominated by bodies that have
no muscle at all. Nothing here is a lineage that swims.

What the change did do is make the question askable. Before it, no experiment on the payoff
could have returned anything, because the controller was not executed. **The remaining reasons a
myocyte is rare are now separable, and section 7 names the first of them**: a body is two cells
for the first hundred thousand ticks of a run, and a two-celled body has one spring and nothing
to undulate.

### ⭐⭐ Re-measured once more, after a state stopped being re-drawn uniformly

Section 7's Group J change, on the same world, the same seed and the same eight founders, over
300,000 ticks after the dawn. **Both columns below are one instrument run twice**, which is the only
honest way to read them: the 2.2% section 7 records was measured on the program *before* a cell was
connected to the gene that built it, and the same measurement on the program as it shipped
afterwards is 4.6%.

| | Before | After |
| --- | --- | --- |
| **Grown cells in a state their own genome names** | **4.56%** | **17.68%** |
| Population | 797 | 848 |
| Mean cells | 6.62 | **6.09** |
| Largest body in the world (of a cap of 64) | 32 | **17** |
| Bodies at the cap | 0.00% | **0.00%** |
| Mean genes | 6.17 | 6.10 |
| Mean depth (of 1,152) | 488 | 483 |
| Living cells | 5,276 | 5,164 |
| Biomass | 33,468 | 32,687 |
| Myocytes | 4 | **0** |
| Devorocytes | 0 | **0** |
| Mean displacement per lifetime | 4.023 | **3.966** |

**The addressing moved by a factor of four and the ecology did not move at all**, and the second
half of that is the finding. Living cells are inside 2.2% of each other and biomass inside 2.4%,
which is the carrying-capacity claim holding for the fifth change running. Mean body size went
*down* by 8%, which is inside the spread between any two runs in this phase — so **the near-certain
addressing miss was not what held bodies at two cells.** What it did change is the rate: bodies are
12% larger between 150,000 and 225,000 ticks, and the two curves meet again by 300,000. Body size
here is living cells over living bodies, and the light decides the numerator.

⚠️ **The failure this change was watched for did not happen, and its opposite did.** If every cell
became developmentally live, bodies would run into `limits.max_cells_per_organism` and every
organism would be a 64-cell blob. Not one body was at the cap at any checkpoint of either run, and
the largest body in the world *fell*, from 32 cells to 17.

**A third run turned the dial up to 1.00 and 0.50 to find out where the blob is**, because a bound
nobody has measured is not a bound. The addressing goes on rising — 25.4% over the run and 36.7% in
its last 25,000 ticks — sclerocytes and sensocytes become half as common again, and **the largest
body in the world becomes 64, with 0.11% of bodies sitting on the cap from tick 275,000 onward.**
That is where the failure mode begins, and it begins exactly where taking the uniform tail off
`trigger_state` says it should: with no tail, no gene can ever be switched off by being pointed at
nothing. Mean body size falls the whole way — 6.62, 6.09, 5.81 — which is the same finding again.
**0.75 and 0.25 are the last setting at which no body reaches the cap at all.**

⚠️ **And there is still no myocyte signal, no devorocyte signal and nothing that travels.** Nought
myocytes against four, both noise in five thousand cells; mean displacement per lifetime 3.966
against 4.023 over a quarter of a million lifetimes, which is a fortieth *shorter*. The
per-checkpoint counts and the full run are in `docs/PHASE7.md`'s Group J.
