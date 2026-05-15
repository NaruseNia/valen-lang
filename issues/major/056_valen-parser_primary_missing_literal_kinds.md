---
scope: valen-parser
severity: major
dimension: correctness
---

# parse_primary missing LongLit/FloatLit/CharLit handling

## 概要
parse_primary() が IntLit/DoubleLit/StringLit/BoolLit のみ処理。Long/Float/Char lexing 実装後にこれらの式がパースエラー。

## 改善案
TokenKind::LongLit, FloatLit, CharLit の match arm を parse_primary() に追加。

## 影響範囲
- crates/valen-parser/
