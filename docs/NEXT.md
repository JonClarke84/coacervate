# What is known, and what to do next

**Written to be picked up cold.** Everything in it is measured, everything has a number, and
nothing in it needs re-deriving. `docs/PHASE7.md` is the working record of how the phase got
here; this is the standing summary and the plan.

| | |
| --- | --- |
| **The finding** | [SPEC.md](../SPEC.md) section 1, with the mechanism in section 10 |
| **The instruments** | `crates/coacervate-app/src/assay.rs` — competition, invasion, contact |
| **The price list** | SPEC section 6, *What each of these cells is actually worth* |
| **This document** | the three candidates — the first is **built and measured**; the other two are open |

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
more than proportionally. Candidate 1 below is the same question asked from the design side; it
has now been built and it works, and **nobody has yet checked whether this signal survives it**,
which is the first thing worth doing with the new profile.

---

## 2. The three candidates, in priority order

### ✅ 1. Increasing returns to size — **built, measured, and it works**

**This candidate is done.** It is `metabolism.scaling_exponent`, it ships inert at 1.0, and
`config/kleiber.toml` is the same world at Kleiber's 0.75. SPEC section 10 carries the mechanism
and the sweep; SPEC section 6 carries what it does to the price of a third cell. What follows is
the short version.

The question this section used to ask was *what would have to be true for a five-celled body to
out-earn five one-celled ones?* The answer was on the **cost** side rather than the income side: a
body of `n` cells now pays `(Σ its cells' upkeep) × n^(k − 1)`, which is Kleiber's law — real
metabolic rate goes as mass^0.75 rather than linearly, and West, Brown and Enquist derive the
exponent from a space-filling distribution network, `d / (d + 1)`. Cells joined by springs sharing
one energy store are such a network.

⭐⭐ **Total living cells, 300,000 ticks, eight founders, seed 42:**

| `scaling_exponent` | **total living cells** | cells/body | at the cap | alive | biomass |
| --- | --- | --- | --- | --- | --- |
| **1.00** — ships | **5,282** | 9.60 | 0.00% | 550 | 33,293 |
| 0.90 | **6,646** | 7.62 | 0.11% | 872 | 40,874 |
| **0.75** — `kleiber` | **10,444** | 12.01 | 3.34% | 869 | 65,714 |
| 0.60 | **25,551** | 41.01 | **28.41%** | 623 | 152,641 |

**The total rises ×4.8, and the population rises with it** — so it is not the concentration every
earlier round measured. The 1.00 row reproduces the world this project has always run.

⚠️⚠️ **0.60 overshoots**: more than a quarter of every body is pressed against
`max_cells_per_organism`, which is a world running into its arena rather than one finding a size.
Three quarters is where most of the effect arrives with the cap still a rarity.

⚠️ **And it is a repricing rather than a payoff.** A third devorocyte goes from −5.05 to −3.38
%/generation and a third myocyte from −1.96 to −1.67; **neither crosses zero.** Nothing here makes
a mouth or a muscle *earn* anything — §6 below is untouched. What is now affordable is size, and
the arm that grows fastest under it is the one made of the cell that already earned.

**What is open, in the order it is worth asking.** Does the mature world at 0.75 grow anything the
linear one did not — the cell census at 300,000 ticks is the cheapest place to look. Does the
density signal below (a third photocyte at ×9 in a dense world) survive the change, since both are
claims about a return to being bigger. And whether the exponent belongs at 0.75 or 0.90 is a
judgement about the cap, not a measurement anybody still needs to take.

### ⭐⭐ 2. Travel per lifetime via `dt` rather than lifespan — the cheapest test

> ⚠️⚠️ **Measured since this was written, and the orthogonality claim below is confirmed — which
> is also why `dt` buys no evolutionary throughput whatever.** See §7. The candidate stands
> exactly as written for what it was proposed for, which is travel; it must not be reached for as
> a way of getting more generations into a night.


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

⭐ **That is exactly why candidate 2 is `dt` and not lifespan**, and why candidate 1 is about an
*exponent* rather than about any price. **The only levers left are ones the economy cannot see,
and ones that change the shape of a curve rather than its height.**

### ⚠️⚠️ Amended by candidate 1, and the amendment is the useful part

`metabolism.scaling_exponent` **is** a cost-side lever and it did not self-cancel, so the rule
above is too strong as written. What actually happened is the rule working exactly as stated and
producing a different outcome anyway:

- The equilibrium *did* re-form in poorer water — the field drawdown goes from **64.7% to 88.0%**
  at 0.75, which is the same absorption `LIFETIME_UPKEEP` produced.
- But the break-even condition is *the marginal body breaks even*, and that body now has a
  **size**. Total tissue is `influx ÷ upkeep per cell`, and upkeep per cell stopped being a
  constant.

> **The corrected rule: a cost-side lever that moves every body's bill by the same factor is
> absorbed. One that changes how the bill varies with body size is not.** `upkeep_scale`, myocyte
> upkeep and `LIFETIME_UPKEEP` are all the first kind, which is why all three failed the same way.

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
- **Any cost-side lever that moves every body's bill by the same factor.** §3 above.
  `upkeep_scale`, myocyte upkeep and `LIFETIME_UPKEEP` have all now failed the same way, from
  three different directions. ⚠️ The qualifier is new and is doing work:
  `metabolism.scaling_exponent` is a cost-side lever that changes how the bill varies with *size*,
  and it is the one that moved total living tissue — see §3's amendment.
