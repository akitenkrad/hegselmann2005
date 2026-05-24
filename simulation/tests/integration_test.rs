//! Hegselmann & Krause (2005) 有界信頼意見力学の統合テスト．
//!
//! `hegselmann_opinion_simulation` ライブラリクレートの公開 API に対して，
//! ・大きな ε → 合意 (占有クラス数 1) への収束
//! ・ε=0 → 意見が不変 (信頼集合が自分のみ)
//! ・算術平均の決定論性 (同一シードで完全再現)
//! ・占有クラス数・相分類の整合
//! を検証する．

use hegselmann_opinion_simulation::config::{Config, StartProfile};
use hegselmann_opinion_simulation::means::MeanOperator;
use hegselmann_opinion_simulation::metrics::{n_occupied_classes, Phase};
use hegselmann_opinion_simulation::simulation::run;

fn base_config(eps: f64, mean: MeanOperator) -> Config {
    Config {
        n: 200,
        eps,
        mean,
        p: 1.0,
        start_profile: StartProfile::Uniform,
        max_iterations: 200,
        tol: 1e-6,
        seed: Some(42),
        output_dir: "results".to_string(),
    }
}

// --------------------------------------------------------------------------- //
// 大きな ε → 合意
// --------------------------------------------------------------------------- //

#[test]
fn large_eps_reaches_consensus() {
    // ε=0.5 ならどの意見も互いに信頼 (|x_i - x_j| ≤ 0.5 が常に成立) → 1 ステップで合意．
    let cfg = base_config(0.5, MeanOperator::Arithmetic);
    let result = run(&cfg);
    let last = result.opinion_history.last().unwrap();
    assert_eq!(
        n_occupied_classes(last),
        1,
        "ε=0.5 では合意 (占有クラス数 1) になるべき"
    );
    assert!(result.converged, "決定論的平均は収束を検知すべき");
}

// --------------------------------------------------------------------------- //
// ε=0 → 意見が不変
// --------------------------------------------------------------------------- //

#[test]
fn zero_eps_leaves_opinions_unchanged() {
    // ε=0 なら信頼集合は自分自身のみ → 算術平均は自分の意見そのもの → 不変．
    let cfg = base_config(0.0, MeanOperator::Arithmetic);
    let result = run(&cfg);
    let initial = &result.opinion_history[0];
    let last = result.opinion_history.last().unwrap();
    assert_eq!(initial.len(), last.len());
    for (a, b) in initial.iter().zip(last.iter()) {
        assert!((a - b).abs() < 1e-12, "ε=0 では意見は変化しないはず");
    }
    // 1 ステップ目で max|Δx| < tol となり即収束する．
    assert!(result.converged);
}

// --------------------------------------------------------------------------- //
// 算術平均の決定論性
// --------------------------------------------------------------------------- //

#[test]
fn arithmetic_mean_is_deterministic() {
    let a = run(&base_config(0.15, MeanOperator::Arithmetic));
    let b = run(&base_config(0.15, MeanOperator::Arithmetic));
    assert_eq!(a.final_iteration, b.final_iteration);
    assert_eq!(a.converged, b.converged);
    let la = a.opinion_history.last().unwrap();
    let lb = b.opinion_history.last().unwrap();
    for (x, y) in la.iter().zip(lb.iter()) {
        assert!(
            (x - y).abs() < 1e-12,
            "同一シードの算術平均は完全再現すべき"
        );
    }
}

// --------------------------------------------------------------------------- //
// 小さな ε → 複数クラスタ (合意しない)
// --------------------------------------------------------------------------- //

#[test]
fn small_eps_leaves_multiple_clusters() {
    let cfg = base_config(0.05, MeanOperator::Arithmetic);
    let result = run(&cfg);
    let last = result.opinion_history.last().unwrap();
    let n_occ = n_occupied_classes(last);
    assert!(
        n_occ >= 2,
        "ε=0.05 では複数のクラスタが残るべき (got {})",
        n_occ
    );
    assert_ne!(Phase::classify(n_occ), Phase::Consensus);
}

// --------------------------------------------------------------------------- //
// ランダム平均 R は収束フラグを立てない
// --------------------------------------------------------------------------- //

#[test]
fn random_mean_never_flags_convergence() {
    let mut cfg = base_config(0.05, MeanOperator::Random);
    cfg.max_iterations = 50;
    let result = run(&cfg);
    assert!(!result.converged, "R は request_stop を発火しない");
    assert_eq!(result.final_iteration, 50, "R は最大反復まで回る");
}
