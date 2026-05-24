#!/usr/bin/env python3
"""
visualize_sweep.py — Hegselmann & Krause (2005) 意見力学 スイープ結果 可視化スクリプト

results/latest (または --sweep_dir 指定先) の sweep_summary.csv を読み，
平均ごとに「占有クラス数 vs ε」の相図と，平均間の合意ブリンク比較を生成する
(論文 Fig. 4–7 風)．占有クラス数は試行平均±標準偏差で描く．

Usage:
    uv run hegselmann-tools visualize-sweep
    uv run hegselmann-tools visualize-sweep --sweep_dir results/20260524_160000_sweep

Outputs:
    output_dir/
    ├── sweep_occupied_classes.png ← 占有クラス数 vs ε (平均ごとの相図，log y)
    └── sweep_consensus_brink.png  ← 合意ブリンク ε* の平均間比較 (棒グラフ)
"""

from __future__ import annotations

import argparse
import os

import matplotlib.pyplot as plt
import numpy as np
import pandas as pd

# --------------------------------------------------------------------------- #
# 日本語フォント設定
# --------------------------------------------------------------------------- #
plt.rcParams["font.family"] = "Hiragino Sans"

# --------------------------------------------------------------------------- #
# カラー設定
# --------------------------------------------------------------------------- #
COLOR_BG = "#FAFAF8"

# 平均ごとの配色 (固定パレット; 未知ラベルは自動割当にフォールバック)．
MEAN_COLORS = {
    "A": "#2196F3",
    "G": "#4CAF50",
    "H": "#F44336",
    "R": "#9C27B0",
}
_FALLBACK_CYCLE = ["#FF9800", "#00BCD4", "#795548", "#607D8B", "#E91E63", "#3F51B5"]


def color_for(mean: str, idx: int) -> str:
    if mean in MEAN_COLORS:
        return MEAN_COLORS[mean]
    return _FALLBACK_CYCLE[idx % len(_FALLBACK_CYCLE)]


# --------------------------------------------------------------------------- #
# データ読み込み・集計
# --------------------------------------------------------------------------- #

def load_summary(sweep_dir: str) -> pd.DataFrame:
    """sweep_summary.csv を読み込む．"""
    path = os.path.join(sweep_dir, "sweep_summary.csv")
    if not os.path.exists(path):
        raise FileNotFoundError(f"sweep_summary.csv が見つかりません: {path}")
    return pd.read_csv(path)


def aggregate(df: pd.DataFrame) -> pd.DataFrame:
    """(mean, eps) ごとに占有クラス数の平均・標準偏差を集計する．"""
    # "mean" 列 (平均演算子) と集計関数名 "mean" の衝突を避けるため named aggregation を使う．
    agg = (
        df.groupby(["mean", "eps"])
        .agg(
            n_mean=("n_occupied_classes", "mean"),
            n_std=("n_occupied_classes", "std"),
        )
        .reset_index()
    )
    agg["n_std"] = agg["n_std"].fillna(0.0)
    return agg


def consensus_brink(agg: pd.DataFrame, mean: str) -> float | None:
    """占有クラス数 (試行平均) が初めて 1 以下になる最小 ε を返す．"""
    sub = agg[agg["mean"] == mean].sort_values("eps")
    consensus = sub[sub["n_mean"].round() <= 1.0]
    if consensus.empty:
        return None
    return float(consensus["eps"].min())


# --------------------------------------------------------------------------- #
# 可視化関数
# --------------------------------------------------------------------------- #

def save_occupied_classes(agg: pd.DataFrame, out_path: str) -> None:
    """占有クラス数 vs ε の相図を平均ごとに重ね描く (log y)．"""
    fig, ax = plt.subplots(figsize=(9, 6), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)

    means = list(dict.fromkeys(agg["mean"].tolist()))
    for idx, mean in enumerate(means):
        sub = agg[agg["mean"] == mean].sort_values("eps")
        c = color_for(mean, idx)
        ax.errorbar(
            sub["eps"], sub["n_mean"], yerr=sub["n_std"],
            color=c, lw=1.8, marker="o", markersize=4, capsize=2,
            label=mean, alpha=0.9,
        )

    ax.axhline(1.0, color="#888888", lw=0.8, linestyle="--", label="合意境界 (1 クラス)")
    ax.set_xlabel("信頼水準 ε")
    ax.set_ylabel("占有クラス数 (試行平均)")
    ax.set_yscale("log")
    ax.set_title("占有クラス数 vs 信頼水準 ε (平均ごとの相図; 論文 Fig. 4–7 風)", fontsize=12)
    ax.legend(fontsize=9, title="平均")
    ax.grid(True, alpha=0.3, which="both")

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


