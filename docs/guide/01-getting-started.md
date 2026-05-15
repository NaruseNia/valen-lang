# はじめに

## Valen とは

Valen は JVM をターゲットとした新しいプログラミング言語です。Rust 風の構文を持ちながら、所有権や借用の概念はありません。代数的データ型（ADT）を言語の中核に据え、exhaustive なパターンマッチ、trait ベースの抽象化、そして整合した失敗モデル（Option/Result/panic）を提供します。Java/Kotlin の資産にそのまま乗れる、ADT 中心の JVM 言語です。

## 前提条件

Valen のコンパイラ `valenc` をビルド・使用するには、以下が必要です。

- **JDK 21 以上** — Valen は JVM 21 をベースラインターゲットとしています
- **Rust toolchain** — `valenc` は Rust で実装されています（`rustup` でインストールできます）

## valenc のビルド

リポジトリをクローンし、リリースビルドを行います。

```sh
git clone https://github.com/NaruseNia/valen-lang.git
cd valen-lang
cargo build --release
```

ビルドが完了すると `target/release/valenc` にコンパイラバイナリが生成されます。

## Hello World

### ソースファイルの作成

Valen のソースファイルは拡張子 `.vln` を使います。`hello.vln` を作成してみましょう。

```valen
package hello;

fn main() {
    println("Hello, Valen!");
}
```

**ポイント:**

- `package` 宣言は必須です。ファイルの先頭に書きます
- エントリポイントは `fn main()` です
- `println` は標準出力に文字列を表示する組み込み関数です

### コンパイルと実行

```sh
valenc compile hello.vln
java -cp . hello.Main
```

`valenc compile` は `.vln` ファイルを JVM の `.class` ファイルに変換します。生成された `.class` は通常の `java` コマンドで実行できます。

## 基本構文の味見

Valen の雰囲気をつかむために、いくつかの基本構文を見てみましょう。

### 変数束縛

```valen
let x = 42;              // 不変（デフォルト）、型は Int と推論される
let mut count = 0;        // 可変にするには mut を付ける
count = count + 1;        // mut なので再代入できる
let name: String = "Valen";  // 型注釈を明示することもできる
```

- `let` で束縛した変数はデフォルトで不変です
- 変更したい場合は `let mut` を使います
- ローカル変数の型は推論されるため、多くの場合は型注釈を省略できます

### 関数定義

```valen
fn add(a: Int, b: Int) -> Int {
    a + b
}

fn greet(name: String) {
    println(f"Hello, {name}!");
}
```

- `fn` キーワードで関数を定義します
- パラメータの型と戻り値の型は明示必須です
- 戻り値が `Unit`（値を返さない）の場合は `-> Unit` を省略できます
- ブロック末尾の式がセミコロンなしで置かれると、それが戻り値になります（Rust と同じルール）

### if 式

Valen では `if` は式です。値を返すことができます。

```valen
let abs_value = if x >= 0 { x } else { -x };

// もちろん文としても使えます
if condition {
    do_something();
}
```

### フォーマット文字列

`f"..."` で文字列補間ができます。

```valen
let name = "world";
let msg = f"Hello, {name}!";
println(msg);  // Hello, world!
```

## 次のステップ

- [型システム](02-types.md) — Valen の型、リテラル、Option について
- [ジェネリクス](03-generics.md) — 型パラメータと bounds
- [クラスとデータクラス](04-classes.md) — class, data class, 継承
- [enum とパターンマッチ](05-enum-and-match.md) — ADT、match 式、exhaustive check
- [trait と impl](06-traits.md) — trait 定義、orphan rule、UFCS、sealed trait
- [失敗モデル](07-failure-model.md) — Option, Result, panic, safe { }
- [Java 相互運用](08-java-interop.md) — import, safe ブロック, @valen.Closed
- [コンパイラアーキテクチャ](09-compiler-architecture.md) — パイプライン、crate 構成、開発方法（コントリビュータ向け）
