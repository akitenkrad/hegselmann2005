//! 平均化操作 (averaging operators)．
//!
//! Hegselmann & Krause (2005) は 2002 年 HK モデルの算術平均を，集約に用いる
//! 「平均」の種類という軸で一般化した．本モジュールは論文の 5 種の平均
//! (A/G/H/P/R) を `MeanOperator` enum として明示的に切り出し，信頼集合内の
//! 意見ベクトルへの適用を [`apply_mean`] に集約する．これにより「各種の平均」と
//! いう論文の一般化が型として表現される．
//!
//! 系統的不等式 (論文 §2):
//! `P_{-∞}(=min) ≤ H=P_{-1} ≤ G=P_0 ≤ A=P_1 ≤ P_p ≤ P_{∞}(=max)  (p ≥ 1)`

use rand::Rng;
use socsim_core::SimRng;

/// 信頼集合内の意見を集約する平均化操作 (averaging strategy)．
///
/// A/G/H/P はいずれも決定論的 (RNG を使わない)．R のみ RNG を使う．
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeanOperator {
    /// 算術平均 `A = P_1`．2002 年 HK モデルの基本規則．
    Arithmetic,
    /// 幾何平均 `G = P_0 = lim_{p→0} P_p` (vals > 0 を要求)．
    Geometric,
    /// 調和平均 `H = P_{-1}` (vals > 0 を要求)．
    Harmonic,
    /// べき平均 (Hölder mean) `P_p`，`p ≠ 0`．
    Power(f64),
    /// ランダム平均 `R = Uniform(min S, max S)`．唯一 RNG を使う．
    Random,
}

impl MeanOperator {
    /// CLI/ログ出力用のラベル．
    pub fn label(&self) -> String {
        match *self {
            MeanOperator::Arithmetic => "A".to_string(),
            MeanOperator::Geometric => "G".to_string(),
            MeanOperator::Harmonic => "H".to_string(),
            MeanOperator::Power(p) => format!("P{}", p),
            MeanOperator::Random => "R".to_string(),
        }
    }

    /// この平均が初期分布として正値 (`vals > 0`) を要求するか．
    /// 幾何・調和平均は 0 を含むと未定義/発散するため開区間を要求する．
    pub fn requires_positive(&self) -> bool {
        matches!(self, MeanOperator::Geometric | MeanOperator::Harmonic)
    }

    /// この平均が決定論的か (= RNG を使わないか)．
    /// 決定論的な平均は不動点に到達したら収束判定で停止できる．
    pub fn is_deterministic(&self) -> bool {
        !matches!(self, MeanOperator::Random)
    }
}

/// 文字列から `MeanOperator` をパースする．
///
/// 受理する形式:
/// - `"A"` → Arithmetic
/// - `"G"` → Geometric
/// - `"H"` → Harmonic
/// - `"R"` → Random
/// - `"P<p>"` (例 `"P0.01"`, `"P100"`, `"P-1"`) → Power(p)
/// - `"P"` → Power(`p_fallback`)．`--mean P --p 100` のように `p` を別フラグで与える用途．
///
/// `p_fallback` は `"P"` (指数なし) のときに用いる既定指数．通常は `--p` の値を渡す．
pub fn parse_mean(s: &str, p_fallback: f64) -> Result<MeanOperator, String> {
    let s = s.trim();
    match s {
        "A" | "a" => Ok(MeanOperator::Arithmetic),
        "G" | "g" => Ok(MeanOperator::Geometric),
        "H" | "h" => Ok(MeanOperator::Harmonic),
        "R" | "r" => Ok(MeanOperator::Random),
        "P" | "p" => {
            if p_fallback == 0.0 {
                return Err("べき平均 P には p ≠ 0 が必要です (--p で指定)".to_string());
            }
            Ok(MeanOperator::Power(p_fallback))
        }
        _ => {
            // "P<p>" 形式
            if let Some(rest) = s.strip_prefix('P').or_else(|| s.strip_prefix('p')) {
                let p: f64 = rest
                    .parse()
                    .map_err(|_| format!("べき平均の指数のパースに失敗: \"{}\"", s))?;
                if p == 0.0 {
                    return Err(
                        "べき平均 P は p ≠ 0 でなければなりません (p=0 は幾何平均 G)".to_string(),
                    );
                }
                Ok(MeanOperator::Power(p))
            } else {
                Err(format!(
                    "不正な平均指定: \"{}\" (A / G / H / P<p> / R のいずれか)",
                    s
                ))
            }
        }
    }
}

