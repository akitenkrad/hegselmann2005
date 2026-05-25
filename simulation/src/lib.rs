//! Hegselmann & Krause (2005) 有界信頼意見力学の再現実装ライブラリ．
//!
//! socsim フレームワーク上に構築した有界信頼意見力学の公開 API を提供する．
//! 平均化操作 (`means`)・世界状態 (`world`)・更新メカニズム (`mechanisms`)・
//! 実行ドライバ (`simulation`)・集計メトリクス (`metrics`)・設定構造体 (`config`)
//! をモジュールとして公開し，バイナリ (`hegselmann`) と統合テストの双方から利用する．
//!
//! 平均化操作の math (`MeanOperator` / `apply_mean` / `parse_mean`) は
//! `socsim-social-dynamics` パック (本リポジトリから移植) を `means` で再エクスポート
//! して共有する．有界信頼更新メカニズム (`mechanisms`) は信頼集合の構築順序を厳密に
//! 保つためローカル実装を維持する (詳細は `mechanisms` のモジュールドキュメントを参照)．

pub mod config;
pub mod means;
pub mod mechanisms;
pub mod metrics;
pub mod simulation;
pub mod world;
