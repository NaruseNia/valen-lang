## TASK-009: Codegen: enum ADT full emit

| 項目 | 内容 |
|------|------|
| ID | TASK-009 |
| 関連要件 | REQ-EMIT-002, REQ-ADT-001, REQ-ADT-003 |
| 規模 | L |
| 依存タスク | TASK-005, TASK-007 |

### 実装概要
enum_emit を PoC から本番品質に拡張する。アクセサメソッド、toString、record バリアントの equals、HIR enum 型からの正しいコンストラクタシグネチャ生成を実装する。

### 対象ファイル
- `crates/valen-codegen/src/enum_emit.rs`

### 実装ステップ
1. record バリアント用アクセサメソッド生成
2. 全バリアント用 toString 生成
3. record バリアント用 equals/hashCode 生成
4. HIR enum 型から emit パイプラインへの接続
5. ジェネリック enum 型の処理
6. テスト追加
