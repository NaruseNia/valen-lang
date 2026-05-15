---
scope: valen-codegen
severity: major
dimension: test_coverage
---

# No unit tests for expr.rs (~1800 lines)

## 概要
expr.rs がcrate最大ファイルだがユニットテスト0件。全テストが e2e 間接テストのみ。

## 改善案
各式種別の lower 関数にユニットテストを追加。

## 影響範囲
- crates/valen-codegen/
