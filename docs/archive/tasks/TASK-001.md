## TASK-001: Parser 拡張（fn params/return/if/match/call）

| 項目 | 内容 |
|------|------|
| ID | TASK-001 |
| 関連要件 | REQ-SYNTAX-002, REQ-TYPE-002, REQ-ADT-002 |
| 規模 | L |
| 依存タスク | - |

### 実装概要
再帰下降パーサを拡張し、関数パラメータ（型注釈付き）、戻り値型注釈、if/else 式、match 式（フルパターンセット）、関数呼び出し式、`::` 付きパス式を処理可能にする。

### 対象ファイル
- `crates/valen-parser/src/parser.rs`
- `crates/valen-ast/src/lib.rs`
- `crates/valen-parser/src/lexer.rs`

### 実装ステップ
1. fn パラメータパース追加（`name: Type, ...` 形式）
2. 戻り値型パース追加（`-> Type` 形式）
3. 型パスパース追加（`Foo::Bar` 等の `::` 区切りパス）
4. if/else 式パース追加
5. match 式パース追加（アーム、パターン、ガード節含む）
6. 関数呼び出し式パース追加（`callee(args)` 形式）
7. メソッド呼び出しパース追加（`receiver.method(args)` 形式）
8. 各構文に対する insta スナップショットテスト追加
