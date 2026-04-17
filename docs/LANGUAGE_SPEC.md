# Valen Language Specification

Version: 0.1-draft (Codex 3巡レビュー後、哲学一本化スコア 82/100)
Last updated: 2026-04-17

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

- [1. 字句構文](spec/01-lexical.md)
- [2. 型](spec/02-types.md)
- [3. 式と文](spec/03-expressions.md)
- [4. 関数](spec/04-functions.md)
- [5. クラス](spec/05-classes.md)
- [6. enum（ADT）](spec/06-enum.md)
- [7. trait / impl](spec/07-traits.md)
- [8. 失敗モデル](spec/08-failure.md)
- [9. パターンマッチ](spec/09-pattern.md)
- [10. 可視性・モジュール](spec/10-modules.md)
- [11. 並行](spec/11-concurrency.md)
- [12. 文字列](spec/12-strings.md)
- [13. コレクション / for](spec/13-collections.md)
- [14. メタプログラミング](spec/14-meta.md)
- [15. DSL / lambda](spec/15-dsl.md)
- [16. ターゲット JVM](spec/16-jvm-target.md)
- [17. サンプル](spec/17-samples.md)
- [18. 今後の仕様課題](spec/18-open-questions.md)
- [19. ライセンス](spec/19-license.md)
- [20. アノテーション](spec/20-annotations.md)
