---
scope: valenc
severity: critical
dimension: correctness
---

# Exhaustiveness check がコンパイラパイプラインから欠落

## 概要

valenc の run_pipeline_with_classpath が exhaustiveness check を実行していない。非網羅的な match 式がコンパイルを通過し、実行時に MatchError を引き起こす。

## 現状

crates/valenc/src/main.rs: パイプラインは parse → resolve → type_check → coherence で終了。`valen_hir::exhaustive::check_exhaustiveness` が呼び出されていない。

```rust
// line 150-162 — coherence is the last check
let coherence_result = valen_hir::coherence::check_coherence(&hir, &[]);
...
Ok(FrontendResult { hir, bodies: tc.bodies })
```

## 問題点

仕様で明示的に要求されている exhaustiveness check パスが省略されている。enum/sealed class の全 variant を網羅していない match 式が silent にコンパイル成功し、実行時にクラッシュ。

## 改善案

coherence 後に `let exhaust = valen_hir::exhaustive::check_exhaustiveness(&hir, &all_items);` を追加。diagnostics を emit し、エラー時は bail。

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

- crates/valen-hir/src/exhaustive.rs
