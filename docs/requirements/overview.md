# 要件定義書: Valen Language

## プロジェクト概要

Valen は JVM をターゲットとする新規プログラミング言語。ADT（代数的データ型）を中核に据え、exhaustive match・trait ベース抽象・整合した失敗モデルを芯とする。Java/Kotlin 資産との相互運用を重視し、Rust の型安全性と JVM エコシステムの実用性を両立する。

## スコープ

### 対象範囲

- Valen 言語の仕様策定と実装（コンパイラ `valenc`）
- JVM 21 baseline / 25 opt-in のバイトコード生成
- Gradle プラグインによるビルド統合
- LSP サーバーによる IDE 支援
- コードフォーマッタ `valenfmt`
- 標準ライブラリ `valen.core` / `valen.collections` / `valen.io`
- Java クラス・ライブラリとの相互運用

### 対象外

- 独自パッケージマネージャ（Gradle/Maven に完全従属）
- セルフホスティング（Phase 3+ で段階検討）
- REPL / Playground（Phase 3）
- Maven プラグイン（Phase 2）

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-SYNTAX-001 | キーワード・識別子・リテラルの字句定義 | Must | Done |
| REQ-SYNTAX-002 | セミコロンルール（Rust 流3分類） | Must | Draft |
| REQ-SYNTAX-003 | コメント（単行・ブロック・doc） | Must | Done |
| REQ-TYPE-001 | プリミティブ名義型（Int〜Nothing） | Must | Done |
| REQ-TYPE-002 | リテラルデフォルト型（42=Int, 3.14=Double, サフィックス） | Must | Done |
| REQ-TYPE-003 | 暗黙数値変換なし・明示変換メソッド | Must | Done |
| REQ-TYPE-004 | 等値比較（== 構造 / === 参照） | Must | Done |
| REQ-TYPE-005 | Option<T> による null 一本化（T? 糖衣） | Must | Done |
| REQ-TYPE-006 | ジェネリクス（erasure, in/out variance） | Must | Draft |
| REQ-TYPE-007 | typealias（所有権なし） | Should | Draft |
| REQ-TYPE-008 | ローカル型推論あり・fn シグネチャ明示 | Must | Done |
| REQ-CLASS-001 | class + primary constructor | Must | Done |
| REQ-CLASS-002 | data class（equals/hashCode/toString/copy 自動生成） | Must | Done |
| REQ-CLASS-003 | 継承（open/abstract/sealed opt-in、単一継承+複数 trait） | Must | Draft |
| REQ-CLASS-004 | sealed class（closed OOP hierarchy） | Must | Draft |
| REQ-CLASS-005 | メソッド解決規則（class 本体優先→trait→UFCS） | Must | Done |
| REQ-CLASS-006 | 可視性（pub/internal/private、module 単位） | Must | Done |
| REQ-CLASS-007 | package 宣言必須 | Must | Draft |
| REQ-CLASS-008 | import（単一型 + alias、MVP） | Must | Draft |
| REQ-ADT-001 | enum（Rust 型 ADT、payload あり/なし） | Must | Done |
| REQ-ADT-002 | exhaustive match（リテラル/分解/ガード/範囲/or/@束縛） | Must | Done |
| REQ-ADT-003 | enum Java ABI（sealed interface + record/singleton） | Must | Done |
| REQ-TRAIT-001 | trait 定義と impl ブロック（inherent impl なし） | Must | Draft |
| REQ-TRAIT-002 | trait 充足は impl ブロックのみ | Must | Draft |
| REQ-TRAIT-003 | orphan rule（module 所有、blanket impl 禁止） | Must | Done |
| REQ-TRAIT-004 | UFCS（Trait::method(receiver, args) 一本化） | Must | Draft |
| REQ-FAIL-001 | Option/Result/panic/Exception の役割分離 | Must | Draft |
| REQ-FAIL-002 | Error trait（E: Error 制約） | Must | Draft |
| REQ-FAIL-003 | ? 演算子（同一E型のみ伝播） | Must | Draft |
| REQ-FAIL-004 | safe {} ブロック（Java 例外→Result） | Must | Draft |
| REQ-FAIL-005 | safe {} 内 Java 戻り値は T?（Option<T>） | Must | Draft |
| REQ-EMIT-001 | Java 21 class file 生成 | Must | Done |
| REQ-EMIT-002 | enum → sealed interface + record/singleton emit | Must | Done |
| REQ-EMIT-003 | class → .class emit（default constructor） | Must | Done |
| REQ-EMIT-004 | Java 25 opt-in サポート | Should | Draft |
| REQ-TOOL-001 | valenc CLI（compile / check / version） | Must | Done |
| REQ-TOOL-002 | Gradle プラグイン（compileValen タスク） | Must | Draft |
| REQ-TOOL-003 | LSP サーバー（syntax error + diagnostics + goto def） | Must | Draft |
| REQ-TOOL-004 | valenfmt（最小版） | Should | Draft |
| REQ-INTEROP-001 | Java クラスの import と利用 | Must | Draft |
| REQ-INTEROP-002 | safe {} による例外変換 | Must | Draft |
| REQ-INTEROP-003 | @valen.Closed で Java sealed を exhaustive match | Must | Draft |
| REQ-STDLIB-001 | valen.core（Option, Result, Error, Iterator） | Must | Draft |
| REQ-STDLIB-002 | valen.collections（List/Map/Set = java.util alias） | Must | Draft |
| REQ-STDLIB-003 | valen.io（基本IOラッパー） | Should | Draft |

## スコープ別要件定義書

| ファイル | スコープ | 要件数 |
|---------|---------|--------|
| [REQ-SYNTAX.md](REQ-SYNTAX.md) | 字句構文 | 3 |
| [REQ-TYPE.md](REQ-TYPE.md) | 型システム | 8 |
| [REQ-CLASS.md](REQ-CLASS.md) | クラス・モジュール | 8 |
| [REQ-ADT.md](REQ-ADT.md) | ADT・パターンマッチ | 3 |
| [REQ-TRAIT.md](REQ-TRAIT.md) | trait・coherence | 4 |
| [REQ-FAIL.md](REQ-FAIL.md) | 失敗モデル | 5 |
| [REQ-EMIT.md](REQ-EMIT.md) | バイトコード生成 | 4 |
| [REQ-TOOL.md](REQ-TOOL.md) | ツール | 4 |
| [REQ-INTEROP.md](REQ-INTEROP.md) | Java 相互運用 | 3 |
| [REQ-STDLIB.md](REQ-STDLIB.md) | 標準ライブラリ | 3 |

## 用語集

| 用語 | 定義 |
|------|------|
| ADT | 代数的データ型（Algebraic Data Type）。enum で表現 |
| exhaustive match | 全パターン網羅チェック付きの match 式 |
| orphan rule | trait impl が許可される条件（trait か型の少なくとも一方が自 module 所有） |
| UFCS | Uniform Function Call Syntax。`Trait::method(receiver, args)` 形式 |
| module | ビルドツール駆動の所有単位。Gradle subproject = 1 module |
| safe ブロック | Java FFI 境界を明示する `safe { ... }` 構文 |
| primary constructor | class 宣言と一体の唯一のコンストラクタ |
| sealed class | 同一 module 内でのみ継承可能な closed hierarchy |
