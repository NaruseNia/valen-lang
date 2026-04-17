# 14. メタプログラミング（MVP）

MVP は最小セット：

- `inline fn` は **Phase 1.5**、MVP は普通の fn のみ
- annotation consumption（読み取り）は Phase 1.5
- `reified` 型パラメータは Phase 2

MVP では annotation の宣言と読み取りは `java.lang.reflect` 経由で可能。
