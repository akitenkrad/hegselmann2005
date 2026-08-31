[English](cli.md) | **日本語**

# CLI

Rust バイナリ `hegselmann` (`cargo run --release -- …` で実行) は `run` と `sweep` の 2 サブコマンドを提供する．

## `run` — 単一実行

単一の `(ε, 平均)` で有界信頼力学を実行する．

```bash
cargo run --release -- run \
    --n 625 --eps 0.15 --mean A --start uniform \
    --max-iterations 100 --tol 1e-6 --seed 42
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--n` | 625 | エージェント数 `n` |
| `--eps` | 0.15 | 対称信頼幅 `ε` |
| `--mean` | A | 平均演算子: `A` / `G` / `H` / `P<p>` (例 `P0.01`, `P100`) / `R`．`P` 単独なら `--p` を使う． |
| `--p` | 1.0 | `--mean P` のべき指数 (`P` 単独時のフォールバック) |
| `--start` | uniform | 初期意見プロファイル (`uniform`) |
| `--max-iterations` | 100 | 最大ステップ数 `T` |
| `--tol` | 1e-6 | 収束許容誤差 (`max|Δx| < tol` で停止; `R` では無視) |
| `--seed` | ランダム | 乱数シード |
| `--output-dir` | results | runvault の results ルート |

各種平均の例:

```bash
cargo run --release -- run --n 625 --eps 0.20 --mean H --seed 42            # 調和平均 (非対称分極)
cargo run --release -- run --n 625 --eps 0.05 --mean P --p 100 --seed 42     # べき平均 P_100
cargo run --release -- run --n 625 --eps 0.05 --mean P0.01 --seed 42         # べき平均 P_0.01
cargo run --release -- run --n 625 --eps 0.03 --mean R --max-iterations 5000 --seed 42  # ランダム平均
```

**出力ファイル:**

実行 1 回が runvault の run ディレクトリ 1 つになる．run ディレクトリが出力先そのものなので，タイムスタンプ付きディレクトリも `latest` シンボリックリンクもこちらでは作らない．最後に完了した run のパスは `runvault` に聞く．

```bash
runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone
```

```
results/
└── hegselmann-averaging/                           ← 実験
    ├── latest_finished -> run_20260831_153702_...  ← 最後に完了した run
    ├── run_20260831_153702_d38d7702_aeaf/          ← <サブコマンド>_<時刻>_<cfg8>_<exec4>
    │   ├── run.json                                ← メタデータ (git commit / 環境 / 論文)
    │   ├── config.json                             ← 封筒．条件は ["parameters"] の下
    │   ├── metrics.csv                             ← long 形式 (step / scope / name / value)
    │   ├── events.jsonl                            ← ステップごとの observation と terminal 行 (相のラベル)
    │   ├── status.json                             ← どう終わったか・所要時間
    │   ├── manifest.csv                            ← artifacts/ と logs/ のハッシュ
    │   └── artifacts/
    │       └── opinions.csv                        ← long-format 意見軌跡: t, agent_id, opinion
    └── figures/                                    ← 可視化スクリプトの出力 (run の外)
        └── run_20260831_153702_d38d7702_aeaf/
```

図は run が終わった後に描くものなので，run ディレクトリの **外** (`<実験>/figures/<run_slug>/`) に置く．`manifest.csv` は `finish()` が確定させるため，後から `artifacts/` に足したファイルはハッシュを持てない．

`metrics.csv` は 1 行 1 値の long 形式である．ステップごとの 4 指標 `n_occupied_classes` / `mean` / `variance` / `max_delta` は `step` を持ち (`step_unit=step`，`scope=run`)，run 全体を 1 つの数で表す `converged` (0.0 / 1.0) と `final_iteration` は `step` を持たない `scope=run` の行として同じファイルに入る．

