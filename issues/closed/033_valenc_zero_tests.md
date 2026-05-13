---
scope: valenc
severity: major
dimension: test_coverage
---

# valenc: Zero test coverage

## 概要

valenc crate にテストが0件。引数パース、終了コード、診断出力、複数ファイルコンパイルが未検証。

## 改善案

assert_cmd で CLI 統合テストを追加。

## 影響範囲

- crates/valenc/src/main.rs
