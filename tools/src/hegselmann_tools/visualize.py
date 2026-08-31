#!/usr/bin/env python3
"""
visualize.py — Hegselmann & Krause (2005) 意見力学 再現実験 可視化スクリプト

runvault の run ディレクトリから意見軌跡 (`artifacts/opinions.csv`) と
メトリクス (`metrics.csv`) を読み，意見軌跡図 (時間×意見 ∈ [0,1]，エージェント
ごとに 1 本の線) とメトリクス時系列図 (占有クラス数・分散・max|Δx|) を生成する
(論文 Fig. 3 風)．

どの run を見るかは `--results_dir` を省略すれば runvault が答える
(`runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone`)．
`results/` を自分で走査して新しそうなディレクトリを当てにいくことはしない．

図は run ディレクトリの *隣* (`results/hegselmann-averaging/figures/<run_slug>/`) に
置く．`manifest.csv` は `finish()` が確定させたもので，run が終わった後に足した
ものはそこに載らないためである．

Usage:
    uv run hegselmann-tools visualize
    uv run hegselmann-tools visualize --results_dir "$(runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone)"
    uv run hegselmann-tools visualize --output_dir out

Outputs:
    output_dir/
    ├── opinion_trajectory.png ← 意見軌跡図 (時間×意見)
    └── metrics_timeseries.png ← 占有クラス数・分散・max|Δx| の時系列
"""

from __future__ import annotations

import argparse
import os

import matplotlib as mpl
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
from runvault.read import artifacts_dir, figures_dir, metrics_wide, runvault_path

from hegselmann_tools.experiment import EXPERIMENT

# --------------------------------------------------------------------------- #
# 日本語フォント設定
# --------------------------------------------------------------------------- #
plt.rcParams["font.family"] = "Hiragino Sans"

# --------------------------------------------------------------------------- #
# カラー設定
# --------------------------------------------------------------------------- #
COLOR_BG = "#FAFAF8"
COLOR_TRAJ = "#2196F3"
COLOR_NCLASS = "#F44336"
COLOR_VAR = "#9C27B0"
COLOR_DELTA = "#FF9800"


# --------------------------------------------------------------------------- #
# データ読み込み
# --------------------------------------------------------------------------- #

def load_opinions(path: str) -> pd.DataFrame:
    """opinions.csv (long-format: t, agent_id, opinion) を読み込む．"""
    if not os.path.exists(path):
        raise FileNotFoundError(f"opinions.csv が見つかりません: {path}")
    return pd.read_csv(path)


def load_metrics(path: str) -> pd.DataFrame:
    """ステップごとのメトリクスを 1 ステップ 1 行の表として読む．

    runvault の `metrics.csv` は long 形式なので `metrics_wide` で横に倒す．
    時間軸の列名は runvault では `step` だが，本モデルの表記は論文に合わせた `t`
    なので，こちら側の呼び名に揃えてから返す (runvault 以前の wide な metrics.csv
    はもともと `t` 列を持つので何もしない)．
    """
    df = metrics_wide(path)
    if "step" in df.columns and "t" not in df.columns:
        df = df.rename(columns={"step": "t"})
    return df


def to_wide(df_long: pd.DataFrame) -> tuple[np.ndarray, np.ndarray]:
    """long-format を (時刻配列, 意見行列 [T × N]) に変換する．"""
    pivot = df_long.pivot(index="t", columns="agent_id", values="opinion")
    pivot = pivot.sort_index()
    ts = pivot.index.to_numpy()
    mat = pivot.to_numpy()  # shape: (T, N)
    return ts, mat


# --------------------------------------------------------------------------- #
# 可視化関数
# --------------------------------------------------------------------------- #

