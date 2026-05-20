---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Visibility Internal Not Enforced

## 概要

Internal visibility is 'currently treated as Pub'. No cross-module access checking.

## 現状

crates/valen-hir/src/lib.rs:L191

## 問題点

Module encapsulation not enforced.

## 改善案

Integrate check_visibility into type checker.

## 影響範囲

- crates/valen-hir/src/ty.rs

## 関連ファイル

- docs/lang/10-modules.md
