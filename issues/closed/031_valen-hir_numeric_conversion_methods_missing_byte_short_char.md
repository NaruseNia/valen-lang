---
scope: valen-hir
severity: major
dimension: spec_coverage
---

# Numeric Conversion Methods Missing Byte Short Char

## 概要

Spec mandates conversion methods for all numeric types but Byte, Short, Char have none in stdlib.

## 現状

stdlib/valen/core/core.vln:L158-L180

## 問題点

Cannot call .toInt() on Byte/Short values.

## 改善案

Add impl blocks for Byte, Short, Char in core.vln.

## 影響範囲

- stdlib/valen/core/core.vln

## 関連ファイル

- docs/lang/02-types.md
