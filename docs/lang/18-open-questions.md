# 18. 今後の仕様課題

## 実装時に詰める項目

1. ~~**enum bytecode ABI の実験検証**~~ → **解決済み（Phase 0 spike）** `docs/enum-abi-report.md` 参照
2. **coherence 仕様補則**
   - generic nominal type の所有判定例（`Vec<Foo>` の所有は `Vec` 側か `Foo` 側か、両方か）
   - Gradle subproject 跨ぎの module 境界運用ルール
   - module ID を決めるビルドツール抽象の formalization（Gradle 以外の backend で同じルールが通じるか）
3. **Java overload resolution 規則**
   - Int vs int vs Integer の優先度
   - null 許容位置
   - **Note:** 暗黙数値変換なし（§2.1）により大幅にシンプル化

## 3巡目 Codex + grill-me で解決済みの項目

以下は 2026-05 の grill-me セッションおよび Codex レビューで解決された。

### 解決済み

4. ~~**UFCS の記法一本化**~~ → **解決:** `Trait::method(receiver, args)` に一本化。`map(xs, f)` / `greet(p)` 形式は削除（§4.4）
5. ~~**`override fn` と trait 実装モデルの噛み合わせ**~~ → **解決:** trait 充足は `impl` ブロックのみ。class 本体 method は trait と無関係。`override fn` は class 継承専用（§5.6, §7.4）
6. **メソッド解決の overload 規則** — 暗黙数値変換なし（§2.1）により候補は型完全一致のみ。残る edge case:
   - 継承した method の候補扱い
   - named arg を含む適用可能性

### 高

7. **module identity**
   - 現状：`Gradle subproject 名 = 1 module`（§10.2）
   - 穴：composite build / included build で subproject 名が衝突しうる、jar 越しの所有 module を downstream がどう読むか未定義
   - 方針候補：canonical module identity を `group:name:version + Gradle project path` などで定義、classfile 横 metadata に埋める

8. **enum Java ABI の internal/private trait lowering**
   - 現状：§6.5.4 で `internal/private trait` は Java 非露出、§6.5.5 でその追加は minor bump
   - 穴：bytecode レベルで「interface を implements しない」「metadata で隠す」「static helper に lower」のどれかが未記述
   - 方針候補：「`internal/private trait` は JVM 公開 interface としては emit せず、Valen 専用 metadata + bridge/lowered dispatch で表現する」を §6.5.4 に追記

### 解決済み（中）

9. ~~**`data class` superclass 継承時の自動生成動作**~~ → **解決:** 自身の primary constructor params のみ対象。親の state は含めない（§5.2）
10. **`@valen.Closed` の annotation 契約詳細**
    - 現状：§20.3 で target を Java sealed interface / sealed class と書いているのみ
    - 不足：`@Target(TYPE)` / `@Retention(CLASS)` / 配布形態（classpath に `valen-runtime.jar` として置くか、コンパイラ組み込みか）
    - 方針候補：`@Target(TYPE) @Retention(CLASS)` を §20.3 に明記、配布は `valen-runtime.jar` を標準

### 解決済み（低）

11. ~~**§9.2 Java 例のコードフェンス**~~ — 修正対象のみ（細部品質）

## grill-me 4巡目で新規確定した仕様（2026-05-11）

| # | 論点 | 決定 | 反映先 |
|---|------|------|--------|
| G1 | UFCS 統一 | `Trait::method(recv, args)` 一本化 | §4.4 |
| G2 | trait 充足 | `impl` ブロックのみ | §5.6, §7.4 |
| G3 | Error 型 | `Error` trait 制約 + 同一E型のみ `?` | §8.2, §8.3 |
| G4 | Java null | `safe {}` 内戻り値は全て `T?` | §8.4 |
| G5 | 数値変換 | 暗黙変換なし、明示メソッドのみ | §2.1 |
| G6 | break/continue | MVP 導入、`break expr;` で値返却可 | §3.4 |
| G7 | 型推論 | ローカル推論あり、fn シグネチャ明示 | §4.5 |
| G8 | import | MVP: 単一 + alias のみ | §10.1 |
| G9 | 等値比較 | `==` 構造比較、`===` 参照比較 | §2.2 |
| G10 | クロージャ | 参照キャプチャ、mut 可 | §15.2 |
| G11 | `as` | MVP は import alias のみ | §1.2 |
| G12 | package | 必須、省略はエラー | §10.1 |
| G13 | data class 継承 | auto-gen は自身 ctor params のみ | §5.2 |
| G14 | リテラル型 | 42=Int, 42L=Long, 3.14=Double, 3.14f=Float | §2.1 |