- **A specialised cell paying for itself under sub-linear scaling.** A third devorocyte at 0.75 is
  −3.38 %/generation and a third myocyte −1.67; both are cheaper than they were and both are still
  a loss by thirty and fifteen times the noise floor. Sub-linear metabolism makes *size*
  affordable, not specialisation.
- **`dt` as a way of getting more generations into a night.** §7. It is a lever on travel per
  lifetime and on nothing else, and that is now measured rather than argued.

---

## 7. ⭐⭐⭐ Throughput: where a tick goes, and what actually buys generations

**The owner's requirement, in his words: he does not want correct physics that would need 10,000
years of real time before anything meaningful appears.** Everything here was measured on the
i5-13400F, seed 42, eight founders, release build. The instruments are `run.rs`'s
`how_fast_the_shipped_world_turns`, `what_a_tick_costs_an_empty_world`,
`what_dt_does_to_a_generation` and `what_a_generation_costs_in_ticks` — the tick-scale counterpart
to what `assay.rs` does for a cell.

### ⭐⭐⭐ The number the owner asked for: generations per wall-clock hour

Two instruments, and the ratios are quoted **within** each rather than across them, because the
two read the world at different ages and a generation lengthens as a run matures.

`how_fast_the_shipped_world_turns` — 60,000 ticks past the founding, 4,385 cells before and 4,374
after, which is as matched a pair as this project can take:

| | ticks/s | ticks per generation | **generations/hour** | in a 12-hour run |
| --- | --- | --- | --- | --- |
| shipped, **before** this round | 757 | 1,196 | **2,277** | 27,300 |
| shipped, **after** | **1,071** | 1,196 | **3,225** | 38,700 |

`what_a_generation_costs_in_ticks` — every arm settled for `200,000 ÷ k` ticks, all on the code
above:

| | ticks/s | ticks per generation | **generations/hour** | in a 12-hour run |
| --- | --- | --- | --- | --- |
| shipped, `k = 1` | 799 | 1,297 | **2,220** | 26,600 |
| `config/tempo.toml`, `k = 8` | 526 | **152** | **12,407** | **148,900** |

**×1.42 from making a tick cheaper** — bit-identical, ships live, no golden vector moves. **×5.59
from making a generation take fewer ticks** — which changes what a run produces, and therefore
ships as a profile nobody runs by accident. Together, **about ×7.9**, though that last figure is
the product of two separately-measured ratios rather than one end-to-end reading.

### The arithmetic, and where it goes wrong

Evolutionary output is generations × population. The cost of a tick rises with population, so
ticks per second falls as population rises and **population cancels** — leaving how cheap a tick
is, and how many ticks a generation takes. ⚠️ **The cancellation only holds for the part of a tick
that is variable**, which is why the first question worth asking is what fraction is not.

### Where a tick went, before any of this

Shipped configuration, 60,000 ticks past the founding, 2,063 organisms holding 4,385 cells:
**1,321.7 µs a tick, 757 ticks a second.**

| pass | µs | share |
| --- | --- | --- |
| behaviour | 579.1 | 43.8% |
| physics | 433.0 | 32.8% |
| resource grid | 155.1 | 11.7% |
| metabolism | 60.6 | 4.6% |
| gather | 46.7 | 3.5% |
| scatter | 25.6 | 1.9% |
| reproduction | 11.5 | 0.9% |
| ageing | 10.0 | 0.8% |
| ledger check | 0.05 | 0.0% |

Inside the two big ones, the two biggest single things were **the shading query in the behaviour
pass at 22.1%** and **the collision search at 22.4%**.

### ⚠️⚠️ A third of a tick was paid whatever was alive

**36% of a tick was fixed cost**, and it was attributable rather than merely bounded:

- **The resource grid, 155 µs.** 36,864 tiles regrown, diffused and capped every tick whatever
  lives on them. Genuinely fixed, and still is.
- **The three spatial-hash rebuilds, 105.35 µs each — measured on an empty crowd.** The shipped
  world lays **50,869 buckets** over 4,000-odd living cells, and the counting sort cleared a
  count for every bucket, prefix-summed across every bucket and copied a cursor for every bucket.
  **Four fifths of the cost of hashing the world's entire population was the world being swept
  rather than the crowd being sorted**, three times a tick.

| crowd | rebuild, before | rebuild, after |
| --- | --- | --- |
| 0 cells | **105.35 µs** | **0.00 µs** |
| 1,000 | 109.47 | 10.27 |
| 4,000 | 132.92 | 44.93 |
| 16,000 | 242.71 | 200.36 |

⭐ **The fix is to remember which buckets were used and walk that list instead**, clearing them on
the rebuild *after* the one that filled them. It is a change of cost and not of result: the runs
no longer lie in bucket order, which nothing observes, while the order of cells *within* a run is
unchanged — so every force is summed in exactly the order it was summed in before. Fixed cost is
now the grid alone, **18% of a tick**.

### What was done, and what each piece bought

All three are **bit-identical**. All six golden vectors hold unchanged.

| change | ticks/s |
| --- | --- |
| before | **757** |
| spatial hash rebuilt in O(cells) rather than O(buckets) | 902 |
| `rayon` on the behaviour pass's read-only half | 1,020 |
| bucket runs packed into one `u32` pair array | **1,071** |

Both ends of that measured at the same point in the same run — seed 42, eight founders, 60,000
ticks past the founding, 4,385 cells before and 4,374 after. `kleiber` moves with it, 650 → 896.

