# Valen Language Specification

Version: 0.3-draft (Phase 2 M14 反映)
Last updated: 2026-05-19

---

## 0. 芯と哲学

Valen の芯は以下の4点。新機能提案はこの芯を補強するかで評価する。

1. **強 ADT**（sum type with payload）
2. **exhaustive match**（Rust フルセット）
3. **trait ベース抽象**（orphan rule 厳格）
4. **整合した失敗モデル**（Option / Result / Exception / panic の役割分離）

**芯の一文:** 「OO の上に ADT を足すのでなく、ADT を中核に据えて JVM へ落とす」

---

## 目次

### 実装済み仕様

- [1. 字句構文](lang/01-lexical.md)
- [2. 型](lang/02-types.md)（ref mut T ミュータブル参照型）
- [3. 式と文](lang/03-expressions.md)（unsafe 式 / as キャスト / deref / ref mut 式）
- [4. 関数](lang/04-functions.md)
- [5. クラス](lang/05-classes.md)
- [6. enum（ADT）](lang/06-enum.md)
- [7. trait / impl](lang/07-traits.md)
- [8. 失敗モデル](lang/08-failure.md)（unsafe block / unsafe fn / safe 短縮構文 / as キャスト / Java 呼び出しモード）
- [9. パターンマッチ](lang/09-pattern.md)
- [10. 可視性・モジュール](lang/10-modules.md)
- [16. ターゲット JVM](lang/16-jvm-target.md)
- [17. サンプル](lang/17-samples.md)
- [18. 今後の仕様課題](lang/18-open-questions.md)
- [19. ライセンス](lang/19-license.md)
- [20. アノテーション](lang/20-annotations.md)

### 将来仕様（未実装）

- [並行](lang/future/concurrency.md)
- [文字列](lang/future/strings.md)
- [コレクション / for](lang/future/collections.md)
- [メタプログラミング](lang/future/meta.md)
- [DSL / lambda](lang/future/dsl.md)
- [将来機能バックログ](lang/future/feature-backlog.md)
