# Handoff — read this first after a context reset

Written 17 August 2026, at the end of a long session. **This is the complete state.** It is
written so a session with no memory of the conversation can pick up without re-deriving
anything. `docs/NEXT.md` is the shorter "what to do next"; this is the full record.

---

## 0. ⭐⭐⭐ The locomotion round — added later the same day, and it changes section 13

Everything below section 1 was written **before** a seventh cell kind existed. It is still
accurate about the six-kind world; this section is what happened next, and `docs/NEXT.md` §8
carries all of it in full with the numbers.

**Built:** `CellKind::Flagellocyte`, a motor. Thrust is *computed, not simulated* — a real
flagellum beats at 100 Hz and `dt` is a sixtieth of a second, so a simulated stroke would be an
aliased one; resistive-force theory integrates it away and gives a mean thrust linear in beat
frequency, which is why `osc_freq` is the magnitude gene. Direction is **outward along the
vector from a cell's adhered partners to itself**, so a symmetric body's motors cancel and where
a lineage puts them is what evolution has to find. `physics.thrust` ships at 0.0;
`config/flagellum.toml` is the same world at 40.

**Why a motor at all, when a myocyte exists:** muscle needs two myocytes, phased, on a bent body
— the scallop theorem forbids less — which is three mutations that pay nothing until all three
arrive. **Biology never crossed that valley either.** The bacterial flagellum's export apparatus
is homologous to the Type III secretion system; twitching motility runs on pili that evolved for
DNA uptake; the cilium sits on microtubule transport that already existed. Every motor in life is
an exaptation of machinery that already worked.

### What is measured

| | |
| --- | --- |
| **Travel** | 88.8 units per lifetime at thrust 40, against 2.1 with the motor off. **The 60–88 unit gap to a stranger closes.** |
| **Cost curve** | Turns over: at thrust 250 a body dies at tick 273, at 600 in three ticks. Running cost goes as force², travel as force. Nobody chose that — it is Stokes drag. |
| **Income, blind** | +3.38% more food found, gross. But at thrust 100 a body finds **8% less** — `light.gradient` is 0.75 and an unsteered body walks out of the light. |
| **The bar** | A flagellocyte costs 0.006/tick — **9.0 over 1,500 ticks** — and blind travel finds 4.7. **It earns half its keep.** Every null in this round is that one ratio. |
| ⚠️ "Steered" | `sensor_gain = −1.0`: scored best, and **it is the motor switching itself off.** See the retraction below. |

### ⚠️⚠️⚠️ The retraction — read this before believing anything about steering

A `sensor_gain` of −1.0 was reported as **orthokinesis** earning 170% of its keep, and as
**+2.358 %/generation** over a blind motor across three seeds. The coefficient is real. The
mechanism is not what it was called.

`behaviour.rs`'s controller is `clamp(resting_amplitude + sensor_gain × signal, 0, 1)` with
`resting_amplitude` at 0.8, so **at a gain of −1.0 any signal at or above 0.8 clamps the amplitude
to nought and the motor stops.** Measured: a gain of −1.0 leaves the motor at **10.5% amplitude**,
and the gain sweep and a plain beat-frequency sweep trace the *same curve*. It is a throttle, not
a rudder. Independently: at fourteen and eighteen cells a body with a motor and a body with a
**sclerocyte** in that slot have the same extent and the same travel to three significant figures
— the motor is not running at all.

So the "170%" figure added back a running cost that was never spent (cost goes as force², and the
motor was at a tenth). And the +2.358 %/gen is *a throttled motor beating a full-throttle one* —
what a negative gain buys is the running cost it avoids. Real, and much smaller than it looked.

⭐ **The lesson, and it is the third instance in one round: a parameter with a good coefficient is
not a mechanism.** What caught this was asking a question with no coefficient in it — *how far
does the body actually move?*

⚠️ **Every body measured in the first five experiments of this round had `sensor_gain = 0`.** The
round priced an organelle with its one steering input nailed shut, and that is what the five
nulls had in common.

### ⚠️⚠️⚠️ The error that voided three experiments, and how it was found

