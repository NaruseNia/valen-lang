# VEP — Valen Enhancement Proposal

Valen の言語機能・ツール・基盤の設計変更を提案・議論・記録するプロセス。

## フォーマット

各 VEP は `VEP-NNN-short-title.md` として本ディレクトリに配置する。

```markdown
# VEP-NNN: タイトル

| 項目 | 内容 |
|------|------|
| ステータス | Draft / Accepted / Implemented / Rejected |
| Phase | 1.5 / 2 / 3 |
| 関連 Issue | #XX |
| 依存 VEP | VEP-NNN |

## 概要
1-2 段落で what と why。

## 設計
技術的な詳細。

## 受入条件
チェックリスト形式。

## 代替案
検討して却下した案。

## 変更履歴
| 日付 | 変更 |
|------|------|
```

## ステータス遷移

```
Draft → Accepted → Implemented
  ↓
Rejected
```

ステータス変更時にファイルを対応ディレクトリに移動:
- `draft/` → `accepted/` → `implemented/`
- `draft/` → `rejected/`

## 一覧

| VEP | タイトル | Phase | 優先度 | ステータス |
|-----|---------|-------|--------|-----------|
| [VEP-001](draft/VEP-001-unsafe-block.md) | unsafe block / unsafe fn | 2 | Should | Draft |
| [VEP-002](draft/VEP-002-effect-like-try.md) | Effect-like try block | 2 | Should | Draft |
| [VEP-003](draft/VEP-003-java-exception-catch.md) | Java Exception catch expression (safe catch) | 2 | Should | Draft |
| [VEP-004](draft/VEP-004-defer-scope-guard.md) | defer / scope guard | 2 | Could | Draft |
| [VEP-005](draft/VEP-005-pipeline-operator.md) | Pipeline operator | 2 | Could | Draft |
| [VEP-006](draft/VEP-006-when-expression.md) | when expression | 3 | Could | Draft |
| [VEP-007](draft/VEP-007-trailing-block-lambda.md) | Trailing block lambda | 2 | Should | Draft |
| [VEP-008](draft/VEP-008-labeled-block.md) | Labeled block / early break | 3 | Could | Draft |
| [VEP-009](draft/VEP-009-anonymous-sum-types.md) | Anonymous sum types | 3 | Could | Draft |
| [VEP-010](draft/VEP-010-row-polymorphism.md) | Row polymorphism / open record | 3 | Could | Draft |
| [VEP-011](draft/VEP-011-refinement-newtype.md) | Refinement / newtype | 2 | Should | Draft |
| [VEP-012](draft/VEP-012-intersection-constraints.md) | Intersection constraints (T: A & B) | 2 | Should | Draft |
| [VEP-013](draft/VEP-013-derive.md) | derive | 2 | Must | Draft |
| [VEP-014](accepted/VEP-014-sealed-trait.md) | sealed trait | 1.5 | Must | Accepted |
| [VEP-015](draft/VEP-015-specialization.md) | Specialization / default impl | 3 | Could | Draft |
| [VEP-016](draft/VEP-016-extension-property.md) | Extension property | 2 | Could | Draft |
| [VEP-017](draft/VEP-017-jdk25-first-class-target.md) | JDK 25 first-class target | 2 | Should | Draft |
| [VEP-018](draft/VEP-018-valhalla-integration.md) | Project Valhalla integration | 3 | Could | Draft |
| [VEP-019](draft/VEP-019-panama-integration.md) | Project Panama integration | 3 | Could | Draft |
| [VEP-020](accepted/VEP-020-java-annotation-authoring.md) | Java annotation authoring | 1.5 | Must | Accepted |
| [VEP-021](draft/VEP-021-nullability-trust-modes.md) | Nullability trust modes | 2 | Should | Draft |
| [VEP-022](draft/VEP-022-hygienic-macro.md) | Hygienic macro | 3 | Could | Draft |
| [VEP-023](draft/VEP-023-compile-time-reflection.md) | Compile-time reflection | 3 | Could | Draft |
| [VEP-024](draft/VEP-024-const-eval.md) | const eval | 2 | Should | Draft |
| [VEP-025](draft/VEP-025-async-await.md) | async / await | 2 | Must | Draft |
| [VEP-026](draft/VEP-026-structured-concurrency.md) | Structured concurrency | 2 | Must | Draft |
| [VEP-027](draft/VEP-027-actor-channel.md) | Actor / channel | 3 | Could | Draft |
| [VEP-028](draft/VEP-028-let-else.md) | let-else | 2 | Must | Draft |
| [VEP-029](draft/VEP-029-if-let-while-let.md) | if let / while let | 2 | Must | Draft |
| [VEP-030](draft/VEP-030-collection-literal.md) | Collection literal | 2 | Should | Draft |
| [VEP-031](draft/VEP-031-range-slice-indexing.md) | Range / slice indexing | 2 | Should | Draft |
| [VEP-032](accepted/VEP-032-default-arguments.md) | Default arguments | 1.5 | Must | Accepted |
| [VEP-033](accepted/VEP-033-operator-overload.md) | Operator overload (trait-based) | 1.5 | Must | Accepted |
| [VEP-034](accepted/VEP-034-annotation.md) | Annotation (declaration + application + runtime) | 1.5 | Must | Accepted |
