# 16. ターゲット JVM

- **21 baseline**：初期互換ターゲット。virtual thread、sealed、record、pattern matching for switch を活用
- **25 first-class opt-in**：`--target 25` を一級ターゲットとして扱う。Scoped Values、Structured Concurrency、Stable Values、primitive patterns、compact object headers など、JDK 25 世代の機能を積極的に利用する
- **Valhalla / Panama は別枠**：JDK 25 first-class 化と、Valhalla / Panama 連携は同一視しない。利用可能な JDK 機能は target ごとに検出し、仕様上の意味と最適化を分離する
- bytecode 直出力
