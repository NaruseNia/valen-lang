# VEP-036: inline fn + reified generics

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 関連 Issue | — |
| 依存 VEP | — |

## 概要

Kotlin 同等の `inline fn` と `reified` 型パラメータを Valen に導入する。`inline fn` はコールサイトに本体をインライン展開し、ラムダ引数も非ボクシングでインライン化する。`reified T` は呼び出し側で具体型に置換され、`value is T`、`value as T`、`T::class` が使える。

## 動機

JVM の型消去（erasure）により、実行時にジェネリクス型引数は消える。以下のパターンが書けない:

```valen
// ❌ 現在: 型消去で T の情報がない
fn <T> isInstance(value: Any) -> Bool {
    value is T  // コンパイルエラー
}
```

`inline fn` + `reified` で解決:

```valen
// ✅ 提案: reified T がコールサイトで具体型に置換される
inline fn <reified T> isInstance(value: Any) -> Bool {
    value is T  // → value instanceof ConcreteType
}

let result = isInstance<String>("hello");  // true
```

## 設計

### 構文

```
inline fn <reified T> name(params) -> ReturnType { body }
inline fn <reified T, U> name(params) -> ReturnType { body }  // T のみ reified、U は通常
```

- `inline` キーワードを `fn` の前に付与
- `reified` は型パラメータ個別に指定（全パラメータが reified である必要はない）
- `inline fn` 内のラムダ引数はデフォルトでインライン化される

### reified T で許可される操作

| 操作 | 構文 | codegen |
|------|------|---------|
| 型チェック | `value is T` | `instanceof ConcreteType` |
| キャスト | `value as T` | `checkcast ConcreteType` |
| クラス取得 | `T::class` | `ldc ConcreteType.class` |

### ラムダのインライン化

`inline fn` に渡されたラムダ引数は呼び出し側にインライン展開される。

```valen
inline fn <T> run(block: fn() -> T) -> T {
    block()
}

fn main() {
    let x = run(|| {
        return;  // ← non-local return: main() から return
    });
}
```

- ラムダ本体が `inline fn` のコールサイトに展開されるため、`return` は `inline fn` ではなく呼び出し元関数から return する（non-local return）
- 将来: `crossinline`（non-local return 禁止）、`noinline`（インライン化しない）修飾子

### Java interop

- Java 側からは `inline fn` は通常のメソッドとして見える
- `reified` は Java 呼び出し時には無効（型消去される）
- Kotlin の `@JvmStatic` 相当は今回スコープ外

## 実装計画

### Phase 1: Parser + AST（`crates/valen-parser`, `crates/valen-ast`）

**変更ファイル:**
- `crates/valen-ast/src/lib.rs` — `FnDecl` に `is_inline: bool` フィールド追加
- `crates/valen-ast/src/lib.rs` — `GenericParam` に `is_reified: bool` フィールド追加
- `crates/valen-ast/src/token.rs` — `Inline` キーワードトークン追加
- `crates/valen-parser/src/lexer.rs` — `inline` トークンのレキシング
- `crates/valen-parser/src/parser.rs` — `inline fn` のパース、`reified` 型パラメータのパース

**影響:**
- `crates/valenfmt/src/printer.rs` — `inline` キーワードの出力対応
- `crates/valen-lsp/src/server.rs` — completion に `inline` キーワード候補追加、semantic tokens で `inline` をハイライト

**テスト:**
- パーサースナップショットテスト（`inline fn`, `reified T`, 組み合わせ）

### Phase 2: HIR（`crates/valen-hir`）

**変更ファイル:**
- `crates/valen-hir/src/lib.rs` — `FnDef` に `is_inline: bool`、`GenericBound` に reified 情報追加
- `crates/valen-hir/src/resolve.rs` — inline fn の定義登録。reified パラメータのバリデーション
- `crates/valen-hir/src/ty.rs` — reified T に対する `is T`、`as T`、`T::class` の型チェック

**バリデーションルール:**
- `reified T` は `inline fn` 内でのみ使用可能（非 inline で使うとコンパイルエラー）
- `reified T` はクラス/trait/enum の型パラメータには使えない（関数のみ）
- `is T` の `T` が reified でない場合はコンパイルエラー

**影響:**
- `crates/valen-hir/src/coherence.rs` — inline fn のトレイト impl への影響確認
- `crates/valen-hir/src/exhaustive.rs` — `is T` パターンの exhaustiveness（Phase 2.1 送り可）

### Phase 3: Codegen — インライン展開（`crates/valen-codegen`）