`founder_with_a_third_cell` pushed a gene with **`osc_freq: 0.0`**. `behaviour.rs`'s `propel`
computes `force = thrust × osc_freq × amplitude` and returns early on `force <= 0.0`, so **every
founder-based "third flagellocyte" was a sclerocyte with a dearer upkeep** — never charged, never
pushed, unable to feel `physics.thrust` at any value. It voids the empty-water price, the nine
92,000-tick invasions, and the diffusion sweep's motor arm.

⚠️ It re-baselines the **myocyte** too: `osc_freq = 0` is a muscle that never contracts, so every
third-myocyte coefficient in this project's history was taken on a muscle at rest.

**Found by an adversarial audit** — four agents reading the code against its own claims, each
finding put to an independent refuter, 19 of 32 surviving. **Nothing else would have found it:**
every instrument agreed with every other, because they were all wired to the same dead cell. The
audit also found a test cited by name in `Physics::propel` that **did not exist** — the one
guaranteeing `thrust = 0.0` ships inert, which protects ~300 recorded coefficients. It exists now.

With a motor that runs: empty water gives −2.458 → −7.612 → extinct, monotone; and in a settled
world a motorised invader manages **0 establishments out of 12** while a third photocyte invades
10 of 12 at +4.66 %/gen in the same water on the same tick.

### ⚠️ Refuted in this round — do not retry

- **A motor pays as a founder's third cell.** −2.5 %/gen in competition; invasion means −10.3,
  −7.2, −15.2 at thrusts 0, 40, 100 with a **between-seed spread of 17 to 24**. A single seed
  read +2.0 and was very nearly written up as a discovery. Three seeds killed it.
- **Lowering `light.diffusion`.** Blind income rises hugely (435% of the bar at zero diffusion)
  **and the control arm collapses with it** — a third photocyte goes +1.678 → +0.102. Slow water
  means tiles under nobody fill to cap and spill; less of the world's light gets eaten at all.
  ⭐ *General form, shared with six earlier rounds: a lever that improves what a specialist earns
  while improving what everyone earns by more is not a lever.*
- **Making the cell cheaper.** It would need to cost under 0.0031/tick, below the photocyte's
  0.004, and that is measured to be where neutral bloat begins.

### What is running

`config/emergence.toml` — tempo ×8 + Kleiber 0.75 + thrust 40, flagged in its own header as
**three changes at once that have not been measured together**, so nothing may be quoted from it.
At 365 Ma it holds bodies of **43.1 ± 18.5 cells** on genomes of **21.7 ± 6.7 genes**.
`config/flagellum.toml` — thrust alone — at 840 Ma holds 6.8-cell bodies on 11.9-gene genomes with
motors, mouths and sensors all present at single figures and not spreading.

---

## 1. Where the project stands

**313 tests green**, `cargo fmt --check` and `clippy -D warnings` clean, everything
committed and pushed to `github.com/JonClarke84/coacervate` on `main`.

| Phase | State |
| --- | --- |
| 1 workspace, config, seeded RNG | done |
| 2 resource grid, physics, energy ledger | done |
| 3 genome, development, mutation | done |
| 4 reproduction, death, detritus | done |
| 5 renderer, PNG dump, window | done |
| 6 egui panels, sliders, charts | done |
| 7 species, names, event log | **A, B, C done** — clustering, binomial naming, event log |
| 7 D, E | **not done** — Darwin marginalia, inspector, museum |
| 8 replay log, scrubbing, chronicle | **not started** — the biggest missing piece |
| 9 GPU compute port | not started |
| 10 polish, packaging | not started |

**Phase 8 matters more than its number suggests.** CLAUDE.md's entire premise is *leave it
running overnight and read what happened in the morning*. The event log is currently a ring
of the last 1,024 events **in memory only**. When a run ends, its history is gone. There is
no chronicle. That is the gap between "an instrument you watch" and "an instrument you can
leave", and it is the one major phase still missing.

---

## 2. The two instruments — this is the project's real asset

Both live in `crates/coacervate-app/src/assay.rs`, `#[cfg(test)]`, run deliberately with
`cargo test --release -- --ignored --nocapture assay`.

