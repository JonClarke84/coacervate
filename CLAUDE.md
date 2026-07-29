# Coacervate

**An open-ended evolution simulator. Not a game.**

A window of primordial soup, lit from above. You set the initial conditions, seed single
cells, and walk away. Cells feed, reproduce with mutation, and selection does the rest. You
come back hours later and read what happened.

The question the whole project exists to answer is: *does complexity appear, and what form
does it take?* There is no score, no objective, and nothing to win.

> **Coacervate** (n.) — a droplet that forms spontaneously when organic molecules
> concentrate out of dilute solution. One of the leading candidates for the first cell-like
> structures on Earth: a self-assembling boundary that turns chemistry into something with
> an inside and an outside.

---

## Read this first

This repository was specified on a Mac and is built on a Windows PC. **A session on the PC
starts with none of the original design conversation**, so this file and [SPEC.md](SPEC.md)
are the entire handoff. Everything that was decided, and why, is written down here. If
something seems arbitrary, check the Decision Log at the bottom before changing it — most
of the apparently odd choices are load-bearing.

| Document | Contents |
| --- | --- |
| `CLAUDE.md` (this file) | Working agreement, architecture, safety limits, Windows setup, decision log |
| [`SPEC.md`](SPEC.md) | The simulation model in implementable detail: genome, development, physics, energy, rendering, file formats |
| [`README.md`](README.md) | Public-facing description. This is a portfolio piece — keep it good. |

---

## Working agreement

These are Jonathan's standing preferences. They normally live in a user-level config on his
Mac, which does **not** travel to the PC — hence their being written down here.

### Development process

- **Test-driven development is mandatory.** Red then green: write the failing test first,
  then the minimum code to pass it. This is not negotiable and it is not optional for
  "simple" changes.
- **Never commit or stage anything unless explicitly asked.** Write the files; leave the
  committing to Jonathan unless he says otherwise.
- **Never push to a remote without explicit confirmation**, and never chain `git push` with
  another command. Committing locally when asked is fine; pushing is always a separate,
  explicitly approved step.
- **Challenge the approach if you have a better idea.** Say so plainly. If Jonathan
  disagrees, follow his direction.
- **Run the full check suite after completing a piece of work**, not after every individual
  edit. For this project that's `cargo fmt --check`, `cargo clippy`, `cargo test`.
- **Don't over-build.** No speculative abstractions, no feature flags, no
  backwards-compatibility shims, no error handling for conditions that cannot occur. Do the
  simplest thing that works. This project has enough genuine difficulty in it without
  invented complexity.

### Code rules

- **`#![forbid(unsafe_code)]` at the root of every crate.** No exceptions. Jonathan cannot
  review Rust, so the compiler has to be the reviewer. This one line is what makes Rust's
  memory-safety and data-race guarantees actually hold rather than merely apply "mostly".
- **`overflow-checks = true` in the release profile.** Rust silently wraps integer overflow
  in release builds by default. In a simulation that runs for billions of ticks, a silent
  wrap is a corrupted experiment you'll never notice. Better to panic loudly.
- **No lossy `as` casts on values that matter.** Use `TryFrom` and handle the failure, or
  restructure so the cast isn't needed. (This mirrors Jonathan's TypeScript rule of never
  using `as` — casts hide real mismatches and rot silently.)
- **Invariants are asserted at runtime, not just in tests.** Energy conservation in
  particular: if the ledger doesn't balance, panic immediately rather than producing eight
  hours of quiet nonsense.
- **The simulation crate must not know that rendering exists.** No `wgpu`, no `winit`, no
  `egui` in `coacervate-sim`. This keeps the core testable in isolation and keeps a future
  browser front-end possible without a rewrite.

### This is a personal project

It lives outside work. Nothing here ever touches Jira, Slack, or any work system.

---

## Architecture

```
coacervate/
├── crates/
│   ├── coacervate-sim/     Pure simulation. No I/O, no rendering, no dependencies
│   │                       beyond std + rand + serde. This is where TDD lives.
│   ├── coacervate-render/  wgpu rendering + egui panels. Reads sim state, draws it.
│   └── coacervate-app/     Binary. winit window, main loop, config, replay log I/O.
└── docs/
```

**Language:** Rust throughout. No JavaScript, no TypeScript, no web layer, no WebAssembly.

