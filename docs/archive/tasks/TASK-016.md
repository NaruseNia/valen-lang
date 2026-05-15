## TASK-016: サンプルプロジェクト

| 項目 | 内容 |
|------|------|
| ID | TASK-016 |
| 関連要件 | （MVP 完了基準） |
| 規模 | M |
| 依存タスク | TASK-013 |

### 実装概要
3つのサンプルプロジェクトを作成する。hello world、shapes（ADT + match + trait）、java-interop（Java クラス利用 + 例外処理）。

### 対象ファイル
- `examples/hello/`
- `examples/shapes/`
- `examples/java-interop/`

### 実装ステップ
1. `examples/hello/` を作成（最小 Gradle ビルド + main.vln）
2. `examples/shapes/` を作成（enum Shape、trait Area、網羅的 match）
3. `examples/java-interop/` を作成（Java クラスインポート、safe {} ブロック）
4. 3プロジェクトすべてが Gradle プラグインでビルドできることを検証
5. `java -cp ... Main` で正常実行できることを検証
