# 21. 将来機能バックログ

この文書は、MVP や Phase 境界をいったん無視して、Valen に将来追加しうる機能を蓄積するためのメモである。

ここに載せることは採用決定ではない。各機能は、正式仕様化の前に次の観点で評価する。

- Valen の芯である ADT / exhaustive match / trait / 失敗モデルを強めるか
- JVM / Java 資産との連携を明確に良くするか
- Kotlin との差分として、Valen らしい設計選択になっているか
- 新しい概念が既存概念と二重化しないか
- 構文糖衣なら、脱糖先が単純で説明可能か

## 21.1 失敗モデル・安全境界

### `unsafe` ブロック / `unsafe fn`

Java interop、FFI、unchecked cast、reflection、Panama 呼び出しなど、Valen の通常の型・失敗モデルでは保証できない操作を明示的に囲う。

候補:

```valen
let raw: String = unsafe {
    java_map.get("key")  // Option 化を bypass する例
};

unsafe fn call_native(ptr: MemorySegment) -> Int {
    ...
}
```

検討点:

- `unsafe` が許可する操作を列挙制にするか、単なる責任境界にするか
- `safe {}` との関係。`safe` は Java exception / null の正規化、`unsafe` は正規化の bypass と分ける
- `unsafe fn` の呼び出し側にも `unsafe` を要求するか
- panic / Exception / Result のどれに落とすべき失敗か

### Effect-like `try`

`Result` / `Option` / Java exception 境界を、ブロック単位で扱う構文。

候補:

```valen
let user = try Result<User, AppError> {
    let row = db.find(id)?;
    User::from_row(row)?
};

let value = try Option<Int> {
    let x = xs.first()?;
    x + 1
};
```

狙い:

- `?` の伝播先をブロックで明示する
- 異なる失敗コンテキストの混在を防ぐ
- 将来の effect system への足場にする

検討点:

- `try` はただのブロック式か、専用の effect boundary か
- `Result<T, E>` の `E` 同一型ルールを維持するか
- Java exception を `try JavaException { ... }` のように扱うか

### Java Exception catch expression

Java ライブラリのラッパーを書くため、Java Exception を明示的に捕捉して Valen の `Result` / ユーザ定義エラーへ変換する構文。

狙いは Java の `throw` / `catch` モデルを Valen 内へ広げることではなく、Java interop 境界で Exception を閉じ込めることにある。Valen のドメイン失敗は引き続き `Result<T, E>` で表現する。

候補 A: `safe` に `catch` arm を付ける

```valen
enum ReadError {
    NotFound(path: String),
    PermissionDenied(path: String),
    Java(cause: JavaException),
}

fn read_text(path: String) -> Result<String, ReadError> {
    safe {
        java.nio.file.Files.readString(java.nio.file.Path.of(path))
    } catch e: java.nio.file.NoSuchFileException {
        Err(ReadError::NotFound(path))
    } catch e: java.nio.file.AccessDeniedException {
        Err(ReadError::PermissionDenied(path))
    } catch e: java.io.IOException {
        Err(ReadError::Java(JavaException::from(e)))
    }
}
```

候補 B: `catch` を式として独立させる

```valen
fn read_text(path: String) -> Result<String, ReadError> {
    catch java.nio.file.Files.readString(java.nio.file.Path.of(path)) {
        e: java.nio.file.NoSuchFileException => Err(ReadError::NotFound(path)),
        e: java.nio.file.AccessDeniedException => Err(ReadError::PermissionDenied(path)),
        e: java.io.IOException => Err(ReadError::Java(JavaException::from(e))),
    }
}
```

候補 C: `Result` 変換を前面に出す

```valen
fn read_text(path: String) -> Result<String, ReadError> {
    java_try {
        java.nio.file.Files.readString(java.nio.file.Path.of(path))
    }.catch(|e: java.nio.file.NoSuchFileException| ReadError::NotFound(path))
     .catch(|e: java.nio.file.AccessDeniedException| ReadError::PermissionDenied(path))
     .catch(|e: java.io.IOException| ReadError::Java(JavaException::from(e)))
}
```

検討点:

