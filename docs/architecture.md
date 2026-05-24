**English** | [日本語](architecture.ja.md)

# Architecture

## Repository structure

A two-project layout: a Cargo workspace + a uv workspace.

```
hegselmann2005/
├── Cargo.toml                 # Cargo workspace root
├── pyproject.toml             # uv workspace root
├── simulation/                # Rust project (hegselmann-opinion-simulation)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs            # CLI (run / sweep)
│   │   ├── lib.rs             # module re-exports for the binary + integration tests
│   │   ├── config.rs          # Config + config.json serialization
│   │   ├── means.rs           # MeanOperator enum (A/G/H/P/R) + apply_mean + parser
│   │   ├── world.rs           # socsim WorldState impl (OpinionWorld, complete graph)
│   │   ├── mechanisms.rs      # socsim Mechanism impl (BoundedConfidenceUpdate, synchronous)
│   │   ├── metrics.rs         # occupied classes, phase classification, consensus brink
│   │   └── simulation.rs      # init + run driver (SimulationBuilder wiring)
│   └── tests/
│       └── integration_test.rs
├── tools/                     # Python project (hegselmann-tools)
│   ├── pyproject.toml
│   └── src/hegselmann_tools/
│       ├── cli.py                       # unified CLI (hegselmann-tools)
│       ├── visualize.py                 # opinion trajectory + metrics
│       ├── visualize_sweep.py           # occupied-classes phase diagram + consensus brink
│       └── show_experiment_settings.py  # display run / sweep settings
└── results/                   # simulation output (gitignored)
```

- `cargo run` launches the `simulation` crate from the workspace root.
- `uv run` invokes the `hegselmann-tools` command exposed by the `tools` member of the uv workspace.

## Model on the socsim framework

The simulation engine is built on the social-simulation framework [rs-social-simulation-tools](https://github.com/akitenkrad/rs-social-simulation-tools) (socsim) — a git dependency, with the commit pinned in `Cargo.lock`. Because the canonical Hegselmann–Krause model is a **complete graph / non-spatial** model, only `socsim-core` (traits) and `socsim-engine` (Simulation / Builder) are used — there is **no `socsim-grid` and no `socsim-net`**.

The socsim APIs used:

- `WorldState` — `OpinionWorld` implements `agent_ids` / `clock` / `clock_mut`. It holds the opinion vector `opinions: Vec<f64>`, the confidence radius `eps`, the `MeanOperator`, and `last_max_delta`.
- `Mechanism` / `Phase::Interaction` — `BoundedConfidenceUpdate` fires in the `Interaction` phase.
- `SequentialScheduler` — agents are activated in ascending `AgentId` order. Because the update is synchronous, the activation order does not affect the result, so the deterministic sequential scheduler is used and `ctx.agent_order` is ignored.
- `StepContext::request_stop` / `Simulation::stop_requested` — early stop on convergence.
- `StepContext::scratch` / `Simulation::run_observed` — passing per-step results (`max_delta`, `converged`) to the driver.
- `SimRng` / `derive_seed` — `derive_seed(root, &[0])` seeds the initial-opinion RNG, `derive_seed(root, &[1])` seeds the engine RNG (used only by the random mean R).

## The bounded-confidence update (synchronous)

The generalized update rule (paper eq. (2)) is

```
x_i(t+1) = M( { x_j(t) : j ∈ I(i, x(t)) } ),   I(i, x) = { j : |x_i(t) − x_j(t)| ≤ ε }
```

where `M` is the selected averaging operator. The mechanism implements **synchronous (simultaneous) update**: it snapshots `prev = world.opinions.clone()`, computes each agent's confidence set and new opinion from `prev`, then writes all new opinions at once. Synchronous update is the limit case of "m groups updating in sequence" (paper §4), so the result is independent of the activation order — hence the deterministic `SequentialScheduler` is sufficient and the scheduler does not affect outcomes. Complete-graph scan is `O(n²)` per step; the paper's `n = 625` is lightweight.

## The averaging operators

For a non-empty multiset `S` of opinions in the confidence set (`m = |S|`, always includes the agent itself):

| Operator | Definition | Notes |
|---|---|---|
| A (arithmetic) | `(1/m) Σ s` | `= P_1`, the 2002 HK rule |
| G (geometric) | `(Π s)^{1/m}` | `= P_0`; requires `s > 0` (computed in log space) |
| H (harmonic) | `m / Σ (1/s)` | `= P_{-1}`; requires `s > 0` |
| P_p (power / Hölder) | `((1/m) Σ s^p)^{1/p}` | `p ≠ 0` |
| R (random) | `Uniform(min S, max S)` | the only operator that uses the RNG |

Systematic inequality: `P_{-∞}(min) ≤ H ≤ G ≤ A ≤ P_p ≤ P_{∞}(max)` for `p ≥ 1`.

A/G/H/P are deterministic and request a stop once `max|Δx| < tol`. R is non-deterministic each step (it draws from the RNG), so it never requests a stop and runs to `max-iterations` (paper Observation 6, Fact 5). To keep G/H well-defined, the initial opinions for those operators are drawn from the open interval `]1e-9, 1[` so that all values stay positive.

## Metrics

- `n_occupied_classes` — bins opinions at `1e-4` resolution and counts non-empty bins (the number of surviving distinct opinions; paper Fact 1/3, Observation 1, Fig. 4–7).
- `phase` — consensus (1) / polarization (2) / plurality (≥3), classified from the occupied-class count.
- `mean_opinion`, `variance` — summary statistics of the stabilized profile.
- `consensus_brink` — the smallest ε at which the occupied-class count first reaches 1 (the numerical estimate of ε*; paper Observation 1, Fact 4), computed in the sweep summary.

## Reproducibility & determinism

For a given seed the run is deterministic (same opinion trajectory). For A/G/H/P the run is fully deterministic (no RNG inside the step); for R the RNG stream (engine seed) is fixed by the seed, so R is also reproducible per seed.

## Future extensions (Phase 3)

The design keeps clean extension points for: a network variant (`socsim-net`, ER/WS/BA) where the confidence set becomes `network neighborhood ∩ opinion distance`; a one-shot paper reproduction (`reproduce`, Fig. 3–7); and analytic PAM-levelling checks. None of these are implemented here.

## References

- Hegselmann, R., & Krause, U. (2005). Opinion Dynamics Driven by Various Ways of Averaging. *Computational Economics*, 25, 381–405. DOI: 10.1007/s10614-005-6296-3.
- Hegselmann, R., & Krause, U. (2002). Opinion Dynamics and Bounded Confidence: Models, Analysis and Simulation. *JASSS*, 5(3), 2.

---
*This file was generated by Claude Code.*
