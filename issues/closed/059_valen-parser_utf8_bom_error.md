---
scope: valen-parser
severity: minor
dimension: edge_case
---

# UTF-8 BOM がエラートークンとしてlexされる

## 概要

仕様はソースファイルを UTF-8 と定義するが、lexer の skip パターンに U+FEFF（BOM）が含まれない。BOM 付き UTF-8 ファイルでバイト0にエラートークンが生成されパース失敗。

## 改善案

lexer 構築前にオプショナルな先頭 BOM をスキップするか、whitespace ハンドリングに '\u{FEFF}' を含める。BOM 付きソースのテスト追加。

## 影響範囲

- crates/valen-parser/src/lexer.rs