/// 信頼集合内の意見 `vals` に平均化操作を適用する．
///
/// `vals` は信頼集合 `I(i,x)` 内のエージェントの意見多重集合 (自分自身を含む)．
/// 空ではないことを仮定する (BC モデルでは少なくとも自分が含まれる)．
/// ランダム平均 R のみ `rng` を使い，それ以外は `rng` を参照しない (決定論)．
///
/// # Panics
/// `vals` が空の場合．幾何・調和平均で `vals` に非正値が含まれる場合 (デバッグ時)．
pub fn apply_mean(op: MeanOperator, vals: &[f64], rng: &mut SimRng) -> f64 {
    debug_assert!(
        !vals.is_empty(),
        "信頼集合は空であってはならない (自分自身を含む)"
    );
    let m = vals.len() as f64;
    match op {
        MeanOperator::Arithmetic => vals.iter().sum::<f64>() / m,
        MeanOperator::Geometric => {
            // G = (Π s)^{1/m} = exp( (1/m) Σ ln s )．アンダーフロー回避のため対数空間で計算．
            let sum_ln: f64 = vals.iter().map(|&s| s.ln()).sum();
            (sum_ln / m).exp()
        }
        MeanOperator::Harmonic => {
            // H = m / Σ (1/s)
            let sum_inv: f64 = vals.iter().map(|&s| 1.0 / s).sum();
            m / sum_inv
        }
        MeanOperator::Power(p) => {
            // P_p = ( (1/m) Σ s^p )^{1/p}  (p ≠ 0)
            let sum_pow: f64 = vals.iter().map(|&s| s.powf(p)).sum();
            (sum_pow / m).powf(1.0 / p)
        }
        MeanOperator::Random => {
            // R = Uniform(min S, max S)．min == max のときはその値を返す (退化区間)．
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for &s in vals {
                if s < lo {
                    lo = s;
                }
                if s > hi {
                    hi = s;
                }
            }
            if hi <= lo {
                lo
            } else {
                rng.gen_range(lo..hi)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use socsim_core::SimRng;

    fn rng() -> SimRng {
        SimRng::from_seed(0)
    }

    #[test]
    fn arithmetic_mean_is_average() {
        let v = [1.0, 2.0, 3.0];
        assert!((apply_mean(MeanOperator::Arithmetic, &v, &mut rng()) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn geometric_mean_of_equal_values() {
        let v = [0.5, 0.5, 0.5];
        assert!((apply_mean(MeanOperator::Geometric, &v, &mut rng()) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn harmonic_mean_known_value() {
        // H(1, 2) = 2 / (1 + 0.5) = 4/3
        let v = [1.0, 2.0];
        assert!((apply_mean(MeanOperator::Harmonic, &v, &mut rng()) - 4.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn power_one_equals_arithmetic() {
        let v = [0.1, 0.4, 0.7];
        let a = apply_mean(MeanOperator::Arithmetic, &v, &mut rng());
        let p1 = apply_mean(MeanOperator::Power(1.0), &v, &mut rng());
        assert!((a - p1).abs() < 1e-12);
    }

    #[test]
    fn systematic_inequality_holds() {
        // H ≤ G ≤ A ≤ P_2 for positive values.
        let v = [0.2, 0.5, 0.9];
        let h = apply_mean(MeanOperator::Harmonic, &v, &mut rng());
        let g = apply_mean(MeanOperator::Geometric, &v, &mut rng());
        let a = apply_mean(MeanOperator::Arithmetic, &v, &mut rng());
        let p2 = apply_mean(MeanOperator::Power(2.0), &v, &mut rng());
        assert!(h <= g + 1e-12);
        assert!(g <= a + 1e-12);
        assert!(a <= p2 + 1e-12);
    }

    #[test]
    fn random_mean_within_minmax() {
        let v = [0.2, 0.5, 0.9];
        let mut r = rng();
        for _ in 0..1000 {
            let x = apply_mean(MeanOperator::Random, &v, &mut r);
            assert!((0.2..=0.9).contains(&x), "R out of range: {}", x);
        }
    }

    #[test]
    fn parse_mean_variants() {
        assert_eq!(parse_mean("A", 0.0).unwrap(), MeanOperator::Arithmetic);
        assert_eq!(parse_mean("G", 0.0).unwrap(), MeanOperator::Geometric);
        assert_eq!(parse_mean("H", 0.0).unwrap(), MeanOperator::Harmonic);
        assert_eq!(parse_mean("R", 0.0).unwrap(), MeanOperator::Random);
        assert_eq!(parse_mean("P0.01", 0.0).unwrap(), MeanOperator::Power(0.01));
        assert_eq!(parse_mean("P100", 0.0).unwrap(), MeanOperator::Power(100.0));
        assert_eq!(parse_mean("P", 100.0).unwrap(), MeanOperator::Power(100.0));
        assert!(parse_mean("P", 0.0).is_err());
        assert!(parse_mean("P0", 0.0).is_err());
        assert!(parse_mean("X", 0.0).is_err());
    }
}
