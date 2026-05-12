---
scope: valen-hir
severity: critical
dimension: correctness
---

# Duplicate name detection is dead code (IndexMap overwrites)

## 概要

Scope::define が IndexMap::insert で前のエントリを上書きするため、check_duplicate_names は既に重複排除されたマップを走査することになり、重複検出が機能しない。

## 現状

crates/valen-hir/src/resolve.rs:L26-L28 — Scope::define は insert で上書き
crates/valen-hir/src/resolve.rs:L112-L128 — check_duplicate_names は重複を見つけられない

```rust
fn define(&mut self, name: SmolStr, id: DefId) {
    self.names.insert(name, id); // overwrites previous
}
```

## 問題点

同名の関数が2つあっても、黙ってシャドーイングされエラーにならない。言語仕様のモジュールスコープ名前一意性に違反。

## 改善案

Scope::define 内で挿入前に既存キーを確認する（IndexMap::insert は旧値を返す）。

## 影響範囲

- crates/valen-hir/src/resolve.rs:L26-L28
- crates/valen-hir/src/resolve.rs:L112-L128
