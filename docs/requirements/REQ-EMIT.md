# 要件定義: バイトコード生成 (REQ-EMIT)

## スコープ概要

Valen コンパイラのバイトコード生成（emit）に関する要件。JVM class file の生成、enum の ABI 戦略、Java 25 opt-in サポートを定義する。

**関連仕様:** [lang/16-jvm-target.md](../lang/16-jvm-target.md)
**Phase:** MVP（Phase 1）/ Phase 1.5

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス |
|----|---------|--------|-----------|
| REQ-EMIT-001 | Java 21 class file 生成（version 65.0） | Must | Done |
| REQ-EMIT-002 | enum → sealed interface + record/singleton emit | Must | Done |
| REQ-EMIT-003 | class → .class emit（default constructor） | Must | Done |
| REQ-EMIT-004 | Java 25 opt-in サポート（`--target 25` フラグ） | Should | Draft |

---

## REQ-EMIT-001: Java 21 class file 生成（version 65.0）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-EMIT-001 |
| **優先度** | Must |
| **ステータス** | Done |
| **Phase** | MVP |

### 説明

Valen コンパイラ（valenc）は Java 21 対応の class file（バージョン 65.0）を生成する。JVM 21 が baseline ターゲットであり、生成された class file は JVM 21 以降で実行可能でなければならない。

- class file のメジャーバージョン: 65（Java 21）
- マイナーバージョン: 0
- 生成には Rust の classfile crate を使用（noak 等）
- 出力ディレクトリはビルドツール（Gradle）が指定する

### 受入条件

- [x] 生成された .class ファイルのバージョンが 65.0 である
- [x] `javap -v` で class file を検証し、major version 65 が確認できる
- [x] JVM 21 で生成された class file が正常にロード・実行される
- [x] JVM 25 で生成された class file が後方互換で実行される
- [x] 不正な bytecode を生成した場合に `VerifyError` ではなくコンパイラエラーとして報告される

### 依存

なし（基盤要件）

---

## REQ-EMIT-002: enum → sealed interface + record/singleton emit

| 項目 | 内容 |
|------|------|
| **ID** | REQ-EMIT-002 |
| **優先度** | Must |
| **ステータス** | Done |
| **Phase** | MVP |

### 説明

Valen の enum（ADT）を JVM バイトコードに変換する戦略。以下の ABI を採用する。

```
enum Shape {
    Circle(radius: Double),
    Rectangle(width: Double, height: Double),
    Point,
}
```

上記は以下の JVM 構造に emit される:

- `Shape` → `sealed interface Shape`
- `Shape.Circle` → `record Shape$Circle(double radius) implements Shape`
- `Shape.Rectangle` → `record Shape$Rectangle(double width, double height) implements Shape`
- `Shape.Point` → `final class Shape$Point implements Shape`（singleton、`INSTANCE` フィールド）

バイナリ命名規則: `Enum$Variant` 固定。

### 受入条件

- [x] payload 付き variant が record として emit される
- [x] payload なし variant が singleton class として emit される
- [x] 親 enum が sealed interface として emit される
- [x] Java 21 の `switch` pattern matching で exhaustive check が動作する
- [x] Jackson / Gson での serialize/deserialize が動作する
- [x] `java.lang.reflect` での class 名前解決が正しく動作する
- [x] Gradle incremental compilation と互換性がある
- [x] バイナリ命名が `Enum$Variant` パターンに従う
- [x] Phase 0 spike 結果（`docs/enum-abi-report.md`）と整合する

### 依存

- REQ-EMIT-001（Java 21 class file 生成）
- REQ-ADT-001（enum 定義）
- REQ-ADT-003（enum Java ABI 仕様）

---

## REQ-EMIT-003: class → .class emit（default constructor）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-EMIT-003 |
| **優先度** | Must |
| **ステータス** | Done |
| **Phase** | MVP |

### 説明

Valen の class 定義を JVM の .class ファイルとして emit する。MVP では primary constructor に対応する default constructor を生成する。

```valen
class Greeter(pub name: String) {
    fn greet(self) -> String {
        f"Hello, {self.name}"
    }
}
```

上記は以下の JVM 構造に emit される:

- `Greeter.class` ファイル
- `<init>(Ljava/lang/String;)V` コンストラクタ
- `name` フィールド（可視性に応じたアクセス修飾子）
- `greet()` メソッド

### 受入条件

- [x] class 定義から .class ファイルが正しく生成される
- [x] primary constructor のパラメータがフィールドとコンストラクタ引数に変換される
- [x] `pub` / `private` / `internal` がアクセス修飾子に正しくマッピングされる
- [x] メソッドが正しいシグネチャで emit される
- [x] 生成された class が `new` で Java からインスタンス化可能
- [x] `data class` の場合に equals/hashCode/toString/copy が自動生成される

### 依存

- REQ-EMIT-001（Java 21 class file 生成）
- REQ-CLASS-001（class + primary constructor）
- REQ-CLASS-006（可視性）

---

## REQ-EMIT-004: Java 25 opt-in サポート（`--target 25` フラグ）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-EMIT-004 |
| **優先度** | Should |
| **ステータス** | Draft |
| **Phase** | Phase 1.5 |

### 説明

`--target 25` フラグを valenc に渡すことで、Java 25（JVM 25）固有の機能を活用した class file を生成する。デフォルトは Java 21（REQ-EMIT-001）のまま。

- class file バージョンを 69.0（Java 25）に変更
- Java 25 で追加された bytecode 命令やクラスファイル属性を利用可能
- Java 25 固有の最適化（primitive value types 等、確定後に詳細化）

### 受入条件

- [ ] `valenc compile --target 25` で class file バージョン 69.0 が生成される
- [ ] `--target 25` 未指定時はデフォルトで Java 21（65.0）が生成される
- [ ] 不正なターゲット指定（`--target 20` 等）でエラーメッセージが表示される
- [ ] Java 25 固有機能を使用した .class が JVM 25 で正常に動作する
- [ ] Java 25 固有機能を使用した .class が JVM 21 で `UnsupportedClassVersionError` となる（意図通り）
- [ ] Gradle plugin がターゲットバージョンの設定を受け渡せる

### 依存

- REQ-EMIT-001（Java 21 class file 生成 — baseline として必要）
- REQ-TOOL-001（valenc CLI — フラグの受け取り）
- REQ-TOOL-002（Gradle プラグイン — ターゲット設定の伝播）

---

## 変更履歴

| 日付 | 変更内容 | 担当 |
|------|---------|------|
| 2026-05-11 | 初版作成 | — |
