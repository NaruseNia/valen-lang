# 18. 今後の仕様課題

実装時に詰める項目：

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
