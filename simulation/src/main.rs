//! Hegselmann & Krause (2005) "Opinion Dynamics Driven by Various Ways of
//! Averaging" — 再現実験の CLI エントリポイント．
//!
//! `run`   : 単一の (ε, 平均) での意見力学を実行する．
//! `sweep` : ε と平均を走査し，条件 1 点ごとに子 run を起こして `runs` 本の
//!           試行を回す．
//!
//! 出力の置き場と同一性は runvault が持つ．タイムスタンプ付きディレクトリも
//! `latest` シンボリックリンクもこちらでは作らず，`Run::start` が決めた run
//! ディレクトリへ書く．

use clap::{Parser, Subcommand};
use runvault::{Lineage, Run, RunOptions};
use serde::Serialize;

use hegselmann_opinion_simulation::config::{parse_start_profile, Config};
use hegselmann_opinion_simulation::means::{parse_mean, MeanOperator};
use hegselmann_opinion_simulation::metrics::{consensus_brink, Phase};
use hegselmann_opinion_simulation::record::{self, DOMAIN, EXPERIMENT, REPO_ID};
use hegselmann_opinion_simulation::simulation::{run, save_opinions};

// ---------------------------------------------------------------------------
// CLI 定義
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "hegselmann",
    about = "Hegselmann & Krause (2005) Opinion Dynamics Driven by Various Ways of Averaging — 再現実験"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 単一の (ε, 平均) で意見力学を実行する．
    Run(RunArgs),
    /// ε を走査し，平均ごとに占有クラス数・合意ブリンクを集計する．
    Sweep(SweepArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// エージェント数 n．
    #[arg(long, default_value_t = 625)]
    n: usize,

    /// 対称信頼幅 ε．
    #[arg(long, default_value_t = 0.15)]
    eps: f64,

    /// 平均化操作: A / G / H / P<p> (例 P0.01, P100) / R．"P" 単独なら --p を使う．
    #[arg(long, default_value = "A")]
    mean: String,

    /// べき平均の指数 p (--mean P または --mean PA<p> の補完値)．
    #[arg(long, default_value_t = 1.0)]
    p: f64,

    /// 初期意見プロファイル (uniform)．
    #[arg(long, default_value = "uniform")]
    start: String,

    /// 最大反復回数 T．
    #[arg(long, default_value_t = 100)]
    max_iterations: usize,

    /// 収束判定の許容誤差 (max|Δx| < tol; R では無視)．
    #[arg(long, default_value_t = 1e-6)]
    tol: f64,

    /// 乱数シード (省略時はランダム)．
    #[arg(long)]
    seed: Option<u64>,

    /// 結果出力ディレクトリ．
    #[arg(long, default_value = "results")]
    output_dir: String,
}

#[derive(Parser, Debug)]
struct SweepArgs {
    /// ε 走査の最小値．
    #[arg(long, default_value_t = 0.0)]
    eps_min: f64,

    /// ε 走査の最大値 (含む)．
    #[arg(long, default_value_t = 0.40)]
    eps_max: f64,

    /// ε 走査の刻み幅．
    #[arg(long, default_value_t = 0.01)]
    eps_step: f64,

    /// カンマ区切りの平均リスト (例 "A,G,H,P0.01,P100,R")．
    #[arg(long, default_value = "A,G,H,P0.01,P100,R")]
    means: String,

    /// べき平均 "P" 単独指定時の指数 p (リスト内で P<p> を使う場合は不要)．
    #[arg(long, default_value_t = 1.0)]
    p: f64,

    /// エージェント数 n．
    #[arg(long, default_value_t = 625)]
    n: usize,

    /// 各 (平均, ε) 条件あたりの独立試行数．
    #[arg(long, default_value_t = 50)]
    runs: usize,

    /// 最大反復回数 T．
    #[arg(long, default_value_t = 100)]
    max_iterations: usize,

    /// 収束判定の許容誤差．
    #[arg(long, default_value_t = 1e-6)]
    tol: f64,

    /// 乱数シード基点 (各試行は derive により独立化する)．
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// 初期意見プロファイル (uniform)．
    #[arg(long, default_value = "uniform")]
    start: String,

