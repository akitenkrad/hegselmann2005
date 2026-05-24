//! Hegselmann & Krause (2005) 有界信頼意見力学の再現実装ライブラリ．
//!
//! socsim フレームワーク上に構築した有界信頼意見力学の公開 API を提供する．
//! 平均化操作 (`means`)・世界状態 (`world`)・更新メカニズム (`mechanisms`)・
//! 実行ドライバ (`simulation`)・集計メトリクス (`metrics`)・設定構造体 (`config`)
//! をモジュールとして公開し，バイナリ (`hegselmann`) と統合テストの双方から利用する．

pub mod config;
pub mod means;
pub mod mechanisms;
pub mod metrics;
pub mod simulation;
pub mod world;
