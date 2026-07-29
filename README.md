# Coacervate

**An open-ended evolution simulator.** A window of primordial soup, lit from above. Seed a
single cell, set the conditions, and walk away.

There is no score, no objective and nothing to win. It exists to answer one question: *does
complexity appear, and what form does it take?*

> **Coacervate** (n.) — a droplet that forms spontaneously when organic molecules
> concentrate out of dilute solution. One of the leading candidates for the first cell-like
> structures on Earth: a self-assembling boundary that turns chemistry into something with
> an inside and an outside.

---

## What it does

Light falls from the surface and replenishes a resource field at a slow, constant rate. That
influx is the only energy entering the world, so it sets a hard carrying capacity — and that
ceiling is the pressure everything else grows out of.

Organisms harvest energy, pay metabolic costs, move, reproduce with mutation, and die.
That's the whole ruleset. Multicellularity, cell specialisation, locomotion, phototaxis and
predation are not implemented. They are things that either happen or don't.

Leave it running overnight and it writes a **chronicle** — a generated natural history of
the run, with named lineages and the moments that mattered:

> *Tick 41,208 — 41.2 Ma.* A cell has failed to separate from its daughter. **Coacervus
> primus** is the first lineage to persist as more than one body.
>
> *Tick 96,540 — 96.5 Ma.* **Coacervus vorax** has begun consuming its neighbours.
>
> *Tick 210,880 — 210.9 Ma.* Mass extinction. 94% of lineages lost.

---

## The interesting bit: a genome that can gain structure

Most evolution simulators give each organism a fixed list of numbers — size, speed,
metabolism — and mutate them. It runs fast and it has a hard ceiling: the only thing that
can ever evolve is a *better single cell*. Novel body plans are unreachable, because there
is no slot for a thing that doesn't exist yet.

Coacervate's genome is instead a **variable-length list of condition-action rules that grow
a body from a seed cell**. Rules fire on a cell's developmental state, dividing it,
attaching the daughter, and specialising it into one of six cell types.

Crucially, the mutation operators include **gene duplication and divergence**: copy a rule,
then let the copy drift. Because rules key on developmental state, a duplicated-then-diverged
rule is a *new body part*. That operator is the engine behind essentially all real
biological complexity, and it's what makes open-ended evolution possible here rather than
merely parameter tuning.

---

## Technical approach

Rust throughout, no web layer. `wgpu` handles both the physics compute and the rendering,
which means simulation state never leaves GPU memory to be drawn — a native renderer reads
the same buffers the physics wrote.

- **`coacervate-sim`** — pure simulation. No I/O, no rendering, no `unsafe`. Built
  test-first, with property tests carrying the guarantees: energy is conserved, genomes stay
  bounded, development terminates, runs reproduce exactly from a seed.
- **`coacervate-render`** — `wgpu` instanced rendering with additive blending and bloom, so
  attached cells merge into a single organic silhouette rather than reading as a string of
  circles. `egui` for panels.
- **`coacervate-app`** — `winit` window, main loop, replay log.

A tested CPU implementation came first and remains the reference: the GPU port is validated
by running both from the same seed and asserting they agree. Compute shaders fail by
producing silently wrong numbers, so a differential test against known-good CPU output is
the only way to trust them.

Everything is deterministic from `(seed, config)`. Every arena is allocated once at startup
and never grows, so the simulation cannot leak — when the population cap is reached, births
simply fail.

See [`SPEC.md`](SPEC.md) for the full model and [`CLAUDE.md`](CLAUDE.md) for the design
reasoning and decision log.

---

## Status

In development. Windows x86-64 first, with a second build targeting a Raspberry Pi 5 driving
an attached LCD — an always-on, deliberately slow edition where meaningful evolutionary
change happens over days rather than minutes, and the generated chronicle is the point.

## Honest limitations

Multicellularity, cell specialisation and a feeding-strategy split are realistic outcomes of
an overnight run. Nervous systems, eyes and animal-like intelligence are not, on any
timescale this will ever run for. The aim is a substrate that *could* go there, not a
simulation tuned to fake an impressive result on a schedule.
