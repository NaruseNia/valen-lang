# 要件定義: クラス・モジュール (CLASS)

プロジェクト: Valen Language
スコープ: クラス定義・継承・可視性・モジュール
最終更新: 2026-05-11

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス | 依存 |
|----|---------|--------|-----------|------|
| REQ-CLASS-001 | class + primary constructor（pub/mut 修飾） | Must | Draft | REQ-SYNTAX-001, REQ-TYPE-001 |
| REQ-CLASS-002 | data class（equals/hashCode/toString/copy 自動生成） | Must | Draft | REQ-CLASS-001 |
| REQ-CLASS-003 | 継承（open/abstract/sealed opt-in、単一継承+複数 trait） | Must | Draft | REQ-CLASS-001, REQ-TRAIT-001 |
| REQ-CLASS-004 | sealed class（closed OOP hierarchy） | Must | Draft | REQ-CLASS-003 |
| REQ-CLASS-005 | メソッド解決規則（class 本体優先→trait→UFCS） | Must | Draft | REQ-CLASS-001, REQ-TRAIT-001 |
| REQ-CLASS-006 | 可視性（pub/internal/private、module 単位） | Must | Draft | REQ-CLASS-001 |
| REQ-CLASS-007 | package 宣言必須 | Must | Draft | REQ-SYNTAX-001 |
| REQ-CLASS-008 | import（単一型 + alias、MVP） | Must | Draft | REQ-CLASS-007 |

---

## REQ-CLASS-001: class + primary constructor

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-SYNTAX-001, REQ-TYPE-001 |
| **説明** | `class` 宣言と一体の primary constructor を定義する。パラメータに `pub` / `mut` 修飾を個別に指定可能。class 本体に instance method と associated function を直接記述する（inherent impl block は存在しない）。 |

### 受け入れ基準

- [ ] `class User(pub name: String, mut age: Int) { ... }` がパーサで正しく解析される
- [ ] primary constructor パラメータの修飾子組み合わせが正しく処理される:
  - [ ] 無修飾 = private field（class 内部のみ `self.name` で参照可）
  - [ ] `pub` = public 読み取り専用 field
  - [ ] `mut` = private 可変 field
  - [ ] `pub mut` = public 可変 field（結合順: 可視性先、`mut` 後）
- [ ] `internal` / `private` 個別指定は MVP で未実装（Phase 1.5+ 送り）、使用時にエラーとなる
- [ ] instance method（`fn method(self) -> T`）が class 本体に直接記述可能
- [ ] associated function（`fn assoc(x: T) -> U`、`self` なし）が class 本体に直接記述可能
- [ ] `impl Class { ... }` 構文（inherent impl block）がパーサエラーとなる
- [ ] associated function の呼び出しが `ClassName::func_name(args)` で可能
- [ ] instance method の呼び出しが `value.method(args)` で可能
- [ ] `static` キーワードが存在しない（使用時にエラー）

---

## REQ-CLASS-002: data class

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-001 |
| **説明** | `data class` を宣言すると `equals` / `hashCode` / `toString` / `copy` が自動生成される。自動生成対象は自身の primary constructor params のみ。常に final であり継承元にはなれない。 |

### 受け入れ基準

- [ ] `data class Point(x: Float, y: Float);` がパーサで正しく解析される
- [ ] `equals` が primary constructor params に基づく構造比較として自動生成される
- [ ] `hashCode` が primary constructor params に基づいて自動生成される
- [ ] `toString` が `ClassName(field1=value1, field2=value2)` 形式で自動生成される
- [ ] `copy` メソッドが自動生成される（named args で一部フィールドのみ変更可能）
- [ ] 自動生成対象は自身の primary constructor params のみ（親 class の state は含めない）
- [ ] `data class` は常に final である
- [ ] `data class` に `open` / `abstract` / `sealed` を付与するとコンパイルエラーとなる
- [ ] `data class` を superclass として継承しようとするとコンパイルエラーとなる
- [ ] `data class` が `sealed` / `open` / `abstract` な superclass を継承することは可能
- [ ] `impl Trait for DataClass { ... }` で trait 実装が可能

---

## REQ-CLASS-003: 継承

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-001, REQ-TRAIT-001 |
| **説明** | 単一 class 継承 + 複数 trait impl を提供する。class はデフォルト final で、`open` / `abstract` / `sealed` で明示的に opt-in する。opt-in は推移しない。method 単位の `open fn` / `override fn` による opt-in 制御。 |

### 受け入れ基準

- [ ] class がデフォルトで final であり、継承しようとするとコンパイルエラーとなる
- [ ] `open class` を宣言すると継承可能になる
- [ ] `abstract class` を宣言すると抽象クラスとして機能する（`abstract fn` が定義可能）
- [ ] `sealed class` を宣言すると同一 module 内でのみ継承可能になる
- [ ] opt-in の推移なし: `open class A` の子 `class B : A` はデフォルト final、`B` からさらに継承するには `open class B : A` が必要
- [ ] method 単位 `open fn`: `open class` 内でも method はデフォルト final、`open fn` を明示したもののみ override 可
- [ ] `override fn` 必須: 親 method を上書きするとき `override fn` がないとコンパイルエラー
- [ ] `super.foo()` は class 親の method のみ呼び出し可能
- [ ] trait default method の呼び出しは UFCS `Trait::foo(self)` を使う
- [ ] 単一 class 継承 + 複数 trait impl の組み合わせが正しく動作する