**The competition assay.** Two founder sets differing by exactly one mutation, seeded
alternately after the dawn so neither arm gets systematically better water. Every later
birth is attributed to its parent's arm. After 42,000 ticks the ratio of living descendants
**is** the selection coefficient. Noise floor **±0.11 %/generation** with three seeds;
resolves about 0.21. **Attribution is complete** — 0 unattributed births in every run ever
taken. Forty minutes instead of a day-long run.

**The invasion assay.** A resident population is left to its plateau, then three arms of
twelve invaders are released into the same water on the same tick. Invasion fitness is the
least-squares slope of log frequency against generations, and each introduction is followed
separately so an **invasion probability** is reported beside the growth rate. Calibrated
against the competition assay: both signs and the arm ratio reproduce. Noise floor
**±1.12 %/gen** — four times coarser, so do not reach for it where the competition assay
can answer. It exists because the competition assay's tight floor **depends on the arms
never meeting**, so a well-mixed world is the one condition it cannot measure.

⚠️ **Both measure the *filling* regime** — two-celled bodies growing into empty water, where
a photocyte earns roughly ten times its equilibrium income. Do not quote a coefficient as a
fact about a mature world without seeding into one.

⚠️ **`GENERATION = 1_225.2`** is the demographic generation time (mean parent age at a
birth). It was 1,753.9 — the mean *lifetime* — until this session. Every %/generation figure
recorded before that fix is **overstated** and multiplies by **0.6986**. No sign, ordering,
ratio or conclusion moved.

**These instruments have caught four of our own records being wrong.** That is the point of
them and it should keep happening.

---

## 3. The measured state of the world

Shipped config, ~300,000 ticks, eight founders, seed 42.

| | |
| --- | --- |
| population | ~550–950 |
| total living cells | ~5,300 |
| biomass | ~33,000 |
| mean body | 9.6 ± 3.0 cells |
| mean genome | ~6 genes |
| field drawdown | ~65% |
| generation | ~1,225 ticks |
| throughput | ~1,071 ticks/s headless |

Selection coefficients, competition assay, shipped world (post-`GENERATION` fix):

| a third… | %/gen |
| --- | --- |
| photocyte | **+1.06** |
| gonocyte | −0.48 |
| sclerocyte | −0.75 |
| myocyte | −1.72 |
| devorocyte | −4.26 to −6.29 |
| myocyte with an adhered sensocyte | −6.01 |

**A photocyte is the only positive cell in the world.** Every other kind is shed within about
two dozen generations.

---

## 4. The chain of findings, in order

Eleven rounds. Each was measured, and several corrected the round before.

1. **Nothing could move at all.** A free body's total velocity is a *conserved quantity of
   the integrator* — internal forces sum to zero, there is no mass, drag is one scalar, so
   `Σv ← Σv × drag`. Measured 5.96e-7 over 2,000 ticks. Fixed with anisotropic drag
   (`physics.drag_anisotropy = 2.0`), which is the slender-body ratio.
2. **A single muscle is reciprocal motion** and produces exactly zero displacement — the
   scallop theorem, correct physics. Locomotion needs **two phased muscles on a bent body**.
   A straight body cannot swim at all, correctly: nothing one-dimensional swims in any fluid.
3. **`movement_cost` was ~1,000× too high.** 0.15 → 0.0001.
4. **A genome addressed 2.2% of its own body.** Behaviour was looked up by matching a cell's
   *state* against gene triggers — 64 states, ~3 genes. Muscles were anatomically present and
   behaviourally disconnected: 56,903 spring-ticks of a myocyte on a spring, **zero** of one
   that moved. Fixed: a cell reads the gene that built it. Muscles now fire (9,901).
5. **Development had the same bug**, which is why mean body size sat at 1.98 cells for the
   first 140,000 ticks of every run ever measured. Mutation now draws states from an
   alphabet the genome actually uses (0.75 for `trigger_state`, 0.25 for `child_state` —
   opposite biases, deliberately).
6. **A body travels ~0.4–0.65 of its own length in a whole lifetime.** Invariant across every
   body and configuration measured.
7. **Contact is inherited, not encountered.** A newborn is placed 6.2 units from its parent,
   two photocytes touch at 6.0, neighbours are ~23 units apart, strangers 60–88.
   **99.9% of a devorocyte's contacts are its own descendants.**
