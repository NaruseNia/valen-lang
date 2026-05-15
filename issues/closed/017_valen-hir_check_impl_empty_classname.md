---
scope: valen-hir
severity: major
dimension: correctness
---

# check_impl passes empty string as class_name

## 概要
ty.rs check_impl で lookup_method_def_id("", &m.name) を呼び出し。空文字列でクラス名マッチせず全 impl のメソッドをグローバル検索、別 impl の同名メソッドを誤返却。

## 改善案
impl.target から実際の型名を渡す。

## 影響範囲
- crates/valen-hir/