---

## REQ-CLASS-004: sealed class

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-004 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-003 |
| **説明** | `sealed class` は closed OOP hierarchy を形成する。permit 対象は `class` と `data class` のみ。permit 範囲は同一 module。各 subtype は独自の state / method / trait impl を持てる。 |

### 受け入れ基準

- [ ] `sealed class Payment;` がパーサで正しく解析される
- [ ] permit 先が `class` と `data class` のみ許可される（`enum` / `trait` は permit 先にできない）
- [ ] permit 範囲が同一 module に限定される（別 module から継承するとコンパイルエラー）
- [ ] 各 subtype が独自の state / method / trait impl を持てる
- [ ] nested 記法（sealed class 本体に permit 先を書く方式）は廃止されている
- [ ] subtype は別ファイルでも書けるが、同一 module に属する必要がある
- [ ] `match` で sealed class hierarchy を使った exhaustive check が動作する
- [ ] sealed class の全 subtype を網羅しない `match` がコンパイルエラーとなる

---

## REQ-CLASS-005: メソッド解決規則

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-005 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-001, REQ-TRAIT-001 |
| **説明** | `value.foo(args)` の解決手順を規定する。class 本体 member → trait method → UFCS 曖昧性エラーの順で解決する。class 本体 method は trait を充足しない（trait 充足は `impl` ブロックのみ）。 |

### 受け入れ基準

- [ ] class 本体に適用可能な member があれば最優先で採用される
- [ ] class 本体に候補がない場合、in-scope な trait method に解決が落ちる
- [ ] trait 候補が複数で曖昧になる場合にコンパイルエラーとなる
- [ ] 曖昧性の解消が UFCS `Trait::foo(value, args)` で可能
- [ ] class 本体に trait と同名・同シグネチャの method があっても trait 充足にならない（独立した method として扱われる）
- [ ] class 本体 method と trait method のシグネチャが異なる場合（arity や型制約で区別可能）、同名でも `override` 不要
- [ ] `Class::foo(args)` は associated function（`self` なし）の呼び出しに限定される
- [ ] メソッド解決のテストが各優先度レベルで最低1ケースずつ存在する

---

## REQ-CLASS-006: 可視性（pub/internal/private、module 単位）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-006 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-001 |
| **説明** | `pub` / `internal` / `private` の3段階可視性を提供する。デフォルトは `internal`。`internal` の範囲は module（ビルドツール駆動で決定）に従う。 |

### 受け入れ基準

- [ ] `pub` が「どこからでも見える」として機能する
- [ ] `internal` が「同一 module 内からのみ見える」として機能する
- [ ] `private` が declaration-private（クラス内・トップレベル内、Kotlin 流）として機能する
- [ ] デフォルト可視性が `internal` である（修飾子省略時）
- [ ] `internal` の範囲が module ID に従う（Gradle subproject 名 = 1 module）
- [ ] `valenc` CLI 単体使用時に `--module <name>` で module ID を指定可能
- [ ] 可視性違反（private member への外部アクセス等）がコンパイルエラーとなる
- [ ] 可視性修飾子が class / fn / field に適用可能

---

## REQ-CLASS-007: package 宣言必須

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-007 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-SYNTAX-001 |
| **説明** | `.vln` ファイルの先頭に `package` 宣言を必須とする。省略した場合はコンパイルエラー。ファイルシステム階層と一致（Java と同様）。package は source 階層と名前空間のみであり、所有権・可視性単位は module の責務。 |

### 受け入れ基準

- [ ] `package com.example.foo;` がファイル先頭に正しくパースされる
- [ ] `package` 宣言がないファイルがコンパイルエラーとなる
- [ ] エラーメッセージが「package 宣言は必須」旨を案内する
- [ ] package path がファイルシステム階層と一致しない場合に警告またはエラーとなる
- [ ] package は名前空間としてのみ機能し、所有権判定（orphan rule 等）に使われない

---

## REQ-CLASS-008: import（単一型 + alias、MVP）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-CLASS-008 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-CLASS-007 |
| **説明** | MVP の import は単一型 import と `as` による alias の2形式のみ。selective import（`{A, B}`）と glob import（`*`）は Phase 1.5+ 送り。 |

### 受け入れ基準

- [ ] `import java.util.List;` が単一型 import としてパースされる
- [ ] `import java.util.concurrent.ConcurrentHashMap as CMap;` が alias 付き import としてパースされる
- [ ] import した型がファイル内で利用可能になる
- [ ] alias で import した型が alias 名で利用可能になる
- [ ] selective import（`import java.util.{List, Map};`）が MVP でパーサエラーとなる
- [ ] glob import（`import java.util.*;`）が MVP でパーサエラーとなる
- [ ] 存在しない型を import した場合にコンパイルエラーとなる
- [ ] import の重複（同名型の二重 import）がコンパイルエラーとなる
