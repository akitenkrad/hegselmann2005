//! socsim フレームワーク上の有界信頼更新メカニズム．
//!
//! Hegselmann & Krause (2005) の一般化モデル (式(2)) を socsim の [`Mechanism`]
//! として実装する．`Interaction` フェーズで発火し，**同期更新**
//! (synchronous / simultaneous) を行う:
//!
//! 1. ステップ開始時の意見をスナップショット `prev = world.opinions.clone()`．
//! 2. 各エージェント i について信頼集合 `I(i) = { j : |x_i - x_j| ≤ ε }` を
//!    `prev` から計算する (完全グラフ走査 O(n) /agent，O(n²) /step)．
//! 3. `apply_mean(world.mean, …)` で信頼集合内意見を集約し新意見を得る．
//! 4. 全エージェントの新意見を一括代入する (同期更新)．
//!
//! 同期更新は「m 個のグループが順に更新する手続きの極限ケース」(論文 §4) であり，
//! 結果は活性化順序に依存しない．したがって Scheduler は `SequentialScheduler`
//! (決定論) とし，`ctx.agent_order` は使わず id 昇順で全エージェントを走査する．
//!
//! `max|Δx_i|` を [`StepContext::scratch`] と `world.last_max_delta` に記録する．
//! **決定論的な平均 (A/G/H/P)** では `max|Δx| < tol` を検知したら
//! [`StepContext::request_stop`] でエンジンに停止を要求する．**ランダム平均 R** は
//! 毎ステップ非決定的に動くため収束判定を使わず，最大反復まで回す
//! (論文 Observation 6, Fact 5)．

use socsim_core::{Mechanism, Phase, Result, StepContext};

use crate::means::apply_mean;
use crate::world::OpinionWorld;

/// 信頼集合内の意見を平均化操作で集約し，全エージェントを同期更新するメカニズム．
pub struct BoundedConfidenceUpdate {
    /// 収束判定の許容誤差 (max|Δx| < tol; 決定論的平均のみ使用)．
    pub tol: f64,
}

impl Mechanism<OpinionWorld> for BoundedConfidenceUpdate {
    fn name(&self) -> &str {
        "bounded_confidence_update"
    }

    fn phases(&self) -> &'static [Phase] {
        &[Phase::Interaction]
    }

    fn apply(&mut self, _phase: Phase, ctx: &mut StepContext<'_, OpinionWorld>) -> Result<()> {
        let n = ctx.world.n();
        let eps = ctx.world.eps;
        let mean = ctx.world.mean;

        // ステップ開始時の意見をスナップショット (同期更新の正本)．
        let prev = ctx.world.opinions.clone();

        // 信頼集合内意見を集める再利用バッファ (毎エージェントのヒープ確保を避ける)．
        let mut conf_set: Vec<f64> = Vec::with_capacity(n);
        let mut new_opinions: Vec<f64> = Vec::with_capacity(n);
        let mut max_delta = 0.0_f64;

        for i in 0..n {
            let xi = prev[i];
            conf_set.clear();
            // 信頼集合 I(i) = { j : |x_i - x_j| ≤ ε } を旧プロファイルから計算 (自分も含む)．
            for &xj in &prev {
                if (xi - xj).abs() <= eps {
                    conf_set.push(xj);
                }
            }
            // 信頼集合は少なくとも自分自身を含むので空ではない．
            let xi_new = apply_mean(mean, &conf_set, ctx.rng);
            let delta = (xi_new - xi).abs();
            if delta > max_delta {
                max_delta = delta;
            }
            new_opinions.push(xi_new);
        }

        // 一括書き戻し (同期更新)．
        ctx.world.opinions = new_opinions;
        ctx.world.last_max_delta = max_delta;

        // ドライバ用にステップ結果を scratch へ．
        ctx.scratch.insert("max_delta", max_delta);

        // 決定論的平均は不動点に到達したら停止．R は収束判定を使わず最大反復まで回す．
        let converged = mean.is_deterministic() && max_delta < self.tol;
        ctx.scratch.insert("converged", converged);
        if converged {
            ctx.request_stop();
        }

        Ok(())
    }
}
