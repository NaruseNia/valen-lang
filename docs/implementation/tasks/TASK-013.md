## TASK-013: Gradle プラグイン

| 項目 | 内容 |
|------|------|
| ID | TASK-013 |
| 関連要件 | REQ-TOOL-002 |
| 規模 | M |
| 依存タスク | TASK-010 |

### 実装概要
Kotlin Gradle プラグインを実装し、compileValen タスクの追加、.vln ソースセットの登録、valenc CLI の呼び出しを行う。

### 対象ファイル
- `gradle-plugin/` ディレクトリ（新規作成）

### 実装ステップ
1. Gradle プラグインのプロジェクト構造を作成（build.gradle.kts）
2. Valen ソースセット規約を登録
3. compileValen タスクを実装（valenc バイナリの呼び出し）
4. クラスパスの受け渡しを構成（Java 依存関係 → valenc）
5. 標準 Gradle ライフサイクルへの組み込み（compileJava が compileValen に依存）
6. シンプルなプロジェクトによる結合テスト