def save_opinion_trajectory(
    ts: np.ndarray,
    mat: np.ndarray,
    out_path: str,
    subtitle: str = "",
) -> None:
    """意見軌跡図 (時間×意見) を保存する (論文 Fig. 3 風)．

    エージェント数が多い場合 (> 400) は線が潰れるため，半透明の細線で重ね描く．
    """
    n_agents = mat.shape[1]
    fig, ax = plt.subplots(figsize=(9, 6), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)

    # エージェント数に応じて線の透明度・太さを調整．
    alpha = max(0.05, min(0.8, 30.0 / max(n_agents, 1)))
    lw = 0.6 if n_agents > 200 else 1.0

    for i in range(n_agents):
        ax.plot(ts, mat[:, i], color=COLOR_TRAJ, alpha=alpha, lw=lw)

    ax.set_xlabel("時刻 t")
    ax.set_ylabel("意見 x ∈ [0, 1]")
    ax.set_ylim(-0.02, 1.02)
    ax.set_xlim(ts.min(), ts.max())
    title = "意見軌跡 (有界信頼意見力学)"
    if subtitle:
        title += f"\n{subtitle}"
    ax.set_title(title, fontsize=12)
    ax.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_metrics_timeseries(df: pd.DataFrame, out_path: str) -> None:
    """占有クラス数・分散・max|Δx| の時系列図を保存する．"""
    fig, axes = plt.subplots(1, 3, figsize=(15, 4.5), facecolor=COLOR_BG)
    fig.suptitle("Hegselmann-Krause 意見力学 — メトリクス時系列", fontsize=13)

    t = df["t"]

    ax = axes[0]
    ax.set_facecolor(COLOR_BG)
    ax.plot(t, df["n_occupied_classes"], color=COLOR_NCLASS, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("占有クラス数")
    ax.set_title("占有クラス数 (生存意見数)")
    ax.set_yscale("log")
    ax.grid(True, alpha=0.3, which="both")

    ax = axes[1]
    ax.set_facecolor(COLOR_BG)
    ax.plot(t, df["variance"], color=COLOR_VAR, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("分散")
    ax.set_title("意見の分散")
    ax.grid(True, alpha=0.3)

    ax = axes[2]
    ax.set_facecolor(COLOR_BG)
    # max|Δx| は 0 を含むため log スケールでは下駄を履かせる．
    delta = df["max_delta"].clip(lower=1e-16)
    ax.plot(t, delta, color=COLOR_DELTA, lw=2)
    ax.set_xlabel("時刻 t")
    ax.set_ylabel("max|Δx|")
    ax.set_title("最大意見変化量 (収束指標)")
    ax.set_yscale("log")
    ax.grid(True, alpha=0.3, which="both")

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-tools visualize",
        description="Hegselmann-Krause 意見力学 意見軌跡 可視化スクリプト",
    )
    p.add_argument(
        "--results_dir", "--results-dir", default=None,
        help=(
            "runvault の run ディレクトリ．未指定時は runvault に最新の run を聞く "
            f"(--experiment {EXPERIMENT} --subcommand run --standalone)．"
        ),
    )
    p.add_argument(
        "--results_root", "--results-root", default="results",
        help="--results_dir 未指定時に runvault が探す results ルート (default: results)",
    )
    p.add_argument(
        "--output_dir", "--output-dir", default=None,
        help=(
            "図の保存先ディレクトリ "
            f"(default: results/{EXPERIMENT}/figures/{{run_slug}})"
        ),
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    run_dir = args.results_dir
    if run_dir is None:
        run_dir = runvault_path(
            EXPERIMENT, args.results_root, subcommand="run", standalone=True
        )

    opinions_path = os.path.join(artifacts_dir(run_dir), "opinions.csv")
    metrics_path = os.path.join(run_dir, "metrics.csv")
    out_dir = args.output_dir if args.output_dir else figures_dir(run_dir)

    os.makedirs(out_dir, exist_ok=True)

    print("=== Hegselmann-Krause 意見力学 可視化 ===")
    print(f"run:        {run_dir}")
    print(f"意見軌跡:   {opinions_path}")
    print(f"メトリクス: {metrics_path}")
    print(f"出力先:     {out_dir}")
    print("-----------------------------------------")

    print("[1/3] 意見軌跡を読み込み中 ...")
    df_op = load_opinions(opinions_path)
    ts, mat = to_wide(df_op)
    print(f"      {mat.shape[0]} ステップ × {mat.shape[1]} エージェント")

    print("[2/3] 意見軌跡図を保存中 ...")
    save_opinion_trajectory(
        ts, mat,
        os.path.join(out_dir, "opinion_trajectory.png"),
        subtitle=f"{mat.shape[1]} エージェント，{mat.shape[0] - 1} ステップ",
    )

    print("[3/3] メトリクス時系列を保存中 ...")
    df_m = load_metrics(metrics_path)
    save_metrics_timeseries(df_m, os.path.join(out_dir, "metrics_timeseries.png"))

    print("-----------------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
