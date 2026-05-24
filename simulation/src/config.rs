//! シミュレーション設定．

use serde::Serialize;

use crate::means::MeanOperator;

/// 初期意見プロファイルの生成方法．
///
/// 現状は一様乱数のみだが，将来の拡張 (二峰性・極端分布など) のために enum 化する．
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartProfile {
    /// `[0,1]` 上の一様乱数 (調和・幾何平均では開区間 `]0,1[`)．
    Uniform,
}

impl StartProfile {
    pub fn label(&self) -> &'static str {
        match self {
            StartProfile::Uniform => "uniform",
        }
    }
}

/// 文字列から `StartProfile` をパースする．
pub fn parse_start_profile(s: &str) -> Result<StartProfile, String> {
    match s.trim() {
        "uniform" => Ok(StartProfile::Uniform),
        _ => Err(format!("不正な初期分布: \"{}\" (uniform のみ対応)", s)),
    }
}

/// 単一実行の設定．
#[derive(Debug, Clone)]
pub struct Config {
    /// エージェント数 n．
    pub n: usize,
    /// 対称信頼幅 ε．
    pub eps: f64,
    /// 平均化操作 (A / G / H / P{p} / R)．
    pub mean: MeanOperator,
    /// べき平均の指数 p (mean が Power のときのログ用に保持)．
    pub p: f64,
    /// 初期意見プロファイル．
    pub start_profile: StartProfile,
    /// 最大反復回数 T．
    pub max_iterations: usize,
    /// 収束判定の許容誤差 (max|Δx| < tol で停止; R では使わない)．
    pub tol: f64,
    /// 乱数シード (None の場合はランダム)．
    pub seed: Option<u64>,
    /// 結果出力ディレクトリ．
    pub output_dir: String,
}

impl Default for Config {
    /// 論文 §3 に近い標準設定 (n=625, ε=0.15, 算術平均)．
    fn default() -> Self {
        Config {
            n: 625,
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
}

/// `config.json` (run 用) のシリアライズ表現．
#[derive(Serialize)]
pub struct RunConfigJson {
    pub command: &'static str,
    pub n: usize,
    pub eps: f64,
    pub mean: String,
    pub p: Option<f64>,
    pub start_profile: &'static str,
    pub max_iterations: usize,
    pub tol: f64,
    pub seed: Option<u64>,
    pub output_dir: String,
}

impl Config {
    /// `config.json` 用の表現を組み立てる．
    pub fn to_run_config_json(&self) -> RunConfigJson {
        let p = match self.mean {
            MeanOperator::Power(p) => Some(p),
            _ => None,
        };
        RunConfigJson {
            command: "run",
            n: self.n,
            eps: self.eps,
            mean: self.mean.label(),
            p,
            start_profile: self.start_profile.label(),
            max_iterations: self.max_iterations,
            tol: self.tol,
            seed: self.seed,
            output_dir: self.output_dir.clone(),
        }
    }
}
