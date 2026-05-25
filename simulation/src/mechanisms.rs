//! 有界信頼更新メカニズム (socsim-social-dynamics パックへ移譲)．
//!
//! Hegselmann & Krause (2005) の一般化モデル (式(2)) を `Interaction` フェーズで
//! **同期更新** (synchronous / simultaneous) する更新規則は，かつて本 crate 内で
//! `BoundedConfidenceUpdate` として自前実装していたが，`socsim-social-dynamics`
//! パックの [`HegselmannKrauseMechanism`] へ移植・共有化された．本リポジトリは
//! 当該パック実装をそのまま再エクスポートして利用する．
//!
//! パック実装は信頼集合 `I(i) = { j : |x_i − x_j| ≤ ε, j ∈ neighbours(i) ∪ {i} }`
//! を **エージェント id 昇順** (自分を所定位置に含む) で構築するため，`apply_mean`
//! の浮動小数点総和順序がローカル旧実装とビット等価になる (socsim #42/#43)．これにより
//! `metrics.csv` の `max_delta` / `variance` が ulp レベルまで一致する．
//!
//! 収束判定 (`max|Δx| < tol` での停止) と `max_delta` の記録はメカニズムではなく
//! ドライバ側 ([`crate::simulation::run`]) が担う．決定論的平均 (A/G/H/P) のみ
//! パックの [`ConvergenceMechanism`] を `PostStep` フェーズに配線して `request_stop`
//! し，ランダム平均 R は収束判定を使わず最大反復まで回す (論文 Observation 6, Fact 5)．

pub use socsim_social_dynamics::{ConvergenceMechanism, HegselmannKrauseMechanism};