**×1.42 on the tick, for no change to a single number the simulation produces.** `rayon` is the
one CLAUDE.md's stack named and nothing had used; SPEC section 2 built the per-organism RNG
streams "which is what allows `rayon` parallelism without breaking reproducibility", and this is
the first thing to spend that guarantee. It is spent narrowly and deliberately: `Behaviour::look`
writes only `want[index]` and `signal[index]` and accumulates nothing, so it is the one pass whose
answer cannot depend on the order the work was done in. **The passes that accumulate — the
collision forces, the harvest, anything adding into a shared total — are not parallelisable
without changing the order of the additions, which changes the roundings, which changes the
world.**

### ⚠️⚠️⚠️ Raising the population does not buy throughput. It **loses** it.

The hypothesis was that a world running mostly empty — 4,000 cells over 50,869 buckets and 36,864
tiles — would carry a much larger population almost free, so mutation supply could be bought for
nothing. **It is measured and it is false, and it is false in the interesting direction.** One
world let to fill, the cost of a tick read off as it goes:

| shipped light | | | eight times the light | | |
| --- | --- | --- | --- | --- | --- |
| **cells** | **ticks/s** | µs/tick | **cells** | **ticks/s** | µs/tick |
| 18 | 5,651 | 177 | 20 | 1,988 | 503 |
| 692 | 2,736 | 366 | 2,169 | 931 | 1,074 |
| 1,385 | 1,893 | 528 | 8,667 | 354 | 2,829 |
| 2,219 | 1,401 | 714 | 17,476 | 190 | 5,252 |
| 2,950 | 1,201 | 833 | 25,093 | 135 | 7,396 |
| 4,026 | 1,034 | 967 | 33,228 | 93 | 10,811 |

**The fixed cost is confirmed at 173 µs** — fit the shipped column and the intercept lands within
four microseconds of the resource grid's measured 167, which is the whole of it. That is 17.9% of
a tick at the shipped population, and the two independent routes to the number agree.

⭐⭐ **But the cost per cell is not constant, and that is what kills the lever.** The shipped
column's slope is **0.197 µs a cell**; the eight-times column's is **0.314**. A denser world costs
*more per cell* than a sparse one, because a bucket that used to hold one cell now holds several
and the collision search has to test every pair in it. Throughput is population × ticks per
second:

| | organisms | ticks/s | product |
| --- | --- | --- | --- |
| shipped | 1,997 | 1,034 | **2.06 M** |
| eight times the light | 10,646 | 93 | **0.99 M** |

**Five times the population is half the evolutionary throughput**, and eight times the arena's
memory for it. The world was indeed running mostly empty, and the right answer was to stop paying
for the emptiness rather than to fill it: deleting the fixed cost gave ×1.42 outright and cost
nothing at all.

⚠️ Worth noticing in the table: the eight-times column starts at **503 µs with twenty cells in
it**, against the shipped column's 177. Nothing is alive in either. The difference is the arenas —
sized from `limits.max_organisms`, so eight times the cap is eight times the memory to walk past
even when it is empty. **A raised cap is not free even before anything is born into it.**

Resident memory, measured on the running process: **34 MB** at the shipped cap of 4,000 organisms
and **185 MB** at 32,000. Both are a long way inside CLAUDE.md's 2 GB target, so memory was never
what stopped this — the cache was.

### ⚠️⚠️ `dt` is not a lever on generations per hour, and this is now measured

`physics.rs`'s `DT` is read by the integrator, by the speed detritus sinks at, and by the clock a
myocyte's oscillation is phased against. It is read by **nothing** in `metabolism.rs`,
`reproduction.rs`, `ledger.rs`, `grid.rs` or `organism.rs`. Upkeep is charged per tick, income
arrives per tick, a body ages one tick per tick and its lifespan is a tick count.

| `DT` | mean age of the living | alive | cells | biomass |
| --- | --- | --- | --- | --- |
| 1 / 60 — ships | **881** | 1,913 | 4,588 | 31,407 |
| 4 / 60 | **893** | 1,368 | 4,915 | 33,414 |
| 8 / 60 | **956** | 379 | 4,893 | 31,574 |

**An eightfold `dt` moved the length of a generation by 8%, and the wrong way.** What it moved was
the ecology — the same tissue in a fifth of the bodies. §2's orthogonality claim is therefore
*confirmed*, and confirming it is exactly what refutes `dt` as a throughput lever: a setting the
economy cannot feel cannot make the economy run faster. ⚠️ The integrator survived 8× without
diverging, but `MAX_STIFFNESS` is `0.04 / dt²` and falls from 144 to 2.25 across that range, so
the founder's own spring of 10.0 is being clamped from 4/60 onward — the ecological drift in the
table is at least partly that rather than the physics.

### ⭐⭐⭐ What does divide ticks-per-generation: run the economy faster

Multiply every per-tick **rate** by `k` and leave every **stock** alone. `config/tempo.toml` is
that at `k = 8`, and its header carries the argument; it is three existing keys and **no code at
all**. Each arm below run for `200,000 ÷ k` ticks, so every one has lived the same amount of its
own history:

| `k` | alive | cells | biomass | births/1k ticks | mean age | cells/body | ticks/s | **gens/hour** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 — ships | 641 | 5,236 | 33,182 | 356.0 | **928** | 8.17 | 799 | **2,220** |
| 2 | 1,380 | 4,887 | 32,009 | 1,568.0 | **442** | 3.54 | 717 | **4,183** |
| 4 | 1,503 | 4,687 | 31,592 | 3,423.0 | **224** | 3.12 | 724 | **8,340** |
| 8 — `tempo` | 1,316 | 5,089 | 32,472 | 6,021.0 | **109** | 3.87 | 526 | **12,407** |

