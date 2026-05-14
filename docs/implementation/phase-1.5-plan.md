# Phase 1.5 実装計画

## 概要

MVP（Phase 1）完了後の補完フェーズ。codegen stub 解消・コンパイラ基盤整備・言語機能追加の3層構造で進行。

**期間目安:** 2-3 ヶ月
**ゴール:** 外部ユーザーが Valen でプロジェクトを書ける + Kotlin ユーザーが違和感なく使える言語機能

---

## マイルストーン

### M6: Codegen 完成（stub 全潰し）

MVP で StubBody だったコードパスを全て実装し、`valenc compile` で生成した .class が JVM 上で実行可能になる。

| タスク | 内容 | 依存 | 規模 |
|--------|------|------|------|
| TASK-018 | trait impl メソッドを対象クラスに emit | — | S |
| TASK-019 | lambda: invokedynamic + LambdaMetafactory | — | L |
| TASK-020 | for ループ: Range 型 stdlib + Iterator impl + codegen | TASK-019 | L |
| TASK-021 | safe{} → Result\<T, JavaException\> 接続 | — | M |
| TASK-022 | lambda stack underflow 修正 | TASK-019 | S |

**詳細:**

#### TASK-018: trait impl emit (#52)
`lower.rs` の `lower_class` で `hir.trait_impls` からメソッドを収集し、対象クラスに emit。

#### TASK-019: lambda (invokedynamic + LambdaMetafactory)
- `expr.rs` に `lower_lambda` 実装
- lambda 本体を同クラス内の private static メソッドとして emit
- `invokedynamic` + `java.lang.invoke.LambdaMetafactory.metafactory` bootstrap
- captures（自由変数）のサポート
- JVM の `FunctionalInterface` との互換性

#### TASK-020: for ループ + Range + Iterator
- `stdlib/valen/core/core.vln` に `Range<T>` 定義追加
- `Iterator` trait の `next()` を使った for ループ desugaring
- `for i in 0..10` → `Range` 構築 + Iterator.next() ループ
- `java.lang.Iterable` → Iterator アダプト

#### TASK-021: safe{} → Result 接続 (#48)
- `synth_safe` の戻り値型を `Result<T, JavaException>` に変更
- `lower_safe` で成功パスを `Result$Ok` wrap、例外パスを `Result$Err(JavaException)` wrap
- prelude（M7）が前提だが、同一コンパイル単位内で stdlib を参照できれば動作

#### TASK-022: lambda stack fix (#53)
TASK-019 で lambda が実装されれば自動解消の可能性大。残ればスタブの stack delta 修正。

**完了条件:** 全 E2E fixture が `compile_fixture`（classfile parse 含む）で通過。StubBody が codegen から消滅。

---

### M7: 基盤整備

コンパイラの型システムと Java 連携を実用レベルに引き上げる。

| タスク | 内容 | 依存 | 規模 |
|--------|------|------|------|
| TASK-023 | prelude（valen.core 型のコンパイラ内蔵） | — | M |
| TASK-024 | typealias パース + HIR + codegen | — | M |
| TASK-025 | classpath 走査（Java .class メタデータ読み取り） | — | L |
| TASK-026 | @valen.Closed（REQ-INTEROP-003） | TASK-025 | M |

**詳細:**

#### TASK-023: prelude
- `resolve.rs` の `resolve_items` 冒頭で、Option/Result/Error/Iterator/JavaException を組み込み定義としてスコープに注入
- `T?` → `Option<T>` の糖衣展開を型チェッカーで正式対応
- `?` 演算子の Result/Option unwrap を型チェッカー + codegen で接続

#### TASK-024: typealias
- parser に `typealias Name<T> = Type<T>;` パース追加（`TokenKind::TypeAlias` は既存）
- HIR に `TypeAlias { name, generics, target }` 追加
- 型解決で alias 展開
- `valen.collections` の `List<T>` = `java.util.List<T>` 等が有効化

#### TASK-025: classpath 走査
- `valenc` に `--classpath` / `-cp` オプション追加
- `ristretto_classfile` で .class ファイルを読み取り、メソッドシグネチャ・フィールド・コンストラクタ情報を取得
- HIR の resolve パスで import された Java 型の情報を foreign type として登録
- 型チェッカーで foreign type のメソッド/フィールド/コンストラクタの引数・戻り値型を検証

#### TASK-026: @valen.Closed
- TASK-025 の classpath 走査で `RuntimeVisibleAnnotations` を読み取り
- `@valen.Closed` 付き Java sealed 型の `PermittedSubclasses` を取得
- exhaustiveness checker に foreign sealed type の variant リストを渡す
- `valen-annotations.jar`（Java annotation 定義）の生成

