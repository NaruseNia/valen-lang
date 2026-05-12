---
scope: valenc
severity: major
dimension: spec_coverage
---

# check command skips coherence checking

## 概要

check サブコマンドが parse, resolve, type-check のみ実行し coherence をスキップ。orphan rule 違反が build でしか検出されない。

## 改善案

check 関数に check_coherence 呼び出しを追加。

## 影響範囲

- crates/valenc/src/main.rs
