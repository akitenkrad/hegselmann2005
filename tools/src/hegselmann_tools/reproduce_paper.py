"""reproduce_paper.py — Hegselmann & Krause (2005) 論文 Figure 一括再現スクリプト．

Hegselmann & Krause (2005)「Opinion Dynamics Driven by Various Ways of
Averaging」(*Computational Economics* 25, 381–405) の主要な定性的結論を，
Rust バイナリ (`cargo run --release -- run / sweep ...`) の単発呼び出しを
連結して一括再現する．各 Figure ごとに対応する CSV を読み込み，PNG を
`results/reproduce_<timestamp>/figures/` に集約する．

論文の中心的主張 (§3, Observation 1, Fact 4, Fig. 4–7) は「同じ ε でも平均演算子
の選び方が定常状態の相 (合意 / 分極 / 多元) を切り替える」というものである．
本スクリプトはこれを 3 つの図で再現する:

    operators : run×6  n=625 ε=0.15 seed=42．A/G/H/P0.01/P100/R を並べ，
                演算子ごとに相 (合意/分極/多元/拡散) が変わることを示すグリッド．
                A→分極，P100→高位合意，R→拡散 が paper の定性的対比．
    a_regimes : run×3  arithmetic A 固定で ε=0.05/0.15/0.25 を並べ，同一演算子で
                ε を上げると 多元 → 分極 → 合意 へ遷移する三相転移を示す．
    sweep     : sweep  n=625 ε∈[0.02,0.30] step=0.02 runs(=10/quick 3)，
                A/G/H/P0.01/P100/R．占有クラス数 vs ε の相図と合意ブリンク ε*
                の演算子間比較 (論文 Observation 1, Fact 4 / Fig. 4–7 風)．

各 spec は観測値 (占有クラス数・相) を論文の期待相と突き合わせ，PASS/off を
`reproduce_summary.json` に記録する．

Usage:
    uv run hegselmann-tools reproduce
    uv run hegselmann-tools reproduce --quick               # 軽量版 (動作確認用)
    uv run hegselmann-tools reproduce --specs operators,sweep
    uv run hegselmann-tools reproduce --output-dir results --workspace-root .
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Callable

import matplotlib.pyplot as plt
import numpy as np

from hegselmann_tools.visualize import load_metrics, load_opinions, to_wide
from hegselmann_tools.visualize_sweep import (
    aggregate,
    color_for,
    consensus_brink,
    load_summary,
)

# --------------------------------------------------------------------------- #
# 表示設定 (CJK フォントが利用不能でも落ちないように try)
# --------------------------------------------------------------------------- #
try:
    plt.rcParams["font.family"] = "Hiragino Sans"
except Exception:  # pragma: no cover - フォント未インストール環境用フォールバック
    pass

COLOR_BG = "#FAFAF8"
COLOR_TRAJ = "#2196F3"
COLOR_CLUSTER = "#534AB7"

# --------------------------------------------------------------------------- #
# 共通定数 / プロジェクトルート解決
# --------------------------------------------------------------------------- #

# このモジュールは tools/src/hegselmann_tools/reproduce_paper.py にある．
# parents[3] が workspace ルート (= cargo workspace ルート)．
# 環境変数 HEGSELMANN_PROJECT_ROOT で上書き可能．
_env_root = os.environ.get("HEGSELMANN_PROJECT_ROOT")
if _env_root:
    PROJECT_ROOT = Path(_env_root).resolve()
else:
    PROJECT_ROOT = Path(__file__).resolve().parents[3]


# --------------------------------------------------------------------------- #
# Figure 仕様データ構造
# --------------------------------------------------------------------------- #


@dataclass
class FigureSpec:
    """1 つの Figure を生成するための実行仕様．

    Attributes:
        id: フィギュア ID (ファイル名・summary キーに使う)．
        subcommand: cargo のサブコマンド (`run` / `sweep`)．
        description: 1 行説明．
        output_basename: PNG ファイル名 (拡張子なし)．
        cli_args: 1 回限りの cargo 呼び出しの引数列．
        cli_args_list: 複数回呼び出す場合の引数列のリスト (グリッド用)．
        panel_labels: cli_args_list 各実行のサブタイトル (グリッドの各パネル用)．
        expected: 各実行の期待相 (panel ごとの期待 phase 文字列のリスト)．
            観測相と突き合わせて PASS/off を判定する．
        render: 描画関数 (各 spec ごとに渡す)．
    """

    id: str
    subcommand: str
    description: str
    output_basename: str
    cli_args: list[str] | None = None
    cli_args_list: list[list[str]] | None = None
    panel_labels: list[str] | None = None
    expected: list[str] | None = None
    render: Callable[["FigureSpec", list[Path], Path], Path] | None = None
    extra: dict = field(default_factory=dict)


# --------------------------------------------------------------------------- #
# cargo 呼び出しヘルパ
# --------------------------------------------------------------------------- #


def ensure_build() -> None:
    """`cargo build --release` を 1 度だけ実行する (失敗時は例外)．"""
    print("=== cargo build --release ===")
    subprocess.run(
        ["cargo", "build", "--release"],
        cwd=PROJECT_ROOT,
        check=True,
    )


def run_cargo(args: list[str], output_dir: Path) -> Path:
    """`cargo run --release -- ...` を呼び出し，生成されたタイムスタンプサブ
    ディレクトリ (`results/<ts>` / `results/<ts>_sweep`) を返す．

    Rust 側は秒解像度のタイムスタンプ (`%Y%m%d_%H%M%S`) でディレクトリを作る
    ため，同一秒内に複数 spec を連続で走らせるとディレクトリ名が衝突する．本
    ラッパは呼び出し直前に秒境界へスリープし，呼び出し時刻より新しい mtime を
    持つディレクトリを「この呼び出しの出力」として採用する．

    Args:
        args: cargo の `--` 以降に渡す引数列．先頭は `run` / `sweep`．
        output_dir: `--output-dir` に渡すディレクトリ (workspace 相対 or 絶対)．

    Returns:
        生成された結果ディレクトリ (絶対パス)．
    """
    output_dir_str = str(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    is_sweep = bool(args) and args[0] == "sweep"

    # 秒境界に確実に乗るよう，次の秒に入るまで小さく待つ (最大 1 秒)．
    now = time.time()
    sleep_for = 1.05 - (now - int(now))
    if sleep_for > 0:
        time.sleep(sleep_for)

    call_start = time.time()

    cmd = (
        ["cargo", "run", "--release", "--quiet", "--"]
        + args
        + ["--output-dir", output_dir_str]
    )
    subprocess.run(cmd, cwd=PROJECT_ROOT, check=True, stdout=subprocess.DEVNULL)

    # 呼び出し開始時刻より新しい mtime を持つディレクトリを探す．
    candidates = []
    for p in output_dir.iterdir():
        if not p.is_dir() or p.name == "latest" or p.name.startswith("reproduce_"):
            continue
        # sweep のときは `*_sweep` のみ，run のときは `_sweep` を除外．
        if is_sweep and not p.name.endswith("_sweep"):
            continue
        if (not is_sweep) and p.name.endswith("_sweep"):
            continue
        try:
            mtime = p.stat().st_mtime
        except OSError:
            continue
        if mtime + 1e-6 >= call_start:
            candidates.append((mtime, p))

    if not candidates:
        raise RuntimeError(
            f"cargo 呼び出し後に新規サブディレクトリが見つかりません: {output_dir} "
            f"(args={args})"
        )
    candidates.sort(key=lambda x: x[0])
    return candidates[-1][1]


# --------------------------------------------------------------------------- #
# 観測量の抽出 (相 / 占有クラス数)
# --------------------------------------------------------------------------- #

# 占有クラス数 → 相ラベルのしきい値．R のような「収束しない拡散状態」は占有
# クラス数が非常に大きくなるので diffuse として別扱いする．
_DIFFUSE_THRESHOLD = 20


def classify_phase(n_occupied: int) -> str:
    """占有クラス数から相ラベルを返す (consensus/polarization/plurality/diffuse)．"""
    if n_occupied >= _DIFFUSE_THRESHOLD:
        return "diffuse"
    if n_occupied <= 1:
        return "consensus"
    if n_occupied == 2:
        return "polarization"
    return "plurality"


def observe_run(run_dir: Path) -> dict:
    """run 結果ディレクトリの最終メトリクスを観測する．"""
    df_m = load_metrics(str(run_dir / "metrics.csv"))
    last = df_m.iloc[-1]
    n_occ = int(last["n_occupied_classes"])
    return {
        "n_occupied_classes": n_occ,
        "mean_opinion": float(last["mean"]),
        "variance": float(last["variance"]),
        "observed_phase": classify_phase(n_occ),
    }


# --------------------------------------------------------------------------- #
# 描画関数
# --------------------------------------------------------------------------- #


def _draw_trajectory(ax, run_dir: Path, title: str) -> None:
    """1 つの run の意見軌跡を 1 つの軸に描く (グリッドのパネル用)．"""
    ax.set_facecolor(COLOR_BG)
    df_op = load_opinions(str(run_dir / "opinions.csv"))
    ts, mat = to_wide(df_op)
    n_agents = mat.shape[1]
    alpha = max(0.04, min(0.6, 30.0 / max(n_agents, 1)))
    lw = 0.5 if n_agents > 200 else 0.9
    for i in range(n_agents):
        ax.plot(ts, mat[:, i], color=COLOR_TRAJ, alpha=alpha, lw=lw)
    # 最終クラスタ中心を水平線で強調 (1e-3 解像度で連続値をビン化)．
    final = np.sort(mat[-1])
    if final.size > 0:
        bucket = [final[0]]
        centers: list[float] = []
        for x in final[1:]:
            if abs(x - bucket[-1]) <= 1e-3:
                bucket.append(x)
            else:
                centers.append(float(np.mean(bucket)))
                bucket = [x]
        centers.append(float(np.mean(bucket)))
        # クラスタが多すぎる (拡散) 場合は線を引かない (視認性のため)．
        if len(centers) <= 8:
            for c in centers:
                ax.axhline(c, color=COLOR_CLUSTER, lw=1.0, alpha=0.5, linestyle="--")
    ax.set_ylim(-0.02, 1.02)
    ax.set_xlim(ts.min(), ts.max())
    ax.set_title(title, fontsize=10)
    ax.grid(True, alpha=0.3)


def _render_operator_grid(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path
) -> Path:
    """論文 §3 / Fig. 4–7 風: 同一 ε で演算子を切り替えたときの相のグリッド．"""
    n = len(run_dirs)
    ncols = 3
    nrows = (n + ncols - 1) // ncols
    fig, axes = plt.subplots(
        nrows, ncols, figsize=(4.2 * ncols, 3.4 * nrows),
        facecolor=COLOR_BG, sharex=False, sharey=True,
    )
    fig.suptitle(
        "Hegselmann–Krause (2005) — 同一 ε=0.15 で平均演算子を切替 (相の対比)",
        fontsize=14,
    )
    axes_flat = np.atleast_1d(axes).flatten()
    labels = spec.panel_labels or [f"run{i}" for i in range(n)]
    for ax, rd, label in zip(axes_flat, run_dirs, labels):
        obs = observe_run(rd)
        title = (
            f"{label}\n占有クラス {obs['n_occupied_classes']} — {obs['observed_phase']}"
            f"  (x̄={obs['mean_opinion']:.2f})"
        )
        _draw_trajectory(ax, rd, title)
    # 余った軸を消す．
    for ax in axes_flat[n:]:
        ax.axis("off")
    for ax in axes_flat[:n]:
        ax.set_xlabel("時刻 t")
    for r in range(nrows):
        axes_flat[r * ncols].set_ylabel("意見 x ∈ [0, 1]")
    fig.tight_layout()
    out_path = figures_dir / f"{spec.output_basename}.png"
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")
    return out_path


def _render_a_regimes(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path
) -> Path:
    """算術平均 A 固定で ε を上げたときの 多元→分極→合意 三相転移の 1×3 パネル．"""
    n = len(run_dirs)
    fig, axes = plt.subplots(
        1, n, figsize=(4.6 * n, 4.2), facecolor=COLOR_BG, sharey=True,
    )
    fig.suptitle(
        "Hegselmann–Krause (2005) — 算術平均 A: ε を上げると 多元→分極→合意",
        fontsize=14,
    )
    axes_flat = np.atleast_1d(axes).flatten()
    labels = spec.panel_labels or [f"run{i}" for i in range(n)]
    for ax, rd, label in zip(axes_flat, run_dirs, labels):
        obs = observe_run(rd)
        title = (
            f"{label}\n占有クラス {obs['n_occupied_classes']} — {obs['observed_phase']}"
        )
        _draw_trajectory(ax, rd, title)
        ax.set_xlabel("時刻 t")
    axes_flat[0].set_ylabel("意見 x ∈ [0, 1]")
    fig.tight_layout()
    out_path = figures_dir / f"{spec.output_basename}.png"
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")
    return out_path


def _render_sweep(
    spec: FigureSpec, run_dirs: list[Path], figures_dir: Path
) -> Path:
    """ε 走査の占有クラス数 相図 + 合意ブリンク棒グラフ (論文 Fig. 4–7 風)．

    `visualize_sweep` の集計・配色・ブリンク推定 (`aggregate` / `color_for` /
    `consensus_brink`) を再利用し，相図とブリンク棒グラフを 1 枚の 2 連パネルに
    並べる．
    """
    assert len(run_dirs) == 1, "sweep には run_dirs 1 個が必要"
    sweep_dir = run_dirs[0]
    df = load_summary(str(sweep_dir))
    agg = aggregate(df)
    eps_max = float(df["eps"].max())

    out_path = figures_dir / f"{spec.output_basename}.png"
    means = list(dict.fromkeys(agg["mean"].tolist()))

    fig, axes = plt.subplots(1, 2, figsize=(15, 6), facecolor=COLOR_BG)
    fig.suptitle(
        "Hegselmann–Krause (2005) — 演算子ごとの占有クラス数 vs ε と合意ブリンク ε*",
        fontsize=14,
    )

    ax = axes[0]
    ax.set_facecolor(COLOR_BG)
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
    ax.set_title("占有クラス数 vs ε (演算子ごとの相図)", fontsize=12)
    ax.legend(fontsize=9, title="平均")
    ax.grid(True, alpha=0.3, which="both")

    ax = axes[1]
    ax.set_facecolor(COLOR_BG)
    brinks: list[float] = []
    labels: list[str] = []
    colors: list[str] = []
    for idx, mean in enumerate(means):
        b = consensus_brink(agg, mean)
        labels.append(mean)
        colors.append(color_for(mean, idx))
        brinks.append(b if b is not None else eps_max * 1.05)
    ax.bar(labels, brinks, color=colors, alpha=0.85)
    for idx, mean in enumerate(means):
        b = consensus_brink(agg, mean)
        txt = f"{brinks[idx]:.3f}" if b is not None else "未到達"
        ax.text(idx, brinks[idx] + eps_max * 0.01, txt, ha="center",
                va="bottom", fontsize=9)
    ax.set_xlabel("平均演算子")
    ax.set_ylabel("合意ブリンク ε*")
    ax.set_title("合意ブリンク ε* の演算子間比較 (Observation 1, Fact 4)", fontsize=12)
    ax.grid(True, alpha=0.3, axis="y")

    fig.tight_layout()
    fig.savefig(out_path, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  保存: {out_path}")

    # 観測ブリンクを spec.extra に控える (summary 用)．
    spec.extra["brinks"] = {
        mean: (consensus_brink(agg, mean)) for mean in means
    }
    return out_path


# --------------------------------------------------------------------------- #
# Figure 仕様カタログ
# --------------------------------------------------------------------------- #


def _build_specs(*, quick: bool) -> list[FigureSpec]:
    """論文の主要 Figure 仕様カタログを構築する (quick モードで一部軽量化)．"""

    n_run = 200 if quick else 625
    max_iter_run = 200
    max_iter_r = 200 if quick else 400

    # --- operators: 同一 ε=0.15 で 6 演算子を並べる (中心的主張) ---
    op_specs = [
        ("A", "A (算術 = P_1)", "polarization"),
        ("G", "G (幾何 = P_0)", "polarization"),
        ("H", "H (調和 = P_-1)", "plurality"),
        ("P0.01", "P_0.01 (≈幾何)", "polarization"),
        ("P100", "P_100 (≈最大)", "consensus"),
        ("R", "R (ランダム)", "diffuse"),
    ]
    op_args_list = []
    for mean, _label, _exp in op_specs:
        mi = max_iter_r if mean == "R" else max_iter_run
        op_args_list.append(
            [
                "run",
                "--n", str(n_run),
                "--eps", "0.15",
                "--mean", mean,
                "--start", "uniform",
                "--max-iterations", str(mi),
                "--seed", "42",
            ]
        )
    operators = FigureSpec(
        id="operators",
        subcommand="run",
        description=(
            f"n={n_run}, ε=0.15 固定で A/G/H/P0.01/P100/R を比較 — "
            "演算子で相が変わる (A:分極 / P100:高位合意 / R:拡散)"
        ),
        output_basename="operators_eps0.15_grid",
        cli_args_list=op_args_list,
        panel_labels=[lbl for _m, lbl, _e in op_specs],
        expected=[exp for _m, _lbl, exp in op_specs],
        render=_render_operator_grid,
    )

    # --- a_regimes: 算術 A で ε を上げて 多元→分極→合意 ---
    a_eps = [("0.05", "ε=0.05", "plurality"),
             ("0.15", "ε=0.15", "polarization"),
             ("0.25", "ε=0.25", "consensus")]
    a_args_list = [
        [
            "run",
            "--n", str(n_run),
            "--eps", e,
            "--mean", "A",
            "--start", "uniform",
            "--max-iterations", str(max_iter_run),
            "--seed", "42",
        ]
        for e, _lbl, _exp in a_eps
    ]
    a_regimes = FigureSpec(
        id="a_regimes",
        subcommand="run",
        description=(
            f"n={n_run}, 算術 A 固定で ε=0.05/0.15/0.25 — 多元→分極→合意 の三相転移"
        ),
        output_basename="a_regimes_eps_sweep",
        cli_args_list=a_args_list,
        panel_labels=[lbl for _e, lbl, _exp in a_eps],
        expected=[exp for _e, _lbl, exp in a_eps],
        render=_render_a_regimes,
    )

    # --- sweep: ε 走査の相図 + 合意ブリンク (Observation 1, Fact 4) ---
    runs_sweep = 3 if quick else 10
    n_sweep = 200 if quick else 625
    eps_step = "0.04" if quick else "0.02"
    sweep_args = [
        "sweep",
        "--n", str(n_sweep),
        "--eps-min", "0.02",
        "--eps-max", "0.30",
        "--eps-step", eps_step,
        "--means", "A,G,H,P0.01,P100,R",
        "--runs", str(runs_sweep),
        "--start", "uniform",
        "--max-iterations", str(max_iter_run),
        "--seed", "42",
    ]
    sweep = FigureSpec(
        id="sweep",
        subcommand="sweep",
        description=(
            f"n={n_sweep}, ε∈[0.02,0.30] step={eps_step}, runs={runs_sweep}, "
            "A/G/H/P0.01/P100/R — 占有クラス数 相図 + 合意ブリンク ε*"
        ),
        output_basename="sweep_phase_diagram",
        cli_args=sweep_args,
        render=_render_sweep,
    )

    return [operators, a_regimes, sweep]


# --------------------------------------------------------------------------- #
# 実行ドライバ
# --------------------------------------------------------------------------- #


def _execute_spec(spec: FigureSpec, cargo_output_dir: Path) -> tuple[list[Path], list[str]]:
    """1 つの spec に対して cargo を必要回数呼び出し，run ディレクトリを返す．"""
    invocations: list[str] = []
    run_dirs: list[Path] = []
    if spec.cli_args_list is not None:
        for args in spec.cli_args_list:
            invocations.append("cargo run --release -- " + " ".join(args))
            run_dirs.append(run_cargo(args, cargo_output_dir))
    elif spec.cli_args is not None:
        invocations.append("cargo run --release -- " + " ".join(spec.cli_args))
        run_dirs.append(run_cargo(spec.cli_args, cargo_output_dir))
    else:
        raise ValueError(f"{spec.id}: cli_args / cli_args_list のどちらかが必要")
    return run_dirs, invocations


def _evaluate_spec(spec: FigureSpec, run_dirs: list[Path]) -> dict:
    """観測相を期待相と突き合わせ，per-run の比較 + PASS/off を構築する．"""
    if spec.subcommand == "sweep":
        # sweep は brinks (render が spec.extra に控える) を主結果にする．
        return {
            "kind": "sweep",
            "brinks": spec.extra.get("brinks", {}),
        }
    comparisons = []
    expected = spec.expected or [None] * len(run_dirs)
    labels = spec.panel_labels or [f"run{i}" for i in range(len(run_dirs))]
    n_pass = 0
    for rd, exp, label in zip(run_dirs, expected, labels):
        obs = observe_run(rd)
        ok = (exp is None) or (obs["observed_phase"] == exp)
        if ok:
            n_pass += 1
        comparisons.append({
            "label": label,
            "expected_phase": exp,
            "observed_phase": obs["observed_phase"],
            "n_occupied_classes": obs["n_occupied_classes"],
            "mean_opinion": round(obs["mean_opinion"], 4),
            "variance": obs["variance"],
            "pass": ok,
        })
    return {
        "kind": "run_grid",
        "comparisons": comparisons,
        "n_pass": n_pass,
        "n_total": len(comparisons),
        "verdict": "PASS" if n_pass == len(comparisons) else "off",
    }


def reproduce(
    spec_ids: list[str] | None,
    output_root: Path,
    cargo_output_dir: Path,
    quick: bool,
    skip_build: bool,
) -> dict:
    """指定 spec を順に実行し，まとめた結果サマリを返す．"""
    all_specs = _build_specs(quick=quick)
    if spec_ids:
        wanted = set(spec_ids)
        specs = [s for s in all_specs if s.id in wanted]
        unknown = wanted - {s.id for s in all_specs}
        if unknown:
            raise ValueError(
                f"未知の spec ID: {sorted(unknown)}．"
                f"利用可能: {[s.id for s in all_specs]}"
            )
    else:
        specs = all_specs

    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base_dir = output_root / f"reproduce_{timestamp}"
    figures_dir = base_dir / "figures"
    base_dir.mkdir(parents=True, exist_ok=True)
    figures_dir.mkdir(parents=True, exist_ok=True)

    print("=== Hegselmann & Krause (2005) 論文 Figure 一括再現 ===")
    print(f"    出力ルート     : {base_dir}")
    print(f"    cargo 出力先   : {cargo_output_dir}")
    print(f"    Figure 出力先  : {figures_dir}")
    print(f"    quick モード   : {quick}")
    print(f"    対象 spec      : {[s.id for s in specs]}")
    print("-------------------------------------------")

    if not skip_build:
        ensure_build()

    spec_results: dict[str, dict] = {}
    for spec in specs:
        print(f"--- {spec.id}: {spec.description} ---")
        t0 = time.monotonic()
        try:
            run_dirs, invocations = _execute_spec(spec, cargo_output_dir)
            cargo_elapsed = time.monotonic() - t0

            t1 = time.monotonic()
            figure_path = spec.render(spec, run_dirs, figures_dir) if spec.render else None
            render_elapsed = time.monotonic() - t1

            evaluation = _evaluate_spec(spec, run_dirs)

            elapsed = time.monotonic() - t0
            spec_results[spec.id] = {
                "id": spec.id,
                "description": spec.description,
                "subcommand": spec.subcommand,
                "cargo_invocations": invocations,
                "run_dirs": [str(p) for p in run_dirs],
                "figure_path": str(figure_path) if figure_path else None,
                "evaluation": evaluation,
                "status": "ok",
                "cargo_seconds": round(cargo_elapsed, 3),
                "render_seconds": round(render_elapsed, 3),
                "total_seconds": round(elapsed, 3),
            }
            verdict = evaluation.get("verdict", "")
            print(
                f"  {spec.id} done in {elapsed:.2f}s "
                f"(cargo {cargo_elapsed:.2f}s + render {render_elapsed:.2f}s)"
                + (f" — {verdict}" if verdict else "")
            )
            if evaluation.get("kind") == "run_grid":
                for c in evaluation["comparisons"]:
                    flag = "PASS" if c["pass"] else "off "
                    print(
                        f"      [{flag}] {c['label']:<16} "
                        f"期待={c['expected_phase']:<13} 観測={c['observed_phase']:<13} "
                        f"(占有クラス {c['n_occupied_classes']})"
                    )
            elif evaluation.get("kind") == "sweep":
                for mean, b in evaluation["brinks"].items():
                    bs = f"{b:.4f}" if b is not None else "未到達"
                    print(f"      合意ブリンク {mean:<6} ε* = {bs}")
        except Exception as e:  # noqa: BLE001
            elapsed = time.monotonic() - t0
            spec_results[spec.id] = {
                "id": spec.id,
                "description": spec.description,
                "subcommand": spec.subcommand,
                "status": "error",
                "error": repr(e),
                "total_seconds": round(elapsed, 3),
            }
            print(f"  {spec.id} failed: {e}", file=sys.stderr)

    summary = {
        "timestamp": timestamp,
        "quick": quick,
        "project_root": str(PROJECT_ROOT),
        "base_dir": str(base_dir),
        "figures_dir": str(figures_dir),
        "cargo_output_dir": str(cargo_output_dir),
        "specs": list(spec_results.values()),
    }
    summary_path = base_dir / "reproduce_summary.json"
    with summary_path.open("w") as f:
        json.dump(summary, f, indent=2, ensure_ascii=False)

    print("-------------------------------------------")
    n_ok = sum(1 for r in spec_results.values() if r.get("status") == "ok")
    n_err = sum(1 for r in spec_results.values() if r.get("status") == "error")
    print(f"完了: ok={n_ok}, error={n_err}")
    print(f"サマリ → {summary_path}")
    print(f"図一覧 → {figures_dir}")
    for f in sorted(figures_dir.iterdir()):
        if f.is_file():
            size_kb = f.stat().st_size / 1024
            print(f"    {f.name:45s} ({size_kb:6.1f} KB)")

    return summary


# --------------------------------------------------------------------------- #
# CLI
# --------------------------------------------------------------------------- #


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="hegselmann-tools reproduce",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument(
        "--specs", default=None,
        help=(
            "カンマ区切りで実行する spec ID (例: operators,sweep)．"
            "未指定時は全 spec を実行する．利用可能: operators,a_regimes,sweep"
        ),
    )
    p.add_argument(
        "--output-dir", "--output_dir", default="results",
        help=(
            "結果出力ルート (workspace ルートからの相対パス)．PNG とサマリは "
            "ここの reproduce_<ts>/ 配下に保存される (default: results)"
        ),
    )
    p.add_argument(
        "--cargo-output-dir", "--cargo_output_dir", default=None,
        help=(
            "cargo の --output-dir に渡すパス．未指定時は --output-dir と同じ "
            "(Rust 出力は results/<inner_ts>/ に置かれる)．"
        ),
    )
    p.add_argument(
        "--workspace-root", "--workspace_root", default=None,
        help=(
            "workspace ルート (絶対パス)．未指定時は本モジュールの位置から推定する "
            "(環境変数 HEGSELMANN_PROJECT_ROOT でも上書き可)．"
        ),
    )
    p.add_argument(
        "--quick", action="store_true",
        help=(
            "簡略化モード: n=200, sweep runs=3 / step=0.04 に縮小実行する．動作確認用．"
            "論文値の検証には使わない．"
        ),
    )
    p.add_argument(
        "--skip-build", action="store_true",
        help="cargo build --release をスキップ (事前にビルド済みのとき)．",
    )
    return p.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)

    global PROJECT_ROOT
    if args.workspace_root:
        PROJECT_ROOT = Path(args.workspace_root).resolve()

    if shutil.which("cargo") is None:
        print(
            "エラー: cargo コマンドが見つかりません．Rust toolchain をインストールしてください．",
            file=sys.stderr,
        )
        return 2

    output_root = Path(args.output_dir)
    if not output_root.is_absolute():
        output_root = PROJECT_ROOT / output_root

    if args.cargo_output_dir is not None:
        cargo_output_dir = Path(args.cargo_output_dir)
    else:
        cargo_output_dir = output_root
    if not cargo_output_dir.is_absolute():
        cargo_output_dir = PROJECT_ROOT / cargo_output_dir

    spec_ids = None
    if args.specs:
        spec_ids = [s.strip() for s in args.specs.split(",") if s.strip()]

    try:
        summary = reproduce(
            spec_ids=spec_ids,
            output_root=output_root,
            cargo_output_dir=cargo_output_dir,
            quick=args.quick,
            skip_build=args.skip_build,
        )
    except Exception as e:  # noqa: BLE001
        print(f"エラー: 再現実行に失敗しました: {e}", file=sys.stderr)
        return 1

    n_err = sum(1 for r in summary["specs"] if r.get("status") == "error")
    return 0 if n_err == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
