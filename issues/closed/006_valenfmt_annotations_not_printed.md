---
scope: valenfmt
severity: critical
dimension: correctness
---

# フォーマッタがほぼ全ての宣言でアノテーションを消失させる

## 概要

print_annotations は AnnotationClassDecl でのみ呼び出される。FnDecl, ClassDecl, DataClassDecl, EnumDecl, TraitDecl, FieldDecl, CtorParam の annotations フィールドは全て無視される。

## 現状

crates/valenfmt/src/printer.rs: print_annotations の呼び出しは print_annotation_class 内のみ（L485付近）。他の print_* メソッドはアノテーションを出力しない。

```rust
fn print_fn_decl(&mut self, f: &FnDecl, ctx: FnCtx) {
    self.write_indent();
    // f.annotations is never printed
    let show_vis = ctx == FnCtx::TopLevel || ctx == FnCtx::ClassMethod;
```

## 問題点

`@Deprecated fn foo() {}` や `@JvmStatic class Bar {}` 等のアノテーション付き宣言をフォーマットすると、アノテーションが消失する。REQ-TOOL-004「整形がコードの意味を変えない」に違反。

## 改善案

print_fn_decl, print_class, print_data_class, print_enum, print_trait, print_field, print_ctor_params の各メソッドの先頭で `self.print_annotations(&x.annotations)` を呼び出す。テストフィクスチャにアノテーション付き各宣言型を追加。

## 影響範囲

- crates/valenfmt/src/printer.rs

## 関連ファイル

- crates/valen-ast/src/lib.rs
- crates/valenfmt/tests/format_tests.rs
