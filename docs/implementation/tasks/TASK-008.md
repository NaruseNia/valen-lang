## TASK-008: Codegen: class/data class

| 項目 | 内容 |
|------|------|
| ID | TASK-008 |
| 関連要件 | REQ-EMIT-003, REQ-CLASS-001, REQ-CLASS-002 |
| 規模 | L |
| 依存タスク | TASK-005 |

### 実装概要
class_emit を拡張し、フィールド・メソッド・コンストラクタ付きフル class と、data class の自動生成を実装する。

### 対象ファイル
- `crates/valen-codegen/src/class_emit.rs`

### 実装ステップ
1. プライマリコンストラクタ付き class の emit（フィールド + init）
2. インスタンスメソッドの emit
3. 関連関数（static メソッド）の emit
4. 継承（extends 句）の emit
5. data class の equals/hashCode/toString/copy 自動生成
6. sealed class の PermittedSubclasses 付き emit
7. javap 検証付きテスト追加
