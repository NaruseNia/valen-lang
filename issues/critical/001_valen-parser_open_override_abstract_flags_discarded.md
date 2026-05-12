---
scope: valen-parser
severity: critical
dimension: correctness
---

# open/override/abstract flags silently discarded in class methods

## 概要

`parse_class_body` で `is_open`, `is_override`, `is_abstract` フラグがパースされるが、`let _ = (is_open, is_override, is_abstract)` で即座に破棄される。メソッド修飾子が黙って消失する。

## 現状

crates/valen-parser/src/parser.rs:L353-L356

```rust
let is_open = self.eat(&TokenKind::Open).is_some();
let is_override = self.eat(&TokenKind::Override).is_some();
let is_abstract = self.eat(&TokenKind::Abstract).is_some();
let _ = (is_open, is_override, is_abstract);
```

## 問題点

`open fn`, `override fn`, `abstract fn` がエラーなしにパースされるが修飾子情報が捨てられる。ユーザーは警告を受けず、下流パスは override メソッドと通常メソッドを区別できない。

## 改善案

FnDecl に修飾子フィールドを追加するか、MethodModifiers 構造体を導入する。意図的に遅延する場合は diagnostic warning を出す。

## 影響範囲

- crates/valen-parser/src/parser.rs:L353-L356

## 関連ファイル

- crates/valen-ast/src/lib.rs:L41-L49
