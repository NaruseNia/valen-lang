# 2. 型

## 2.1 プリミティブ名義型

- `Int` — JVM 上の整数型に対応する名義型（実装詳細として int/Integer を切り替えるが仕様では保証しない）
- `Long`, `Float`, `Double`, `Char`, `Bool`, `Byte`, `Short`, `String`, `Unit`, `Nothing`

## 2.2 null / 欠損

Valen の欠損表現は **`Option<T>` に一本化**。

- `T?` は `Option<T>` の糖衣構文
- `T!` は **内部型のみ**（ユーザ記述不可、IDE 警告として表示のみ）
- `null` リテラルは使えない（Java 相互運用時にのみ経由）

## 2.3 ジェネリクス

- `<T>` 形式、宣言時に `in`/`out` variance 指定可
- erasure（JVM 互換）
- `reified` 型パラメータは Phase 2（MVP は普通のジェネリクス）

## 2.4 typealias

```valen
typealias UserId = Int;
```

**所有権を生まない** — orphan rule 判定上、typealias は元の型の所有として扱われない。