8. **Predation is out of reach.** Invasion analysis: **nought establishments out of
   thirty-six** independent introductions, across dispersal ×1, ×32 and ×128 — including a
   world where 76% of a mouth's contacts are strangers and a photocyte invades at +30.6%/gen
   in the same water on the same tick.
9. **Income is endogenous, so cost-side levers self-cancel.** Tripling `LIFETIME_UPKEEP`
   moved field drawdown 52% → 63.4% and the founder's net income +0.0108 → +0.0072; the
   equilibrium re-formed with the marginal body at break-even in poorer water.
   ⚠️ **Amended in round 11** — see §5.
10. **Nothing is super-additive.** Six candidate cell pairs, three seeds. The only interaction
    that reproduces is **negative**: photocyte on photocyte, −1.24 ± 0.27, about 5σ.
    Photosynthesis has decreasing returns, which is sub-linear self-shading showing up.
11. **Body size was a quotient.** Total tissue is pinned by the light, so mean body size is
    just biomass ÷ population. Every "bodies got bigger" result was the same tissue divided
    among fewer bodies. **Until Kleiber.**

---

## 5. ⭐ The breakthrough: sub-linear metabolic scaling

`metabolism.scaling_exponent`, shipped at **1.0** (inert). A body's charge is
`(Σ its cells' upkeeps) × n^(k−1)`, applied to the **sum** so per-kind costs stay intact.

**Kleiber's law.** Real metabolic rate scales as mass^0.75, not linearly — one of the most
robust quantitative laws in biology, across 27 orders of magnitude. A larger organism spends
*less per unit mass*. West, Brown and Enquist derive the exponent from distribution networks
being space-filling fractals; this world's cells are joined by springs and share one energy
store, which is such a network.

300,000 ticks, eight founders, seed 42:

| k | **total living cells** | cells/body | alive | biomass | drawdown |
| --- | --- | --- | --- | --- | --- |
| **1.00** ships | 5,282 | 9.60 | 550 | 33,293 | 64.7% |
| 0.90 | 6,646 | 7.62 | 872 | 40,874 | 66.3% |
| **0.75** `kleiber.toml` | **10,444** | **12.01** | 869 | 65,714 | 77.6% |
| 0.60 | 25,551 | 41.01 | 623 | 152,641 | 88.0% |

**Total living tissue rose ×4.8 and the population rose with it.** Not a quotient — more
tissue, in more bodies, that are also individually larger. First time in the project's
history. Both sides of the arithmetic move as predicted: time to the reproduction bar goes as
`1/(I − n^(k−1)u)`, whose denominator grows with size, and lifespan goes as `n^(1−k)`.

⚠️ **0.60 overshoots** — 28.4% of bodies pinned at the 64-cell cap. `kleiber.toml` ships at
0.75, where the cap is a 3.3% rarity. Gate bounded 0.5..=1.0.

**It reprices specialisation but does not rescue it:** photocyte +1.24 → +2.57, myocyte
−1.96 → −1.67, devorocyte −5.05 → −3.38. **Neither negative crosses zero.** What became
affordable is *size*, not *function*. Asserted in tests so no future round misreads it.

**⭐ And it amends finding 9, which is the most useful general lesson here:**

> A cost lever that moves **every body's bill by the same factor** is absorbed by the
> equilibrium. One that changes **how the bill varies with size** is not.

Six earlier nulls were the first kind. This was the second.

**Observed in the running world:** body size 10.48 **± 12.17** cells, against the linear
world's 9.6 ± 3.0. The mean barely moved; **the spread quadrupled.** `docs/frames/kleiber-150k.png`
shows rosettes, filaments, branched forms and pairs coexisting — the first frame with several
body plans at once.

---

## 6. Throughput — ~8× more evolution per hour

| | ticks/s | ticks/gen | gens/hour | 12-hour run |
| --- | --- | --- | --- | --- |
| before | 757 | 1,196 | 2,277 | 27,300 |
| after (bit-identical) | **1,071** | 1,196 | 3,225 | 38,700 |
| `config/tempo.toml` | 526 | **152** | **12,407** | **~149,000** |

