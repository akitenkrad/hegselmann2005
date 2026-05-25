//! 初期化と実行ドライバ (SimulationBuilder 配線)．

use std::fs::File;
use std::io::BufWriter;

use csv::Writer;
use rand::Rng;

use socsim_core::{derive_seed, SimRng};
use socsim_engine::{SequentialScheduler, SimulationBuilder};
use socsim_social_dynamics::max_abs_delta;

use crate::config::{Config, StartProfile};
use crate::mechanisms::{ConvergenceMechanism, HegselmannKrauseMechanism};
use crate::metrics::Metrics;
use crate::world::OpinionWorld;

// 単一 root シードから用途別の独立な決定論的 RNG ストリームを派生させるラベル．
/// 初期意見分布の生成用 RNG ラベル．
const RNG_WORLD_INIT: u64 = 0;
/// socsim エンジン (= ランダム平均 R の抽出) 用 RNG ラベル．
const RNG_ENGINE: u64 = 1;

/// 幾何・調和平均で意見を正に保つための下限 (開区間 `]ε_pos, 1[` の左端)．
const POSITIVE_FLOOR: f64 = 1e-9;

/// シミュレーション全体の実行結果．
pub struct SimulationResult {
    /// 各ステップ (t=0 を含む) のメトリクス履歴．
    pub metrics_history: Vec<Metrics>,
    /// 各ステップの意見スナップショット (opinions.csv 用)．`opinions[t][i]`．
    pub opinion_history: Vec<Vec<f64>>,
    /// 決定論的平均が収束したか (R では常に false)．
    pub converged: bool,
    /// 収束 (または最終) 反復番号．
    pub final_iteration: usize,
}

/// 初期意見ベクトルを生成する．
///
/// `[0,1]` 上の一様乱数を引く．幾何・調和平均では数値安定のため開区間
/// `]POSITIVE_FLOOR, 1[` から引き，意見が常に正であることを保証する．
pub fn init_opinions(cfg: &Config, rng: &mut SimRng) -> Vec<f64> {
    let (lo, hi) = if cfg.mean.requires_positive() {
        (POSITIVE_FLOOR, 1.0)
    } else {
        (0.0, 1.0)
    };
    match cfg.start_profile {
        StartProfile::Uniform => (0..cfg.n).map(|_| rng.gen_range(lo..hi)).collect(),
    }
}

