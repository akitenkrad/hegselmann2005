//! 評価指標．
//!
//! 論文 §3 のベンチマークに対応する指標を計算する．中心は
//! [`n_occupied_classes`] (生存意見数) と [`Phase`] 分類 (合意/分極/多元)．

use serde::Serialize;

/// 占有クラスの分割解像度 (論文の `10^-4` 刻みに対応)．
pub const CLASS_RESOLUTION: f64 = 1e-4;

/// 安定後の相 (phase) 分類．占有クラス数で判定する．
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 合意 (consensus): 占有クラス数 == 1．
    Consensus,
    /// 分極 (polarization): 占有クラス数 == 2．
    Polarization,
    /// 多元 (plurality): 占有クラス数 ≥ 3．
    Plurality,
}

impl Phase {
    /// 占有クラス数から相を分類する．
    pub fn classify(n_occupied: usize) -> Phase {
        match n_occupied {
            0 | 1 => Phase::Consensus,
            2 => Phase::Polarization,
            _ => Phase::Plurality,
        }
    }

    /// CSV/JSON 用の整数コード (consensus=1 / polarization=2 / plurality=3)．
    pub fn code(&self) -> u8 {
        match self {
            Phase::Consensus => 1,
            Phase::Polarization => 2,
            Phase::Plurality => 3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Phase::Consensus => "consensus",
            Phase::Polarization => "polarization",
            Phase::Plurality => "plurality",
        }
    }
}

/// 占有クラス数 (= 生存した相異なる意見数) を数える．
///
/// 意見空間を [`CLASS_RESOLUTION`] 刻みのビンに分割し，意見が存在する非空ビンを
/// 数える．論文 Fact 1/3, Observation 1, Fig. 4–7 に対応する中心指標．
pub fn n_occupied_classes(opinions: &[f64]) -> usize {
    if opinions.is_empty() {
        return 0;
    }
    let mut bins: Vec<i64> = opinions
        .iter()
        .map(|&x| (x / CLASS_RESOLUTION).round() as i64)
        .collect();
    bins.sort_unstable();
    bins.dedup();
    bins.len()
}

/// 平均意見 `x̄`．
pub fn mean_opinion(opinions: &[f64]) -> f64 {
    if opinions.is_empty() {
        return 0.0;
    }
    opinions.iter().sum::<f64>() / opinions.len() as f64
}

/// 意見の分散．
pub fn variance(opinions: &[f64]) -> f64 {
    if opinions.is_empty() {
        return 0.0;
    }
    let m = mean_opinion(opinions);
    opinions.iter().map(|&x| (x - m) * (x - m)).sum::<f64>() / opinions.len() as f64
}

/// 1 ステップ分のメトリクス (metrics.csv の 1 行)．
#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    /// ステップ番号 t．
    pub t: usize,
    /// 占有クラス数 (生存意見数)．
    pub n_occupied_classes: usize,
    /// 平均意見．
    pub mean: f64,
    /// 意見の分散．
    pub variance: f64,
    /// 相コード (consensus=1 / polarization=2 / plurality=3)．
    pub phase: u8,
    /// 直近ステップの max|Δx| (step 0 では NaN を 0 として記録)．
    pub max_delta: f64,
}

impl Metrics {
    /// 意見ベクトルからメトリクスを計算する．
    pub fn compute(opinions: &[f64], t: usize, max_delta: f64) -> Self {
        let n_occ = n_occupied_classes(opinions);
        Metrics {
            t,
            n_occupied_classes: n_occ,
            mean: mean_opinion(opinions),
            variance: variance(opinions),
            phase: Phase::classify(n_occ).code(),
            max_delta,
        }
    }
}

/// 合意ブリンク `ε*` の数値推定: 占有クラス数が初めて 1 になる最小 ε．
///
/// `samples` は `(eps, n_occupied_classes)` の (ε 昇順を仮定しない) 列．
/// 占有クラス数が 1 となる最小の ε を返す．存在しなければ `None`．
/// 論文 Observation 1, Fact 4 の `ε*` 推定に対応する (sweep 集計で使う)．
pub fn consensus_brink(samples: &[(f64, usize)]) -> Option<f64> {
    samples
        .iter()
        .filter(|(_, n)| *n <= 1)
        .map(|(eps, _)| *eps)
        .fold(None, |acc, eps| match acc {
            None => Some(eps),
            Some(best) => Some(best.min(eps)),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_cluster_is_one_class() {
        let v = vec![0.5, 0.5000001, 0.4999999];
        assert_eq!(n_occupied_classes(&v), 1);
    }

    #[test]
    fn distinct_opinions_count_as_classes() {
        let v = vec![0.1, 0.5, 0.9];
        assert_eq!(n_occupied_classes(&v), 3);
    }

    #[test]
    fn phase_classification() {
        assert_eq!(Phase::classify(1), Phase::Consensus);
        assert_eq!(Phase::classify(2), Phase::Polarization);
        assert_eq!(Phase::classify(5), Phase::Plurality);
    }

    #[test]
    fn brink_is_smallest_consensus_eps() {
        let samples = [(0.05, 8), (0.10, 3), (0.20, 1), (0.30, 1)];
        assert_eq!(consensus_brink(&samples), Some(0.20));
    }

    #[test]
    fn brink_none_when_no_consensus() {
        let samples = [(0.05, 8), (0.10, 3)];
        assert_eq!(consensus_brink(&samples), None);
    }
}