**What worked:**
- **The spatial hash rebuilt O(buckets), not O(cells)** — 50,869 buckets swept three times a
  tick against ~4,000 cells, costing 105 µs on an *empty* crowd. Now O(cells). 757 → 902,
  then 1,071 after packing the arrays for cache.
- **`rayon` on `Behaviour::look`** (22% of a tick): 244 → 124 µs. **Bit-identical by
  construction** — it writes only `want[index]`/`signal[index]` and accumulates nothing.
  SPEC §2's per-organism RNG streams exist precisely so this is safe.
- **`config/tempo.toml`, no code at all** — multiply every per-tick *rate* by 8 (`influx`,
  `cap`, `upkeep_scale`), leave every *stock* alone. Mean age 928 → 109 ticks. Biomass and
  total living cells do not move.

⚠️ **The tempo profile halves the mean body** (8.17 → 3.87) with the population doubling, and
the step falls oddly between two settings rather than spreading — an unidentified
discretisation threshold. **Do not read a body-size result off tempo until someone finds it.**

**Still on the table:** collision search `push_apart` (25% of a tick) and the grid (18%).
`docs/NEXT.md` §7 carries a bit-identical parallel design for the first.

---

## 7. Refuted — do not spend a run rediscovering these

| Idea | Why it failed |
| --- | --- |
| **A second currency (nutrient), flat quota** | `k` cancels out of the reproduction gate; composition-blind. |
| **Liebig's minimum, `min(energy, nutrient)`** | `min(aP,bG) > θc(P+G)` is **homogeneous of degree one** — scale a body and both sides double. A minimum adds sensitivity to *composition*, not to *size*. A concavity cannot make a convexity. Priced: control collapsed from +1.215 to +0.205. |
| **Nutrient from below** | Source and return centroids separate by 53 units on a 1,152-unit column, and diffusion erases a feature that size in 549 ticks against a 1,737-tick lifetime. |
| **Dispersal** | Buys strangers by destroying contact. ⚠️ **But the round that rejected it read a broken instrument** — the control arm was going extinct. The invasion assay later settled it properly: still no establishment. |
| **A depth-dependent current (`physics.current`)** | A shear creates relative motion in proportion to **depth separation**, and two bodies close enough to touch are ≤6 units apart vertically. Needs `current ≈ 400` to close a 23-unit gap. Shipped inert at 0.0 as a recorded negative. |
| **Widening the shadow (`light.shadow_spread`)** | Points the **wrong way**: a cone from an upper cell covers more of its *own* lower cells, and bodies are compact. spread 0→6 took bodies 9.66 → 2.95 cells. Shipped inert at 0.0. |
| **Density** | Concentration, not a return to size — total cells flat while population falls. And it censors the assay (an arm goes extinct). |
| **`LIFETIME_UPKEEP`** | Equilibrium absorbs it (finding 9). |
| **`dt`** | **Not a throughput lever.** `DT` is read by the integrator, sinking and the oscillator — by *nothing* in metabolism, reproduction or the ledger. Upkeep is per *tick*. An 8× `dt` moved generation length 8% the wrong way. |
| **Raising the population** | **Negative.** Cost per cell is not constant — denser buckets mean more collision pairs. 5× population gave **half** the throughput. |
| **Gravity / sinking** | A muscle cannot oppose a uniform force, for the same conservation reason as finding 1. Contraction changes the rate of fall by 0.1%. |

---

## 8. ⚠️ Claims of ours that did not reproduce

Recorded so they are not re-quoted. **Every one was caught by re-measurement.**

1. **"The five-cell swimmer is structurally sterile, zero offspring from sixteen founders."**
   False. It produces 111 — and a committed test in the same repo asserts >48 and passes.
2. **"Nothing has an increasing return to being more than one thing" (as a headline).** The
   third-photocyte coefficient it rested on did not reproduce: +0.04 claimed, +1.52 measured.
   The *conclusion* survived by other routes, but not that number.
