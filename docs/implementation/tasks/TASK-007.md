## TASK-007: 網羅性検査

| 項目 | 内容 |
|------|------|
| ID | TASK-007 |
| 関連要件 | REQ-ADT-002, REQ-INTEROP-003 |
| 規模 | M |
| 依存タスク | TASK-004 |

### 実装概要
enum、sealed class、`@valen.Closed` 付き Java sealed type に対するパターンマッチの網羅性検査を実装する。

### 対象ファイル
- `crates/valen-hir/src/exhaustive.rs`

### 実装ステップ
1. match アームからパターン行列を構築
2. enum バリアントの網羅性チェック
3. sealed class サブタイプの網羅性チェック
4. `@valen.Closed` Java sealed type の網羅性チェック
5. 不足パターンの診断メッセージ出力
6. テスト追加
