---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# internal visibility treated same as pub

## 概要

check_visibility が Vis::Internal を Vis::Pub と同一視。仕様 §10 では internal は同一モジュール内のみ可視。

## 改善案

モジュールコンテキストパラメータを追加し、internal のスコープを検証。

## 影響範囲

- crates/valen-hir/src/lib.rs:L66-L80
