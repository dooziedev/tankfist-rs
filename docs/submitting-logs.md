# Submitting Overwatch logs

Every number in this project comes from Overwatch logs. If you record some, they help check and
improve the rewrite: more players, more situations, more patches. This takes about 10 minutes.

## What a log contains

The Workshop scripts write numbers only: Doomfist's position, speed, aim, which buttons are held,
health and cooldowns, every 16 ms, plus damage events. Nothing about your account, name or PC.
(The file name is just the date and time.)

## 1. Turn on log files (once)

In Overwatch: **Options → Gameplay → General → Enable Workshop Inspector Log File → On**.
Logs are saved to:

```text
Documents\Overwatch\Workshop\Log-YYYY-MM-DD-HH-MM-SS.txt
```

A new file starts each time you open a custom game.

## 2. Load a script

1. **Play → Custom Games → Create → Settings → Workshop**.
2. Open a script from the [`workshop/`](../workshop) folder of this repo, copy all of it, and
   use the paste button at the top of the Workshop settings screen.
3. Start the game.

Which script:

| Script | What it's for | How |
|---|---|---|
| `doomfist_logger.txt` | **Your own play** (most useful) | Press **Interact** to start a take, Interact again to stop. Crouch+Interact resets. |
| `auto_slam_tests.txt`, `auto_punch_tests.txt`, `auto_hit_tests.txt`, `auto_hit_tests2.txt`, `auto_gun_tests.txt` | A dummy Doomfist runs a test suite by itself | Stand on open ground (the hit tests want a straight wall 30-60 m in front), face forward, press **Interact**, wait for the "done" message. |
| `turn_and_bhop.txt` | Your own play plus bunny-hop bot tests | Interact records you; Primary Fire + Interact runs the bot tests. |
| `playback_*.txt` | A bot replays a recorded run | Melee + Interact starts the playback; Interact records yourself. |

Tips for your own takes:
- Keep each take short and about one thing (e.g. "punch, jump cancel, turn left").
- Note roughly what you did in each take; it makes the log far easier to use.
- Odd tech and edge cases are especially welcome.

## 3. Submit it

1. Open a new issue with the **Log submission** form:
   **Issues → New issue → Log submission**.
2. Fill in which script you used and what you did in each take (the game shows the take number
   when you start one).
3. Attach the log: drag the `.txt` file into the form. Files up to 25 MB attach directly; zip
   anything bigger.
4. Submit.

## Testing the rewrite against your log yourself

With [Rust](https://rustup.rs) installed:

```bash
cargo run --bin analyze -- "path/to/Log-....txt"
```

summarises every take, and

```bash
cargo run --bin compare -- sim "path/to/Log-....txt" --ow-take 1
```

replays take 1's inputs through the Rust Doomfist and shows where it starts to differ from what
really happened. If you find a split, mention the take and time in the issue.