**Mean age divides by `k`** — 928, 442, 224, 109 against a perfect 928, 464, 232, 116 — and the
per-capita birth rate multiplies by `k` to match. ⭐ **Biomass and total living cells do not move**
across an eightfold change: the world holds the same amount of life and turns it over eight times
faster. This is not a cost-side lever self-cancelling in the §3 sense, because both sides of every
body's books are multiplied and the balance between them is exactly what it was.

⚠️ **Generations per hour goes up ×5.6 rather than ×8, and the shortfall is the price of the
births.** A tick at `k = 8` costs 526 ticks a second against 799 — not because there is more
tissue, since there is not, but because there are **seventeen times as many births in it**, and a
birth is a genome copied, mutated and developed into a body. That is the one part of a tick that
scales with the tempo rather than with the population, and it is where the next optimisation of
this profile would go.

⚠️⚠️ **The price is that the mean body halves**, 8.17 cells to 3.87, with the population roughly
doubling — the same tissue differently divided. The step is between `k = 1` and `k = 2` and then
flat, which reads like a discretisation threshold rather than a slope. **Nobody has found out
which, and that is the first thing worth doing with this profile.** Until somebody has, a
body-size result must not be read off it.

### What is left, in the order it is worth taking

1. **The collision search, 25% of a tick.** Bound by waiting for memory rather than by arithmetic:
   4,000 cells spread over 50,869 buckets means nothing it wants is ever in cache. A bit-identical
   parallel form exists — walk the neighbours read-only in parallel, recording the overlapping
   pairs, then apply them sequentially in the original index order, which reproduces the
   summation order exactly. It needs a bounded per-cell pair buffer, which is the part to design
   carefully under CLAUDE.md's allocate-once rule.
2. **The resource grid, 18% and all of the remaining fixed cost.** Parallelisable per tile; the
   care needed is that `regrow` and `spill` hand the ledger an `f64` sum, and a parallel reduction
   would change the order those add in.
3. **Coarser buckets.** Occupancy is 0.08 cells per bucket, which is a world running mostly empty.
   Wider buckets would trade cache misses for candidate tests. ⚠️ It changes the order neighbours
   come back in, so it **moves every golden vector**, and it is the only item here that does.

---

## 8. ⭐⭐⭐ The propulsion organelle — predictions, written down before the measurement

**Recorded 17 August 2026, before a line of it was built.** The rule this project has been
running on all week is that a predicted sign goes on paper first, because four of the last
eleven rounds refuted a prediction of mine and that is only worth something if the prediction
was not written afterwards.

The design, so the predictions can be read against something concrete:

- **A seventh `CellKind`, the flagellocyte.** Its function is thrust, **computed rather than
  simulated** — no stroke, no beat to resolve. This is what resistive-force theory does for
  real microswimmers and it is the only tractable choice here: a real flagellum beats at
  ~100 Hz and `dt = 1/60 s` cannot resolve anything above about 3 Hz. Simulating the stroke
  is the Nyquist trap; computing the thrust is not.
- **Direction: outward along the cell's own geometry** — the unit vector from the mean
  position of the cell's adhered partners to the cell itself. A direction development already
  chose, that rotates as the body flexes, and that **sums to nothing on a symmetric body**.
  That last property is the whole point: a rosette with flagellocytes all round goes nowhere,
  and only a lineage that puts them on one side moves. Placement stays the evolved part.
  A cell with no adhesions has no direction and produces no thrust, so a single cell cannot
  swim and the founder gains nothing by accident.
- **Magnitude from `osc_freq`**, the gene field a myocyte already uses as a beat frequency.
  Not a hack: in resistive-force theory thrust is linear in beat frequency at a fixed
  waveform, so this is the actual relation. It also costs the genome nothing — no new field,
  no change to the mutation table's shape — and it puts a flagellocyte exactly one
  `child_kind` mutation from a myocyte **with its frequency parameter already tuned**, which
  is an exaptation and is how every motor in biology actually arrived.
- **Modulated by `sensor_gain`**, again the myocyte's own field, applied to *magnitude only*.
  Magnitude modulation on a body with flagellocytes on two sides is a turn, so taxis can
  evolve — but the gain's size, its sign and whether it is zero at all stay evolved. Nothing
  points thrust at anything; see CLAUDE.md's amended decision-log row for the line between a
  motor and a tactic.
- **Charged through `movement_cost`, the myocyte's own coefficient**, as force × distance
  moved. In a drag-dominated world that is `∝ F²`, which is the right power law for Stokes
  drag, and sharing the coefficient makes "is a flagellum better than a muscle" an
  apples-to-apples question rather than a comparison of two invented prices.

### The predictions

1. **Travel per lifetime rises from ~0.65 body lengths to more than 5**, at a thrust costing
   under a tenth of income. **High confidence, and it deserves none** — a steady external
   force against linear drag gives a terminal velocity and displacement linear in time, and
   nothing cancels it. That is arithmetic, not biology, and it is the one prediction here that
   would be embarrassing to get wrong.
2. **The flagellocyte prices *negative* in the competition assay, around −1 to −3 %/gen.**
   The assay measures the filling regime — two-celled bodies growing into empty water — and in
   empty water there is nowhere worth going, so a motor is pure cost. ⚠️ **A negative here is
   not a failure of the organelle**, and this is written down now precisely so it cannot be
   read as one later.
