**English** | [日本語](cli.ja.md)

# CLI

The Rust binary `hegselmann` (run via `cargo run --release -- …`) exposes two subcommands: `run` and `sweep`.

## `run` — single simulation

Run the bounded-confidence dynamics for one `(ε, mean)` pair.

```bash
cargo run --release -- run \
    --n 625 --eps 0.15 --mean A --start uniform \
    --max-iterations 100 --tol 1e-6 --seed 42
```

| Flag | Default | Description |
|---|---|---|
| `--n` | 625 | number of agents `n` |
| `--eps` | 0.15 | symmetric confidence radius `ε` |
| `--mean` | A | averaging operator: `A` / `G` / `H` / `P<p>` (e.g. `P0.01`, `P100`) / `R`. `P` alone uses `--p`. |
| `--p` | 1.0 | power exponent for `--mean P` (or fallback for bare `P`) |
| `--start` | uniform | initial opinion profile (`uniform`) |
| `--max-iterations` | 100 | maximum number of steps `T` |
| `--tol` | 1e-6 | convergence tolerance (stop when `max|Δx| < tol`; ignored for `R`) |
| `--seed` | random | RNG seed |
| `--output-dir` | results | runvault results root |

Examples for the various means:

```bash
cargo run --release -- run --n 625 --eps 0.20 --mean H --seed 42            # harmonic (asymmetric polarization)
cargo run --release -- run --n 625 --eps 0.05 --mean P --p 100 --seed 42     # power mean P_100
cargo run --release -- run --n 625 --eps 0.05 --mean P0.01 --seed 42         # power mean P_0.01
cargo run --release -- run --n 625 --eps 0.03 --mean R --max-iterations 5000 --seed 42  # random mean
```

**Output files:**

Each execution is stored as a runvault run directory. The run directory *is* the output location, so no timestamped directory and no `latest` symlink are created here. Ask `runvault` for the path of the most recent finished run.

```bash
runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone
```

```
results/
└── hegselmann-averaging/                           ← experiment
    ├── latest_finished -> run_20260831_153702_...  ← the last run that finished
    ├── run_20260831_153702_d38d7702_aeaf/          ← <subcommand>_<time>_<cfg8>_<exec4>
    │   ├── run.json                                ← metadata (git commit / environment / paper)
    │   ├── config.json                             ← an envelope; the conditions sit under ["parameters"]
    │   ├── metrics.csv                             ← long form (step / scope / name / value)
    │   ├── events.jsonl                            ← per-step observations + the terminal row (phase label)
    │   ├── status.json                             ← how it ended and how long it took
    │   ├── manifest.csv                            ← hashes of artifacts/ and logs/
    │   └── artifacts/
    │       └── opinions.csv                        ← long-format opinion trajectory: t, agent_id, opinion
    └── figures/                                    ← what the plotting scripts write (outside the run)
        └── run_20260831_153702_d38d7702_aeaf/
```

Figures are drawn after the run has ended, so they go **outside** the run directory (`<experiment>/figures/<run_slug>/`). `manifest.csv` is settled by `finish()`, so a file added to `artifacts/` afterwards would carry no hash.

`metrics.csv` is long form, one value per row. The four per-step metrics `n_occupied_classes` / `mean` / `variance` / `max_delta` carry a `step` (`step_unit=step`, `scope=run`); `converged` (0.0 / 1.0) and `final_iteration`, which describe the whole run with one number each, sit at `scope=run` with no `step`.

**The phase is not a metric.** consensus / polarization / plurality is a label rather than a number, and it follows uniquely from `n_occupied_classes` on the same row (1 / 2 / ≥3) — assigning it a number would add nothing. The final phase is therefore kept as a label on the `terminal` row of `events.jsonl` (`"phase": "polarization"`); the same row carries convergence and censoring through `outcome` / `censored` / `budget`. See [`show-experiment-settings`](visualization.md#show-experiment-settings) for displaying the conditions.

## `sweep` — ε sweep across means

Sweep ε and aggregate the occupied-class count and consensus brink per averaging operator.

```bash
cargo run --release -- sweep \
    --eps-min 0.0 --eps-max 0.40 --eps-step 0.01 \
    --means A,G,H,P0.01,P100,R --n 625 --runs 50 --seed 42
```

| Flag | Default | Description |
|---|---|---|
| `--eps-min` | 0.0 | minimum ε |
| `--eps-max` | 0.40 | maximum ε (inclusive) |
| `--eps-step` | 0.01 | ε step |
| `--means` | A,G,H,P0.01,P100,R | comma-separated list of operators |
| `--p` | 1.0 | exponent for a bare `P` in the list |
| `--n` | 625 | number of agents |
| `--runs` | 50 | independent trials per `(mean, ε)` |
| `--max-iterations` | 100 | maximum steps |
| `--tol` | 1e-6 | convergence tolerance |
| `--seed` | 42 | seed base (each trial derives an independent seed) |
| `--start` | uniform | initial opinion profile |
| `--output-dir` | results | runvault results root |

Each trial derives an independent seed via `derive_seed(seed, &[hash(mean), eps.bits, run])`, so trials are reproducible and uncorrelated.

**Output files:**

A sweep is one **parent** run plus one **child** run per `(mean, ε)` point. The parent's `config.json` holds the grid definition itself and no per-condition metrics; a child is that condition's `runs` trials.

```bash
runvault path --experiment hegselmann-averaging --latest --subcommand sweep
```

```
results/hegselmann-averaging/
├── sweep_20260831_155216_2d267af5_f1b4/     ← parent: the grid definition (subcommand=sweep)
└── sweep-point_20260831_155216_..._9194/    ← child, one per (mean, ε) (subcommand=sweep-point)
    ├── config.json                          ← the condition (n, eps, mean, p, runs, …)
    ├── metrics.csv                          ← the condition summarized: n_units, n_converged, means
    └── events.jsonl                         ← one observation + one terminal row per trial
```

The child's subcommand is `sweep-point`, not `run`: a `run` is a single simulation, whereas a child is `runs` simulations of one condition, and letting two different things share one name would make `runvault path --subcommand run` ambiguous.

Each trial is one `terminal` row in the child's `events.jsonl` — one row per row of the former `sweep_summary.csv`, carrying `seed`, `t` (the final iteration), `censored` (the negation of `converged`), `budget`, `phase` (a label), `n_occupied_classes`, `mean_opinion`, `variance` and `max_delta`. The per-trial values are deliberately *not* metrics: putting them in `metrics.csv` would make `(run_uid, step, scope, name)` collide across trials. The child's `metrics.csv` therefore holds the condition's aggregate only (`n_units`, `n_converged`, `convergence_rate`, `mean_n_occupied_classes`, `mean_opinion_mean`, `mean_opinion_variance`, `mean_final_iteration`).

The command also prints the estimated consensus brink ε* per mean (smallest ε at which the trial-averaged occupied-class count first reaches 1). It is printed, not recorded: it is a derived quantity that can be rebuilt from the children's terminal rows at any time, and storing the bare number inside a run would lose which ε grid it was measured on.

---
*This file was generated by Claude Code.*