- `catch` arm は declaration order で評価し、Java と同じく subtype から supertype の順を要求するか
- `Throwable` / `Error` まで捕捉可能にするか。基本は `Exception` 以下に制限し、`Error` は panic 相当として扱う案が自然
- checked exception だけを対象にするか、runtime exception も明示 catch 可能にするか
- catch しなかった Java Exception は `JavaException` として `Err` に包むか、コンパイルエラーにするか
- `safe {}` 内の Java null は従来通り `Option<T>` 化するか、catch 構文では戻り値だけ生型にするか
- `catch` arm の戻り型はすべて同じ型に統一し、暗黙の `Exception -> Error` 変換は行わない

当面の有力案:

- `safe { ... } catch ...` を Java 境界専用構文にする
- 成功時は `Ok(value)`、catch arm は `Err(domain_error)` を明示的に返す
- Valen 内では `throw` 文を導入しない
- Java ラッパー層で Exception を Valen の ADT エラーへ畳み込む用途に限定する

### `defer` / scope guard

リソース解放を `Result` や panic と整合させるための構文。

```valen
fn read(path: String) -> Result<String, IoError> {
    let f = File::open(path)?;
    defer f.close();
    f.read_to_string()
}
```

検討点:

- Java `AutoCloseable` との対応
- `defer` 中の失敗を握りつぶすか、panic か、合成 Result か
- `using` / `try-with-resources` 風構文との比較

## 21.2 式・制御構文

### pipeline 演算子

値を左から右へ流す構文。ネストした関数適用や `map` / `filter` 連鎖を読みやすくする。

候補:

```valen
let names =
    users
    |> filter(|u| u.active)
    |> map(|u| u.name)
    |> sort();
```

脱糖候補:

- `x |> f` => `f(x)`
- `x |> T::method(arg)` => `T::method(x, arg)`
- `x |> .method(arg)` => `x.method(arg)` を許すかは別途検討

検討点:

- UFCS の `Trait::method(receiver, args)` と整合するか
- メソッドチェーンと二重化しないか
- `Result` / `Option` の `?` と組み合わせた時の優先順位
- `|>` 以外に `then` / `pipe` 関数で足りるか

### `when`

`match` より軽い条件分岐、または値を取らない exhaustive 分岐として使う構文。

候補 A: Kotlin 風の条件列挙

```valen
let label = when {
    n < 0 => "negative",
    n == 0 => "zero",
    else => "positive",
};
```

候補 B: `match` の糖衣

```valen
let label = when n {
    0 => "zero",
    1..=9 => "small",
    else => "large",
};
```

検討点:

- `match` と役割が重なりすぎないか
- `else` と `_` のどちらを使うか
- exhaustive check の対象にするか
- guard 付き `match` で十分ではないか

### trailing block

最後の引数がラムダのとき、括弧の外へ出せる構文。DSL と resource management に効く。

```valen
transaction(db) {
    insert_user(user)?;
}

html {
    body {
        text("hello")
    }
}
```

検討点:

- receiver lambda とセットで導入するか
- `fn f(x: Int, block: () -> Unit)` のような通常ラムダだけで始めるか
- 制御構文に見える API をどこまで許すか
- Java SAM 変換と組み合わせるか

### labeled block / early break from block

ループではないブロックから値付きで脱出する構文。

```valen
let user = 'resolve: {
    if cached.isSome() { break 'resolve cached.unwrap(); }
    load_user(id)?
};
```

検討点:

- `return` より狭い早期脱出として導入価値があるか
- `try` ブロックと二重化しないか

## 21.3 ADT・型システム

### 匿名和型

名前付き enum を定義せず、その場で閉じた和型を表現する。

候補:

```valen
fn parse_token(s: String) -> |Ident(String) | Number(Int) | Symbol(Char)| {
    ...
}
```

狙い:

- 小さな parser / visitor / interop adapter で型定義のノイズを減らす
- exhaustive match の価値を局所的な型にも広げる

検討点:

- Java ABI にどう落とすか。匿名 sealed hierarchy か、コンパイラ内部型か
- public API に露出を許すか
- variant 名の衝突と import 表示
- 名前付き enum への昇格 refactor を容易にするか

### row polymorphism / open record

匿名 record や部分 record を扱う型。JavaBean / JSON / DB row との相性を上げる可能性がある。

```valen
fn display(x: { name: String, age: Int, ... }) -> String {
    f"{x.name} ({x.age})"
}
```

検討点:

- nominal type 中心の方針と衝突しないか
- Java interop では reflection に寄りすぎないか
- trait 制約で代替できるか

### refinement / newtype

