---
scope: valen-ast
severity: minor
dimension: design
---

# Type Span Returns Option Unnecessarily

## 概要

Type::span() returns Option<Span> but every variant always returns Some. Parser duplicates as type_span().

## 現状

crates/valen-ast/src/lib.rs:L523-L534

## 問題点

Misleading API, DRY violation.

## 改善案

Change return type to Span.

## 影響範囲

- crates/valen-ast/src/lib.rs:L523-L534

## 関連ファイル

- crates/valen-parser/src/parser.rs:L2520
