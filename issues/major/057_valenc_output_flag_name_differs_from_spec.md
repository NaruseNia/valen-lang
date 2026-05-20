---
scope: valenc
severity: major
dimension: spec_coverage
---

# Output Flag Name Differs From Spec

## 概要

Spec says --output/-o but implementation uses --out/-o.

## 現状

crates/valenc/src/main.rs:L38

## 問題点

--output not accepted.

## 改善案

Change field to 'output' or add #[arg(long = "output")].

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

- docs/requirements/REQ-TOOL.md
