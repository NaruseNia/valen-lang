---
scope: valenfmt
severity: major
dimension: spec_coverage
---

# --check mode ignored for stdin input

## 概要
stdin 入力時に --check フラグが完全無視。フォーマット結果が常に stdout に出力。

## 改善案
stdin ブランチでも cli.check を確認し、差分時 exit 1。

## 影響範囲
- crates/valenfmt/