これが最大の変更。`inline fn` の呼び出しをコールサイトに展開するインライナーを実装する。

**変更ファイル:**
- `crates/valen-codegen/src/expr.rs` — `TypedExprKind::Call` の処理でインライン展開

**インライン展開の手順:**

1. **HIR レベルでのインライン化**（推奨）:
   - `TypedExprKind::Call` で callee が `inline fn` の場合、HIR の TypedBody を呼び出し側にコピー
   - 仮引数を実引数の TypedExpr で置換
   - `reified T` を具体型で置換
   - 結果を通常の式として codegen に渡す

2. **reified 置換**:
   - `is T` → `instanceof ConcreteType`（JvmOp::Instanceof に変換）
   - `as T` → `checkcast ConcreteType`（JvmOp::Checkcast に変換）
   - `T::class` → `ldc ConcreteType.class`（JvmOp::Ldc に変換）

3. **ラムダインライン化**:
   - ラムダ引数の TypedBody を inline fn 本体内の `block()` 呼び出し箇所にコピー
   - non-local return: ラムダ内の `return` を呼び出し元関数の return に変換
   - JVM bytecode では `try-catch` + 専用例外クラスで non-local return を実現（Kotlin と同じ手法）

**影響:**
- `crates/valen-codegen/src/jvm_ir.rs` — `Instanceof` op は既存、`Ldc` class リテラル追加
- `crates/valen-codegen/src/emit.rs` — class リテラルの定数プール登録
- `crates/valen-codegen/src/lower.rs` — inline fn は `.class` ファイルにもメソッドとして残す（Java 互換）

### Phase 4: ドキュメント

| ファイル | 変更内容 |
|---------|---------|
| `docs/lang/04-functions.md` | `inline fn` セクション追加 |
| `docs/lang/02-types.md` | reified 型パラメータ記述更新（Phase 2 → 実装済み） |
| `docs/lang/future/meta.md` | inline fn / reified を future から削除 |
| `docs/guide/03-generics.md` | reified の使い方ガイド追加 |
| `docs/guide/08-java-interop.md` | Java interop での制約記述 |
| `docs/LANGUAGE_SPEC.md` | インデックス更新 |
| `docs/implementation/comprehensive-plan.md` | ステータス更新 |
| `AGENTS.md` | 不要（crate 追加なし） |

### Phase 5: LSP 対応

| 変更 | 内容 |
|------|------|
| semantic tokens | `inline` キーワードのハイライト |
| completion | `inline fn` テンプレート候補 |
| hover | inline fn の展開先情報表示（Phase 2.1） |
| diagnostics | reified 制約エラーの表示 |

## リスク・制約

| リスク | 対策 |
|--------|------|
| インライン展開によるバイトコード膨張 | 再帰的 inline fn を禁止。深いネストに警告 |
| non-local return の複雑性 | Kotlin と同じ try-catch 方式。Phase 1 では non-local return なしで出して段階的に追加も可 |
| `inline fn` の変更で呼び出し側の再コンパイルが必要 | Gradle incremental compile で対応。ドキュメントに注意事項記載 |
| Java からの呼び出し時に reified が効かない | ドキュメントで明記。通常メソッドとしてフォールバック |

## 段階的リリース戦略

大きな機能のため、以下の順で段階的にリリースする:

| ステップ | 内容 | 単独で価値がある |
|---------|------|----------------|
| **Step 1** | `inline fn` パース + AST + HIR（インライン展開なし、通常関数として動作） | ✅ 構文予約 |
| **Step 2** | `reified T` パース + 型チェック（`is T` / `as T` / `T::class` のバリデーション） | ✅ エラーメッセージ改善 |
| **Step 3** | codegen: `inline fn` 本体のインライン展開 + reified 置換 | ✅ 主機能 |
| **Step 4** | codegen: ラムダ引数のインライン化 + non-local return | ✅ Kotlin 互換 |
| **Step 5** | LSP + ドキュメント + テスト | ✅ 仕上げ |

各ステップは独立した PR として出せる。

## 代替案

| 案 | 却下理由 |
|----|---------|
| `Class<T>` 隠し引数方式（インライン化なし） | ラムダのインライン化ができない。Kotlin 互換でない |
| turbofish `::<T>` で reified | 構文は Kotlin 形式に決定済み |
| reified のみ（inline fn なし） | reified は inline fn に依存（Kotlin セマンティクス） |

## 変更履歴

| 日付 | 変更 |
|------|------|
| 2026-05-25 | Draft 作成。grill-me で設計決定 |
