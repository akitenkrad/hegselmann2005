//! Hegselmann & Krause (2005) "Opinion Dynamics Driven by Various Ways of
//! Averaging" — 再現実験の CLI エントリポイント．
//!
//! `run`   : 単一の (ε, 平均) での意見力学を実行する．
//! `sweep` : ε を走査し，平均ごとに占有クラス数・合意ブリンクを集計する．

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use chrono::Local;
use clap::{Parser, Subcommand};
use csv::Writer;

use hegselmann_opinion_simulation::config::{parse_start_profile, Config};
use hegselmann_opinion_simulation::means::{parse_mean, MeanOperator};
use hegselmann_opinion_simulation::metrics::{consensus_brink, Phase};
use hegselmann_opinion_simulation::simulation::{
    ensure_output_dir, run, save_metrics, save_opinions,
};

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

/// `sweep_summary.csv` の 1 行 ((mean, eps, run) → 最終メトリクス)．
#[derive(serde::Serialize)]
struct SweepRow {
    mean: String,
    eps: f64,
    run: usize,
    seed: u64,
    converged: bool,
    final_iteration: usize,
    n_occupied_classes: usize,
    mean_opinion: f64,
    variance: f64,
    phase: u8,
    max_delta: f64,
}

/// `sweep_config.json` の構造体．
#[derive(serde::Serialize)]
struct SweepConfigJson {
    command: &'static str,
    eps_min: f64,
    eps_max: f64,
    eps_step: f64,
    means: Vec<String>,
    n: usize,
    runs: usize,
    max_iterations: usize,
    tol: f64,
    seed: u64,
    start_profile: String,
}

/// latest シンボリックリンクを (再) 作成する．
fn refresh_latest(output_dir: &str, target: &str) {
    let symlink_path = Path::new(output_dir).join("latest");
    if symlink_path.is_symlink() {
        let _ = fs::remove_file(&symlink_path);
    }
    #[cfg(unix)]
    {
        let _ = std::os::unix::fs::symlink(target, &symlink_path);
    }
}

// ---------------------------------------------------------------------------
// run
// ---------------------------------------------------------------------------

