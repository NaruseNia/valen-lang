---
scope: valen-hir
severity: critical
dimension: spec_coverage
---

# No Open Class Inheritance Check

## 概要

Classes are spec-final by default but inheriting from final class silently succeeds. No open/abstract/sealed validation.

## 現状

crates/valen-hir/src/resolve.rs:L302-L329

## 問題点

class Foo {} class Bar : Foo {} compiles without error.

## 改善案

Add validation pass checking superclass ClassDefKind is Open/Abstract/Sealed.

## 影響範囲

- crates/valen-hir/src/resolve.rs

## 関連ファイル

- docs/lang/05-classes.md
