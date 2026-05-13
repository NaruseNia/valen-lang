# 実装計画: Valen Language

## 概要

Phase 0（基盤整備）完了済み。Phase 1（MVP）に向けて、コンパイラパイプライン全段を実装する。タスクは依存関係順に並べ、parser → HIR → codegen → tool の流れで進行。

既存の Phase 全体計画は [IMPLEMENTATION_PLAN.md](../IMPLEMENTATION_PLAN.md) を参照。

## 実装順序

| フェーズ | タスク | 依存 | 規模 |
|---------|--------|------|------|
| 1 | ~~TASK-001: Parser 拡張（fn params/return/if/match/call）~~ ✅ | - | L |
| 1 | ~~TASK-002: Parser 拡張（enum/trait/impl/import/package 宣言）~~ ✅ | - | L |
| 1 | ~~TASK-003: Parser 拡張（for/while/loop/break/continue/lambda）~~ ✅ | - | M |
| 2 | ~~TASK-004: HIR 設計と名前解決~~ ✅ | TASK-001, TASK-002 | L |
| 2 | ~~TASK-005: 型検査（プリミティブ・ジェネリクス・推論）~~ ✅ | TASK-004 | L |
| 2 | ~~TASK-006: Coherence / orphan rule 検証~~ ✅ | TASK-004 | M |
| 2 | ~~TASK-007: Exhaustiveness check（enum/sealed）~~ ✅ | TASK-004 | M |
| 3 | ~~TASK-008: Codegen — class/data class emit~~ ✅ | TASK-005 | L |
| 3 | ~~TASK-009: Codegen — enum ADT フル emit~~ ✅ | TASK-005, TASK-007 | L |
| 3 | ~~TASK-010: Codegen — fn/method/expression bytecode~~ ✅ | TASK-005 | L |
| 3 | ~~TASK-011: Codegen — Java interop（import 解決・safe ブロック）~~ ✅ | TASK-005 | L |
| 4 | ~~TASK-012: 標準ライブラリ（valen.core）~~ ✅ | TASK-010 | M |
| 4 | TASK-013: Gradle プラグイン | TASK-010 | M |
| 4 | ~~TASK-014: LSP サーバー（MVP）~~ ✅ | TASK-004, TASK-005 | M |
| 4 | TASK-015: valenfmt（最小版） | TASK-001 | S |
| 5 | TASK-016: サンプルプロジェクト（hello/shapes/java-interop） | TASK-013 | M |
| 5 | ~~TASK-017: E2E テスト・CI 拡充~~ ✅ | TASK-010 | M |

## トレーサビリティマトリクス

| 要件ID | タイトル | 関連タスク | カバレッジ |
|--------|---------|-----------|-----------|
| REQ-SYNTAX-001 | 字句定義 | (Phase 0 完了) | Full |
| REQ-SYNTAX-002 | セミコロンルール | TASK-001 | Full |
| REQ-SYNTAX-003 | コメント | (Phase 0 完了) | Full |
| REQ-TYPE-001 | プリミティブ名義型 | TASK-005 | Full |
| REQ-TYPE-002 | リテラルデフォルト型 | TASK-001, TASK-005 | Full |
| REQ-TYPE-003 | 暗黙変換なし | TASK-005 | Full |
| REQ-TYPE-004 | 等値比較 | TASK-005, TASK-010 | Full |
| REQ-TYPE-005 | Option null 一本化 | TASK-005, TASK-012 | Full |
| REQ-TYPE-006 | ジェネリクス | TASK-005 | Full |
| REQ-TYPE-007 | typealias | TASK-004 | Full |
| REQ-TYPE-008 | 型推論 | TASK-005 | Full |
| REQ-CLASS-001 | class + primary ctor | TASK-002, TASK-008 | Full |
| REQ-CLASS-002 | data class | TASK-002, TASK-008 | Full |
| REQ-CLASS-003 | 継承 | TASK-002, TASK-005, TASK-008 | Full |
| REQ-CLASS-004 | sealed class | TASK-002, TASK-007, TASK-008 | Full |
| REQ-CLASS-005 | メソッド解決 | TASK-004 | Full |
| REQ-CLASS-006 | 可視性 | TASK-004 | Full |
| REQ-CLASS-007 | package 必須 | TASK-002, TASK-004 | Full |
| REQ-CLASS-008 | import | TASK-002, TASK-004 | Full |
| REQ-ADT-001 | enum ADT | TASK-002, TASK-009 | Full |
| REQ-ADT-002 | exhaustive match | TASK-001, TASK-007, TASK-010 | Full |
| REQ-ADT-003 | enum Java ABI | TASK-009 | Full |
| REQ-TRAIT-001 | trait/impl | TASK-002, TASK-004 | Full |
| REQ-TRAIT-002 | impl ブロックのみ | TASK-004 | Full |
| REQ-TRAIT-003 | orphan rule | TASK-006 | Full |
| REQ-TRAIT-004 | UFCS 統一 | TASK-004 | Full |
| REQ-FAIL-001 | 役割分離 | TASK-005, TASK-012 | Full |
| REQ-FAIL-002 | Error trait | TASK-012 | Full |
| REQ-FAIL-003 | ? 演算子 | TASK-005, TASK-010 | Full |
| REQ-FAIL-004 | safe ブロック | TASK-011 | Full |
| REQ-FAIL-005 | Java null → T? | TASK-011 | Full |
| REQ-EMIT-001 | Java 21 class file | (Phase 0 完了) | Full |
| REQ-EMIT-002 | enum emit | (Phase 0 PoC) + TASK-009 | Partial |
| REQ-EMIT-003 | class emit | (Phase 0 PoC) + TASK-008 | Partial |
| REQ-EMIT-004 | Java 25 opt-in | TASK-010 | Partial |
| REQ-TOOL-001 | valenc CLI | TASK-010 | Full |
| REQ-TOOL-002 | Gradle プラグイン | TASK-013 | Full |
| REQ-TOOL-003 | LSP | TASK-014 | Full |
| REQ-TOOL-004 | valenfmt | TASK-015 | Full |
| REQ-INTEROP-001 | Java import | TASK-011 | Full |
| REQ-INTEROP-002 | safe 例外変換 | TASK-011 | Full |
| REQ-INTEROP-003 | @valen.Closed | TASK-007, TASK-011 | Full |
| REQ-STDLIB-001 | valen.core | TASK-012 | Full |
| REQ-STDLIB-002 | valen.collections | TASK-012 | Full |
| REQ-STDLIB-003 | valen.io | TASK-012 | Partial |

## マイルストーン

| マイルストーン | 含まれるタスク | 完了条件 |
|--------------|--------------|---------|
| M0: Phase 0 完了 | (完了済み) | `class Foo {}` → `Foo.class` PoC + enum ABI spike |
| M1: Parser 完成 | TASK-001〜003 | 全 MVP 構文を parse し AST を生成 |
| M2: 型チェック通過 | TASK-004〜007 | サンプルコードが HIR lowering まで通過 |
| M3: Bytecode 生成 | TASK-008〜011 | `valenc build` で .class 出力、`java` で実行可能 |
| M4: ツール統合 | TASK-012〜015 | Gradle build + LSP + fmt 動作 |
| M5: MVP 出荷 | TASK-016〜017 | 3 サンプルプロジェクトが Gradle で build + 実行可能 |
