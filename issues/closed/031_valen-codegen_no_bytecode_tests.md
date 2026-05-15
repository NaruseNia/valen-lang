---
scope: valen-codegen
severity: major
dimension: test_coverage
---

# No bytecode-level correctness tests

## 概要
テストがメタデータ（method数, field数, access flags）のみ検証。実際の命令列やスタックバランスは未検証。

## 改善案
命令列検証テストを追加。

## 影響範囲
- crates/valen-codegen/
