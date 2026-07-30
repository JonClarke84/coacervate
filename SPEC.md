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
  across architectures. **One exception, and it is not optional:** the energy ledger's five
  accounts are `f64`, because they are running totals over the whole run rather than state.
  See section 5 for the arithmetic — at `f32` they stop accumulating inside the first minute.

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
influx = 0.012           # energy per grid tile per tick
cap = 8.0                # max energy a tile can hold
gradient = 0.75          # 0 = uniform, 1 = fully top-weighted
patchiness = 0.15        # spatial noise amplitude
diffusion = 0.04         # lateral spread per tick

[physics]
drag = 0.92              # velocity retained per tick (high — viscous regime)
collision_stiffness = 40.0
spring_damping = 0.35

[metabolism]
upkeep_scale = 1.0       # global multiplier on all cell upkeep ("temperature")
movement_cost = 0.15     # energy per unit of work done by contraction
reproduction_threshold = 2.2   # × body construction cost
offspring_share = 0.45   # fraction of parent energy passed to offspring

[mutation]
point_rate = 0.06        # per gene, per reproduction
point_sigma = 0.12       # gaussian magnitude on numeric fields
duplication_rate = 0.02  # per genome
deletion_rate = 0.02
insertion_rate = 0.01
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

**Profiles.** Ship named presets rather than expecting anyone to tune 25 sliders from cold:

| Profile | Intent |
| --- | --- |
| `default` | Balanced. The starting point for experiments on the PC. |
| `slow` | `max_ticks_per_second` reduced so meaningful change happens over hours rather than minutes. For leaving it up on a second screen and noticing it rather than watching it. |
| `bloom` | High light influx. Demonstrates stagnation under abundance. |
| `famine` | Low influx. Demonstrates selection pressure and extinction. |

---

## 4. The resource field

A coarse grid, `grid_cols × grid_rows`, each tile holding a scalar energy value.

Per tick, for each tile:

```
target      = cap × light_profile(y) × (1 + patchiness × noise(x, y))
regrowth    = influx × light_profile(y)
tile       += min(regrowth, target - tile)      // never exceeds target
```

where

```
light_profile(y) = 1 - gradient × (y / height)
```

so light is strongest at the surface and dimmest at depth. **The gradient is what gives
movement a reason to exist** — without it there is no spatial structure for phototaxis to
discover, and swimming has no payoff.

Lateral diffusion runs after regrowth: a simple 5-point stencil at rate `diffusion`. This
smooths harvest shadows and prevents organisms from permanently strip-mining a single tile.

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
| `dissipated` | Energy spent on metabolism and movement — permanently gone |
| `influx_total` | Cumulative energy added by light since `t = 0` |

Invariant:

```
field + biomass + detritus + dissipated  ==  initial_total + influx_total   (± 1e-3 relative)
```

Checked every tick in debug, every 1,000 ticks in release. **On violation, panic.** Eight
hours of quietly wrong output is worse than a crash.

### The five accounts are `f64`. This is an exception to section 2, and it is required.

Section 2 mandates `f32` for simulation *state*, for GPU compatibility. The ledger accounts
are not state in that sense — they are running totals over the whole life of a run, and at
`f32` they stop working long before an overnight run finishes.

The arithmetic is not close. `f32` carries 24 bits of mantissa, so `x + y` silently returns
`x` unchanged once `x / y` exceeds about 16.7 million. At the default config, light adds
roughly `0.012 × 36,864 ≈ 442` per tick to `influx_total`. That total passes 16.7 million
after about 38,000 ticks — *under a minute* — and from then on `influx_total` simply stops
increasing while `field` keeps being credited. The invariant then fails, not because
anything is wrong, but because the accumulator ran out of room. Worse, summing 36,864 `f32`
tiles naively to compute `field` loses precision in the same way on every single tick.

So: **tiles stay `f32`; the five accounts are `f64`; and the per-tick sum over tiles
accumulates in `f64`.** The GPU port in section 14 must do the same — a reduction over
tiles returns to the CPU as a `f64` total, or is performed in two stages to avoid the same
loss. This costs nothing on the GPU side, because the accounts are never read by a shader.

Flow:

- Light adds to `field` and `influx_total`.
- Photocytes move `field → biomass`.
- Devorocytes move `detritus → biomass`, or `biomass → biomass` when predating.
- Metabolism and movement move `biomass → dissipated`.
- Death moves `biomass → detritus`.
- Detritus decays at a fixed rate into the field tile beneath it, moving `detritus → field`,
  while sinking slowly (this is the marine snow, which is both atmospheric and functional).

---

## 6. Cells

```rust
struct Cell {
    pos: Vec2,
    vel: Vec2,
    radius: f32,        // derived from cell type
    kind: CellKind,
    state: u8,          // developmental identity, 0..=63
    energy_flow: f32,   // last tick's net gain, for rendering brightness
}
```

### Cell kinds

The trade-offs matter more than the list. Each kind must be *worth* specialising into under
some circumstance and not others, or differentiation never evolves.

