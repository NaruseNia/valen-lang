---
scope: valenc
severity: major
dimension: correctness
---

# Coherence check passes empty imports slice

## 概要
check_coherence(&hir, &[]) で import リスト未渡し。classpath の Java 型に対する orphan rule 違反を検出不能。

## 改善案
resolve_result からインポート情報を抽出して渡す。

## 影響範囲
- crates/valenc/
