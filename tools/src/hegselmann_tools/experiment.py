"""runvault 上の実験名．

Rust 側の `record::EXPERIMENT` と同じ値でなければ `runvault path` が run を
見つけられない．4 つのツールが同じ文字列を書くと片方だけ直したときに気づけない
ので，1 箇所に置く．
"""

from __future__ import annotations

EXPERIMENT = "hegselmann-averaging"
