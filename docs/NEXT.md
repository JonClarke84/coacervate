# What is known, and what to do next

**Written to be picked up cold.** Everything in it is measured, everything has a number, and
nothing in it needs re-deriving. `docs/PHASE7.md` is the working record of how the phase got
here; this is the standing summary and the plan.

| | |
| --- | --- |
| **The finding** | [SPEC.md](../SPEC.md) section 1, with the mechanism in section 10 |
| **The instruments** | `crates/coacervate-app/src/assay.rs` — competition, invasion, contact |
| **The price list** | SPEC section 6, *What each of these cells is actually worth* |
| **This document** | the three things worth trying next, in order, and why |

---

## 1. Where the project stands

> **Coacervate as specified produces photosynthetic multicellular life with real body-plan
> diversity, gene duplication driving serial repetition, and named lineages that split and go
> extinct. It does not produce predation or locomotion.**

A body travels about **0.4 of its own length in a whole lifetime** — on the order of 8 world units
— against neighbouring cells about **23 units** away and the nearest body of an unrelated lineage
**60 to 88 units** away. Every specialised cell is paying, every tick, for machinery it can never
get into position to use.

The five measurements behind that are set out in SPEC section 1 and are not repeated here. The one
that matters for planning is the last of them:

⚠️ **The dense world is nine times steeper and nobody knows why.** A third photocyte is worth
**+12.80 %/generation** at four times density against **+1.45** in the shipped world — **+8.94**
against **+1.01** at the corrected generation time below — monotone across six densities with no
threshold anywhere between them. **This is the strongest unexplained signal in the project.** A
world in which a third photocyte is worth nine times more should not exist if income is strictly
linear in photocytes, which section 6 says it is. Something about density is changing what a cell
earns, and whatever it is, it is the only place this project has yet found where being bigger pays
more than proportionally. Candidate 1 below is the same question asked from the design side, and
this is the measurement that says the question has an answer.

---

## 2. The three candidates, in priority order

### ⭐⭐⭐ 1. Increasing returns to size — the deepest, and the real answer

**Nothing in this model makes two cells worth more together than apart.** Income is linear in
photocytes, upkeep is linear in cells, the reproduction bar is linear in cells, lifespan is linear
in cells, and occlusion is actively *sub*-linear because a bigger body self-shades more. That is
why a third photocyte is worth precisely its own cost and every other third cell is a pure loss at
about **−0.35 %/generation for every 0.001/tick of upkeep, whatever the cell does**. Growth is
therefore a random walk, and a specialised cell is a bet nothing can pay off.

**This is a missing mechanism, not a wrong constant.** No value of any number in `config/` changes
the exponent on a linear term. So this is the one candidate that needs a design round rather than
an afternoon, and it is the one that would actually change the world.

The question to design against, stated so that any proposal can be checked against it in one line:

> **What would have to be true for a five-celled body to out-earn five one-celled ones?**

Two footholds already exist. The density signal above says the world already contains *some*
super-linearity nobody has accounted for — find it before inventing one. And the assay prices any
answer in about four minutes a run, which is the whole reason it was built: a proposal can be
refuted before a line of it is written, exactly as the self-shading muscle payoff was.

### ⭐⭐ 2. Travel per lifetime via `dt` rather than lifespan — the cheapest test

Locomotion fails on distance: 8 units of travel against 23 to a neighbour and 60–88 to a stranger.
The obvious lever is to let a body live longer, and **lifespan is the wrong lever and this is
measured**. Lifespan is a cost-side quantity — `LIFETIME_UPKEEP × cells ÷ cost per tick` — and
every cost-side lever in this world is self-cancelling because income is endogenous. See §3 below.

`dt` is different in kind. It multiplies the *physics* — how far a spring's oscillation carries a
body per tick — **without the economy noticing**, because upkeep, income and the reproduction bar
are all charged per tick and per cell and know nothing about how far anything moved. So a larger
`dt` buys travel per lifetime at no cost-side price, which is exactly the trade every other lever
has refused.

⚠️ **Check the orthogonality before trusting any of it.** The claim above — that `dt` moves the
physics and leaves the economy alone — is an argument, not a measurement, and it is the kind of
argument this project has been wrong about before. The test is cheap: run the shipped world at two
values of `dt` and compare the ledger's per-tick accounts, the population, mean body size and the
assay's noise floor. If any of those move, `dt` is not orthogonal and the idea is finished.

⚠️⚠️ **And it is expensive in recordings even if it works.** Changing `dt` invalidates:

- **`MAX_STIFFNESS`**, which is derived from `dt²` — a stability bound, so getting this wrong is
  an integrator that explodes rather than a world that is merely different;
- **every recorded `osc_freq`**, which is in radians per simulated second;
- **the golden vectors**, all six of them.

That is a real bill and it should be paid deliberately rather than discovered. It is still the
cheapest of the three, because it is one number and a sweep.