    /// 結果出力ベースディレクトリ．
    #[arg(long, default_value = "results")]
    output_dir: String,
}

// ---------------------------------------------------------------------------
// 補助
// ---------------------------------------------------------------------------

/// 小数点以下の桁数を文字列表現から推定する．
fn step_decimals(v: f64) -> usize {
    let s = format!("{}", v);
    match s.find('.') {
        Some(pos) => s.len() - pos - 1,
        None => 0,
    }
}

/// `eps_min..=eps_max` を `eps_step` 刻みの等差数列に展開する (浮動小数点誤差を丸める)．
fn eps_range(eps_min: f64, eps_max: f64, eps_step: f64) -> Vec<f64> {
    assert!(eps_step > 0.0, "eps-step は正でなければなりません");
    let n_steps = ((eps_max - eps_min) / eps_step + 0.5e-9).floor() as usize;
    let decimals = step_decimals(eps_step);
    let factor = 10_f64.powi(decimals as i32);
    (0..=n_steps)
        .map(|i| ((eps_min + eps_step * i as f64) * factor).round() / factor)
        .collect()
}

/// スイープ親 run の実験条件 (グリッド定義そのもの)．
#[derive(Serialize)]
struct SweepParameters {
    eps_min: f64,
    eps_max: f64,
    eps_step: f64,
    means: Vec<String>,
    n: usize,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
    start_profile: &'static str,
}

/// スイープの子 run ((平均, ε) 1 点) の実験条件．
///
/// `run` の条件に `runs` が付いた形で，`run` とは別のサブコマンド名を持つ．
/// 同じ `run` を名乗らせると，「1 本のシミュレーション」と「同一条件の
/// `runs` 本」という中身の違う 2 つが 1 つの名前に同居し，`runvault path
/// --subcommand run` がどちらを返すか分からなくなる．
#[derive(Serialize)]
struct SweepPointParameters {
    n: usize,
    eps: f64,
    mean: String,
    /// べき平均 `P_p` の指数．他の平均では意味を持たないので `None`．
    p: Option<f64>,
    start_profile: &'static str,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    let mean = parse_mean(&args.mean, args.p).unwrap_or_else(|e| panic!("{}", e));
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    // シードを実体化してから記録する．--seed 省略時にシミュレーション側で
    // rand::random に落とすと，実際に使われたシードがどこにも残らない．
    let seed = args.seed.unwrap_or_else(rand::random::<u64>);

    let p = match mean {
        MeanOperator::Power(p) => p,
        _ => args.p,
    };

    // 出力先は Run::start が run ディレクトリを決めた後に確定する．
    let mut cfg = Config {
        n: args.n,
        eps: args.eps,
        mean,
        p,
        start_profile,
        max_iterations: args.max_iterations,
        tol: args.tol,
        seed: Some(seed),
        output_dir: String::new(),
    };

    let parameters = cfg.to_parameters(seed);
    let mut rv = Run::start(
        RunOptions::new(EXPERIMENT, "run")
            .repo_id(REPO_ID)
            .domain(DOMAIN)
            .results_root(&args.output_dir)
            .parameters(&parameters)
            .expect("runvault: parameters の組み立てに失敗")
            .seed_pointers(["/seed"])
            .master_seed(seed)
            .replication(record::replication()),
    )
    .expect("runvault: run の開始に失敗");

    // run ディレクトリが出力先そのものになる．意見の軌跡は artifacts/ の下へ．
    cfg.output_dir = rv.dir().join("artifacts").to_string_lossy().into_owned();

    println!("=== Hegselmann-Krause 意見力学 再現実験 ===");
    println!(
        "n: {} | ε: {} | 平均: {} | 初期分布: {} | max_iter: {} | tol: {}",
        cfg.n,
        cfg.eps,
        cfg.mean.label(),
        cfg.start_profile.label(),
        cfg.max_iterations,
        cfg.tol,
    );
    println!("シード: {}", seed);
    println!("出力先: {}", rv.dir().display());
    println!("-------------------------------------------");

