"""hegselmann-tools show-experiment-settings — 実行結果の設定表示．

runvault の run ディレクトリの config.json (封筒．条件は `parameters` の下) を読み，
実行時に使われた全パラメータを整形表示する．run / sweep / sweep-point のどれかは
run.json の `subcommand` が答える．runvault 以前の flat な config.json /
sweep_config.json も読める．

run ディレクトリのパスは次で取れる:
    runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone
    runvault path --experiment hegselmann-averaging --latest --subcommand sweep

Usage:
    hegselmann-tools show-experiment-settings
    hegselmann-tools show-experiment-settings --results-dir "$(runvault path --experiment hegselmann-averaging --latest --subcommand run --standalone)"
    hegselmann-tools show-experiment-settings --json

シンボリックリンクの解決は共有ヘルパ `socsim_tools.io.resolve_results_dir` に委譲する．
run / sweep の設定テーブルは本モデル固有の整形 (run の「平均演算子 (p=…)」複合行，
sweep の ε 走査・平均リスト連結) を含むため repo 固有のまま残す (本 repo は非 LLM
モデルで run_metadata を持たないため，metadata ブロックも `--json` の構造も従来と同一)．
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from runvault.read import config_parameters, load_run_meta, runvault_path
from socsim_tools.io import resolve_results_dir

from hegselmann_tools.experiment import EXPERIMENT


def _load_config(results_dir: Path) -> tuple[dict, Path, str]:
    """run ディレクトリの実験条件と，それがどのサブコマンドのものかを返す．

    runvault の config.json は封筒で，条件は `parameters` の下にある．どの
    サブコマンドかは run.json が答える (`sweep_config.json` はもう書かれない)．
    """
    # 設定が無いことは «まだ sweep_config.json の方かもしれない» という意味なので，
    # ここでは欠落を失敗として扱わない (下で sweep_config.json を見る)．
    params = config_parameters(results_dir, required=False)
    if params is not None:
        meta = load_run_meta(results_dir, required=False)
        if meta is not None:
            kind = str(meta.get("subcommand", "run"))
        else:
            # legacy: 自前で書いていた config.json は "command" を持つ
            kind = "sweep" if params.get("command") == "sweep" else "run"
        return params, results_dir / "config.json", kind

    sweep_cfg = results_dir / "sweep_config.json"
    if sweep_cfg.exists():
        with sweep_cfg.open() as f:
            return json.load(f), sweep_cfg, "sweep"

    raise FileNotFoundError(
        f"設定ファイルが見つかりません: {results_dir}\n"
        f"  期待されるファイル: config.json (runvault の封筒 / legacy の flat) "
        f"または sweep_config.json (legacy の sweep)"
    )


def render_run_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 70)
    lines.append("実行設定 (run)")
    lines.append("=" * 70)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 70)
    lines.append(f"エージェント数 n : {cfg.get('n', '-')}")
    lines.append(f"信頼水準 ε       : {cfg.get('eps', '-')}")
    mean = cfg.get("mean", "-")
    p = cfg.get("p")
    if p is not None:
        lines.append(f"平均演算子       : {mean}  (p = {p})")
    else:
        lines.append(f"平均演算子       : {mean}")
    lines.append(f"初期分布         : {cfg.get('start_profile', '-')}")
    lines.append(f"最大反復         : {cfg.get('max_iterations', '-')}")
    lines.append(f"収束許容誤差 tol : {cfg.get('tol', '-')}")
    lines.append(f"シード           : {cfg.get('seed', '-')}")
    # 出力先は run ディレクトリそのものなので条件には含まれない (legacy のみ持つ)．
    if cfg.get("output_dir") is not None:
        lines.append(f"出力先           : {cfg['output_dir']}")
    lines.append("=" * 70)
    return "\n".join(lines)


def render_sweep_config(cfg: dict, source: Path) -> str:
    lines: list[str] = []
    lines.append("=" * 70)
    lines.append("実行設定 (sweep)")
    lines.append("=" * 70)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 70)
    lines.append(
        f"ε 走査           : {cfg.get('eps_min', '-')}:{cfg.get('eps_max', '-')}:{cfg.get('eps_step', '-')}"
    )
    means = cfg.get("means", [])
    lines.append(f"平均演算子       : {', '.join(means) if means else '-'}")
    lines.append(f"エージェント数 n : {cfg.get('n', '-')}")
    lines.append(f"試行数 runs      : {cfg.get('runs', '-')}")
    lines.append(f"初期分布         : {cfg.get('start_profile', '-')}")
    lines.append(f"最大反復         : {cfg.get('max_iterations', '-')}")
    lines.append(f"収束許容誤差 tol : {cfg.get('tol', '-')}")
    lines.append(f"シード基点       : {cfg.get('seed', '-')}")
    lines.append("=" * 70)
    return "\n".join(lines)


def render_sweep_point_config(cfg: dict, source: Path) -> str:
    """スイープの子 run ((平均, ε) 1 点 × runs 試行) の条件．"""
    lines: list[str] = []
    lines.append("=" * 70)
    lines.append("実行設定 (sweep-point — スイープの (平均, ε) 1 点)")
    lines.append("=" * 70)
    lines.append(f"設定ファイル: {source}")
    lines.append("-" * 70)
    lines.append(f"エージェント数 n : {cfg.get('n', '-')}")
    lines.append(f"信頼水準 ε       : {cfg.get('eps', '-')}")
    mean = cfg.get("mean", "-")
    p = cfg.get("p")
    if p is not None:
        lines.append(f"平均演算子       : {mean}  (p = {p})")
    else:
        lines.append(f"平均演算子       : {mean}")
    lines.append(f"初期分布         : {cfg.get('start_profile', '-')}")
    lines.append(f"試行数 runs      : {cfg.get('runs', '-')}")
    lines.append(f"最大反復         : {cfg.get('max_iterations', '-')}")
    lines.append(f"収束許容誤差 tol : {cfg.get('tol', '-')}")
    lines.append(f"シード基点       : {cfg.get('seed', '-')}")
    lines.append("=" * 70)
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="hegselmann-tools show-experiment-settings",
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--results-dir", "--results_dir",
        default=None,
        help=(
            "run ディレクトリ．未指定時は runvault に最新の run を聞く "
            f"(--experiment {EXPERIMENT} --subcommand run --standalone)．"
        ),
    )
    parser.add_argument(
        "--results-root", "--results_root", default="results",
        help="--results-dir 未指定時に runvault が探す results ルート (default: results)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="表ではなく JSON 形式で出力する．",
    )
    args = parser.parse_args(argv)

    if args.results_dir is None:
        results_dir = Path(
            runvault_path(
                EXPERIMENT, args.results_root, subcommand="run", standalone=True
            )
        )
    else:
        results_dir = resolve_results_dir(args.results_dir)
    if not results_dir.exists():
        print(f"エラー: ディレクトリが存在しません: {results_dir}", file=sys.stderr)
        return 1

    cfg, cfg_path, kind = _load_config(results_dir)

    if args.json:
        payload = {"source": str(cfg_path), "kind": kind, "config": cfg}
        print(json.dumps(payload, indent=2, ensure_ascii=False))
    elif kind == "run":
        print(render_run_config(cfg, cfg_path))
    elif kind == "sweep-point":
        print(render_sweep_point_config(cfg, cfg_path))
    else:
        print(render_sweep_config(cfg, cfg_path))
    return 0


if __name__ == "__main__":
    sys.exit(main())
