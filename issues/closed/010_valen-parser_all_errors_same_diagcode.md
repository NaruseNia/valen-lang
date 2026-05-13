---
scope: valen-parser
severity: major
dimension: error_handling
---

# All parser errors use same DiagCode (V0101)

## 概要

ほぼ全エラーが PARSE_EXPECTED_EXPR (V0101) を使用。セミコロン不足、識別子不足、クラス名不足も同じコード。

## 改善案

PARSE_EXPECTED_TOKEN (V0102), PARSE_EXPECTED_IDENT (V0103) 等を追加。

## 影響範囲

- crates/valen-parser/src/parser.rs:L1341-L1367
