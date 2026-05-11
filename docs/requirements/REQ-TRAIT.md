# 要件定義: trait・coherence (TRAIT)

プロジェクト: Valen Language
スコープ: trait 定義・impl ブロック・orphan rule・UFCS
最終更新: 2026-05-11

---

## 要件一覧

| ID | タイトル | 優先度 | ステータス | 依存 |
|----|---------|--------|-----------|------|
| REQ-TRAIT-001 | trait 定義 + impl ブロック（inherent impl なし） | Must | Draft | REQ-SYNTAX-001, REQ-CLASS-001 |
| REQ-TRAIT-002 | trait 充足は impl ブロックのみ（class 本体 method ≠ trait） | Must | Draft | REQ-TRAIT-001, REQ-CLASS-005 |
| REQ-TRAIT-003 | orphan rule（module 所有、blanket impl 禁止、global 一意性） | Must | Draft | REQ-TRAIT-001, REQ-CLASS-006 |
| REQ-TRAIT-004 | UFCS（Trait::method(receiver, args) 一本化） | Must | Draft | REQ-TRAIT-001 |

---

## REQ-TRAIT-001: trait 定義 + impl ブロック

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TRAIT-001 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-SYNTAX-001, REQ-CLASS-001 |
| **説明** | `trait` でインターフェースを定義し、`impl Trait for Type` ブロックで実装を提供する。`impl Type { ... }`（inherent impl block）は存在しない。class の instance method / associated function は class 本体に直接書く。レシーバは明示 `self` / `mut self` のみ（借用 `&self` / `&mut self` は導入しない）。 |

### 受け入れ基準

- [ ] trait 定義が正しくパースされる:
  ```valen
  trait Area {
      fn area(self) -> Float;
  }
  ```
- [ ] trait に default method を定義可能:
  ```valen
  trait Display {
      fn display(self) -> String { "default" }
  }
  ```
- [ ] `impl Trait for Type { ... }` ブロックが正しくパースされる:
  ```valen
  impl Area for Shape {
      fn area(self) -> Float { ... }
  }
  ```
- [ ] `impl Type { ... }`（inherent impl block）がパーサエラーとなる
- [ ] レシーバとして `self`（immutable）と `mut self`（mutable）が使用可能
- [ ] `&self` / `&mut self`（借用レシーバ）がパーサエラーとなる（所有権モデルなし）
- [ ] trait の required method を全て実装しないとコンパイルエラーとなる
- [ ] default method を override 可能
- [ ] trait のバイトコード emit が Java interface として正しく出力される

---

## REQ-TRAIT-002: trait 充足は impl ブロックのみ

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TRAIT-002 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-TRAIT-001, REQ-CLASS-005 |
| **説明** | trait method の実装は `impl Trait for Type { ... }` ブロック内でのみ成立する。class 本体の method が trait の method と同名・同シグネチャであっても、trait 充足にはならない（両者は独立）。これにより trait 充足の明示性を保証する。 |

### 受け入れ基準

- [ ] class 本体に trait と同名・同シグネチャの method があっても trait 充足にならない:
  ```valen
  trait Show { fn show(self) -> String; }
  class User(pub name: String) {
      fn show(self) -> String { self.name }  // これは trait 充足ではない
  }
  // impl Show for User { ... } が別途必要
  ```
- [ ] `impl Show for User` なしで `User` を `Show` が要求される文脈に渡すとコンパイルエラーとなる
- [ ] class 本体に同名 method がある場合、`value.show()` は class 本体 method を優先解決する
- [ ] trait method を呼び出すには UFCS `Show::show(value)` または trait が唯一の候補の場合に `value.show()` で解決される
- [ ] エラーメッセージが「trait 充足は impl ブロックで行う必要がある」旨を案内する
- [ ] class 本体 method と trait method が異なるシグネチャの場合、arity や型で区別して共存可能

---

## REQ-TRAIT-003: orphan rule（module 所有、blanket impl 禁止）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TRAIT-003 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-TRAIT-001, REQ-CLASS-006 |
| **説明** | `impl Trait for Type` を許可する条件を orphan rule として規定する。所有単位は module。blanket impl は MVP 全面禁止（std 限定）。同一 trait/type 対に対する impl はグローバル一意。 |

### 受け入れ基準

- [ ] `Trait` が現在の module で定義されている場合に impl が許可される
- [ ] `Type` の outermost nominal type constructor が現在の module に所有されている場合に impl が許可される
- [ ] foreign trait for foreign type がコンパイルエラーとなる（例: `impl java.util.List for java.lang.String` 不可）
- [ ] typealias を介した所有権回避がコンパイルエラーとなる（`typealias MyList = java.util.List<Int>` に対する impl 不可）
- [ ] blanket impl（`impl<T: Foo> Bar for T`）が MVP でコンパイルエラーとなる
- [ ] 同一 trait/type 対に対する impl がグローバル一意であり、二重定義がコンパイルエラーとなる
- [ ] downstream module での再定義が禁止される
- [ ] orphan rule 違反のエラーメッセージが「trait か型の少なくとも一方が自 module 所有である必要がある」旨を案内する
- [ ] orphan rule の所有単位が package ではなく module であることを検証するテストが存在する

---

## REQ-TRAIT-004: UFCS（Trait::method(receiver, args) 一本化）

| 項目 | 内容 |
|------|------|
| **ID** | REQ-TRAIT-004 |
| **優先度** | Must |
| **ステータス** | Draft |
| **依存** | REQ-TRAIT-001 |
| **説明** | UFCS（Uniform Function Call Syntax）を `Trait::method(receiver, args)` 形式で提供する。trait method の曖昧性解消、trait default method の super 呼び出し相当に使用する。`Class::func(args)` は associated function 呼び出しに限定。 |

### 受け入れ基準

- [ ] `Trait::method(value, args)` で trait method を明示的に呼び出せる:
  ```valen
  impl Area for Shape { fn area(self) -> Float { ... } }
  let a = Area::area(shape);  // UFCS 呼び出し
  ```
- [ ] 複数 trait が同名 method を持つ場合、UFCS で曖昧性を解消できる:
  ```valen
  trait A { fn foo(self) -> Int; }
  trait B { fn foo(self) -> Int; }
  // value.foo() はコンパイルエラー（曖昧）
  A::foo(value)  // OK: trait A の foo
  B::foo(value)  // OK: trait B の foo
  ```
- [ ] trait default method を `impl` ブロック内から UFCS で呼び出せる（`super` 相当）:
  ```valen
  impl Display for User {
      fn display(self) -> String {
          let base = Display::display(self);  // default method 呼び出し
          f"User: {base}"
      }
  }
  ```
- [ ] `Class::func(args)` は associated function（`self` なし）の呼び出しに限定される
- [ ] class 本体の instance method を `Class::method(value, ...)` で呼ぶことは不可（`value.method()` を使う）
- [ ] UFCS 呼び出しのバイトコード emit が正しく生成される
- [ ] 曖昧性解消と UFCS 呼び出しのテストが各パターンで存在する