**完了条件:** `import java.util.ArrayList; let list = ArrayList(); list.add("hello");` が型チェック通過。`valen.collections` の typealias が動作。

---

### M8: 言語機能 + ツール

Kotlin/Java ユーザー向けの快適機能とツール充実。

| タスク | 内容 | 依存 | 規模 |
|--------|------|------|------|
| TASK-027 | デフォルト引数 | — | M |
| TASK-028 | 演算子オーバーロード（trait ベース） | TASK-023 | M |
| ~~TASK-029~~ | ~~sealed trait~~ ✅ | — | S |
| ~~TASK-030~~ | ~~annotation（宣言 + 付与 + ランタイム保持）~~ ✅ | — | L |
| TASK-031 | valenfmt 最小実装（TASK-015） | — | M |
| TASK-032 | LSP 拡充（#49: completion/hover/semantic tokens） | TASK-023 | L |

**詳細:**

#### TASK-027: デフォルト引数
- parser: `fn foo(x: Int = 42)` パース
- AST `Param` に `default: Option<Expr>` 追加
- 型チェッカー: 呼び出し側で引数省略時にデフォルト値を挿入
- codegen: オーバーロードメソッド生成（Java 互換）or デフォルト値を呼び出し側に埋め込み

#### TASK-028: 演算子オーバーロード
- `valen.core` に `trait Add<Rhs, Out> { fn add(self, rhs: Rhs) -> Out; }` 等を定義
- 型チェッカー: `a + b` で `Add` trait の `add` メソッドを探索
- プリミティブ型には組み込み impl（既存のまま）
- ユーザー定義型は `impl Add for MyType` で演算子を定義

#### TASK-029: sealed trait
- parser: `sealed trait` 構文追加
- HIR: `TraitDef` に `is_sealed: bool` フラグ
- exhaustiveness: sealed trait を sealed class と同様に処理

#### TASK-030: annotation
- parser: `annotation class Foo(val x: Int)` 宣言構文
- parser: `@Foo(x = 1)` 付与構文
- HIR: annotation 情報の保持
- codegen: `RuntimeVisibleAnnotations` attribute として .class に emit

#### TASK-031: valenfmt
- `crates/valenfmt` の `todo!()` を実装
- parse → AST → pretty-print パイプライン
- ルール: indent（4 spaces）、brace style（same-line）、trailing semicolon 正規化
- `--check` モードと自動修正モード

#### TASK-032: LSP 拡充
- completion: キーワード + 型名 + 関数名 + メソッド名
- hover: 型情報表示
- semantic tokens: キーワード / 型 / 関数 / 変数 着色
- (cross-file は TASK-025 の classpath 走査が前提)

**完了条件:** Kotlin からの移植コストが「大きな違和感なし」レベル。VSCode で日常開発体験。

---

## タスク依存関係

```
M6 (stub 潰し)
  TASK-018 ──────────────────────┐
  TASK-019 (lambda) ──┬── TASK-022 (stack fix)
                      └── TASK-020 (for/Range/Iterator)
  TASK-021 (safe→Result) ───────┘

M7 (基盤)
  TASK-023 (prelude) ───────────┐
  TASK-024 (typealias) ─────────┤
  TASK-025 (classpath) ── TASK-026 (@valen.Closed)
                                │
M8 (言語機能 + ツール)            │
  TASK-027 (default args) ──────┤
  TASK-028 (operator OL) ── TASK-023
  TASK-029 (sealed trait) ──────┤
  TASK-030 (annotation) ────────┤
  TASK-031 (valenfmt) ──────────┤
  TASK-032 (LSP拡充) ── TASK-023, TASK-025
```

---

## Phase 2 送り（Phase 1.5 スコープ外）

| 項目 | 理由 |
|------|------|
| `suspend fn` / async | virtual thread 統合戦略の仕様化が必要 |
| `reified` 型パラメータ | inline fn の上位機能 |
| `inline fn` | Phase 1.5 では優先度低 |
| annotation processing (KSP 相当) | annotation 基盤が先 |
| DSL receiver lambda | 仕様上 extension と隔離する設計が必要 |
| multiline f-string `f"""..."""` | 便利だが優先度低 |
| init block / secondary constructor | Kotlin 互換だが MVP で primary ctor で十分 |
| field override | 複雑、Phase 2 で再評価 |
| nested / inner class | JVM 実装が複雑 |
| reflection 統合 | java.lang.reflect ラッパー、Phase 2 |
| Maven プラグイン | Gradle が先 |
| 独自 collection façade | java.util typealias で十分 |

---

## 変更履歴

| 日付 | 変更 |
|------|------|
| 2026-05-13 | grill-me 4巡で策定。M6/M7/M8 3層構造確定 |
