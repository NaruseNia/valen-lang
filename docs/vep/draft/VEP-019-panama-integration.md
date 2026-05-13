# VEP-019: Project Panama integration

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

Foreign Function & Memory API (Project Panama) を Valen の安全境界で包み、JVM 上での native library 呼び出しの標準ルートを提供する。

## 設計

`unsafe extern "c" fn strlen(ptr: MemorySegment) -> Long` の形式で FFI 宣言する。`unsafe` / `Result` / resource management の設計を実戦投入する場となる。`MemorySegment` / `Arena` の標準ライブラリ露出方法、lifetime の扱い、native failure の `Result` 変換規約が検討事項。