3. **It prices materially less negative, and plausibly positive, when seeded into a mature
   drawn-down world** — where the field is 65% eaten and un-grazed water is a real prize.
   **This is the interesting measurement and the one I am least sure of.** If 2 and 3 come out
   the same, the motor is simply too expensive and the price is wrong. If they differ, then
   locomotion pays exactly where theory says it should, which would be the first time in this
   project that a specialisation's value depended on the state of the world rather than on its
   own coefficient.
4. **Predation still does not establish**, unless thrust closes 60–88 units within a lifetime.
   The reach gap is four to five orders of magnitude against real bacteria and one organelle
   does not obviously close it. **Measure the travel first and predict predation from it**,
   rather than hoping.

### ⚠️ The cost of building it, stated in advance

A seventh kind changes `CellKind::ALL.len()` from 6 to 7, and `mutation.rs` draws `child_kind`
and `new_kind` uniformly over that list. **Every golden vector moves**, whatever value thrust
ships at, because the RNG stream itself differs. The old values are to be kept verbatim in the
tests as history rather than deleted, and the re-record is to be a commit of its own so that it
is legible in the log as a re-baseline and not as a result.

### ⭐⭐⭐ Result 1: the organelle moves a body, and the trade-off is real

`assay.rs`'s `what_a_motor_buys_in_travel`, seed 42, one body alone in a lit world it cannot
share, `LIFETIME` = 2,000 ticks. The body is `[photocyte, photocyte, gonocyte, flagellocyte]` —
a chain with the motor at one end, since a motor in the middle of a symmetric body pushes
against its own other half. The control is the identical genome with `osc_freq` at nought,
which for a motor is a thrust of nought: same cells, same buoyancy, same springs, same upkeep,
same shape, differing only in whether the motor is running.

| `thrust` | travel | control | ratio | lived | **body lengths** |
| --- | --- | --- | --- | --- | --- |
| 0 | 2.10 | 2.10 | 1.0 | 1,708 | 0.07 |
| 5 | 6.48 | 2.10 | 3.1 | 1,708 | 0.22 |
| 15 | 29.61 | 2.10 | 14.1 | 1,708 | 0.99 |
| 40 | **88.84** | 2.10 | 42.2 | 1,708 | 2.96 |
| 100 | **232.61** | 2.10 | 110.6 | 1,708 | **7.75** |
| 250 | 46.67 | 2.10 | 22.2 | **273** | 1.56 |
| 600 | 0.00 | 2.10 | 0.0 | **3** | 0.00 |

**Prediction 1 holds.** Travel per lifetime goes from two-thirds of a body length to 7.75, and
it was right to say the confidence in it deserved nothing — a steady external force against
linear drag is arithmetic.

⭐⭐ **What was not predicted, and is the better half of the result: the curve turns over.** At
250 the body dies at tick 273 instead of 1,708, and at 600 it dies in three ticks. **Speed is
bought with life**, because the running cost goes as the square of the force while the travel
goes only as the force. That is not a number anybody chose — it is Stokes drag, and it arrived
with the physics. It means `osc_freq` is under real selection rather than drifting free: there
is a fastest a lineage can usefully go, it is a soft optimum rather than a cliff, and it sits
inside the range mutation can reach.

⚠️ **At thrust 40 a body covers 88 units in a lifetime**, which is the top of the 60–88 range to
the nearest unrelated body measured in round 7. **The reach gap closes here.** That is a
statement about geometry and not yet about selection.

### ⚠️ Where the next question actually lies

Reaching a stranger is not the same as it being worth reaching, and there is a valley hiding in
the obvious reading. A devorocyte that can now travel is **two** mutations, not one — the mouth
and the motor — and this project has ten rounds of evidence that two-mutation payoffs do not
happen here.

So the motor has to pay for something a single mutation can collect, and the candidate already
in the world is **escaping your own family's grazing shadow**: the field runs 65% drawn down at
equilibrium, `light.patchiness` is 0.5, and round 7 measured that 99.9% of a body's contacts are
its own descendants — which means a body is surrounded by relatives eating the same tiles. A
motor that carries a body out of that costs one mutation and pays immediately, and **predation
becomes reachable afterwards, from a lineage that already has motors, which is the exaptation
ladder rather than a valley.**

⚠️ `light.patch_drift` is 0.0006 units a tick — about **one world unit in a whole lifetime**, so
the patches are effectively fixed. A world whose patches drifted faster than a body could sit
still would be a second, stronger reason to move. **That is a lever to measure and not to
assume**, and it is the first candidate if the motor prices badly.

### ⭐⭐⭐ Result 2: the mechanism — moving *does* find more food, inside a window

`assay.rs`'s `does_moving_find_more_food_than_staying_put`. One body, alone in a lit world it
cannot share, so the only depletion in the water is the hole the body is eating itself. Gross
income over 1,500 ticks: what the travel **found**, with everything the motor spent added back
in closed form, against the identical genome with the motor switched off.

| `thrust` | travel | gross gain | share of what a still body earns |
| --- | --- | --- | --- |
| 0 | 1.8 | +0.000 | — (the control) |
| **40** | **76.2** | **+4.696** | **+3.38 %** |
| 100 | 200.8 | −11.091 | −7.99 % |

⚠️⚠️ **Going faster finds *less* food, and that is gross income rather than cost.** The reason is
`light.gradient = 0.75`: this world is strongly top-weighted, a motor pushes along the body's own
geometry, and **nothing steers it**. A slow body samples fresh water near the light; a fast one
performs a long unsteered walk in a world where most directions are darker than where it began.

