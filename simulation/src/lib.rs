//! Hegselmann & Krause (2005) 有界信頼意見力学の再現実装ライブラリ．
//!
//! socsim フレームワーク上に構築した有界信頼意見力学の公開 API を提供する．
//! 平均化操作 (`means`)・世界状態 (`world`)・更新メカニズム (`mechanisms`)・
//! 実行ドライバ (`simulation`)・集計メトリクス (`metrics`)・設定構造体 (`config`)
//! をモジュールとして公開し，バイナリ (`hegselmann`) と統合テストの双方から利用する．
//!
//! 平均化操作の math (`MeanOperator` / `apply_mean` / `parse_mean`) と有界信頼更新
//! メカニズム (`HegselmannKrauseMechanism` / `ConvergenceMechanism`) はいずれも
//! `socsim-mechanisms` パック (本リポジトリから移植) を `means` / `mechanisms`
//! で再エクスポートして共有する．パックの HK 実装は信頼集合を id 昇順 (自分を所定
//! 位置に含む) で構築するため，旧ローカル実装と出力がビット等価である (socsim #42/#43)．

pub mod config;
pub mod means;
pub mod mechanisms;
pub mod metrics;
pub mod simulation;
pub mod world;
