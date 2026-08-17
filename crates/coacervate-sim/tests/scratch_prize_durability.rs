//! SCRATCH — a measurement, not a shipped test. Delete after reading.
//!
//! Does the prize instrument B measures survive being taken, and does it survive a population?

use coacervate_sim::cell::Vec2;
use coacervate_sim::config::{Config, RawConfig, spec_defaults};
use coacervate_sim::grid::Grid;
use coacervate_sim::ledger::Ledger;

const LIFETIME: u32 = 1739;
const MOTOR_PER_TICK: f64 = 0.006;
const WIDTH: f32 = 2048.0;
const HEIGHT: f32 = 1152.0;
const CROWD: usize = 2_200;

struct Stir(u64);

impl Stir {
    fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        let bits = u16::try_from(self.draw() >> 48).expect("sixteen bits fit in a u16");
        f32::from(bits) / 65_536.0
    }
}

fn settings(uptake: f64, diffusion: f64) -> Config {
    let mut raw: RawConfig = spec_defaults();
    raw.light.uptake = uptake;
    raw.light.diffusion = diffusion;
    raw.light.patch_drift = 0.0;
    raw.validate().expect("a configuration the program accepts")
}

/// `assay.rs`'s `best_water_near`, copied so the two instruments aim the same way.
fn best_near(grid: &Grid, at: Vec2, reach: f32) -> (Vec2, f64) {
    let mut best = (at, f64::from(grid.tiles()[grid.tile_at(at)]));

    for step in 1..=16u16 {
        let (sin, cos) = (f32::from(step) * std::f32::consts::TAU / 16.0).sin_cos();
        for away in [reach * 0.5, reach] {
            let there = Vec2::new(
                (at.x + cos * away).rem_euclid(WIDTH),
                (at.y + sin * away).clamp(0.0, HEIGHT),
            );
            let held = f64::from(grid.tiles()[grid.tile_at(there)]);
            if held > best.1 {
                best = (there, held);
            }
        }
    }

    best
}

fn field_mean(grid: &Grid) -> f64 {
    let tiles = u32::try_from(grid.tiles().len()).expect("a tile count fits in a u32");
    grid.total_energy() / f64::from(tiles)
}

/// Sixteen centres, spread over the world on a lattice, well clear of one another.
fn centres(offset: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    for row in 0..4u16 {
        for col in 0..4u16 {
            out.push(Vec2::new(
                (f32::from(col) + 0.5)
                    .mul_add(WIDTH / 4.0, offset)
                    .rem_euclid(WIDTH),
                (f32::from(row) + 0.5) * (HEIGHT / 4.0),
            ));
        }
    }
    out
}

