## TASK-002: Parser 拡張（enum/trait/impl/import/package）

| 項目 | 内容 |
|------|------|
| ID | TASK-002 |
| 関連要件 | REQ-CLASS-001, REQ-CLASS-002, REQ-CLASS-003, REQ-CLASS-004, REQ-CLASS-005, REQ-CLASS-006, REQ-CLASS-007, REQ-CLASS-008, REQ-ADT-001, REQ-TRAIT-001 |
| 規模 | L |
| 依存タスク | - |

### 実装概要
class（コンストラクタパラメータ、ボディメソッド付き）、data class、enum（バリアント付き）、trait、impl ブロック、import、package 宣言のパースを実装する。

### 対象ファイル
- `crates/valen-parser/src/parser.rs`
- `crates/valen-ast/src/lib.rs`

### 実装ステップ
1. class パース追加（プライマリコンストラクタパラメータ + ボディ）
2. data class パース追加
3. enum パース追加（ペイロード付きバリアント、ユニットバリアント）
4. trait 定義パース追加（メソッドシグネチャ）
5. impl ブロックパース追加
6. package 宣言パース追加
7. import パース追加（単一インポート + as エイリアス）
8. スナップショットテスト追加
