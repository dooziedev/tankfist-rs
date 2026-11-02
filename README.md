# Doomfist Rewrite

Overwatch's Doomfist rebuilt in Rust, tick for tick. Nearly every number was measured from
Overwatch with Workshop logging scripts and checked by replaying the logged inputs through the
Rust version.

No game assets are included or ever committed. A built-in stand-in mannequin is used unless you
bring a model exported from your own copy of the game (see [docs/animations.md](docs/animations.md)).

## Play it

```bash
cargo run --release --bin bean
```

Mouse look, WASD, Space jump, right mouse Rocket Punch (hold to charge), Shift Seismic Slam,
E Power Block, V quick melee. Tab third person, T slow motion, R reset, H respawn dummies,
F record, Esc mouse, [ and ] change sensitivity.

Mouse sensitivity and FOV use Overwatch's numbers and are saved in `bean_settings.txt`
(`sensitivity = 5`, `fov = 103` by default; this file is git-ignored).

## What's in it

| Path | What it is |
|---|---|
| `src/abilities/` | Movement, passive, Power Block, Rocket Punch, Seismic Slam (one file each) |
| `src/tunables.rs` | Every number, tagged `[measured <date>]`, `[partly measured]` or `[guess]` |
| `src/doomfist.rs`, `src/world.rs` | Shared state, tick order, dummies and their knockback |
| `src/log.rs`, `src/replay.rs` | Read Workshop logs; replay their inputs through the sim |
| `src/playback.rs`, `src/record.rs` | Turn any recording into a Workshop playback script; write logs from the sim |
| `src/bin/bean/` | Playable 3D test map, skinned animation, model loading |
| `src/bin/analyze.rs` | Summaries and tables for Workshop logs |
| `src/bin/compare.rs` | Line up two runs (or the sim against a log) and show where they split |
| `workshop/` | Workshop scripts: manual logger, automatic test suites, playback |
| `logs/` | Measurement logs from Overwatch (numbers only) used by the tests |
| `tests/` | Ability tests plus replays of real logs that must keep matching |

## Measuring in Overwatch

1. Turn on **Enable Workshop Inspector Log File** in Overwatch's options (logs go to
   `Documents\Overwatch\Workshop\`).
2. Paste a script from `workshop/` into a custom game:
   - `doomfist_logger.txt`: records you (Interact starts/stops a take)
   - `auto_*_tests.txt`: a dummy Doomfist runs a test suite by itself (Interact)
   - `turn_and_bhop.txt`, playback scripts: record yourself and/or replay recorded inputs on a bot
3. Analyse:

```bash
cargo run --bin analyze -- logs/hit-01.txt
```

```bash
cargo run --bin compare -- sim logs/compare-01.txt --ow-take 1
```

## Status

Measured and replaying within a few cm of Overwatch: walking, jumping, air control, Rocket Punch
(charge, dash, jump cancel and camera boost, airborne punches), Seismic Slam (aim, steering, bunny
hop, re-press cancel), Power Block, empowered punch, passive temporary health, hit damage,
knockback and wall slams.

Not built yet: Hand Cannon (measuring script ready: `workshop/auto_gun_tests.txt`), Meteor Strike,
surface-angle bounce tech.

## Assets policy

`assets/` is git-ignored, and `git config core.hooksPath scripts/hooks` enables a pre-commit
check that refuses 3D models, animations, textures and audio anywhere in the repo.