#[test]
#[ignore = "SCRATCH: three dawned fields and ~1.5M grid ticks; run deliberately with --ignored"]
fn scratch_does_the_prize_survive_being_taken() {
    let motor_lifetime = MOTOR_PER_TICK * f64::from(LIFETIME);
    println!("\na motor costs {motor_lifetime:.3} energy units over a {LIFETIME}-tick lifetime");

    for (uptake, diffusion) in [(0.01f64, 0.04f64), (0.30, 0.002), (0.90, 0.002)] {
        println!(
            "\n################ uptake {uptake:.2}, diffusion {diffusion:.3} ################"
        );
        let config = settings(uptake, diffusion);
        let mut grid = Grid::new(&config);
        let mut ledger = Ledger::new(0.0);

        for _ in 0..30_000 {
            grid.tick(&mut ledger);
        }
        let start = ledger.influx_total();
        for _ in 0..1_000 {
            grid.tick(&mut ledger);
        }
        let influx_rate = (ledger.influx_total() - start) / 1_000.0;
        println!(
            "dawned field: mean tile {:.4}, whole-world influx {influx_rate:.4} per tick",
            field_mean(&grid)
        );

        // ---------------------------------------------------------------------------
        // 1. Time constants: how fast a tile is stripped, how fast it comes back.
        // ---------------------------------------------------------------------------
        let tile = grid.tile_at(Vec2::new(1024.0, 576.0));
        let full = f64::from(grid.tiles()[tile]);

        let mut down = Vec::new();
        for _ in 0..20_000 {
            let held = f64::from(grid.tiles()[tile]);
            grid.harvest(&mut ledger, tile, uptake * held);
            grid.tick(&mut ledger);
            down.push(f64::from(grid.tiles()[tile]));
        }
        let grazed = *down.last().expect("the trace has entries");
        let mark = full - 0.632 * (full - grazed);
        let t_down = down
            .iter()
            .position(|held| *held <= mark)
            .map_or(0, |i| i + 1);

        let mut up = Vec::new();
        for _ in 0..60_000 {
            grid.tick(&mut ledger);
            up.push(f64::from(grid.tiles()[tile]));
        }
        let back = *up.last().expect("the trace has entries");
        let mark = grazed + 0.632 * (back - grazed);
        let t_up = up
            .iter()
            .position(|held| *held >= mark)
            .map_or(0, |i| i + 1);
        let t_full = up
            .iter()
            .position(|held| *held >= grazed + 0.95 * (back - grazed))
            .map_or(0, |i| i + 1);

        println!(
            "ONE tile grazed, neighbours full: {full:.4} -> {grazed:.4}; strip t63 = {t_down} \
             ticks ({:.3} lifetimes), refill t63 = {t_up} ticks ({:.3} lifetimes), 95% back in \
             {t_full} ({:.3} lifetimes)",
            f64::from(u32::try_from(t_down).expect("fits")) / f64::from(LIFETIME),
            f64::from(u32::try_from(t_up).expect("fits")) / f64::from(LIFETIME),
            f64::from(u32::try_from(t_full).expect("fits")) / f64::from(LIFETIME),
        );

        // The same question with the whole neighbourhood grazed too — which is what a
        // population does. A 21x21 block of tiles is 168 world units across, wider than a
        // motor's 88-unit reach.
        let mut block = Vec::new();
        for down_by in -10i16..=10 {
            for across in -10i16..=10 {
                block.push(grid.tile_at(Vec2::new(
                    f32::from(across).mul_add(8.0, 1024.0).rem_euclid(WIDTH),
                    f32::from(down_by).mul_add(8.0, 576.0).clamp(0.0, HEIGHT),
                )));
            }
        }
        for _ in 0..20_000 {
            for at in &block {
                let held = f64::from(grid.tiles()[*at]);
                grid.harvest(&mut ledger, *at, uptake * held);
            }
            grid.tick(&mut ledger);
        }
        let hollow = f64::from(grid.tiles()[tile]);

        let mut up = Vec::new();
        for _ in 0..60_000 {
            grid.tick(&mut ledger);
            up.push(f64::from(grid.tiles()[tile]));
        }
        let back = *up.last().expect("the trace has entries");
        let mark = hollow + 0.632 * (back - hollow);
        let t_up = up
            .iter()
            .position(|held| *held >= mark)
            .map_or(0, |i| i + 1);
        let t_full = up
            .iter()
            .position(|held| *held >= hollow + 0.95 * (back - hollow))
            .map_or(0, |i| i + 1);
        println!(
            "a 168-unit BLOCK grazed: centre at {hollow:.4}; refill t63 = {t_up} ticks ({:.3} \
             lifetimes), 95% back in {t_full} ({:.3} lifetimes)",
            f64::from(u32::try_from(t_up).expect("fits")) / f64::from(LIFETIME),
            f64::from(u32::try_from(t_full).expect("fits")) / f64::from(LIFETIME),
        );

        // ---------------------------------------------------------------------------
        // 2. The lifetime race. What one perfectly aimed move is actually worth, in the
        //    same units as a motor's whole-lifetime keep, with the prize taken rather
        //    than looked at.
        // ---------------------------------------------------------------------------
        for empty in [true, false] {
            let mut crowd = Vec::new();
            if !empty {
                let mut rng = Stir(0x5EED_1234);
                for _ in 0..CROWD {
                    crowd.push(grid.tile_at(Vec2::new(rng.unit() * WIDTH, rng.unit() * HEIGHT)));
                }
            }

            // ⭐ The crowd grazes on EVERY tick from here on, races included. Without that the
            // field recovers to the empty case between reaches and the population is not there.
            macro_rules! step {
                () => {{
                    for at in &crowd {
                        let held = f64::from(grid.tiles()[*at]);
                        grid.harvest(&mut ledger, *at, uptake * held);
                    }
                    grid.tick(&mut ledger);
                }};
            }

            for _ in 0..30_000 {
                step!();
            }

            if empty {
                println!(
                    "\n-- empty world, nobody else grazing: mean tile {:.4}",
                    field_mean(&grid)
                );
            } else {
                let mut under = 0.0;
                for at in &crowd {
                    under += f64::from(grid.tiles()[*at]);
                }
                let opened = ledger.influx_total();
                for _ in 0..1_000 {
                    step!();
                }
                println!(
                    "\n-- a crowd of {CROWD} grazers, grazing throughout: mean tile {:.4}, mean \
                     tile UNDER a grazer {:.4}, whole-world influx {:.4}/tick, i.e. {:.5} per \
                     grazer per tick",
                    field_mean(&grid),
                    under / f64::from(u32::try_from(CROWD).expect("fits")),
                    (ledger.influx_total() - opened) / 1_000.0,
                    (ledger.influx_total() - opened)
                        / 1_000.0
                        / f64::from(u32::try_from(CROWD).expect("fits"))
                );
            }

            println!(
                "reach | best-here at t0 | stay  | move  | GAIN over a lifetime | swim  | \
                 SWIM GAIN"
            );

            for reach in [16.6f32, 88.0, 500.0] {
                let here = centres(reach * 0.037);
                let mut there = Vec::new();
                let mut snapshot = 0.0;
                for at in &here {
                    let (best, held) = best_near(&grid, *at, reach);
                    snapshot += held - f64::from(grid.tiles()[grid.tile_at(*at)]);
                    there.push(best);
                }

                // A third arm that keeps moving, covering `reach` over its whole life in the
                // direction of the best water it can see.
                let mut heading = Vec::new();
                for (from, to) in here.iter().zip(&there) {
                    let (dx, dy) = (to.x - from.x, to.y - from.y);
                    let far = dx.hypot(dy).max(1e-6);
                    heading.push((dx / far, dy / far));
                }

                let (mut stay, mut moved, mut swum) = (0.0f64, 0.0f64, 0.0f64);
                for step in 0..LIFETIME {
                    let along = f32::from(u16::try_from(step).unwrap_or(u16::MAX))
                        * (reach / f32::from(u16::try_from(LIFETIME).expect("fits")));
                    for index in 0..here.len() {
                        let at = grid.tile_at(here[index]);
                        let held = f64::from(grid.tiles()[at]);
                        stay += grid.harvest(&mut ledger, at, uptake * held);

                        let at = grid.tile_at(there[index]);
                        let held = f64::from(grid.tiles()[at]);
                        moved += grid.harvest(&mut ledger, at, uptake * held);

                        let (dx, dy) = heading[index];
                        let swimmer = Vec2::new(
                            dx.mul_add(along, here[index].x).rem_euclid(WIDTH),
                            dy.mul_add(along, here[index].y).clamp(0.0, HEIGHT),
                        );
                        let at = grid.tile_at(swimmer);
                        let held = f64::from(grid.tiles()[at]);
                        swum += grid.harvest(&mut ledger, at, uptake * held);
                    }
                    step!();
                }

                let n = f64::from(u32::try_from(here.len()).expect("fits"));
                println!(
                    "{reach:5.1} | {:15.4} | {:5.3} | {:5.3} | {:+20.3} | {:5.3} | {:+9.3}",
                    snapshot / n,
                    stay / n,
                    moved / n,
                    (moved - stay) / n,
                    swum / n,
                    (swum - stay) / n
                );

                // Let the holes fill again before the next reach.
                for _ in 0..10_000 {
                    step!();
                }
            }
        }
    }
}