    let result = run(&cfg);
    save_opinions(&result.opinion_history, &cfg.output_dir);
    record::log_simulation(&mut rv, &result);
    // run は全ステップを観測して metrics.csv に残しているので，観測時刻も全ステップ．
    let observed: Vec<u64> = result.metrics_history.iter().map(|m| m.t as u64).collect();
    record::log_terminal(&mut rv, "run", seed, cfg.max_iterations, observed, &result);

    let last = result.metrics_history.last().unwrap();
    let phase = Phase::classify(last.n_occupied_classes);
    println!(
        "収束: {} | 反復回数: {}",
        if result.converged { "Yes" } else { "No" },
        result.final_iteration
    );
    println!(
        "占有クラス数: {} | 相: {} | 平均意見: {:.4} | 分散: {:.4e}",
        last.n_occupied_classes,
        phase.label(),
        last.mean,
        last.variance,
    );

    let dir = rv.finish().expect("runvault: run の完了に失敗");
    println!("意見軌跡 → {}/artifacts/opinions.csv", dir.display());
    println!("メトリクス → {}/metrics.csv", dir.display());
    println!("終端の相   → {}/events.jsonl", dir.display());
    println!("設定       → {}/config.json", dir.display());
}

// ---------------------------------------------------------------------------
// sweep
// ---------------------------------------------------------------------------