**Stack:** `winit` (window), `wgpu` (GPU compute *and* rendering), `egui` (control panels
and charts), `rayon` (CPU parallelism), `serde` + `postcard` + `zstd` (replay log).

**Target:** A single Windows `.exe`. Windows 11, x86-64, MSVC toolchain.

### Why native and not a web app

This was considered seriously and rejected for a structural reason, not a preference.

The simulation state lives in GPU memory because that is where the physics runs. A native
renderer draws directly from those same buffers — the data never moves, so drawing a
hundred thousand organisms costs almost nothing. A browser front-end would have to read
state back from the GPU, serialise it, push it over a WebSocket, deserialise it in
JavaScript and draw it to a canvas. At the scale this project is built for, that pipeline
becomes the bottleneck and the *display* ends up limiting the experiment. You would be
paying for a GPU-scale world and then watching a decimated sample of it through a straw.

Native also removes the entire JavaScript toolchain — no Node, no npm, no bundler, no
WebSocket protocol to design — and gives direct shader access, which is where the visual
quality actually comes from.

The cost is that sharing it is harder (a Windows executable rather than a link) and the UI
chrome is plainer than HTML and CSS would allow. Both were judged acceptable. The
`coacervate-sim` crate is kept rendering-free specifically so that a browser front-end
remains possible later if that changes.

### Target hardware

Development and long runs happen on the PC:

| | |
| --- | --- |
| CPU | Intel Core i5-13400F — 10 cores (6 performance + 4 efficiency), 16 threads |
| RAM | 16 GB DDR4 — **the tighter constraint of the two machines; size buffers for this** |
| GPU | NVIDIA RTX 4070 Ti — ~7,680 shader cores, 12 GB VRAM |
| Disk | 1 TB NVMe |
| OS | Windows 11 |

There is a Mac (M4, 24 GB) but it is **not** a build target. The macOS binary was
deliberately dropped from scope: it would be maintained and never run.

---

## Hard limits and safety

The PC is a dedicated machine, so the sim is allowed to use it fully. But an unbounded
simulation is still capable of thrashing swap or filling a disk overnight, and the defences
below are designed so that **they hold even if the simulation code is wrong.**

### Allocate once, never grow

Every arena — organisms, cells, springs, resource grid — is allocated at startup at fixed
capacity derived from the config, and never resized. When the population cap is reached,
births *fail* rather than allocating. (This is also biologically reasonable: a full world
should mean nowhere to reproduce into.) **A simulation that cannot allocate cannot leak.**

### Caps

| Limit | Default | Why it exists |
| --- | --- | --- |
| Max organisms | 4,000 (CPU) / 100,000 (GPU) | Memory and per-tick cost |
| Max cells per organism | 64 | Bounds development and physics |
| Max genes per genome | 128 | **Critical** — gene duplication is exponential bloat without a cap |
| Max development steps | 16 | Bounds body-growing cost |
| Resident memory target | < 2 GB | Leaves headroom on a 16 GB machine |
| Replay log budget | 8 GB, rotating | Bounded overnight footprint |
| Wall-clock bound | 12 h default | Runs end deliberately rather than when noticed |

The genome cap deserves emphasis. Gene duplication is the mutation operator that makes
complexity possible — it is the entire reason the genome design is what it is — and it is
also an exponential bloat machine. A lineage that duplicates faster than selection punishes
it will grow a genome into the megabytes and take the process down with it. **Never remove
or raise this cap without also adding a metabolic cost per gene.**

### Operating-system backstop

Windows Job Objects enforce a memory ceiling the process cannot exceed regardless of what
the code does. The launcher applies one, so a leak kills the run rather than the machine.
Set process priority to below-normal if Jonathan is using the PC for anything else.

### Bounded runs

Every run terminates on whichever comes first: the wall-clock bound, the generation bound,
or extinction. Shutdown is graceful — finish the tick, flush a final snapshot, write the
chronicle, exit. `Ctrl-C` does the same thing. Killing the process at any moment is safe:
the replay log is append-only and always consistent on disk.

### It cannot damage the machine

Worth stating plainly, because the concern came up: a userspace process cannot brick
hardware. The realistic bad outcomes are a slow machine (swap thrash), a hot and loud one
(CPU saturation), or a full disk. All three are prevented above, and all three are
recoverable by killing the process.

---

## Windows setup

**Jonathan has not developed on Windows before.** This section is written for a first run
through, in order. Everything is PowerShell unless stated.

