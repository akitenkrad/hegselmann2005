//! 平均化操作 (averaging operators)．
//!
//! 旧来は本 crate 内で `MeanOperator` / `apply_mean` / `parse_mean` を自前実装して
//! いたが，これらは `socsim-mechanisms` パックへ (本リポジトリから) 移植され
//! 共有化された．本モジュールはパックの型をそのまま再エクスポートし，CLI の
//! `--mean` パーサや config との橋渡しのみを担う (math はパックに一本化)．
//!
//! 系統的不等式 (論文 §2):
//! `P_{-∞}(=min) ≤ H=P_{-1} ≤ G=P_0 ≤ A=P_1 ≤ P_p ≤ P_{∞}(=max)  (p ≥ 1)`

pub use socsim_mechanisms::means::{apply_mean, parse_mean, MeanOperator};