**That is an optimum nobody chose.** It falls out of two settings that were never picked
together, and it is what makes the whole picture consistent — the same shape appears in travel
(where cost kills the body above 250), in gross income (peaks at 40) and in invasion fitness.

### ⚠️⚠️⚠️ Result 3: prediction 3 is NOT confirmed, and the reading that seemed to confirm it was one seed

`assay.rs`'s `what_a_motor_is_worth_where_there_is_somewhere_to_go`. A third flagellocyte
released as a **rare invader into a resident population that has already settled and drawn the
field down** — the regime the competition assay cannot produce at any setting.

**What seed 42 said, and what was very nearly written up as a discovery:**

| `thrust` | motor, %/gen | a third photocyte (calibration) |
| --- | --- | --- |
| 0 | −19.90 | +2.59 |
| **40** | **+1.99** | +10.24 |
| 100 | −30.76 | +5.59 |

That reads as a motor being worth 22 %/generation more when it can push, and at thrust 40 as the
first specialisation other than a photocyte ever measured **positive** in this project.

**What three seeds say:**

| `thrust` | mean %/gen | spread | the three seeds |
| --- | --- | --- | --- |
| 0 | −10.34 | 16.9 | −19.9, −8.1, −3.0 |
| **40** | **−7.20** | 19.6 | **+2.0**, −17.7, −5.9 |
| 100 | −15.16 | 23.7 | −30.8, −7.0, −7.7 |

**The difference between a motor that can push and one that cannot is 3.1 %/generation against a
between-seed spread of 17 to 24.** The +2.0 is the first entry on the middle row — one seed out of
nine. There is no effect here.

⭐ **Why the instrument cannot answer this, which is the part worth keeping.** The ±1.12 noise
floor was measured on arms near neutrality, and this arm is nowhere near it. A founder plus one
flagellocyte is a **three-celled body with a single photocyte** paying for a cell that costs more
than that photocyte earns, so the invaders crash almost immediately and the slope of a log
frequency that has gone to nothing is badly estimated. The variance is a property of measuring a
strongly negative arm.

⚠️ **So the experiment is refuted, not the hypothesis** — which is a different and more useful
position. Result 2 still stands: a motor finds 3.38% more food, deterministically, on a
four-celled body with two photocytes. **The untested claim is that a motor pays for a body big
enough to afford one**, and neither assay can ask it, because the founder-plus-one design always
hangs the marginal cell on the smallest body in the world. That is the next experiment, and it
needs an instrument that does not yet exist.

### ⚠️ Two errors of mine in this round, both caught by re-measurement

1. **`earns` computed the wrong quantity.** It returned `Δbiomass + Δdissipated` and called it
   gross harvest. `Ledger::overflow` credits `dissipated` with every unit drained from a tile too
   full to hold the light, across the whole field, every tick — so it read **22,820 for a
   four-cell body** whose upkeep is 0.02 a tick, and all three arms agreed to four significant
   figures because they were measuring the weather. Now `Δbiomass`, with the motor's spend added
   back analytically.
2. **The first competition-assay test asserted `best > inert + 0.22` and passed** — on a margin
   of 0.237, at the one thrust that kills a body outright. A fluke. It is recorded in the test's
   own documentation rather than quietly rewritten, because an assertion that passes on noise
   converts *"we did not measure this"* into *"we measured it and it was fine"*.

### ⭐⭐⭐ Result 4: the arithmetic behind every null, and the one lever that moves it

Every null this round produced is one ratio, showing up in four instruments.

> **A flagellocyte costs 0.006 a tick to own — 9.0 over a 1,500-tick window — and the travel it
> buys finds 4.7. The organelle earns about half its keep.**

There are exactly two ways out and only one is honest.

**Make the cell cheaper.** It would have to cost under 0.0031 a tick, which is *below the
photocyte's 0.004*. `CellKind::upkeep`'s own note spends most of its length arguing why that is
the line: it was measured for the myocyte, and at 0.002 a tick myocytes rise steadily through a
run and reach 2.4% of bodies **while mean displacement falls to the lowest reading in the
sweep**. That is a motor spreading because it is cheap, not because it moves anything, and in a
census it would be indistinguishable from the result this round is looking for.

**⭐⭐⭐ Make moving find more — and this is measured.** `assay.rs`'s
`what_the_field_has_to_be_like_for_moving_to_pay`, sweeping `light.diffusion` at thrust 40:

| `light.diffusion` | gross gain | **as a share of the 9.0 it costs** | travel |
| --- | --- | --- | --- |
| 0.000 | +39.14 | **435 %** | 76.2 |
| 0.005 | +17.71 | **197 %** | 76.2 |
| 0.010 | +10.27 | **114 %** | 76.2 |
| 0.020 | +5.92 | 66 % | 76.2 |
| **0.040** ships | **+4.70** | **52 %** | 76.2 |
| 0.080 | +5.47 | 61 % | 76.2 |

**The crossover is at a diffusion of about 0.011.** Below it a motor earns more than it costs;
at the shipped 0.04 it earns half. Travel is identical in every row, so this is the *field*
changing and not the organelle.

⭐ **And the mechanism is the one from real biology.** `light.diffusion` is how fast a hole a
body eats is refilled by its neighbours. At 0.04 the water fills the hole faster than a body can
deepen it, so there is nothing to outrun and staying put is free. Slow it and a body sits in a
depletion zone of its own making — which is exactly why motility pays for real plankton, and why
diffusion limitation is the standard explanation for it.