### 1. Windows Terminal and PowerShell 7

Install both from the Microsoft Store, or:

```powershell
winget install --id Microsoft.WindowsTerminal
```

```powershell
winget install --id Microsoft.PowerShell
```

PowerShell 7 (`pwsh`) is a genuine improvement over the built-in Windows PowerShell 5.1 and
behaves much more like a Unix shell.

If a script is ever blocked by execution policy:

```powershell
Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned
```

### 2. Git for Windows

```powershell
winget install --id Git.Git
```

This also gives Claude Code a proper Bash tool, which makes its shell behaviour much closer
to what it does on macOS. Then enable long paths, because Rust build directories get deep
and Windows' 260-character default limit will otherwise bite:

```powershell
git config --global core.longpaths true
```

### 3. Visual Studio Build Tools — **the step most likely to go wrong**

Rust on Windows uses Microsoft's linker. Without it, `rustup` installs fine and then every
build fails with a confusing linker error.

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools
```

Then **launch the Visual Studio Installer** and tick the **"Desktop development with C++"**
workload. The bare Build Tools install does not include it by default. This is several
gigabytes and takes a while. Do not skip it, and do not skip the workload.

### 4. Rust

Download and run `rustup-init.exe` from <https://rustup.rs>, or:

```powershell
winget install --id Rustlang.Rustup
```

Accept the default host triple, which should be `x86_64-pc-windows-msvc`. Close and reopen
the terminal, then confirm:

```powershell
rustc --version; cargo --version
```

### 5. Windows Defender exclusion — do not skip this

Real-time scanning inspects every file `cargo` writes, and Rust builds write a very large
number of small files. Excluding the build directory typically cuts compile times by more
than half. Run PowerShell **as Administrator**:

```powershell
Add-MpPreference -ExclusionPath "$env:USERPROFILE\Code\coacervate\target"
```

### 6. Claude Code

```powershell
irm https://claude.ai/install.ps1 | iex
```

Native Windows is officially supported and needs no Node.js runtime. Two things to know:
sandboxing is **not** available on native Windows (it requires WSL2), so commands run
directly against the system; and if Claude Code can't find Git Bash automatically, set
`CLAUDE_CODE_GIT_BASH_PATH` to point at it.

WSL2 was considered and rejected: reaching the GPU from WSL2 requires an extra translation
layer, and wrong compute shaders fail by producing silently incorrect numbers rather than
error messages. Fewer layers between the code and the hardware is worth more here than
nicer shell ergonomics.

### 7. GPU

Nothing to install beyond a current NVIDIA driver. `wgpu` talks to the card through
DirectX 12, which is already present on Windows 11. No CUDA toolkit is needed — see the
Decision Log for why CUDA was rejected.

### 8. Remote access from the Mac (optional but recommended)

If the PC is the only development machine, SSH is what lets Jonathan work on this from
somewhere other than the desk. The Mac is on `192.168.4.52`; the router is `192.168.4.1`.

Windows 11 includes an OpenSSH server but it is off by default. In PowerShell **as
Administrator**:

```powershell
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
```

```powershell
Start-Service sshd
```

```powershell
Set-Service -Name sshd -StartupType Automatic
```

Confirm the firewall rule exists — a missing rule looks identical to the server not
running:

```powershell
Get-NetFirewallRule -Name *ssh*
```

Make PowerShell the default SSH shell, otherwise you land in `cmd.exe`:

```powershell
New-ItemProperty -Path "HKLM:\SOFTWARE\OpenSSH" -Name DefaultShell -Value "C:\Program Files\PowerShell\7\pwsh.exe" -PropertyType String -Force
```

Find the PC's address with `ipconfig`, then from the Mac generate a dedicated key:

```bash
ssh-keygen -t ed25519 -C "mac-to-coacervate-pc"
```

**Key authentication gotcha, and it catches nearly everyone:** Windows OpenSSH reads
`C:\Users\<you>\.ssh\authorized_keys` for normal accounts, but for accounts in the
Administrators group it ignores that file entirely and reads
`C:\ProgramData\ssh\administrators_authorized_keys` instead — which must also have its
permissions restricted to Administrators and SYSTEM or sshd silently refuses to use it.
Because of this, `ssh-copy-id` does not reliably work against Windows and the key needs
placing by hand. On a personal PC the account is almost certainly an administrator, so
assume the second path.

Finally, reserve the PC's address in the router's DHCP settings so it doesn't move, and
stop the machine sleeping mid-experiment:

```powershell
powercfg /change standby-timeout-ac 0
```

### 9. Long runs that survive a disconnect

Windows OpenSSH tends to kill child processes when the session ends, which is not what you
want for an overnight run. The robust answer is Task Scheduler configured to *run whether
the user is logged on or not*, which detaches the process properly. **Verify this actually
survives a disconnect before relying on it** — do not assume.

---

## Verifying visual work

There is no browser here, so the usual approach of driving a page and screenshotting it
does not apply. The replacement, which must be built early rather than retrofitted:

- `--dump-frame <path>` renders one frame to a PNG and exits.
- `F12` while running dumps the current frame to `runs/<id>/frames/`.
- Claude reads those PNGs directly to see what it has built.

Without this, every visual change becomes Jonathan describing what's wrong in prose. Build
it in phase 5, alongside the first renderer.

**A UI change is not complete until a frame has been dumped and looked at.**

---

## Distribution

1. **Now** — a local `.exe`, run from the build directory.
2. **Soon** — a zip on itch.io. Free, no gatekeeping, no waiting period, and it is the
   natural home for an experimental simulation. This is how it gets sent to friends.
3. **Eventually, if it works well** — Steam. Free app, but the
   [Steam Direct](https://partner.steamgames.com/steamdirect) fee is $100 per application
   and is only refundable after $1,000 of revenue, which a free release will never reach —
   so treat it as roughly £80 spent. There is also a 30-day wait after paying the fee before
   release, a coming-soon store page that must be public for at least two weeks, a Valve
   review of one to five days, capsule art at several sizes, and tax onboarding (W-8BEN for
   a UK individual).

Nothing about distribution changes the build. It is a Windows executable either way, and
the architecture should not be shaped by it.

**Name check:** `Primordial` was the original working title and is unusable — it is
[already on Steam](https://store.steampowered.com/app/1263910/Primordial/), and the
surrounding space is crowded with `Primordia`, `Primordialis`, `Primordial Empire`,
`Primordial Genesis` and others, several of which are adjacent evolution sims.
`Coacervate` returned no Steam results at all. Related prior art worth being aware of:
[Primordial Empire](https://store.steampowered.com/app/3450780/Primordial_Empire/),
[Primordialis](https://store.steampowered.com/app/3011360/Primordialis/) and
[Biosys Inc](https://store.steampowered.com/app/486410/Biosys_Inc/). Those are *games*, with
designed progression. This is an instrument. That difference is the point.

---

## Character of the thing

It runs on a second screen while you work. That is a real design constraint, not flavour:

- Resizable window that behaves itself at any aspect ratio. Never steals focus.
- **Visually calm.** No flashing, no sudden camera moves, nothing that pulls the eye. When
  something dramatic happens in the simulation, the *log* says so — the screen does not
  shout. This is easy to violate accidentally.
- A screensaver mode that hides all UI and shows only the world.
- Deep time: tick counts are displayed as millions of years, so a long run reads as Earth's
  history rather than a number going up.
- Lineages get generated binomial names (*Coacervus primus*) so they are things you can
  refer to rather than coloured dots.
- The event log is written in a naturalist's register: *first adhesion event*; *a lineage
  has begun consuming its neighbours*; *mass extinction — 94% of lineages lost*.
- Leaving it running overnight produces a **chronicle**: a generated natural history of the
  run, waiting to be read in the morning. This is the payoff for the entire walk-away
  premise, and it is mostly presentation over data already being collected.

---

## Build phases

Each phase ends green: tests pass, `clippy` is clean, and the thing is demonstrably working.

| # | Phase | Done when |
| --- | --- | --- |
| 1 | Workspace, config, seeded RNG, test harness | `cargo test` runs; a config round-trips; the RNG is reproducible |
| 2 | Resource grid + physics + energy ledger | Energy conservation holds over 100k ticks as a property test |
| 3 | Genome, development, mutation | A genome grows a deterministic body; duplication/divergence tested; caps hold under fuzzing |
| 4 | Reproduction, death, detritus | Headless run reaches equilibrium without extinction or explosion |
| 5 | Renderer + PNG dump | A frame renders; Claude can see it |
| 6 | egui panels, sliders, live charts | Initial conditions settable; run controllable |
| 7 | Species clustering, naming, event log, inspector, museum | Speciation is visible and named |
| 8 | Replay log, scrubbing, chronicle | An overnight run can be replayed and read |
| 9 | GPU compute port | Matches the CPU reference from the same seed |
| 10 | Polish — screensaver mode, packaging | Ships as a zip |

Phases 1–4 are headless and pure. **Do not start the renderer before the simulation
produces a stable ecology in a headless run** — a beautiful renderer showing a world that
dies in thirty seconds teaches you nothing.

---

## Decision log

Recorded so they are not silently relitigated. Each was argued through.

| Decision | Reasoning |
| --- | --- |
| **Developmental genome, not a parameter vector** | This is the single most important decision in the project. A fixed list of numbers (size, speed, metabolism) can only ever evolve a *better single cell* — multicellularity and novel body plans are unreachable because there is no slot for a thing that doesn't exist yet. A variable-length genome encoding a growth program, with gene duplication and divergence as mutation operators, is what lets a lineage *gain* structure. That operator is the engine behind essentially all real biological complexity. Without it you get slightly rounder dots and a screensaver. |
| **Rust** | Best throughput, `rayon` makes multicore nearly free, compiles to both native and (later, if wanted) WebAssembly from one source. Jonathan has never written Rust — hence `forbid(unsafe_code)`, heavy property testing, and tests as the readable contract. |
| **`wgpu`, not CUDA** | CUDA is marginally faster on a 4070 Ti but its Rust tooling has been unreliable, and it would lock the simulation to that one machine forever. `wgpu` compiles the same shaders to DirectX 12, Vulkan, Metal and WebGPU. |
| **No Bevy** | Its entity-component system wants to own the data layout; this project needs flat fixed-capacity arrays for cache behaviour *and* GPU compute. You would fight it for the whole project and pay long compile times for the privilege. |
| **No Tauri / Dioxus** | Would allow a prettier UI in HTML and CSS, but reintroduces the entire JavaScript toolchain for the chrome alone, while the world view still needs a native GPU surface underneath. |
| **CPU reference implementation before GPU** | GPU compute fails by producing silently wrong numbers. A tested CPU implementation turns "is my shader correct?" from a debugging exercise into a differential test: same seed, same results. Without it the GPU port is guesswork. |
| **Energy strictly conserved and asserted** | Unbalanced energy is how these simulations quietly become either runaway blooms or instant extinctions. Light influx is the *only* source; it sets a hard carrying capacity, and that carrying capacity is the pressure that drives everything else. |
| **Predation emergent, not scripted** | A body is a denser package of energy than the surrounding soup, so eating one is simply a better strategy. Whether a herbivore/predator split appears is one of the genuinely interesting outcomes — coding it in would be answering the question in advance. |
| **Reactive behaviour first, neural networks later** | Evolved parameters on a fixed reactive controller get organisms moving and taxis evolving cheaply. Networks whose inputs and outputs wire to whatever sensory and contractile cells the body actually grew are far more interesting and far more expensive. The architecture leaves room; phase 1 does not build it. |
| **Asexual reproduction only, for now** | Sexual recombination changes evolutionary dynamics dramatically and adds a great deal of machinery (mate finding, compatibility, genome alignment). Worth revisiting once the asexual case is stable. |
| **Deterministic from seed + config** | When something interesting happens you will want to see it again. Everything derives from one seeded PRNG; no wall-clock time, no thread-scheduling dependence, no unordered iteration. |
| **Bounded expectations** | Multicellularity, cell specialisation, size and shape diversification and plausibly a feeding-strategy split are realistic overnight outcomes. Nervous systems, eyes and animal-like intelligence are not, on any timescale this project will run. Build a substrate that *could* go there and be honest about the timescale rather than tuning the sim to fake an impressive result. |

---

## Honest risks

- **The model may simply not produce interesting complexity.** Most evolution simulators
  don't. Compute is the second-most-important variable; the genome design is the first. If
  the developmental genome can't express complexity, no amount of GPU throughput helps.
- **Balance is genuinely hard.** Too much light and everything blooms and stagnates; too
  little and everything dies. Expect to spend real time on the energy economy, and build the
  observability early so you can see *why* a run failed.
- **A gorgeous renderer showing boring organisms is still boring.** The visuals depend on
  evolution producing shapes worth looking at. That's the simulation's job.
