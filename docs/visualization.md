**English** | [日本語](visualization.ja.md)

# Visualization

The Python package `hegselmann-tools` (a uv workspace member) reads the runvault run directories under `results/` and produces figures. Install once with `uv sync` at the workspace root. Which run to read is resolved with `runvault path`, never by globbing `results/` or guessing the newest directory.

```bash
uv sync
uv run hegselmann-tools visualize
uv run hegselmann-tools visualize-sweep
uv run hegselmann-tools show-experiment-settings
uv run hegselmann-tools reproduce
```

The CLI dispatches to one of four subcommands via argparse; arguments after the subcommand are passed straight to the corresponding module.

## `visualize` — opinion trajectory

Reads `artifacts/opinions.csv` and `metrics.csv` from a `run` result (by default the latest one, via `runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone`) and writes:

- `opinion_trajectory.png` — the opinion trajectory (x = time, y = opinion in `[0,1]`, one line per agent; paper Fig. 3 style). With many agents the lines are drawn as translucent thin strokes so clusters remain visible.
- `metrics_timeseries.png` — three panels: occupied-class count (log y), variance, and `max|Δx|` (log y, the convergence indicator).

```bash
uv run hegselmann-tools visualize --results_dir "$(runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone)"
```

| Flag | Default | Description |
|---|---|---|
| `--results_dir` | the latest `run` | the runvault run directory |
| `--results_root` | results | where `runvault path` looks when `--results_dir` is omitted |
| `--output_dir` | `results/hegselmann-averaging/figures/{run_slug}` | figure output directory |

## `visualize-sweep` — phase diagram & consensus brink

Rebuilds a one-row-per-trial table from the `terminal` rows of the sweep children's `events.jsonl` (a spread needs the individual trials, not the condition's mean) and writes:

- `sweep_occupied_classes.png` — occupied-class count vs ε, one curve per averaging operator (log y, with trial mean ± std error bars; paper Fig. 4–7 style). The dashed line marks the consensus boundary (1 class).
- `sweep_consensus_brink.png` — a bar chart comparing the consensus brink ε* across means (paper Observation 1, Fact 4). Means that never reach consensus within the swept range are annotated as such.

```bash
uv run hegselmann-tools visualize-sweep --sweep_dir "$(runvault path --experiment hegselmann-averaging --latest --subcommand sweep)"
```

| Flag | Default | Description |
|---|---|---|
| `--sweep_dir` | the latest `sweep` | the sweep parent's run directory |
| `--results_root` | results | where `runvault path` looks when `--sweep_dir` is omitted |
| `--output_dir` | `results/hegselmann-averaging/figures/{run_slug}` | figure output directory |

A `sweep_summary.csv` from before the runvault migration is still read as-is when one is present.

## `show-experiment-settings`

Pretty-prints the conditions of a run directory. runvault's `config.json` is an envelope, so the conditions are read from its `parameters`; which of `run` / `sweep` / `sweep-point` it was comes from `run.json`'s `subcommand`. The pre-runvault flat `config.json` and `sweep_config.json` are still read. Use `--json` for machine-readable output.

```bash
uv run hegselmann-tools show-experiment-settings
uv run hegselmann-tools show-experiment-settings --results-dir "$(runvault path --experiment hegselmann-averaging --latest --subcommand sweep)"
uv run hegselmann-tools show-experiment-settings --json
```

## `reproduce` — one-shot paper-figure bundle

Runs the model across the paper's headline experiments in a single command and writes the figures plus a machine-readable summary. It orchestrates the Rust binary (`cargo run --release -- run / sweep …`), asks runvault where each invocation's run landed, reads it, renders the PNGs, and checks the observed per-operator regime against the paper's expectation.

```bash
uv run hegselmann-tools reproduce              # full paper-faithful run (n=625)
uv run hegselmann-tools reproduce --quick      # fast smoke run (n=200, fewer trials)
uv run hegselmann-tools reproduce --specs operators,sweep
uv run hegselmann-tools reproduce --skip-build # skip cargo build when already built
```

Outputs land under `results/reproduce_{YYYYMMDD_HHMMSS}/`:

- `figures/operators_eps0.15_grid.png` — at a fixed `ε=0.15`, a 2×3 grid of opinion trajectories for `A / G / H / P0.01 / P100 / R`, illustrating how the averaging operator alone shifts the steady-state regime (paper §3, Fig. 4–7). The arithmetic mean `A` polarizes, the high-exponent power mean `P100` collapses to a single high cluster, and the random mean `R` never clusters (stays diffuse).
- `figures/a_regimes_eps_sweep.png` — the arithmetic mean `A` at `ε = 0.05 / 0.15 / 0.25`, showing the plurality → polarization → consensus transition for one operator as `ε` grows.
- `figures/sweep_phase_diagram.png` — the occupied-class count vs ε per operator (log y) alongside the consensus-brink ε* bar chart (paper Observation 1, Fact 4).
- `reproduce_summary.json` — per-spec record of the cargo invocations, run directories, figure paths, and an observed-vs-expected comparison: each operator panel lists its expected regime, the observed regime and occupied-class count, and a `pass` flag; the spec carries a `PASS` / `off` verdict. The sweep spec records the per-operator consensus brink ε*.

| Flag | Default | Description |
|---|---|---|
| `--specs` | all | comma-separated spec IDs to run (`operators`, `a_regimes`, `sweep`) |
| `--output-dir` | results | output root (relative to the workspace root) |
| `--cargo-output-dir` | `{output-dir}` | directory passed to the Rust `--output-dir` (the runvault results root) |
| `--workspace-root` | inferred | workspace root (also overridable via `HEGSELMANN_PROJECT_ROOT`) |
| `--quick` | off | shrink `n` and trial counts for a fast smoke run |
| `--skip-build` | off | skip `cargo build --release` |

## Note on fonts

The scripts set `font.family = "Hiragino Sans"` for Japanese labels (macOS). On other platforms, substitute an installed CJK font in the `plt.rcParams` line at the top of `visualize.py` / `visualize_sweep.py`.

---
*This file was generated by Claude Code.*