fn cmd_run(args: RunArgs) {
    let mean = parse_mean(&args.mean, args.p).unwrap_or_else(|e| panic!("{}", e));
    let start_profile = parse_start_profile(&args.start).unwrap_or_else(|e| panic!("{}", e));

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let output_dir = format!("{}/{}", args.output_dir, timestamp);

    let p = match mean {
        MeanOperator::Power(p) => p,
        _ => args.p,
    };

    let cfg = Config {
        n: args.n,
        eps: args.eps,
        mean,
        p,
        start_profile,
        max_iterations: args.max_iterations,
        tol: args.tol,
        seed: args.seed,
        output_dir: output_dir.clone(),
    };

    ensure_output_dir(&cfg.output_dir);

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
    println!("シード: {:?}", cfg.seed);
    println!("出力先: {}", cfg.output_dir);
    println!("-------------------------------------------");

    let result = run(&cfg);
    save_metrics(&result.metrics_history, &cfg.output_dir);
    save_opinions(&result.opinion_history, &cfg.output_dir);

    // config.json
    {
        let path = format!("{}/config.json", cfg.output_dir);
        let file = File::create(&path).expect("config.json の作成に失敗");
        serde_json::to_writer_pretty(BufWriter::new(file), &cfg.to_run_config_json())
            .expect("config.json の書き込みに失敗");
    }

    refresh_latest(&args.output_dir, &timestamp);

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
    println!("意見軌跡 → {}/opinions.csv", cfg.output_dir);
    println!("メトリクス → {}/metrics.csv", cfg.output_dir);
    println!("設定       → {}/config.json", cfg.output_dir);
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

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let sweep_dir = format!("{}/{}_sweep", args.output_dir, timestamp);
    fs::create_dir_all(&sweep_dir).expect("sweep ディレクトリの作成に失敗");

    let n_total = means.len() * epss.len() * args.runs;

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
    println!("出力先: {}", sweep_dir);
    println!("---------------------------------------------------");

    let mut summary_rows: Vec<SweepRow> = Vec::with_capacity(n_total);
    let mut done = 0usize;

    for mean in &means {
        for &eps in &epss {
            for run_idx in 0..args.runs {
                // 各 (mean, eps, run) に独立なシードを派生させる (explicit identity)．
                let seed = socsim_core::derive_seed(
                    args.seed,
                    &[mean_label_hash(mean), eps.to_bits(), run_idx as u64],
                );

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
                    output_dir: sweep_dir.clone(),
                };

                let result = run(&cfg);
                let last = result.metrics_history.last().unwrap();

                summary_rows.push(SweepRow {
                    mean: mean.label(),
                    eps,
                    run: run_idx,
                    seed,
                    converged: result.converged,
                    final_iteration: result.final_iteration,
                    n_occupied_classes: last.n_occupied_classes,
                    mean_opinion: last.mean,
                    variance: last.variance,
                    phase: last.phase,
                    max_delta: last.max_delta,
                });

                done += 1;
            }
            println!(
                "[{}/{}] 平均={} ε={:.4} 完了 ({} 試行)",
                done,
                n_total,
                mean.label(),
                eps,
                args.runs,
            );
        }
    }

    // sweep_summary.csv
    {
        let path = format!("{}/sweep_summary.csv", sweep_dir);
        let file = File::create(&path).expect("sweep_summary.csv の作成に失敗");
        let mut wtr = Writer::from_writer(BufWriter::new(file));
        for row in &summary_rows {
            wtr.serialize(row).expect("サマリ行の書き込みに失敗");
        }
        wtr.flush().expect("フラッシュに失敗");
    }

    // sweep_config.json
    {
        let config_json = SweepConfigJson {
            command: "sweep",
            eps_min: args.eps_min,
            eps_max: args.eps_max,
            eps_step: args.eps_step,
            means: mean_specs.clone(),
            n: args.n,
            runs: args.runs,
            max_iterations: args.max_iterations,
            tol: args.tol,
            seed: args.seed,
            start_profile: start_profile.label().to_string(),
        };
        let path = format!("{}/sweep_config.json", sweep_dir);
        let file = File::create(&path).expect("sweep_config.json の作成に失敗");
        serde_json::to_writer_pretty(BufWriter::new(file), &config_json)
            .expect("sweep_config.json の書き込みに失敗");
    }

    refresh_latest(&args.output_dir, &format!("{}_sweep", timestamp));

    // 合意ブリンクを平均ごとに推定して表示する (試行平均の占有クラス数を使う)．
    println!("===================================================");
    println!("スイープ完了: {} 実行", n_total);
    println!("---------------------------------------------------");
    println!("合意ブリンク ε* (試行平均占有クラス数が初めて 1 に到達する最小 ε):");
    for mean in &means {
        let label = mean.label();
        // (eps, 平均占有クラス数) を構築．
        let mut per_eps: Vec<(f64, usize)> = Vec::new();
        for &eps in &epss {
            let rows: Vec<&SweepRow> = summary_rows
                .iter()
                .filter(|r| r.mean == label && (r.eps - eps).abs() < 1e-12)
                .collect();
            if rows.is_empty() {
                continue;
            }
            let avg =
                rows.iter().map(|r| r.n_occupied_classes).sum::<usize>() as f64 / rows.len() as f64;
            per_eps.push((eps, avg.round() as usize));
        }
        match consensus_brink(&per_eps) {
            Some(b) => println!("  {:<6} → ε* ≈ {:.4}", label, b),
            None => println!(
                "  {:<6} → ε* 未到達 (ε_max={} まで合意なし)",
                label, args.eps_max
            ),
        }
    }
    println!("---------------------------------------------------");
    println!("サマリ → {}/sweep_summary.csv", sweep_dir);
    println!("設定   → {}/sweep_config.json", sweep_dir);
}

/// 平均ラベルを u64 にハッシュして派生シードのラベルに使う (explicit identity)．
fn mean_label_hash(mean: &MeanOperator) -> u64 {
    let label = mean.label();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in label.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
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