⚠️ **Lowering diffusion changes the whole world's economy, not just the motor's income**, because
the field also becomes worse at spreading light from where it fell. So it has to be measured
against a plain founder *in the same altered world*.

### What is left to do

1. **`does_a_motor_pay_for_itself_once_the_water_stops_mixing`** — the competition assay at
   diffusion 0.04, 0.02, 0.01 and 0.005, with a third photocyte in every run as the control that
   catches "everything got better". Its ±0.11 %/generation floor is forty times tighter than the
   invasion assay's variance on a crashing arm. **This is the decisive reading of the round.**
2. **`is_a_motor_worth_having_on_a_body_that_can_afford_one`** — two seven-celled arms differing
   only in whether the seventh cell is a sclerocyte or a flagellocyte, because every coefficient
   this project has ever taken on a specialisation put that cell on the *smallest body in the
   world*.
3. If 1 reads positive, the shipped `light.diffusion` is the change, and it wants its own
   profile and its own re-recorded golden vectors.

### ⚠️ Result 5: the diffusion lever fails, and the control is what shows why

`does_a_motor_pay_for_itself_once_the_water_stops_mixing`, competition assay, seed 42, thrust 40:

| `light.diffusion` | a third flagellocyte | **a third photocyte (control)** |
| --- | --- | --- |
| **0.040** ships | −2.460 | **+1.678** |
| 0.010 | −2.803 | **+0.102** |
| 0.005 | −2.559 | **+0.356** |

The motor does not improve. **And neither does the photocyte** — the best cell in the world loses
nine tenths of its advantage over the same range. That is the confounder the control arm was put
there to catch, and it caught it: slowing the water does not only sharpen the hole a body eats, it
makes the field worse at moving light away from where it fell, so tiles under nobody fill to
`light.cap` and spill into `dissipated` while tiles under bodies are grazed flat. **Less of the
world's light gets eaten at all.**

So Result 4's +39 at zero diffusion is real and is a fact about **one body alone** — a single body
in an empty ocean gains everything from sharper structure and pays nothing for the light nobody
collects. A population pays for it.

⭐ **The general form, which six earlier rounds share:** a lever that improves what a specialist
earns while improving what everyone earns by more is not a lever. A competition assay measures a
*difference*, and a difference is what the world's poverty cancels out of.

### ⭐⭐⭐ Result 6: what all five nulls had in common — nothing was ever steering

Every body measured in this round was built with **`sensor_gain = 0`**. The modulation
`behaviour.rs` applies to a motor's magnitude — the same controller a myocyte is driven by, sign
and all — had never once been switched on in a measurement. The whole round priced an organelle
with its one steering input nailed shut.

`can_a_motor_that_can_steer_pay_for_itself`, on a five-celled body — two photocytes, a gonocyte, a
sensocyte adhered to a flagellocyte:

| `sensor_gain` | gross gain | **share of the 9.0 a motor costs** | travel |
| --- | --- | --- | --- |
| 0.0 | +7.93 | 88 % | 59.9 |
| +0.5 | +3.41 | 38 % | 76.4 |
| +1.0 | +3.41 | 38 % | 76.4 |
| **−0.5** | +14.24 | **158 %** | 29.6 |
| **−1.0** | **+15.34** | **170 %** | 6.3 |

⭐⭐⭐ **A steered motor earns 170% of its keep. The winning sign is negative, and that is
orthokinesis.** A negative gain drives the motor *softer* where the sensor reads more light, so a
body races through the dark and crawls in the bright — and a population of such bodies piles up
where it is worth being. Travel falls to 6.3 units, which looks like failure and is the mechanism:
**the point is not to go far, it is to stop somewhere good.**

**It is what real bacteria do, and for the same reason.** *E. coli* has no rudder and no idea which
way is up a gradient. It modulates how long it swims before tumbling, and that alone climbs one.
This world reproduced that constraint by accident — thrust points along a body's own geometry and
nothing steers it — and then reproduced the solution.

⚠️ **This is one body alone and says nothing about selection.**
`is_a_steered_motor_worth_more_than_a_blind_one` puts the two arms one `sensor_gain` mutation apart
through the competition assay at three seeds, and is the reading that matters.

### ⭐⭐⭐ Result 7: steering pays, and it is the first thing other than a photocyte that ever has

`is_a_steered_motor_worth_more_than_a_blind_one`. Two five-celled bodies — two photocytes, a
gonocyte, a sensocyte, a flagellocyte — **one `sensor_gain` mutation apart**, through the
competition assay whose noise floor is ±0.11 %/generation.

| seed | %/generation |
| --- | --- |
| 42 | **+3.327** |
| 43 | **+3.336** |
| 44 | **+0.412** |
| **mean** | **+2.358** |

**All three positive**, the weakest at four times the noise floor. For scale, a third photocyte —
the only cell in this world ever measured positive — is worth **+1.06 %/generation** in the
shipped world. **A motor wired to a sensor with a negative gain is worth more than twice that.**

⚠️ **Read exactly what this says.** It is *steered motor* against *blind motor*, not *motorised
body* against *plain founder*. It does not say a motor is worth having. It says that **given** a
body has grown a motor and a sensor beside it, the sign of the one number joining them is worth
+2.36 %/generation — and that number is a single point mutation, with fleeing one sign flip from
gathering.

That is the rung above the one this project has been stuck on. The question is no longer *is any
specialisation worth anything* — one is, and by a wide margin. It is **whether evolution can
assemble three things that only pay together**, which is the ordinary hard problem of adaptation
rather than a world in which nothing can pay at all.