3. **Phase 4's contact statistics** (13.5% → 58.8% of cells in foreign contact at 4× density,
   8× predation). Measured: **0.4723 → 0.5274 (×1.12)** and predation ×1.09. What density
   actually buys is the **stranger share, ×116** — a statistic no document quoted.
4. **`GENERATION`** was the mean lifetime, not a generation time — and the correction runs
   *opposite* to the way it was first described. Coefficients multiply by 0.6986.
5. **"A minimum stops the reproduction gate being homogeneous."** Wrong; see §7.

---

## 9. Open questions

- **Q: why is the dense world nine times steeper?** *Answered*: not super-linearity — a full
  world converts a fixed advantage into competitive exclusion. Standing ratio is 11× the
  birth ratio; the plain arm keeps being born and keeps dying.
- **The tempo body-size threshold** (§6). Unidentified.
- **`reseed_on_extinction`** is a config key that deliberately does nothing.
- **`point_sigma`'s upper bound of 1** is not derived from SPEC — it is a standard deviation,
  not a probability.
- **`spring_damping` has no stated semantics** in SPEC; a meaning was chosen in `physics.rs`.
- **Trig and log are what IEEE 754 does not pin**, so a body's shape is reproducible on *this*
  toolchain. Phase 8's archive and Phase 9's GPU port both need to verify rather than assume.
- **Phase 8 hazard**: `LIFETIME_UPKEEP` and friends are *constants*, and the replay log embeds
  only `config.toml`. A replay recorded before a constant changes will replay wrongly and
  silently. Bump a snapshot version byte and refuse an old one.

---

## 10. What to do next

**The owner's stated goal: complex life with body parts, evolving in hours not millennia.
He has said he is open to changing CLAUDE.md if it works against us, and open to gamifying.**

**1. The propulsion organelle.** A seventh cell kind whose function is thrust, computed
rather than simulated — direction along the cell's own axis (development already chooses it),
magnitude from genome parameters, modulated by whatever sensor the body grew.

Why this and not more muscle work: **real life never crossed this valley.** The bacterial
flagellum's export apparatus is homologous to the **Type III secretion system** — machinery
built to *secrete*, co-opted to move. Twitching motility runs on pili that evolved from DNA
uptake. The eukaryotic cilium is built on microtubule transport that already existed. Nothing
in biology paid for a half-built engine. So a motor that works from **one mutation** is not a
cheat, it is the biological case.

It also sidesteps the timestep: a real flagellum beats at 100 Hz and `dt = 1/60` cannot
resolve anything above ~3 Hz. **Do not simulate the stroke; compute the thrust.** That is what
real microswimmer modelling does (resistive-force theory, squirmer models).

⚠️ Costs: a seventh `CellKind` shifts every `child_kind` mutation draw, so recorded figures
move. And thrust must **not** point at food — a motor is a gift like a photocyte's harvest;
a strategy is not.

**2. The CLAUDE.md edit, proposed and not yet made.** Decision-log line *"Predation emergent,
not scripted"* is being read as forbidding **motors** as well as **strategies**. Add that the
rule bars scripted strategies and not organs: a photocyte that harvests and a flagellocyte
that pushes are the same kind of gift, and what must stay emergent is whether evolution grows
one, how many, where, at what angle, and wired to which sensor.

**3. Close the reach gap.** *E. coli* is 2 µm and swims 30 µm/s — **15 body-lengths per
second**, ~18,000 per lifetime. Coacervate manages **0.65**. Ocean bacteria sit 10–20
body-lengths apart and cross to a neighbour in about a second. We are short by four or five
orders of magnitude, and **that ratio is the single number behind both predation and
locomotion**.

**4. Then Phase 8** — the replay log and chronicle. It is the premise of the whole project
and every long run so far has been forgotten as it ended.

**Do not** spatially zoom out / coarse-grain regions. It is a large architectural change to a
313-test codebase and it does not touch the binding constraint, which is a *ratio* that scales
with the zoom.

---

## 11. Working practice — things that will waste an hour otherwise

- **`cargo` is NOT on PATH.** `export PATH="$HOME/.cargo/bin:$PATH"`.
- **⚠️ PowerShell has been failing this session.** Use the **Bash** tool; paths as
  `/c/Users/joncl/...`. `check.ps1` may need `-ExecutionPolicy Bypass`.