`typealias` ではなく所有権を持つ軽量 wrapper。

```valen
newtype UserId = Int;
newtype Email = String where Email::is_valid;
```

狙い:

- orphan rule 上の所有を明確に持てる
- primitive obsession を避ける
- バリデーション済み値を型で区別する

検討点:

- Valhalla value class と将来連携できるか
- runtime cost を仕様で保証するか、実装詳細に留めるか
- `derive` と組み合わせる trait 群

### intersection / union constraints

trait 境界を複合的に表現する。

```valen
fn write_all<T: Read & Close>(x: T) -> Result<Unit, IoError> { ... }
```

検討点:

- anonymous sum type と union type を混同しない
- Java wildcard / intersection bound との対応

## 21.4 trait・自動実装

### `derive`

構造から明らかな trait 実装を生成する。

```valen
#[derive(Eq, Hash, Debug, Clone)]
enum Color {
    Red,
    Rgb(Int, Int, Int),
}
```

候補 trait:

- `Eq` / `Hash`
- `Debug` / `Display`
- `Clone` / `Copy` 相当
- `Ord` / `PartialOrd`
- `Serialize` / `Deserialize`
- `Error`

検討点:

- annotation 構文 `@` と Rust 風 `#[...]` のどちらを使うか
- derive macro まで開くか、builtin derive のみにするか
- Java `equals` / `hashCode` / `toString` と完全連動させるか
- orphan rule と coherence にどう組み込むか

### sealed trait / closed trait

trait 実装集合を閉じ、trait 上の exhaustive match を許す。

```valen
sealed trait Expr

impl Expr for Lit
impl Expr for Add
```

検討点:

- enum と役割が重なりすぎないか
- Java sealed interface と ABI を揃えるか
- downstream crate / module での impl 禁止をどう表現するか

### specialization / default impl

generic trait impl の上に、より具体的な型向け実装を許す。

検討点:

- coherence を壊しやすい
- JVM dispatch で説明可能か
- Rust の specialization と同じ不安定さを持ち込まないか

### extension property

trait method に加えて、読み取り専用 property 風の拡張を許す。

```valen
trait HasLength {
    prop length(self) -> Int;
}
```

検討点:

- Kotlin の extension property と同じ錯覚を生まないか
- 実体は method であることを仕様上明確にできるか

## 21.5 Java / JVM 連携

### Java にあり Valen にない機能の棚卸し

Java 互換性のために、Java にある機能を Valen がどう扱うかを明示的に棚卸しする。目的は Java の全面コピーではなく、Java ライブラリのラッパー作成・既存資産利用・JVM toolchain 連携で困らない境界を決めること。

| Java 機能 | Valen 方針候補 | 理由 |
|-----------|----------------|------|
| checked exception | Java 境界で `safe { ... } catch ...` により `Result` / ADT エラーへ変換 | Valen 内に `throw` を広げず、失敗モデルを維持する |
| try-with-resources | `defer` / `using` / `AutoCloseable` adapter を検討 | Java wrapper 実装で必要。Result と close failure の関係を詰める |
| method overloading | Java 呼び出し時のみ厳密解決。Valen API では原則 named function / trait で回避 | overload は型推論・named arg・nullability と衝突しやすい |
| static members | Java interop として参照可能。Valen 側は module-level fn / associated fn を優先 | JVM ABI 互換は必要だが、言語モデルは単純化する |
| nested / inner class | Java interop で参照可能。Valen 定義では nested type を限定検討 | Java ライブラリ利用で避けられない |
| anonymous class | Java SAM / interface adapter で代替。Valen には原則入れない | trait / lambda と概念が重なる |
| lambda / method reference | lambda は採用済み。Java method reference 相当は UFCS / function pointer として検討 | Java API への渡し込みで必要 |
| records | Valen `data class` / record-like class と ABI 対応を検討 | Java から扱いやすい値オブジェクトが必要 |
| sealed class / interface | Valen enum / sealed hierarchy と対応。`@valen.Closed` で Java 側 exhaustive opt-in | exhaustive match の芯に直結 |
| annotations | Java annotation authoring を検討。Valen compiler attribute と分離する | framework 連携で必要 |
| reflection | runtime reflection は Java 経由。Valen metadata / compile-time reflection は別途検討 | Java framework 互換と Valen 型情報の両立 |
| modules (`module-info.java`) | JPMS 連携を検討。ただし Valen module identity とは分ける | orphan rule / coherence の module と JPMS は目的が違う |
| generics wildcard | Java interop 境界で扱う。Valen 表面構文は variance 指定を優先 | `? extends` / `? super` をそのまま持ち込むと複雑 |
| varargs | Java 呼び出しで対応。Valen 定義側は配列 / collection 引数を優先 | overload 解決と絡むため限定的に扱う |
| synchronized / monitor | 標準ライブラリまたは Java interop として扱う。言語構文化は慎重 | virtual thread 時代の設計と合わない可能性 |
| primitive numeric widening | 導入しない | 暗黙変換なしの既定方針を維持 |
| nullable reference | 導入しない。`Option<T>` に一本化 | 失敗モデルの芯を維持 |
| inheritance-heavy OOP | class 継承は限定。ADT / trait を優先 | Valen の差別化軸を維持 |

