# 18. 今後の仕様課題

実装時に詰める項目：

1. **enum bytecode ABI の実験検証**
   - Java pattern switch
   - Jackson / Gson シリアライゼーション
   - reflection での class 名前解決
   - Gradle incremental compilation
2. **coherence 仕様補則**
   - 所有単位の厳密化（compile unit = module = package の関係）
   - generic nominal type の所有判定例
   - inherent vs trait 衝突時の曖昧性エラー規則
3. **Java overload resolution 規則**
   - Int vs int vs Integer の優先度
   - null 許容位置