### ⚠️ The honest state of the round

| | |
| --- | --- |
| A motor moves a body | ✅ 88 units a lifetime, the whole gap to a stranger |
| Moving finds more food | ✅ +3.38% blind, +70% over its own keep when steered |
| A motor pays as a founder's third cell | ❌ −2.5 %/gen, and the invasion assay cannot resolve it |
| Lowering `light.diffusion` rescues it | ❌ the control collapses with it |
| Making the cell cheaper rescues it | ❌ that is measured to be where neutral bloat begins |
| **Wiring a sensor to it pays** | ✅ **+2.358 %/gen, three seeds, all positive** |

### What to do next

1. **Price the assembly, one rung at a time.** What is a sensocyte worth to a body that already
   has a motor? What is a motor worth to a body that already has a sensor? Both are single
   mutations from a body one step away, both are competition-assay questions, and together they
   say whether the ladder has a rung missing or is merely long.
2. **Watch a long run for the combination.** `config/flagellum.toml` at 840 Ma carries motors,
   mouths and sensors all at single figures — present, produced, not spreading. The chronicle now
   streams as it happens, so the moment a lineage grows a sensor beside a motor is a line in the
   log rather than something lost to the ring.
3. ⚠️ **Do not ship `physics.thrust` live on the strength of Result 7.** What is measured is the
   value of a wiring, not the value of an organelle, and this round has already had one result
   destroyed by replication.

### ⚠️ Result 8: a motor is still a loss on a body that already senses

`is_a_motor_worth_having_on_a_body_that_already_senses`. Two five-celled bodies, **one
`child_kind` mutation apart** — two photocytes, a gonocyte, a sensocyte, and a fifth cell that is
either a sclerocyte (the cheapest, most inert thing in the world) or a flagellocyte at
`sensor_gain = −1.0`. Both arms carry the same gain, built by taking the motorised genome and
swapping that one field, so `one_mutation_apart` accepts them.

| seed | %/generation |
| --- | --- |
| 42 | −0.344 |
| 43 | −1.307 |
| 44 | −3.198 |
| **mean** | **−1.616** |

**All three negative.** Steering halves the penalty — the founder's third flagellocyte is −2.5
and this is −1.6 — but it does not close it.

⚠️ **Seed 42 alone reads −0.344, which is nearly break-even**, and it was described that way once
before the other two arrived. It is one seed. This round has now been burned by a single seed
twice.

### ⭐ The three coefficients together, which is the useful form

On a five-celled sensing body, against a sclerocyte in the same slot:

| the fifth cell | %/generation |
| --- | --- |
| a sclerocyte | 0 — the reference |
| **a steered motor** | **−1.62** |
| a blind motor | ≈ **−3.97**, being −1.62 less the +2.36 that steering is worth |

**Steering is worth +2.36 and the motor is worth −3.97, and the sum is still negative.** What is
now known precisely is the size of the hole: **1.6 %/generation**, which is about 1.5 energy units
over a lifetime on a body that earns 139.

### ⚠️ What this does and does not close off

It does **not** say locomotion cannot pay here. Every arm measured in this round has been a body
of four or five cells with two photocytes, because that is what the competition assay's
one-mutation constraint allows. `metabolism.scaling_exponent` at 0.75 produces bodies of
**forty-nine** cells in `config/emergence.toml`, and **a motor's cost is flat in body size while
the food its travel finds is not.**

Nothing has measured a motor on a body of that size. The one attempt —
`is_a_motor_worth_having_on_a_body_that_can_afford_one`, at seven cells — came back with a
between-seed spread of 28 %/generation, and its own instrument check refused to let the numbers be
quoted.

**That is the open question**, and it needs an instrument that does not exist yet: a competition
assay whose resident *and* both arms are large bodies, so the marginal cell is hung on something
that can afford it.

### ⚠️ Result 9: predation is three parts in ten thousand, and body size does not change it

`run.rs`'s `how_much_of_what_a_body_eats_is_another_body`. `Ledger::predate` moves energy from one
organism to another and `Ledger::harvest` moves it out of the field; both keep running totals, and
their ratio is the honest form of *what share of a world's income is second-hand?* Eight founders,
seed 42, 60,000 ticks each:

| profile | predated | harvested | **second-hand share** | living cells |
| --- | --- | --- | --- | --- |
| **the shipped world** | 469.3 | 1,438,307 | **0.000326** | 4,166 |
| `kleiber` 0.75 | 383.8 | 1,472,429 | 0.000261 | 5,649 |
| emergence: Kleiber + motors | 419.5 | 1,472,378 | 0.000285 | 5,420 |

A long `emergence` run's chronicle says *"A devorocyte has taken energy out of another body. Living
tissue is being eaten in this world as well as the water and the dead."* **That sentence is true
and this table is the whole of it** — predation is real, and it is a rounding error in the world's
accounts.

⭐⭐ **And the shipped world is the highest of the three, which refutes the hypothesis the test was
written to check.** The argument was geometric and looked sound: Kleiber makes bodies large, a body
of fifty cells presents far more surface than a body of one, so mouths should meet foreign tissue
more often for reasons owing nothing to hunting. Kleiber has **36% more living cells and less
predation**.

Whatever extra surface a large body has is surface against **its own other cells**. That is round
7's finding arriving from a new direction — *contact in this world is inherited, not encountered* —
and it says that making bodies bigger buys them more of their own family rather than more
strangers.

⚠️ **Do not retry "grow the bodies and predation will follow."** It is now measured, in the profile
built for it.