fn cmd_sweep(args: SweepArgs) {
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    let mean_specs: Vec<String> = args
        .means
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let means: Vec<MeanOperator> = mean_specs
        .iter()
        .map(|s| parse_mean(s, args.p).unwrap_or_else(|e| panic!("{}", e)))
        .collect();

    let epss = eps_range(args.eps_min, args.eps_max, args.eps_step);
    let n_total = means.len() * epss.len() * args.runs;

    let sweep_parameters = SweepParameters {
        eps_min: args.eps_min,
        eps_max: args.eps_max,
        eps_step: args.eps_step,
        means: mean_specs.clone(),
        n: args.n,
        runs: args.runs,
        max_iterations: args.max_iterations,
        tol: args.tol,
        seed: args.seed,
        start_profile: start_profile.label(),
    };

    // 親 run: (平均, ε) のグリッド定義そのものを parameters に持つ．個別条件の
    // 指標は書かない．親は 1 本のシミュレーションではないので master_seed を
    // 名乗らず，base seed は /parameters.seed と seed_pointers 経由で
    // execution_hash に残る．sweep_id は runvault が親の run_slug で埋める．
    let parent = Run::start(
        RunOptions::new(EXPERIMENT, "sweep")
            .repo_id(REPO_ID)
            .domain(DOMAIN)
            .results_root(&args.output_dir)
            .parameters(&sweep_parameters)
            .expect("runvault: sweep の parameters の組み立てに失敗")
            .seed_pointers(["/seed"])
            .sweep_parent()
            .replication(record::replication()),
    )
    .expect("runvault: sweep 親 run の開始に失敗");

    let sweep_id = parent
        .sweep_id()
        .expect("runvault: sweep 親に sweep_id がありません")
        .to_string();
    let parent_run_uid = parent.run_uid().to_string();

    println!("=== Hegselmann-Krause 意見力学 パラメータスイープ ===");
    println!(
        "n: {} | 平均: {} 種 | ε: {} 値 ({}..={}, step {}) | 試行: {} | 合計: {} 実行",
        args.n,
        means.len(),
        epss.len(),
        args.eps_min,
        args.eps_max,
        args.eps_step,
        args.runs,
        n_total,
    );
    println!("シード (base): {}", args.seed);
    println!("出力先: {}", parent.dir().display());
    println!("---------------------------------------------------");

    // 合意ブリンクの推定に使う (平均, ε) ごとの試行平均占有クラス数．
    let mut per_mean_eps: Vec<(String, f64, f64)> = Vec::with_capacity(means.len() * epss.len());
    let mut done = 0usize;

    for mean in &means {
        for &eps in &epss {
            let params = SweepPointParameters {
                n: args.n,
                eps,
                mean: mean.label(),
                p: match mean {
                    MeanOperator::Power(p) => Some(*p),
                    _ => None,
                },
                start_profile: start_profile.label(),
                runs: args.runs,
                max_iterations: args.max_iterations,
                tol: args.tol,
                seed: args.seed,
            };

            // 子は「その (平均, ε) の試行群」そのもの．master_seed は親と同じ
            // base で，条件が違えば config_hash が違うので run としては別物になる．
            // 同じ条件の繰り返しは無いので replicate_index は 0．
            let mut child = Run::start(
                RunOptions::new(EXPERIMENT, "sweep-point")
                    .repo_id(REPO_ID)
                    .domain(DOMAIN)
                    .results_root(&args.output_dir)
                    .parameters(&params)
                    .expect("runvault: 子 run の parameters の組み立てに失敗")
                    .seed_pointers(["/seed"])
                    .master_seed(args.seed)
                    .replicate_index(0)
                    .lineage(Lineage {
                        sweep_id: Some(sweep_id.clone()),
                        parent_run_uid: Some(parent_run_uid.clone()),
                        ..Default::default()
                    })
                    .replication(record::replication()),
            )
            .expect("runvault: 子 run の開始に失敗");

            let mut trials: Vec<record::TrialOutcome> = Vec::with_capacity(args.runs);
            for run_idx in 0..args.runs {
                // 各 (mean, eps, run) に独立なシードを派生させる (explicit identity)．
                let seed = record::trial_seed(args.seed, mean, eps, run_idx);

                let cfg = Config {
                    n: args.n,
                    eps,
                    mean: *mean,
                    p: match mean {
                        MeanOperator::Power(p) => *p,
                        _ => args.p,
                    },
                    start_profile,
                    max_iterations: args.max_iterations,
                    tol: args.tol,
                    seed: Some(seed),
                    output_dir: String::new(),
                };

                let result = run(&cfg);
                // sweep が見るのは各試行の最終ステップだけなので，観測時刻もそこ 1 点．
                record::log_terminal(
                    &mut child,
                    &format!("trial-{run_idx}"),
                    seed,
                    args.max_iterations,
                    [result.final_iteration as u64],
                    &result,
                );
                trials.push(record::TrialOutcome::from_result(&result));

                done += 1;
            }
            record::log_condition_summary(&mut child, &trials);

            let mean_n_occupied = trials
                .iter()
                .map(|t| t.n_occupied_classes as f64)
                .sum::<f64>()
                / trials.len() as f64;
            per_mean_eps.push((mean.label(), eps, mean_n_occupied));

            child.finish().expect("runvault: 子 run の完了に失敗");

            println!(
                "[{}/{}] 平均={} ε={:.4} 完了 ({} 試行) → 平均占有クラス数={:.2}",
                done,
                n_total,
                mean.label(),
                eps,
                args.runs,
                mean_n_occupied,
            );
        }
    }

    let dir = parent
        .finish()
        .expect("runvault: sweep 親 run の完了に失敗");

    // 合意ブリンクを平均ごとに推定して表示する (試行平均の占有クラス数を使う)．
    // 推定値そのものは記録しない — 子 run の終端イベントから同じ手順でいつでも
    // 組み直せる派生量であり，run の中に置くと «どの ε 刻みで測ったか» を失った
    // 数字だけが残る．
    println!("===================================================");
    println!("スイープ完了: {} 実行", n_total);
    println!("---------------------------------------------------");
    println!("合意ブリンク ε* (試行平均占有クラス数が初めて 1 に到達する最小 ε):");
    for mean in &means {
        let label = mean.label();
        let per_eps: Vec<(f64, usize)> = per_mean_eps
            .iter()
            .filter(|(m, _, _)| *m == label)
            .map(|(_, eps, avg)| (*eps, avg.round() as usize))
            .collect();
        match consensus_brink(&per_eps) {
            Some(b) => println!("  {:<6} → ε* ≈ {:.4}", label, b),
            None => println!(
                "  {:<6} → ε* 未到達 (ε_max={} まで合意なし)",
                label, args.eps_max
            ),
        }
    }
    println!("---------------------------------------------------");
    println!("スイープ定義 → {}/config.json", dir.display());
    println!("各条件の試行は子 run (subcommand=sweep-point) の events.jsonl にあります");
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run(args) => cmd_run(args),
        Commands::Sweep(args) => cmd_sweep(args),
    }
}
