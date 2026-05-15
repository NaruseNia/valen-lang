# コンパイラアーキテクチャ

この章はコントリビュータ向けです。Valen コンパイラ `valenc` の内部構造と開発方法について説明します。

## パイプライン概要

`valenc` は `.vln` ソースファイルを JVM の `.class` ファイルに変換します。パイプラインは3段構成です。

```
.vln source
    │
    ▼
┌──────────────────┐
│   valen-parser   │  logos lexer + hand-written recursive descent
│                  │  → Vec<Item> (AST)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│    valen-hir     │  名前解決 → 型検査 → coherence → exhaustiveness
│                  │  → Hir (typed IR)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  valen-codegen   │  HIR → JvmClass/JvmMethod/JvmOp → .class bytes
│                  │
│    lower.rs      │  HIR Def → codegen IR (symbolic refs)
│    emit.rs       │  codegen IR → ristretto_classfile → bytes
└──────────────────┘
```

### 各段の役割

1. **Parser**: ソースコードをトークン列に分解し（lexer）、AST を構築します（parser）
2. **HIR**: AST を受け取り、名前解決・型検査・trait coherence・match の網羅性検査を行い、型付き中間表現（HIR）を生成します
3. **Codegen**: HIR から JVM バイトコードを生成し、`.class` ファイルとして出力します

## crate 構成

プロジェクトは機能ごとに crate に分割されています。

| Crate | 役割 | 主な依存 |
|-------|------|---------|
| `valen-ast` | AST 型定義、TokenKind、Span | なし |
| `valen-diagnostics` | エラー・警告の共通構造体 | `valen-ast` |
| `valen-parser` | lexer + parser | `valen-ast`, `valen-diagnostics`, `logos` |
| `valen-hir` | 名前解決・型検査・coherence・exhaustiveness | `valen-ast`, `valen-diagnostics` |
| `valen-codegen` | JVM バイトコード生成 | `valen-hir`, `ristretto_classfile` |
| `valenc` | コンパイラ CLI（エントリポイント） | 全 crate |
| `valen-lsp` | LSP サーバー | `valen-parser`, `valen-hir`, `async-lsp` |
| `valenfmt` | コードフォーマッタ | `valen-parser` |

### 依存関係の方向

```
valenc ──┬── valen-codegen ── valen-hir ──┬── valen-ast
         │                                 └── valen-diagnostics ── valen-ast
         ├── valen-parser ──┬── valen-ast
         │                  └── valen-diagnostics
         └── valen-diagnostics

valen-lsp ──┬── valen-parser
             └── valen-hir

valenfmt ── valen-parser
```

## Parser（valen-parser）

### Lexer

`logos` crate を使ったトークン分割を行います。トークンの種類は `valen-ast` の `TokenKind` に定義されています。

### Parser

手書きの再帰下降パーサーです。`logos` lexer が生成したトークン列を消費して `Vec<Item>`（AST ノードのリスト）を構築します。

パーサーのエラー回復は最小限ですが、複数のエラーを一度に報告できるよう設計されています。

## HIR（valen-hir）

HIR 層は以下の4つのフェーズを順に実行します。

### 名前解決（resolve）

`resolve.rs` がすべての識別子を定義に紐付けます。prelude として以下の型が自動的にスコープに注入されます。

- `Option` / `Some` / `None`
- `Result` / `Ok` / `Err`
- `Error`
- `Iterator`
- `Range`
- `JavaException`

### 型検査（typecheck）

型推論と型の整合性検査を行います。Valen はローカル変数の型推論をサポートしますが、関数のパラメータと戻り値の型は明示必須です。

### Coherence 検査

trait の orphan rule を検証し、同一の trait/type 対に対する重複 impl がないことを確認します。

### Exhaustiveness 検査

`match` 式が対象の型の全ケースを網羅しているかを検査します。`enum`、`sealed class`、`sealed trait` の全バリアント/permit が処理されていなければコンパイルエラーとなります。

## Codegen（valen-codegen）

Codegen は2段階で動作します。

### lower.rs — HIR → codegen IR

HIR の定義（`Def`）を codegen IR に変換します。codegen IR は以下の構造体で表現されます。

- `JvmClass` — JVM クラスに対応
- `JvmMethod` — メソッドに対応
- `JvmOp` — JVM 命令に対応（symbolic reference を含む）

この段階では JVM のクラスファイルフォーマットの詳細に依存せず、論理的な構造を扱います。

### emit.rs — codegen IR → .class bytes

`JvmClass` を `ristretto_classfile` の `ClassFile` 構造体に変換し、最終的にバイト列（`.class` ファイル）を出力します。

`ristretto_classfile` はJVM クラスファイルフォーマットを扱うための Rust crate です。constant pool の管理、attribute の構築、バイトコードのエンコードなどを担当します。

### enum の JVM 表現

| Valen | JVM |
|-------|-----|
| `enum Foo { ... }` | `public sealed interface Foo permits Foo$A, Foo$B, ...` |
| payload あり `A(x: Int)` | `public final record Foo$A(int x) implements Foo` |
| payload なし `B` | `public final class Foo$B implements Foo` + `INSTANCE` singleton |

## LSP サーバー（valen-lsp）

`async-lsp` crate を使った LSP サーバーです。MVP では以下の機能を提供します。

- シンタックスエラーの診断
- 型エラーの診断
- Go to Definition

将来的には別リポジトリに分離する予定です。

## フォーマッタ（valenfmt）

`valen-parser` の AST を使ってソースコードを整形します。MVP では最小限の機能（ブレーススタイル、インデント、末尾セミコロン）を提供します。

## テスト

テストは workspace 全体で320以上あります。以下のコマンドで実行できます。

```sh
cargo test --workspace
```

E2E テストは `valen-codegen` crate 内の fixture として管理されています。`.vln` ソースをパース → HIR → codegen → `.class` 出力まで一気通貫で検証します。

## ビルドとプリコミットチェック

開発時は `mise` タスクランナーを使います。コードに変更を加えたら、コミット前に必ず以下を通してください。

```sh
mise run precommit    # check + clippy + fmt + test を一括実行
```

個別に実行する場合:

```sh
mise run check        # 型と借用チェック（最速のゲート）
mise run clippy       # lint（warning = error）
mise run fmt          # フォーマットチェック（修正は mise run fmt:fix）
mise run test         # 全テスト
mise run build        # ビルド確認
```

CI と同じフルパイプラインをローカルで実行する場合:

```sh
mise run ci           # fmt + clippy + test + build + doc
```

## コントリビューション時のポイント

- **すべての `pub` な型・関数に英語の doc comment を付ける**（`///` / `//!`）
- **`#[allow(...)]` や `#[ignore]` は最終手段**。使う場合はコメントで理由を書く
- **コミットメッセージは英語**、conventional prefix（`feat:`, `fix:`, `refactor:` 等）を使う
- **設計判断に迷ったら聞く**。特に ADT / match / trait / 失敗モデル に関わる変更は確認必須
