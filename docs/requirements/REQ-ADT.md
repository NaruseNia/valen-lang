# 要件定義: ADT・パターンマッチ (ADT)

プロジェクト: Valen Language
スコープ: enum（ADT）定義・パターンマッチ・Java ABI
最終更新: 2026-05-11

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス | 依存 |
|----|---------|--------|-----------|------|
| REQ-ADT-001 | enum（Rust 型 ADT、payload/unit variant、:: アクセス） | Must | Draft | REQ-SYNTAX-001, REQ-TYPE-001 |
| REQ-ADT-002 | exhaustive match（リテラル/分解/ガード/範囲/or/@束縛/wildcard） | Must | Draft | REQ-ADT-001 |
| REQ-ADT-003 | enum Java ABI（sealed interface + record/singleton、$命名） | Must | Draft | REQ-ADT-001 |

---

## REQ-ADT-001: enum（Rust 型 ADT）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-ADT-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-SYNTAX-001, REQ-TYPE-001 |
| **説明** | Valen の enum は Rust 風 ADT（class と完全分離）。variant は payload（named fields）を持てる unit variant と payload variant の2種。閉じた sum type として match で exhaustive check 対象。variant アクセスは `::` スコープ演算子。enum 自体は独自 method を持たない（trait impl 経由のみ）。 |

### 受け入れ基準

- [ ] payload variant が定義可能: `Circle(r: Float)`（named fields）
- [ ] unit variant が定義可能: `Point`（payload なし）
- [ ] 1つの enum 内に payload variant と unit variant を混在可能:
  ```valen
  enum Shape {
      Circle(r: Float),
      Rect(w: Float, h: Float),
      Point,
  }
  ```
- [ ] variant アクセスが `::` スコープ演算子で可能: `Shape::Circle(r = 5.0)`, `Shape::Point`
- [ ] enum が独自 method を持てない（class 本体のように fn を直接書くとパーサエラー）
- [ ] enum に対する振る舞い追加は `impl Trait for Enum` 経由のみ
- [ ] enum が閉じた sum type であり、variant の追加は定義元ファイルでのみ可能
- [ ] enum と sealed class の使い分けが仕様として明確（enum = data の和、sealed class = 振る舞いの階層）
- [ ] enum のパーサテスト・型チェックテストが存在する

---

## REQ-ADT-002: exhaustive match

| 項目 | 内容 |
|------|------|
| **ID** | REQ-ADT-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-ADT-001 |
| **説明** | Valen の `match` 式は Rust フルセットのパターンマッチを提供する。Valen enum / sealed class に対して exhaustive check（全パターン網羅チェック）を強制する。Java 型は `@valen.Closed` 付きのみ exhaustive 対象。 |

### 受け入れ基準

- [ ] **リテラルパターン**: `0 => "zero"` で定数マッチが動作する
- [ ] **範囲パターン**: `1..=9 => "small"` で範囲マッチが動作する
- [ ] **or パターン**: `10 | 20 | 30 => "round"` で複数パターンの論理和マッチが動作する
- [ ] **ガードパターン**: `n if n < 0 => "negative"` で条件付きマッチが動作する
- [ ] **構造分解パターン**: `Shape::Circle(r) => ...` で enum variant の field を束縛できる
- [ ] **複数フィールド分解**: `Shape::Rect(w, h) => ...` で複数 field を同時束縛できる
- [ ] **@束縛パターン**: `p @ User(name = "admin", ..) => admin_action(p)` で値全体を束縛しつつ分解できる
- [ ] **rest パターン**: `..` で残りのフィールドを無視できる
- [ ] **wildcard パターン**: `_ => "other"` で全てのパターンにマッチする
- [ ] **Valen enum exhaustive**: 全 variant を網羅しない match がコンパイルエラーとなる
- [ ] **Valen sealed class exhaustive**: 全 subtype を網羅しない match がコンパイルエラーとなる
- [ ] **Java 型（@valen.Closed あり）**: `@valen.Closed` 付き Java sealed hierarchy が exhaustive check 対象となる
- [ ] **Java 型（@valen.Closed なし）**: `@valen.Closed` なしの Java sealed は open-world 扱いで wildcard `_` 必須
- [ ] match が式として値を返す（`let x = match v { ... };`）
- [ ] 各パターン種別のテストが最低1ケースずつ存在する

---

## REQ-ADT-003: enum Java ABI

| 項目 | 内容 |
|------|------|
| **ID** | REQ-ADT-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-ADT-001 |
| **説明** | Valen enum の JVM バイトコード表現を規定する。sealed interface を enum 本体、record を payload variant、singleton class を unit variant として emit する。binary naming は `EnumName$VariantName`（`$` 区切り）で凍結。 |

### 受け入れ基準

- [ ] enum 本体が `sealed interface` として emit される:
  ```java
  public sealed interface Shape permits Shape$Circle, Shape$Rect, Shape$Point {}
  ```
- [ ] payload variant が `record` として emit される:
  ```java
  public static final record Shape$Circle(double r) implements Shape {}
  ```
- [ ] unit variant が singleton class として emit される:
  ```java
  public static final class Shape$Point implements Shape {
      public static final Shape$Point INSTANCE = new Shape$Point();
      private Shape$Point() {}
  }
  ```
- [ ] binary naming が `EnumName$VariantName`（`$` 区切り）である（MVP 凍結、変更不可）
- [ ] Java reflection で `Class.forName("com.example.Shape$Circle")` が解決可能
- [ ] `pub trait` の impl が Java interface として variant record の `implements` に公開される
- [ ] `internal` / `private` trait の impl は Java surface に露出しない
- [ ] Valen は serializer を提供しない（Jackson/Gson 等の設定は利用者責任）
- [ ] 特別な reflection helper や registry は提供しない
- [ ] variant 追加・削除・payload 変更が semver major 相当として扱われる
- [ ] バイトコード emit の統合テスト（emit → `javap` 検証 or classfile 解析）が存在する