### ⭐ 3. A non-locomotor payoff for a muscle

The first muscle earns nothing, because one oscillating spring is a reciprocal stroke and goes
nowhere — so a lineage sheds it before a second can arrive beside it. Give the *first* muscle
something to do that is not travel, and the valley has a floor to stand on.

**What has been measured, and it is not encouraging on its own.** Shape-change-for-shading was
priced at **+0.85 %/generation** against a **−2.5** bill (+0.59 against −1.75 re-recorded) — and
that was **self-shading only**, a body changing how much of its own light it blocks. `rest_length`
already collects that whole channel for nothing, one point mutation away, which is why it was
refuted before it was built.

**What has not been tried is a payoff that reaches something already touching the body.** Two
shapes it could take:

- **Holding on to another body** — a contraction that grips rather than swims. A body already in
  contact does not have to cross 60 units of water to reach one.
- **Drawing food in** — a contraction that moves water or detritus toward a mouth, so a bite's
  reach comes from the muscle rather than from travel.

Both share the property that makes them worth trying: **they earn without crossing the distance
that defeats everything else.** Neither is designed and neither is priced. Price first — the assay
takes an afternoon and has now refuted two payoff designs before either was written.

---

## 3. The owner's hypothesis, and the measurement that bears on it

> *Longer lifespans would let a body get somewhere.*

⚠️ **Measured, and it does not work — not weakly, but not at all.** `LIFETIME_UPKEEP` was
**tripled**, and the equilibrium simply absorbed it:

| | before | after tripling |
| --- | --- | --- |
| Field drawdown | 52% | **63.4%** |
| Founder's net income, per tick | +0.0108 | **+0.0072** |

The world re-formed with the marginal body at break-even in **poorer water**. Nothing about how
far a body gets in a lifetime changed, because what changed was how many bodies the water carries.

**The general shape of it, which is worth carrying to every future proposal: income in this world
is endogenous.** How rich the water is depends on how many bodies are eating it, so any change
that makes life dearer kills bodies until the survivors' water is rich enough to pay the new bill —
and any change that makes life cheaper adds bodies until it is poor enough to stop paying. The
equilibrium condition is *the marginal body breaks even*, and a cost-side lever moves the water
rather than the outcome. SPEC section 3 records `upkeep_scale` failing the same way from the
opposite direction, and section 6's myocyte-price sweep is a third instance.

⭐ **That is exactly why candidate 2 is `dt` and not lifespan**, and why candidate 1 is about the
income *exponent* rather than about any price. **The only levers left are ones the economy cannot
see, and ones that change the shape of the income curve rather than its height.**

---

## 4. Two of our own records were wrong, and both are corrected

Caught this week by re-measurement, recorded here so nobody re-derives them.

### `config/dense.toml`'s rationale — corrected

The file, and `main.rs`'s doc comment on
`the_dense_profile_is_the_shipped_world_with_less_water_in_it`, both claimed a Phase 4 measurement:
**13.5%** of cells in foreign contact rising to **58.8%** at four times density, and **eight
times** the predation. Re-measured with `assay.rs`'s `what_a_mouth_meets`, 60,000 ticks, seed 42,
32 devorocyte-carrying founders:

| | shipped | dense | |
| --- | --- | --- | --- |
| contact fraction | 0.4723 | **0.5274** | **×1.12**, not 13.5% → 58.8% |
| predation | — | — | **×1.09**, not ×8 |
| **stranger share** | 0.0004 | **0.0463** | **×116** |

**The open-world figure is the one to trust**: 0.4723 reproduces to four decimals against the
committed assertion in `a_current_buys_strangers_by_spending_contact`, so Phase 4's number is the
wrong one. Bodies in the shipped world were never rarely in contact — they were in contact with
their own descendants, which is a different fact and one density barely touches.

⭐ **What density actually buys is the stranger share, ×116** — a statistic neither document
quoted, and the one the profile should have been read against all along. It is still not enough:
a third devorocyte establishes in nought of thirty-six introductions, including in a world at
three quarters strangers.

### `Contacts::stranger_share`'s doc comment — corrected

It said the shipped world is **0.0010**. SPEC's own sweep table, the assertion in
`a_current_buys_strangers_by_spending_contact` and this week's re-measurement all say **0.0004**.
Only that one line was wrong, and it was the line most likely to be quoted, being the one attached
to the arithmetic.

---

## 5. ⚠️⚠️ `GENERATION` was the mean lifetime, and every coefficient is re-recorded

`assay.rs`'s `GENERATION` was **1,753.9**. That is the mean **lifetime** — how long a body lives.
A generation is the mean age of a parent at the moment it has a child, and that is **1,225.2**,
measured over ticks 50,000–150,000 and 102,622 births. (Over a whole run it is 1,214.3; the
equilibrium window is quoted because the filling phase is full of young parents.)

