---
scope: valenc
severity: major
dimension: test_coverage
---

# No multi-file compilation tests

## 概要
REQ-TOOL-001 が複数ファイル一括コンパイルを規定。テストは全て単一ファイル。

## 改善案
マルチファイルフィクスチャと相互参照テストを追加。

## 影響範囲
- crates/valenc/
