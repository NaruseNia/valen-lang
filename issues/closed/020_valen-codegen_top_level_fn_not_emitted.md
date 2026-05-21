---
scope: valen-codegen
severity: major
dimension: spec_coverage
---

# Top Level Fn Not Emitted

## 概要

Top-level functions not emitted. lower_hir() skips DefKind::Fn entirely.

## 現状

crates/valen-codegen/src/lower.rs:L20-L68

## 問題点

Programs with only free-standing functions produce no bytecode.

## 改善案

Generate synthetic class to host top-level functions as static methods.

## 影響範囲

- crates/valen-codegen/src/lower.rs

## 関連ファイル

(none)