The two are far apart because a body here reaches its reproduction bar at around tick 458 of its
own life and goes on breeding until it dies. Selection compounds a ratio of descendants once per
**birth**, not once per death.

### The direction, which is the opposite of the obvious one

A shorter generation is **more** generations in the same window — 42,000 ticks is **34.3** rather
than 23.9 — so the same log-ratio spread over more of them is a **smaller** coefficient.

> **Every %/generation figure in this project multiplies by 0.6986. Every generation *count*
> multiplies by 1.4315.**

⭐⭐ **No sign changes, no ordering changes, no ratio changes and no conclusion changes.** One
factor multiplies every coefficient at once, including both sides of every break-even comparison,
so nothing in the week's results has moved. **The week's findings should not be re-read as having
changed.** Only the unit they are quoted in was wrong.

### The headline coefficients, old beside new

| | was, at 1,753.9 | **now, at 1,225.2** |
| --- | --- | --- |
| Window | 23.9 generations | **34.3 generations** |
| Competition noise floor | ±0.16 %/gen | **±0.11** |
| Competition resolution, three seeds | 0.3 | **0.21** |
| Placed-arm noise floor | ±0.13 | **±0.09** |
| Invasion noise floor | ±1.6 | **±1.12** |
| Invasion resolution, three seeds | 5 | **3.5** |
| Cost of a silent cell, per 0.001/tick upkeep | −0.5 %/gen | **−0.35** |
| A third **photocyte** (raw / excess, s42) | +1.78 / +1.52 | **+1.24 / +1.06** |
| A third **`rest_length`**, the largest free shape change | +0.71 | **+0.50** |
| A third **sclerocyte** | −1.07 | **−0.75** |
| A third **myocyte**, holding still | −2.46 | **−1.72** |
| A third **myocyte**, beating | −2.7 to −4.4 | **−1.89 to −3.07** |
| A third **devorocyte** | −6.1 to −9.0 | **−4.26 to −6.29** |
| A **myocyte with an adhered sensocyte** | −8.6 | **−6.01** |
| **Arm B**, a body that genuinely swims | −10.30 (−10.03, −9.85, −11.01) | **−7.20** (−7.01, −6.88, −7.69) |
| Arm B against its own held-still twin | +1.0 ± 1.7 | **+0.7 ± 1.2** |
| Arriving in the best water a lifetime's swim away | −0.01 ± 0.13 | **−0.01 ± 0.09** |
| Invasion: a third photocyte, ×1, three seeds | +5.21 / +8.28 / +7.31 | **+3.64 / +5.78 / +5.11** |
| Invasion: a third devorocyte, ×1, three seeds | −17.03 / −28.94 / −22.27 | **−11.90 / −20.22 / −15.56** |
| Invasion: a third photocyte at dispersal ×128 | +43.78 | **+30.58** |
| Invasion: a third devorocyte at dispersal ×128 | −17.20 | **−12.02** |
| A muscle's break-even bar | +2.5 | **+1.75** |
| The whole measured value of shape | +0.85 | **+0.59** |
| A third photocyte, dense against shipped | +12.80 against +1.45 | **+8.94 against +1.01** |

⚠️ **The long sweep tables are left as they were taken**, with a banner on each saying so — the
fourteen-condition light sweep in `a_body_that_genuinely_swims_is_still_priced_below_one_that_does_not`,
its ceiling table, and `docs/PHASE7.md`'s Group L1 list. One factor multiplies all of them, so
nothing about their shape depends on which divisor is read, and re-typing sixty numbers is more
likely to introduce an error than to remove one.

⭐ **It touches nothing in the simulation.** `assay.rs` is `#[cfg(test)] mod assay;` inside
`coacervate-app`; the constant is used only to turn a tick count into a generation count for
reporting. `coacervate-sim` cannot see it. The four assertion thresholds quoted in %/generation
are all one-sided in the direction the rescaling makes safer, and all still pass.

---

## 6. What not to spend a run rediscovering

- **Predation.** Nought establishments in thirty-six independent introductions, in worlds up to
  76% strangers where the instrument demonstrably resolves. It is arithmetic — a devorocyte costs
  0.009 a tick against a photocyte's 0.004 and must find a stranger to earn anything.
- **Mixing.** A current buys strangers by spending contact and no setting gives both; wider
  dispersal reaches 76% strangers in a *fuller* world and the mouth still does not invade. Neither
  ships, both are recorded in `assay.rs`.
- **The price of a muscle.** A sevenfold sweep of myocyte upkeep moved the standing population of
  muscle 4.7-fold and moved the number of bodies **born** with two or more myocytes not at all.
  Supply, not price.
- **A self-shading payoff for a muscle.** Refuted on the measured coefficient of the exact
  configuration it proposed to seed, before a line of it was written.
- **Any cost-side lever.** §3 above. `upkeep_scale`, myocyte upkeep and `LIFETIME_UPKEEP` have all
  now failed the same way, from three different directions.