追加で検討すべき Java 互換機能:

- Java enum との相互運用。Valen enum は ADT だが、Java enum を match 対象としてどう扱うか
- Java builder / fluent API の wrapper 生成。named args / trailing block と組み合わせる余地
- JavaBean property adapter。`getX()` / `setX()` を property 風に見せるか
- `Optional<T>` と `Option<T>` の変換規約
- `Stream<T>` と Valen collection / iterator の変換規約
- `CompletableFuture<T>` と structured concurrency / async 境界の変換規約
- `ServiceLoader` / SPI との連携
- serialization framework 向け metadata / annotation strategy

### JDK 25 first-class target

JDK 25 を単なる opt-in ではなく、一級ターゲットとして扱う案。

背景:

- JDK 25 は 2025-09-16 に GA になった Java SE 25 実装であり、JDK 21 の次世代ターゲットとして現実的
- 多くの vendor が JDK 25 系を長期サポート対象として扱う見込み
- Valen は新規言語なので、古い JVM 互換よりも JDK 25 世代の JVM 機能を先に設計へ取り込める

候補:

- `--target 21`: 互換 baseline。Java 21 LTS 世代の広い実行環境を対象にする
- `--target 25`: first-class target。JDK 25 世代の標準・preview・incubator 機能を明示 opt-in で活用する
- `--target latest`: 開発実験用。preview feature や Valhalla / Panama の早期検証に使う

JDK 25 で特に見るべき機能:

- Scoped Values: request context / task-local state の安全な表現
- Structured Concurrency: Valen の task scope 設計との接続
- Stable Values: lazy initialization / once cell 相当の標準化候補
- primitive types in patterns / switch: Valen pattern lowering の最適化候補
- compact object headers: ADT / data class の object density への影響
- Vector API: numeric / collection stdlib の最適化候補
- module import declarations / compact source files: Valen へ直接輸入するより、Java interop と tooling で参照

検討点:

- JDK 25 preview / incubator 機能に依存する場合、Valen の安定仕様に含めるか、target-specific optimization に留めるか
- `--enable-preview` が必要な機能を使う場合、Gradle plugin / CLI / LSP がどう表示するか
- JDK 21 target と JDK 25 target で ABI が分岐する場合、Java から見た互換性をどう守るか
- JDK 25 first-class 化は Valhalla 採用決定ではない。Valhalla 連携は別途 feature detection と fallback を設計する

### Project Valhalla 連携

JVM value classes / primitive classes を Valen の `newtype`、小さな record、Option 最適化に活用する。

候補:

- `newtype UserId = Int` を value class として emit
- `Option<Int>` などの boxing 削減
- 小さな enum / record の flat layout
- `--target 25` 以降で opt-in

検討点:

- JVM バージョンごとに ABI が変わる問題
- Java から見た API の安定性
- Valhalla がない target への fallback
- 「仕様上の意味」と「最適化」を混ぜない

### Project Panama 連携

Foreign Function & Memory API を Valen の安全境界で包む。

候補:

```valen
unsafe extern "c" fn strlen(ptr: MemorySegment) -> Long;
```

狙い:

- JVM 上で native library を呼ぶ標準ルートを持つ
- `unsafe` / `Result` / resource management の設計を実戦投入する

検討点:

- `MemorySegment` / `Arena` を標準ライブラリにどう露出するか
- lifetime を所有権なしでどう扱うか
- native failure を `Result` に変換する規約
- checked exception ではなく panic すべき境界はどこか

