"""hegselmann-tools — Hegselmann & Krause (2005) 意見力学 ツール統合 CLI．

Usage:
    hegselmann-tools visualize [...]
    hegselmann-tools visualize-sweep [...]
    hegselmann-tools show-experiment-settings [...]

各サブコマンドに続く引数は，対応するモジュールの argparse がそのまま受け取る．
サブコマンドレベルで `--help` を付けると，そのサブコマンド自身のヘルプが表示される．

dispatcher の組み立ては共有ヘルパ `socsim_tools.cli.build_dispatcher` に委譲する
(prog 名・サブコマンド・ヘルプ文・argv ルーティングは従来と同一)．可視化/設定表示の
実体 (visualize / visualize_sweep / show_experiment_settings) は repo 固有のまま．
"""

from __future__ import annotations

from socsim_tools.cli import build_dispatcher

main = build_dispatcher(
    prog="hegselmann-tools",
    description="Hegselmann & Krause (2005) 意見力学 可視化・分析ツール",
    subcommands={
        "visualize": (
            "単一実行結果 (意見軌跡) の可視化",
            "hegselmann_tools.visualize:main",
        ),
        "visualize-sweep": (
            "スイープ結果 (占有クラス数の相図) の可視化",
            "hegselmann_tools.visualize_sweep:main",
        ),
        "show-experiment-settings": (
            "実行結果ディレクトリの設定 (config.json / sweep_config.json) の表示",
            "hegselmann_tools.show_experiment_settings:main",
        ),
    },
)


if __name__ == "__main__":
    main()
