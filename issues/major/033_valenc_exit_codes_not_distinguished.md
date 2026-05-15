---
scope: valenc
severity: major
dimension: spec_coverage
---

# Exit codes not distinguished (1 vs 2)

## 概要
仕様は 0=成功, 1=コンパイルエラー, 2=CLI引数エラー。現状は anyhow::Result で全て exit 1。

## 改善案
process::exit(1) / process::exit(2) を明示的に使用。

## 影響範囲
- crates/valenc/
