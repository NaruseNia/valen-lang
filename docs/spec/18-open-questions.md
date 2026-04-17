# 18. 今後の仕様課題

## 実装時に詰める項目

1. **enum bytecode ABI の実験検証**
   - Java pattern switch
   - Jackson / Gson シリアライゼーション
   - reflection での class 名前解決
   - Gradle incremental compilation
2. **coherence 仕様補則**
   - generic nominal type の所有判定例（`Vec<Foo>` の所有は `Vec` 側か `Foo` 側か、両方か）
   - Gradle subproject 跨ぎの module 境界運用ルール
   - module ID を決めるビルドツール抽象の formalization（Gradle 以外の backend で同じルールが通じるか）
3. **Java overload resolution 規則**
   - Int vs int vs Integer の優先度
   - null 許容位置

## 3巡目 Codex レビューで挙がった積み残し（優先度付き）

以下は 2026-04 の Codex 3巡目レビュー（内容 82-83/100）で指摘された、83-85 へ到達するための残課題。今回の改訂では open-questions に棚上げし、次の改訂サイクルで詰める。

### 重大

4. **UFCS の記法一本化**
   - 現状：§4.4 は `map(xs, f)` を UFCS と定義、§5.6 は曖昧性解消を `Trait::foo(v)`、§17.3 は `greet(p)` を UFCS と呼ぶ
   - 矛盾：`foo(x)` が top-level fn か trait UFCS かを構文で区別できない
   - 方針候補：**UFCS = `Trait::foo(v)` 一本化、`foo(v)` は top-level fn 限定**（章横断で修正）

5. **`override fn` と trait 実装モデルの噛み合わせ**
   - 現状：§5.6 は「class 本体 method が trait requirement と同一 signature なら `override fn` 必須」、§7.2 は「trait 実装は `impl Trait for Type` 専用」
   - 矛盾：class 本体 method は trait 充足の場ではないはずなのに `override` を要求している
   - 方針候補：`override fn` を **class 継承専用** に切る。trait 充足は `impl Trait for Type` ブロック内でのみ成立させる

6. **メソッド解決の overload 規則**
   - §5.6 は「適用可能 signature で候補形成」まで。未定義の edge case：
     - class 内 overload 同士の優先（`fn show(self)` vs `fn show(self, fmt)`）
     - 継承した method の候補扱い
     - named arg を含む適用可能性
     - 数値変換 / generic 制約で複数適用可能になったときの tie-break
   - 方針候補：Java overload resolution 規則（§18 項目 3）と合流させる

### 高

7. **module identity**
   - 現状：`Gradle subproject 名 = 1 module`（§10.2）
   - 穴：composite build / included build で subproject 名が衝突しうる、jar 越しの所有 module を downstream がどう読むか未定義
   - 方針候補：canonical module identity を `group:name:version + Gradle project path` などで定義、classfile 横 metadata に埋める

8. **enum Java ABI の internal/private trait lowering**
   - 現状：§6.5.4 で `internal/private trait` は Java 非露出、§6.5.5 でその追加は minor bump
   - 穴：bytecode レベルで「interface を implements しない」「metadata で隠す」「static helper に lower」のどれかが未記述
   - 方針候補：「`internal/private trait` は JVM 公開 interface としては emit せず、Valen 専用 metadata + bridge/lowered dispatch で表現する」を §6.5.4 に追記

### 中

9. **`data class` superclass 継承時の自動生成動作**
   - 現状：§5.2 は「sealed/open/abstract superclass 継承は可」だが、`equals`/`hashCode`/`copy` が super state を見るかが未定義
   - 方針候補：
     - オプション A（厳格）: MVP では superclass を marker（state / methodなし）に限定
     - オプション B（Kotlin 同様）: 自動生成対象は primary constructor field のみと明記、superclass state は無視

10. **`@valen.Closed` の annotation 契約詳細**
    - 現状：§20.3 で target を Java sealed interface / sealed class と書いているのみ
    - 不足：`@Target(TYPE)` / `@Retention(CLASS)` / 配布形態（classpath に `valen-runtime.jar` として置くか、コンパイラ組み込みか）
    - 方針候補：`@Target(TYPE) @Retention(CLASS)` を §20.3 に明記、配布は `valen-runtime.jar` を標準

### 低

11. **§9.2 Java 例のコードフェンス**
    - `@Closed sealed interface Color` 例が ```valen フェンスになっているが、これは Java ソース
    - 方針：```java フェンスに修正（細部品質）
