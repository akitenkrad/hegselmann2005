//! socsim フレームワーク上の有界信頼意見力学の世界状態．
//!
//! `OpinionWorld` は socsim の [`WorldState`] を実装する．意見は連続値
//! `x_i ∈ [0,1]` (調和・幾何平均では開区間 `]0,1[`) の 1 次元ベクトルで，
//! 空間占有もネットワーク位相も持たない (正準モデルは完全グラフ)．
//! したがって `socsim-grid` / `socsim-net` は不使用である．
//!
//! BC モデルの「近傍」(信頼集合) は固定位相ではなく，意見距離
//! `|x_i - x_j| ≤ ε` で毎ステップ動的に決まる完全グラフ上の部分集合であり，
//! 更新メカニズム ([`crate::mechanisms::BoundedConfidenceUpdate`]) 内で
//! 全エージェント走査により計算する．

use socsim_core::{AgentId, SimClock, WorldState};

use crate::means::MeanOperator;

/// 有界信頼意見力学の世界状態．
pub struct OpinionWorld {
    /// シミュレーションクロック．
    pub clock: SimClock,
    /// エージェント ID (`0..n`，ソート済み)．
    pub agents: Vec<AgentId>,
    /// 各エージェントの意見 `x_i(t) ∈ [0,1]` (H/G では `]0,1[`)．index = agent_id．
    pub opinions: Vec<f64>,
    /// 対称信頼幅 ε．
    pub eps: f64,
    /// 平均化操作 (A / G / H / P{p} / R)．
    pub mean: MeanOperator,
    /// 直近ステップの `max|Δx_i|` (収束判定用)．初期値は +∞．
    pub last_max_delta: f64,
}

impl OpinionWorld {
    /// 初期意見ベクトルから世界状態を構築する．
    pub fn new(opinions: Vec<f64>, eps: f64, mean: MeanOperator, t_max: u64) -> Self {
        let agents = (0..opinions.len() as u64).map(AgentId).collect();
        OpinionWorld {
            clock: SimClock::new(t_max),
            agents,
            opinions,
            eps,
            mean,
            last_max_delta: f64::INFINITY,
        }
    }

    /// エージェント数 n．
    pub fn n(&self) -> usize {
        self.opinions.len()
    }
}

impl WorldState for OpinionWorld {
    fn agent_ids(&self) -> Vec<AgentId> {
        // すでにソート済みだが，契約 (sorted) を明示するためそのまま返す．
        self.agents.clone()
    }

    fn clock(&self) -> &SimClock {
        &self.clock
    }

    fn clock_mut(&mut self) -> &mut SimClock {
        &mut self.clock
    }
}