**相 (phase) は指標にしない．** consensus / polarization / plurality は数ではなくラベルであり，しかも同じ行の `n_occupied_classes` (1 / 2 / 3 以上) から一意に決まる — 数字を割り当てても情報は増えない．最終的な相は `events.jsonl` の `terminal` 行にラベルのまま置く (`"phase": "polarization"`)．同じ行が `outcome` / `censored` / `budget` で収束と打ち切りも表す．条件の表示は [`show-experiment-settings`](visualization.ja.md#show-experiment-settings) を参照．

## `sweep` — 平均をまたいだ ε 走査

ε を走査し，平均演算子ごとに占有クラス数・合意ブリンクを集計する．

```bash
cargo run --release -- sweep \
    --eps-min 0.0 --eps-max 0.40 --eps-step 0.01 \
    --means A,G,H,P0.01,P100,R --n 625 --runs 50 --seed 42
```

| フラグ | 既定値 | 説明 |
|---|---|---|
| `--eps-min` | 0.0 | ε 最小値 |
| `--eps-max` | 0.40 | ε 最大値 (含む) |
| `--eps-step` | 0.01 | ε 刻み |
| `--means` | A,G,H,P0.01,P100,R | カンマ区切りの演算子リスト |
| `--p` | 1.0 | リスト内の `P` 単独指定の指数 |
| `--n` | 625 | エージェント数 |
| `--runs` | 50 | 各 `(平均, ε)` あたりの独立試行数 |
| `--max-iterations` | 100 | 最大ステップ数 |
| `--tol` | 1e-6 | 収束許容誤差 |
| `--seed` | 42 | シード基点 (各試行は独立シードを派生) |
| `--start` | uniform | 初期意見プロファイル |
| `--output-dir` | results | runvault の results ルート |

各試行は `derive_seed(seed, &[hash(平均), eps.bits, run])` で独立シードを派生させるため，試行は再現可能かつ無相関である．

**出力ファイル:**

スイープは **親** run 1 本と，`(平均, ε)` 1 点ごとの **子** run から成る．親の `config.json` はグリッド定義そのものを持ち，条件ごとの指標は持たない．子はその条件の `runs` 本の試行である．

```bash
runvault path --experiment hegselmann-averaging --latest --subcommand sweep
```

```
results/hegselmann-averaging/
├── sweep_20260831_155216_2d267af5_f1b4/     ← 親: グリッド定義 (subcommand=sweep)
└── sweep-point_20260831_155216_..._9194/    ← 子: (平均, ε) 1 点 (subcommand=sweep-point)
    ├── config.json                          ← その条件 (n, eps, mean, p, runs, …)
    ├── metrics.csv                          ← 条件の集約: n_units, n_converged, 各平均
    └── events.jsonl                         ← 試行ごとの observation 1 行 + terminal 1 行
```

子のサブコマンド名は `run` ではなく `sweep-point` である．`run` は 1 本のシミュレーション，子は同一条件の `runs` 本で，中身の違う 2 つを同じ名前に同居させると `runvault path --subcommand run` がどちらを返すか分からなくなる．

試行 1 本は子の `events.jsonl` の `terminal` 行 1 本に対応する (旧 `sweep_summary.csv` の 1 行がそのまま 1 行になる)．行が持つのは `seed`・`t` (最終反復)・`censored` (`converged` の否定)・`budget`・`phase` (ラベル)・`n_occupied_classes`・`mean_opinion`・`variance`・`max_delta` である．試行ごとの値をあえて指標にしないのは，`metrics.csv` に入れると試行間で (`run_uid`, `step`, `scope`, `name`) が重複するためである．したがって子の `metrics.csv` が持つのは条件の集約だけになる (`n_units`, `n_converged`, `convergence_rate`, `mean_n_occupied_classes`, `mean_opinion_mean`, `mean_opinion_variance`, `mean_final_iteration`)．

コマンドは平均ごとの合意ブリンク ε* の推定値 (試行平均の占有クラス数が初めて 1 になる最小 ε) も表示する．これは表示するだけで記録しない — 子の terminal 行からいつでも組み直せる派生量であり，run の中に数字だけ置くと「どの ε 刻みで測ったか」が失われるためである．

---
*This file was generated by Claude Code.*
