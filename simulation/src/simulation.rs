//! 初期化と実行ドライバ (SimulationBuilder 配線)．

use std::fs::{self, File};
use std::io::BufWriter;

use csv::Writer;
use rand::Rng;

use socsim_core::{derive_seed, SimRng};
use socsim_engine::{SequentialScheduler, SimulationBuilder};

use crate::config::{Config, StartProfile};
use crate::mechanisms::BoundedConfidenceUpdate;
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
/// 更新規則は [`BoundedConfidenceUpdate`] が `Interaction` フェーズで同期適用し，
/// 活性化順序は [`SequentialScheduler`] が id 昇順で与える (同期更新なので順序は
/// 結果に無関係)．早期停止は決定論的平均が `max|Δx| < tol` で `request_stop`
/// する．ランダム平均 R は収束判定を使わず最大反復まで回す．
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
    let mut sim = SimulationBuilder::new(world)
        .scheduler(Box::new(SequentialScheduler))
        .seed(derive_seed(root, &[RNG_ENGINE]))
        .add_mechanism(Box::new(BoundedConfidenceUpdate { tol: cfg.tol }))
        .build();

    let mut metrics_history: Vec<Metrics> = Vec::new();
    let mut opinion_history: Vec<Vec<f64>> = Vec::new();

    // 初期状態 (t=0) を記録．
    metrics_history.push(Metrics::compute(&opinions, 0, 0.0));
    opinion_history.push(opinions);

    let mut converged = false;
    let mut final_iteration = cfg.max_iterations;

    sim.run_observed(|report| {
        let t = report.t as usize;
        let max_delta = *report
            .scratch
            .get::<f64>("max_delta")
            .expect("max_delta が scratch に存在しません");
        let step_converged = *report
            .scratch
            .get::<bool>("converged")
            .expect("converged が scratch に存在しません");

        metrics_history.push(Metrics::compute(&report.world.opinions, t, max_delta));
        opinion_history.push(report.world.opinions.clone());

        converged = step_converged;
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
pub fn save_metrics(metrics: &[Metrics], output_dir: &str) {
    let path = format!("{}/metrics.csv", output_dir);
    let file = File::create(&path).expect("metrics.csv の作成に失敗");
    let mut wtr = Writer::from_writer(BufWriter::new(file));
    for m in metrics {
        wtr.serialize(m).expect("メトリクス書き込みに失敗");
    }
    wtr.flush().expect("フラッシュに失敗");
}

/// 出力ディレクトリを作成する．
pub fn ensure_output_dir(output_dir: &str) {
    fs::create_dir_all(output_dir).expect("出力ディレクトリの作成に失敗");
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