/// シミュレーションを実行する．
///
/// socsim の [`Simulation`](socsim_engine::Simulation) エンジンを駆動する．
/// 更新規則は `socsim-social-dynamics` パックの [`HegselmannKrauseMechanism`] が
/// `Interaction` フェーズで同期適用し，活性化順序は [`SequentialScheduler`] が
/// id 昇順で与える (同期更新なので順序は結果に無関係)．早期停止は決定論的平均
/// (A/G/H/P) のときのみパックの [`ConvergenceMechanism`] を `PostStep` フェーズに
/// 配線して `max|Δx| < tol` で `request_stop` する．ランダム平均 R は収束判定を
/// 使わず最大反復まで回す．
///
/// `max_delta` (および収束フラグ) はメカニズムではなくドライバ側で，観測した連続
/// ステップの意見スナップショット間 [`max_abs_delta`] として算出する (id 昇順の
/// 要素差なので旧ローカル実装とビット等価)．
pub fn run(cfg: &Config) -> SimulationResult {
    let root = cfg.seed.unwrap_or_else(rand::random);

    // 初期意見分布 (root から派生した init RNG)．
    let mut init_rng = SimRng::from_seed(derive_seed(root, &[RNG_WORLD_INIT]));
    let opinions = init_opinions(cfg, &mut init_rng);

    // 世界状態とエンジンを構築 (engine RNG = ランダム平均 R の抽出ストリーム)．
    let world = OpinionWorld::new(
        opinions.clone(),
        cfg.eps,
        cfg.mean,
        cfg.max_iterations as u64,
    );
    let mut builder = SimulationBuilder::new(world)
        .scheduler(Box::new(SequentialScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(Box::new(HegselmannKrauseMechanism::new(cfg.eps, cfg.mean)));
    // 決定論的平均のみ収束で早期停止する (R は最大反復まで回す)．
    if cfg.mean.is_deterministic() {
        builder = builder.add_mechanism(Box::new(ConvergenceMechanism::new(cfg.tol)));
    }
    let mut sim = builder.build();

    let mut metrics_history: Vec<Metrics> = Vec::new();
    let mut opinion_history: Vec<Vec<f64>> = Vec::new();

    // 初期状態 (t=0) を記録．
    metrics_history.push(Metrics::compute(&opinions, 0, 0.0));
    opinion_history.push(opinions);

    let mut converged = false;
    let mut final_iteration = cfg.max_iterations;
    let deterministic = cfg.mean.is_deterministic();

    sim.run_observed(|report| {
        let t = report.t as usize;
        // 連続ステップ間の最大変位 (id 昇順の要素差; 旧 BoundedConfidenceUpdate の
        // ステップ内 max|x_new − x_old| とビット等価)．
        let prev = opinion_history
            .last()
            .expect("opinion_history は t=0 を含む");
        let max_delta = max_abs_delta(prev, &report.world.opinions);

        metrics_history.push(Metrics::compute(&report.world.opinions, t, max_delta));
        opinion_history.push(report.world.opinions.clone());

        // 決定論的平均が不動点に到達したか (R では常に false)．
        converged = deterministic && max_delta < cfg.tol;
        final_iteration = t;
    })
    .expect("シミュレーションの実行に失敗");

    SimulationResult {
        metrics_history,
        opinion_history,
        converged,
        final_iteration,
    }
}

/// 意見履歴を long-format CSV (t, agent_id, opinion) に保存する．
pub fn save_opinions(opinion_history: &[Vec<f64>], output_dir: &str) {
    let path = format!("{}/opinions.csv", output_dir);
    let file = File::create(&path).expect("opinions.csv の作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    wtr.write_record(["t", "agent_id", "opinion"])
        .expect("ヘッダ書き込みに失敗");
    for (t, opinions) in opinion_history.iter().enumerate() {
        for (i, &x) in opinions.iter().enumerate() {
            wtr.write_record(&[t.to_string(), i.to_string(), format!("{:.10}", x)])
                .expect("レコード書き込みに失敗");
        }
    }
    wtr.flush().expect("フラッシュに失敗");
}

/// メトリクス履歴を CSV に保存する．
///
/// 書き出し機構は `socsim_results::write_csv` に委譲する (各行を `serialize` し
/// 先頭行にヘッダを書く csv クレットの標準挙動; 従来の手書き writer とバイト等価)．
/// 行構造体 [`Metrics`] は repo 固有のままで，writer だけを共有化する．
pub fn save_metrics(metrics: &[Metrics], output_dir: &str) {
    let path = format!("{}/metrics.csv", output_dir);
    socsim_results::write_csv(metrics, &path).expect("metrics.csv の書き込みに失敗");
}

/// 出力ディレクトリを作成する．
pub fn ensure_output_dir(output_dir: &str) {
    socsim_results::ensure_dir(output_dir).expect("出力ディレクトリの作成に失敗");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::means::MeanOperator;

    fn test_config() -> Config {
        Config {
            n: 200,
            eps: 0.15,
            mean: MeanOperator::Arithmetic,
            p: 1.0,
            start_profile: StartProfile::Uniform,
            max_iterations: 100,
            tol: 1e-6,
            seed: Some(42),
            output_dir: "results".to_string(),
        }
    }

    #[test]
    fn same_seed_is_deterministic() {
        let a = run(&test_config());
        let b = run(&test_config());
        assert_eq!(a.final_iteration, b.final_iteration);
        assert_eq!(a.converged, b.converged);
        let la = a.opinion_history.last().unwrap();
        let lb = b.opinion_history.last().unwrap();
        for (x, y) in la.iter().zip(lb.iter()) {
            assert!((x - y).abs() < 1e-12);
        }
    }

    #[test]
    fn initial_metrics_recorded_at_t0() {
        let r = run(&test_config());
        assert_eq!(r.metrics_history[0].t, 0);
        assert_eq!(r.opinion_history[0].len(), 200);
    }
}
