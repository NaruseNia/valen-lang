---
scope: valen-parser
severity: critical
dimension: correctness
---

# Nested generics `>>` ambiguity causes parse failure

## 概要

`Map<String, List<Int>>` のようなネストしたジェネリクス型が `>>` を Shr トークンとしてlexするためパースに失敗する。

## 現状

crates/valen-parser/src/parser.rs:L405-L429 と lexer.rs:L151-L152: logos は `>>` を単一の Shr トークンとして生成。parse_type_path_segment() は各ジェネリクスブラケットを閉じるために `Gt` トークンを期待するが、`List<Int>>` の閉じ `>>` は Shr トークン1つであり Gt 2つではない。

```rust
// In parse_type_path_segment:
if self.eat(&TokenKind::Lt).is_some() {
    while !self.at(&TokenKind::Gt) && !self.at_eof() { ... }
    self.expect(TokenKind::Gt)?;  // fails when token is Shr
}
```

## 問題点

ネストしたジェネリクス型（`Map<K, List<V>>`、`Option<List<String>>`等）が全てパースエラーになる。C++ でも知られた古典的問題。内側の `>` は消費されるが、外側の `>>` は Shr トークンであり Gt ではないため失敗。

## 改善案

ジェネリクスパース文脈で Shr を特別に扱う: `>` を期待しているときに Shr を見たら、それを消費して合成的な Gt トークンを push back する（またはカウンタを使用）。

## 影響範囲

- crates/valen-parser/src/parser.rs:L405-L429

## 関連ファイル

- crates/valen-parser/src/lexer.rs:L151-L152
