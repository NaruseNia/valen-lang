---
scope: valen-codegen
severity: major
dimension: fixture_coverage
---

# No error-case fixtures

## 概要
32 e2e フィクスチャが全て正常系のみ。エラーパス（未解決型、codegen 失敗等）のテストなし。

## 改善案
エラーケースフィクスチャを追加。

## 影響範囲
- crates/valen-codegen/