| Kind | Radius | Upkeep/tick | Function |
| --- | --- | --- | --- |
| `Photocyte` | 3.0 | 0.004 | Harvests from the field tile it occupies, rate ∝ local energy × exposure. **Occluded by cells above it** — this is what rewards spread-out, branching body plans over compact blobs. |
| `Devorocyte` | 2.6 | 0.009 | On contact, drains energy from detritus, or from another organism's cells at a rate reduced by that cell's toughness. |
| `Myocyte` | 2.8 | 0.014 | Oscillates the rest length of its springs. Costs `movement_cost` × work done. The only source of locomotion. |
| `Sclerocyte` | 3.4 | 0.002 | High spring stiffness, high toughness. No metabolic function — pure structure and defence. |
| `Sensocyte` | 2.0 | 0.006 | Samples a local gradient (light, detritus, or foreign biomass — determined by its gene) and emits a scalar signal. |
| `Gonocyte` | 3.2 | 0.005 | Accumulates energy toward reproduction. An organism with no gonocyte cannot reproduce. |

**Design intent:** photocytes want surface area and light, which means being high up and
spread out. Devorocytes want contact, which means reaching things. Myocytes cost energy but
are the only way to reach either. Sclerocytes are the answer to predation but contribute
nothing. Requiring a gonocyte means reproduction has a real structural cost. If in play-
testing one kind dominates every lineage, the costs are wrong — tune before adding kinds.

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

### Development

Deterministic, bounded, and pure:

```
cells = [ Cell { state: 0, kind: Photocyte, pos: origin } ]
for step in 0..max_dev_steps:
    for each cell in cells (in stable index order):
        find the FIRST gene where
            gene.trigger_state == cell.state
            && step within [gene.min_step, gene.max_step]
        if none: continue
        apply the action:
            Divide      → append a daughter at `angle` from the parent's axis,
                          with child_state / child_kind; if `adhere`, create a spring
            Differentiate → change this cell's kind and state in place
            Terminate   → mark this cell inert; it fires no further genes
        if cells.len() == max_cells_per_organism: stop entirely
```

The body is a **pure function of the genome**. Same genome, same body, every time — which
makes it trivially testable and means the museum can rebuild any archived organism exactly.

First-match-wins ordering means gene *position* carries information, so reordering is a
meaningful mutation. Non-firing genes accumulate as neutral genetic material, which is
exactly where duplication finds raw material to diverge — this is a feature, not waste.

### Mutation

Applied at reproduction, in this order:

1. **Point mutation** — each gene, with `point_rate`: perturb numeric fields by
   `N(0, point_sigma)`; discrete fields re-draw uniformly.
2. **Gene duplication** — with `duplication_rate`: copy a random gene, insert adjacent.
3. **Gene deletion** — with `deletion_rate`: remove a random gene.
4. **Gene insertion** — with `insertion_rate`: insert a fully random gene.
5. **Reordering** — swap two adjacent genes.
6. **Whole-genome duplication** — with `genome_duplication_rate`: append a full copy.

**Hard cap at `max_genes`.** Duplication is exponential without it. If genome bloat still
appears under selection, add a small metabolic cost per gene rather than raising the cap —
that is how real genomes are disciplined.

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
vel = (vel + force × dt) × drag        // drag ≈ 0.92, so velocity ≈ force
pos += vel × dt
```

Neighbour queries use a **uniform spatial hash** sized to the largest cell radius. This is
the single most important performance decision on the CPU side: it takes collision detection
from O(n²) to roughly O(n), which at a few thousand cells is a larger win than any language
or hardware choice.

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
amplitude = clamp(0.3 + sensor_gain × signal, 0.0, 1.0)
rest_len  = base_rest × (1 + amplitude × 0.4 × sin(t × osc_freq + osc_phase))
```

Each `Sensocyte` outputs a normalised gradient magnitude toward its `sensor_target`,
sampled from the resource grid or from nearby foreign biomass.

Because `sensor_gain` is signed and evolvable, both attraction and avoidance are reachable
by mutation, and phototaxis, detritus-seeking and predator-avoidance are all *discoverable*
rather than coded. Undulating locomotion falls out of neighbouring myocytes evolving
compatible `osc_phase` values.

**Phase 2 — evolved neural networks**, whose inputs and outputs wire to whatever sensory
and contractile cells the body actually grew, so anatomy and behaviour co-evolve. Far more
interesting, far more expensive per tick. The architecture leaves room; do not build it in
phase 1.

---

## 10. Life cycle

**Metabolism.** Each tick, every cell pays `upkeep × upkeep_scale`. `upkeep_scale` is the
"temperature" slider — raising it is a live environmental pressure. Organisms whose energy
reaches zero die.

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
genus. Colour derives from a hash of the genome so lineages are visually distinct and
**drift in hue as they drift genetically** — you can literally watch speciation happen.

**Events.** Append-only, written in a naturalist's register. Detect and record:

- first adhesion (the origin of multicellularity in this run)
- first appearance of each cell kind
- first predation event
- speciation and extinction, by name
- new records: body size, cell count, genome length, population
- mass extinction (population falls by >50% within 5,000 ticks)
- environmental changes made by the user

> *Tick 41,208 — 41.2 Ma.* A cell has failed to separate from its daughter. **Coacervus
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