- **`export CARGO_TARGET_DIR=/c/Users/joncl/ct2`.** `/c/Users/joncl/ct` is often locked by a
  stale process; the repo's own `target/` gets locked by any running `coacervate-app`.
- **Commit with `git commit -F <file>`**, never `-m` with a long message — quotes and
  apostrophes break the shell parse. Write the message with the Write tool first.
- **⚠️ NEVER round-trip a source file through PowerShell `Get-Content`/`Set-Content`.** PS 5.1
  double-encodes UTF-8 and silently corrupts em-dashes. Use the Edit tool. To repair: re-encode
  the mojibake to codepage 1252 bytes and decode as UTF-8.
- **Background jobs die when a tool call returns.** Use `nohup … & disown`, or run in the
  foreground inside one call with `wait`.
- **Subagent prompts have twice tripped safeguard false-positives.** Rephrase plainly and
  retry; it is not the content.
- **Long runs**: `--config <profile> --ticks N`, or unbounded with a 12-hour wall-clock
  default. `--dump-frame <path>` re-runs from scratch deterministically, so a frame at
  400,000 ticks costs a 400,000-tick run.
- **Verify with**: `cargo test --manifest-path Cargo.toml --workspace --release -- --include-ignored`,
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.

---

## 12. Config profiles

| file | what it is |
| --- | --- |
| `default.toml` | the shipped world; **every inert key at its inert value** |
| `kleiber.toml` | `scaling_exponent = 0.75` — ⭐ the breakthrough |
| `tempo.toml` | every per-tick rate ×8 — ~4× generations per hour |
| `dense.toml` | same energy in a quarter of the water |
| `seasonal.toml` | `season_amplitude = 0.25` |
| `current.toml` | a recorded negative, not a recommendation |

**Keys shipped inert** (default = the world every recorded figure was measured on):
`physics.current = 0.0`, `light.season_amplitude = 0.0`, `light.shadow_spread = 0.0`,
`metabolism.scaling_exponent = 1.0`. Each has a test asserting the default world is
bit-for-bit unchanged. **This discipline is why 300+ recorded coefficients are still valid,
and it should not be broken.**

---

## 13. The honest summary

**What this specification produces:** photosynthetic multicellular life with real body-plan
diversity, gene duplication driving serial repetition, named lineages that split and go
extinct, and — since Kleiber — genuine size and shape diversification.

**What it does not produce:** predation or locomotion. Both are the same number — a body
travels 0.4 of its own length in a lifetime, against strangers 60–88 units away. Everything
specialised exists to *reach* something, and nothing here can reach anything.

⭐⭐⭐ **⚠️ AMENDED BY SECTION 0, THE SAME DAY.** The distance half of that is no longer true. A
flagellocyte carries a body **88 units in a lifetime**, which is the whole of the gap to a
stranger, and it does it for a price the physics set rather than one anybody chose. What replaced
the distance problem is a **price** problem, and it is now known to one decimal: a motor costs
0.006 a tick and blind travel earns 0.0037. **The reach gap is closed and the ledger is not.**

⚠️ **And the ledger does not close.** For a while it looked as though it did — a motor with a
sensor and a negative gain appeared to earn 170% of its keep — and that was the motor switching
itself off; see the retraction in section 0. Every honest reading has a motor as a loss: −2.5
%/generation as a founder's third cell, −1.6 on a five-celled body that already senses, and no
trend with body size that survives its own between-seed spread.

**Predation is measured too, and it is three parts in ten thousand of world income** — in the
shipped world, under Kleiber, and under Kleiber-plus-motors alike, with the *shipped* world highest
of the three. Making bodies bigger does not buy predation; the extra surface a large body has is
surface against its own other cells.

**The best thing built here is still not a feature.** It is that this project can tell a real
effect from a hopeful one in forty minutes, and used that — in this round alone — to kill its own
most exciting result the same afternoon it produced it, to catch a helper computing the wrong
quantity by four orders of magnitude, and to catch an assertion that passed on noise. Every one of
those was found by re-measurement rather than by review. **A finding that survives that is worth
something; the discipline is worth more than any of the findings.**