def save_consensus_brink(agg: pd.DataFrame, eps_max: float, out_path: str) -> None:
    """平均間の合意ブリンク ε* を棒グラフで比較する．"""
    means = list(dict.fromkeys(agg["mean"].tolist()))
    brinks: list[float] = []
    labels: list[str] = []
    colors: list[str] = []
    for idx, mean in enumerate(means):
        b = consensus_brink(agg, mean)
        labels.append(mean)
        colors.append(color_for(mean, idx))
        # 未到達は ε_max を上回るバーで示す (注釈付き)．
        brinks.append(b if b is not None else eps_max * 1.05)

    fig, ax = plt.subplots(figsize=(8, 5), facecolor=COLOR_BG)
    ax.set_facecolor(COLOR_BG)
    bars = ax.bar(labels, brinks, color=colors, alpha=0.85)

    for idx, (mean, b) in enumerate(zip(means, brinks)):
        reached = consensus_brink(agg, mean) is not None
        txt = f"{b:.3f}" if reached else "未到達"
        ax.text(idx, b + eps_max * 0.01, txt, ha="center", va="bottom", fontsize=9)

    ax.set_xlabel("平均演算子")
    ax.set_ylabel("合意ブリンク ε*")
    ax.set_title("合意ブリンク ε* の平均間比較 (論文 Observation 1, Fact 4)", fontsize=12)
    ax.grid(True, alpha=0.3, axis="y")

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")


# --------------------------------------------------------------------------- #
# メイン
# --------------------------------------------------------------------------- #

def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-tools visualize-sweep",
        description="Hegselmann-Krause 意見力学 スイープ結果 可視化スクリプト",
    )
    p.add_argument(
        "--sweep_dir", "--sweep-dir", default="results/latest",
        help="スイープ出力ディレクトリ (default: results/latest)",
    )
    p.add_argument(
        "--output_dir", "--output-dir", default=None,
        help="図の保存先ディレクトリ (default: {sweep_dir}/figures)",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    out_dir = args.output_dir if args.output_dir else os.path.join(args.sweep_dir, "figures")
    os.makedirs(out_dir, exist_ok=True)

    print("=== Hegselmann-Krause 意見力学 スイープ可視化 ===")
    print(f"スイープ: {args.sweep_dir}")
    print(f"出力先:   {out_dir}")
    print("-------------------------------------------------")

    print("[1/3] sweep_summary.csv を読み込み中 ...")
    df = load_summary(args.sweep_dir)
    agg = aggregate(df)
    eps_max = float(df["eps"].max())
    print(f"      平均 {df['mean'].nunique()} 種 × ε {df['eps'].nunique()} 値")

    print("[2/3] 占有クラス数の相図を保存中 ...")
    save_occupied_classes(agg, os.path.join(out_dir, "sweep_occupied_classes.png"))

    print("[3/3] 合意ブリンク比較を保存中 ...")
    save_consensus_brink(agg, eps_max, os.path.join(out_dir, "sweep_consensus_brink.png"))

    print("-------------------------------------------------")
    print("合意ブリンク ε* (占有クラス数が初めて 1 になる最小 ε):")
    for mean in dict.fromkeys(agg["mean"].tolist()):
        b = consensus_brink(agg, mean)
        print(f"  {mean:<6} → " + (f"{b:.4f}" if b is not None else "未到達"))

    print("-------------------------------------------------")
    print("完了．出力ファイル一覧:")
    for f in sorted(os.listdir(out_dir)):
        size_kb = os.path.getsize(os.path.join(out_dir, f)) / 1024
        print(f"  {f:35s} ({size_kb:6.1f} KB)")


if __name__ == "__main__":
    main()
