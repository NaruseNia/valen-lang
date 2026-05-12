---
scope: valen-diagnostics
severity: major
dimension: idiomatic_rust
---

# No Display impl for Severity, DiagCode, Diagnostic

## 概要

Severity, DiagCode, Diagnostic に Display 実装がなく、ユーザー向けエラー表示を消費者がアドホックに実装する必要がある。

## 改善案

Display を実装: DiagCode → `V{:04}`, Severity → 小文字文字列, Diagnostic → `{severity}[{code}]: {message}`。

## 影響範囲

- crates/valen-diagnostics/src/lib.rs
