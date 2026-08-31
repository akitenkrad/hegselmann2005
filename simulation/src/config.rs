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

/// `run` の実験条件 (runvault の `config.json` の `parameters` に入る)．
#[derive(Serialize)]
pub struct RunParameters {
    pub n: usize,
    pub eps: f64,
    pub mean: String,
    /// べき平均 `P_p` の指数．他の平均では意味を持たないので `None`
    /// (0 で埋めると「p=0 の P」= 幾何平均という別の条件に見える)．
    pub p: Option<f64>,
    pub start_profile: &'static str,
    pub max_iterations: usize,
    pub tol: f64,
    pub seed: u64,
}

impl Config {
    /// runvault の `config.json` に入れる実験条件を組み立てる．
    ///
    /// 出力先は run ディレクトリそのものなので条件ではない (旧 `config.json` が
    /// 持っていた `output_dir` / `command` は runvault 側の `run.json` に
    /// `subcommand` として入るため，ここからは落とす)．
    ///
    /// `seed` は `Option` ではなく実体化した値を受け取る．`--seed` 省略時に
    /// シミュレーション側で `rand::random` に落とすと，実際に使われたシードが
    /// どこにも残らないため，呼び出し側が先に確定させる．
    pub fn to_parameters(&self, seed: u64) -> RunParameters {
        RunParameters {
            n: self.n,
            eps: self.eps,
            mean: self.mean.label(),
            p: match self.mean {
                MeanOperator::Power(p) => Some(p),
                _ => None,
            },
            start_profile: self.start_profile.label(),
            max_iterations: self.max_iterations,
            tol: self.tol,
            seed,
        }
    }
}