### Java annotation authoring

Valen コードから Java annotation を付与・宣言できるようにする。

```valen
@Deprecated("use newApi")
pub fn old_api() -> Unit { ... }
```

検討点:

- `@` は Java annotation、`#[...]` は Valen compiler attribute のように分離するか
- retention / target / repeatable の指定
- annotation 引数に許す式の範囲

### nullability trust modes

Java の `@NonNull` / `@Nullable` をどこまで信用するかを設定可能にする。

候補:

- default: すべて `T?`
- strict annotations: 信頼できる annotation package のみ `T`
- unsafe trust: classpath annotation を全面採用

検討点:

- Valen の失敗モデルを壊さない default を維持する
- build tool で trust list を明示する

## 21.6 メタプログラミング

### hygienic macro

AST ベースの hygienic macro。文字列置換は禁止。

候補:

```valen
macro assert_eq(left, right) {
    ...
}
```

検討点:

- compile-time API の安定性
- IDE / LSP が展開前後をどう扱うか
- derive だけで足りる範囲を先に見極める

### compile-time reflection

型・field・variant 情報をコンパイル時に読む API。

用途:

- serializer 生成
- database mapper
- exhaustive UI renderer

検討点:

- Java reflection と Valen metadata の二重化
- private member を読める範囲
- incremental compilation への影響

### const eval

コンパイル時に純粋式を評価する。

```valen
const PAGE_SIZE: Int = 1024 * 4;
```

検討点:

- 許す式の範囲
- panic を compile error にするか
- Java static final との対応

## 21.7 並行・非同期

### async / await

JVM virtual thread を baseline としつつ、非同期境界を型で表す構文を持つか。

検討点:

- virtual thread で十分な領域と、structured concurrency が必要な領域の分離
- `Result` と cancellation の関係
- Java `CompletableFuture` / reactive library との相互運用

### structured concurrency

task scope を言語または標準ライブラリで提供する。

```valen
let result = task_scope {
    let a = async { fetch_a() };
    let b = async { fetch_b() };
    combine(a.await?, b.await?)
};
```

検討点:

- Java StructuredTaskScope との対応
- cancellation を panic / Result / 専用型のどれにするか

### actor / channel

ADT と match を活かす message passing。

```valen
enum Msg {
    Increment,
    Get(reply_to: Sender<Int>),
}
```

検討点:

- 標準ライブラリ機能で十分か
- exhaustive match と protocol evolution の相性

## 21.8 標準ライブラリ・ツール

### parser combinator / pattern-centric library

ADT と exhaustive match を前面に出した parser / visitor / transform ライブラリ。

### property-based testing

ADT の generator を derive できるテスト基盤。

```valen
#[derive(Arbitrary)]
enum Command { ... }
```

### snapshot testing

compiler / formatter / LSP の golden test を標準化する。

### package metadata / module identity 強化

coherence と orphan rule のため、module identity を classfile metadata と build tool metadata の両方で扱う。

## 21.9 構文糖衣候補

### `let-else`

パターン不一致時の早期脱出。

```valen
let Some(user) = find_user(id) else {
    return Err(AppError::NotFound(id));
};
```

### `if let` / `while let`

単一パターンの簡易 match。

```valen
if let Some(x) = maybe_x {
    use(x);
}
```

### collection literal

```valen
let xs = [1, 2, 3];
let map = {"a": 1, "b": 2};
```

検討点:

- 標準 collection の名義型とどう結びつけるか
- Java collection へ落とすか、Valen collection を持つか

### range / slice

```valen
xs[1..]
xs[..10]
xs[1..=10]
```

検討点:

- Java collection では slice が view か copy か
- bounds error は panic か Result か Option か

## 21.10 優先度を決める時の仮分類

芯を強める候補:

- Effect-like `try`
- `derive`
- sealed trait
- 匿名和型
- `let-else` / `if let`
- `newtype`

JVM 連携を強める候補:

- JDK 25 first-class target
- Java にあり Valen にない機能の棚卸し
- Project Valhalla 連携
- Project Panama 連携
- Java annotation authoring
- nullability trust modes
- module identity metadata

便利機能だが二重化に注意する候補:

- pipeline 演算子
- `when`
- trailing block
- extension property
- async / await

特に慎重に扱う候補:

- specialization
- hygienic macro
- row polymorphism / open record
- `unsafe` の範囲拡大
