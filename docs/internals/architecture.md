# アーキテクチャ仕様: Valen Language

## 概要

Valen コンパイラ `valenc` は Rust で実装され、`.vln` ソースから JVM `.class` ファイルを生成する。パイプラインは 3 段構成。

## コンパイラパイプライン

```
.vln source + stdlib/valen/core/core.vln (embedded)
    │
    ▼
┌──────────────┐
│ valen-parser │  logos lexer + hand-written recursive descent
│              │  → Vec<Item> (AST)
└──────┬───────┘
       │
       ▼
┌──────────────┐
│  valen-hir   │  stdlib parse → name resolution → type check → coherence → exhaustiveness
│              │  → Hir (typed IR)
│  stdlib.rs   │  embeds core.vln via include_str!, parses to prelude AST
└──────┬───────┘
       │
       ▼
┌──────────────┐
│valen-codegen │  HIR → codegen IR → JVM bytecode
│              │
│  lower.rs    │  HIR Def → JvmClass/JvmMethod/JvmOp (symbolic refs)
│  emit.rs     │  JvmClass → ristretto_classfile ClassFile → bytes
│              │  → Vec<ClassFileOutput> (.class bytes)
└──────────────┘
```

## Crate 構成

| Crate | 役割 | 依存先 |
|-------|------|--------|
| `valen-ast` | AST 型定義、TokenKind、Span | — |
| `valen-diagnostics` | エラー・警告の共通構造体 | `valen-ast` |
| `valen-parser` | lexer + parser | `valen-ast`, `valen-diagnostics`, `logos` |
| `valen-hir` | 名前解決・型検査・coherence・exhaustiveness | `valen-ast`, `valen-diagnostics`, `valen-parser` |
| `valen-codegen` | JVM bytecode 生成 | `valen-hir`, `ristretto_classfile` |
| `valenc` | コンパイラ CLI | 全 crate |
| `valen-lsp` | LSP サーバー | `valen-parser`, `valen-hir`, `async-lsp` |
| `valenfmt` | コードフォーマッタ | `valen-parser` |

## JVM ターゲット

- **Baseline:** Java 21 LTS（class file version 65.0）
- **Opt-in:** Java 25（`--target 25` フラグ）
- バイトコード生成: `ristretto_classfile` 0.31

### enum ABI 戦略

| Valen 構文 | JVM 表現 |
|-----------|---------|
| `enum Foo { ... }` | `public sealed interface Foo permits Foo$A, Foo$B, ...` |
| payload variant `A(x: Int)` | `public final record Foo$A(int x) implements Foo` |
| unit variant `B` | `public final class Foo$B implements Foo` + `INSTANCE` singleton |

詳細: [docs/archive/enum-abi-report.md](../archive/enum-abi-report.md)

## ビルドシステム統合

- **Gradle プラグイン**（Kotlin 実装）: `compileValen` タスクで `.vln` → `.class` 変換
- Gradle subproject = 1 module（orphan rule / sealed permit / internal 可視性の単位）
- `valenc` CLI 単体利用時: `--module <name>` フラグで module 指定

## 外部ツール

| ツール | 実装言語 | 範囲 |
|--------|---------|------|
| LSP | Rust | syntax error, type diagnostics, goto definition（MVP） |
| valenfmt | Rust | brace style, indent, trailing semicolon（最小版） |

## 制約・前提条件

- 所有権・借用モデルなし（JVM GC 依存）
- `static` キーワード不採用（self 有無で instance/associated を区別）
- inherent impl なし（class 本体に method、trait は impl ブロック）
- blanket impl 禁止（MVP）

## 関連要件

- REQ-EMIT-001〜004（バイトコード生成）
- REQ-TOOL-001〜004（ツール）
- REQ-INTEROP-001〜003（Java 相互運用）

## 詳細仕様書

言語仕様の詳細は [docs/lang/](../lang/) 配下を参照:

| ファイル | 内容 |
|---------|------|
| [LANGUAGE_SPEC.md](../LANGUAGE_SPEC.md) | 仕様インデックス |
| [lang/01-lexical.md](../lang/01-lexical.md) | 字句構文 |
| [lang/02-types.md](../lang/02-types.md) | 型システム |
| [lang/03-expressions.md](../lang/03-expressions.md) | 式と文 |
| [lang/04-functions.md](../lang/04-functions.md) | 関数 |
| [lang/05-classes.md](../lang/05-classes.md) | クラス |
| [lang/06-enum.md](../lang/06-enum.md) | enum（ADT） |
| [lang/07-traits.md](../lang/07-traits.md) | trait / impl |
| [lang/08-failure.md](../lang/08-failure.md) | 失敗モデル |
| [lang/09-pattern.md](../lang/09-pattern.md) | パターンマッチ |
| [lang/10-modules.md](../lang/10-modules.md) | 可視性・モジュール |
| [lang/20-annotations.md](../lang/20-annotations.md) | アノテーション |
