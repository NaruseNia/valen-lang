# enum とパターンマッチ

Valen の中核機能である代数的データ型（ADT）としての `enum` と、それを安全に分解する `match` 式について説明します。

## enum の定義

Valen の `enum` は Rust 風の ADT（代数的データ型）です。各バリアントはデータ（payload）を持つことも、持たないこともできます。

```valen
enum Shape {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}
```

- `Circle` と `Rect` は名前付きフィールドの payload を持つバリアントです
- `Point` は payload を持たないバリアントです
- enum は閉じた型（closed type）であり、定義されたバリアント以外は存在しません

## バリアントの生成

バリアントにはスコープ演算子 `::` でアクセスします。payload があるバリアントは名前付き引数で値を渡します。

```valen
let circle = Shape::Circle(r = 5.0);
let rect = Shape::Rect(w = 10.0, h = 20.0);
let point = Shape::Point;
```

## match 式

`match` は式です。各アーム（arm）のパターンに値を照合し、最初にマッチしたアームの式を評価して値を返します。

```valen
fn describe(s: Shape) -> String {
    match s {
        Shape::Circle(r) => f"半径 {r} の円",
        Shape::Rect(w, h) => f"{w}x{h} の長方形",
        Shape::Point => "点",
    }
}
```

### パターンの種類

Valen の `match` は豊富なパターンをサポートしています。

#### リテラルパターン

整数や文字列などのリテラル値に直接マッチします。

```valen
match status_code {
    200 => "OK",
    404 => "Not Found",
    500 => "Internal Server Error",
    _ => "Unknown",
}
```

#### 範囲パターン

`..=` で閉区間の範囲にマッチします。

```valen
match score {
    0..=59 => "不合格",
    60..=79 => "合格",
    80..=100 => "優秀",
    _ => "範囲外",
}
```

#### or パターン

`|` で複数の値をまとめてマッチします。

```valen
match day {
    "Saturday" | "Sunday" => "休日",
    _ => "平日",
}
```

#### ガード（if 条件）

パターンの後に `if` 条件を付けて、追加の条件で絞り込めます。

```valen
match user {
    User(name, age) if age >= 20 => f"成人: {name}",
    User(name, _) => f"未成年: {name}",
}
```

ガード付きアームは exhaustive check 上、無条件には網羅したと見なされません。ガードが実行時に `false` になる可能性があるためです。

```valen
// コンパイルエラー: 負の値が網羅されていない
match n {
    x if x >= 0 => "非負",
}

// OK: ワイルドカードで残りをカバー
match n {
    x if x >= 0 => "非負",
    _ => "負",
}
```

or パターンにガードを付ける場合、ガードは or パターン全体にかかります。

```valen
match n {
    2 | 4 | 6 if n < 10 => "小さい偶数",
    _ => "その他",
}
```

#### 構造分解（destructuring）

enum バリアントやデータクラスのフィールドを取り出せます。

```valen
match shape {
    Shape::Circle(r) => 3.14159 * r * r,
    Shape::Rect(w, h) => w * h,
    Shape::Point => 0.0,
}
```

#### @束縛（binding）

`@` を使うと、パターンにマッチした値全体を変数に束縛しつつ、内部も分解できます。

```valen
match user {
    p @ User(name = "admin", ..) => admin_action(p),
    User(name, ..) => regular_action(name),
}
```

`..` は「残りのフィールドを無視する」ことを意味します。

#### ワイルドカード

`_` はあらゆる値にマッチする「何でもパターン」です。

```valen
match value {
    42 => "特別な値",
    _ => "その他",
}
```

## let-else（パターン束縛 + 早期脱出）

`let-else` は、refutable（失敗しうる）パターンで変数を束縛し、パターンが一致しない場合は `else` ブロックで脱出する構文です。`else` ブロックは必ず分岐を終了する式（`return`、`break`、`continue`、`panic`）を含む必要があります。

```valen
fn getHealth(world: World, entity: Entity) -> Int {
    let Option::Some(health) = world.getComponent(entity, "Health") else {
        return 0;
    };
    health
}
```

`let-else` を使うと、深くネストした `match` を避けてフラットなコードが書けます。

```valen
// match を使う場合（ネストが深くなる）
fn process(result: Result<Data, Error>) -> String {
    match result {
        Result::Ok(data) => {
            match data.parse() {
                Result::Ok(parsed) => parsed.toString(),
                Result::Err(_) => return "parse error",
            }
        }
        Result::Err(e) => return f"error: {e}",
    }
}

// let-else を使う場合（フラット）
fn process(result: Result<Data, Error>) -> String {
    let Result::Ok(data) = result else { return "error"; };
    let Result::Ok(parsed) = data.parse() else { return "parse error"; };
    parsed.toString()
}
```

`else` ブロックの型は `Nothing`（ボトム型）でなければコンパイルエラーになります。つまり、`else` ブロックは必ず `return`、`break`、`continue`、または `panic` で終わらなければなりません。

```valen
// コンパイルエラー: else ブロックが分岐を終了していない
let Option::Some(v) = opt else { 42 };
```

## exhaustive check（網羅性検査）

Valen の `match` は**厳密な網羅性検査**を行います。対象の型のすべてのケースをカバーしていないとコンパイルエラーになります。

### enum の網羅性

```valen
// コンパイルエラー: Point が未処理
match shape {
    Shape::Circle(r) => f"circle {r}",
    Shape::Rect(w, h) => f"rect {w}x{h}",
    // Shape::Point が抜けている!
}
```

すべてのバリアントを書くか、`_` で残りを処理する必要があります。

### sealed class / sealed trait の網羅性

`sealed class` と `sealed trait` も enum と同様に厳密な網羅性検査が行われます。

```valen
sealed class Payment;
class Card(pub number: String) : Payment();
data class Cash : Payment();

match payment {
    Card(number) => f"カード: {number}",
    Cash => "現金",
    // 全 permit を網羅しているので OK
}
```

```valen
sealed trait Expr {
    fn eval(self) -> Int;
}
class Lit {}
class Add {}

impl Expr for Lit { fn eval(self) -> Int { 0 } }
impl Expr for Add { fn eval(self) -> Int { 1 } }

match expr {
    Lit => "リテラル",
    Add => "加算",
    // sealed trait の全実装を網羅しているので OK
}
```

## enum と sealed class の使い分け

enum と sealed class は一見似ていますが、役割が異なります。「表現したいもの」ではなく「許される操作」で選んでください。

| | enum | sealed class |
|---|---|---|
| 位置づけ | ADT、**データの和** | closed OOP hierarchy、**振る舞いの階層** |
| バリアント/サブタイプ | payload を保持するデータコンテナ | 独自の state / method / trait impl を持てる |
| 独自メソッド | 持てない（trait impl 経由のみ） | 持てる |
| 継承関係 | なし（フラット） | 親-子の階層を作れる |

**選択の指針:**

- **データの和を表したい** → `enum` を使う
- **振る舞いの階層を表したい** → `sealed class` を使う

迷ったらまず `enum` を試してください。バリアントごとに独自のメソッドや状態が必要になった時点で `sealed class` を検討しましょう。

### 例: enum が適切なケース

```valen
enum JsonValue {
    Null,
    Bool(value: Bool),
    Number(value: Float),
    Str(value: String),
    Array(items: List<JsonValue>),
    Object(entries: Map<String, JsonValue>),
}

// trait impl でメソッドを追加
impl Display for JsonValue {
    fn display(self) -> String {
        match self {
            JsonValue::Null => "null",
            JsonValue::Bool(v) => f"{v}",
            JsonValue::Number(v) => f"{v}",
            JsonValue::Str(v) => f"\"{v}\"",
            JsonValue::Array(_) => "[...]",
            JsonValue::Object(_) => "{...}",
        }
    }
}
```

### 例: sealed class が適切なケース

```valen
sealed class Widget;

class Button(pub label: String) : Widget() {
    fn on_click(self) {
        println(f"Button '{self.label}' clicked");
    }
}

class TextInput(pub placeholder: String, mut value: String) : Widget() {
    fn clear(self) {
        self.value = "";
    }
}
```

## JVM 上の表現

Valen の enum は JVM では以下のように表現されます。Java から Valen の enum を使う場合に知っておくと役立ちます。

```java
// Shape enum の JVM 表現

// sealed interface として生成
public sealed interface Shape permits Shape$Circle, Shape$Rect, Shape$Point {}

// payload ありバリアント → record
public static final record Shape$Circle(double r) implements Shape {}
public static final record Shape$Rect(double w, double h) implements Shape {}

// payload なしバリアント → singleton class
public static final class Shape$Point implements Shape {
    public static final Shape$Point INSTANCE = new Shape$Point();
    private Shape$Point() {}
}
```

- payload を持つバリアントは Java の `record` になります
- payload を持たないバリアントは singleton クラスになります（メモリ効率のため）
- Java 側からは `Shape$Circle` のように `$` 区切りの名前でアクセスできます

## if let と while let

`match` の 1-arm 版として `if let` が使えます。ネストした `match` をフラットに書き直すのに便利です。

```valen
// match だと冗長
match getComponent(entity, "Position") {
    Option::Some(pos) => println(f"({pos.x}, {pos.y})"),
    Option::None => {},
}

// if let で簡潔に
if let Some(pos) = getComponent(entity, "Position") {
    println(f"({pos.x}, {pos.y})");
}
```

`else` 節や `else if let` チェーンも使えます:

```valen
if let Some(pos) = getComponent(entity, "Position") {
    println(f"pos: ({pos.x}, {pos.y})");
} else if let Some(vel) = getComponent(entity, "Velocity") {
    println(f"vel only");
} else {
    println("no components");
}
```

`while let` はパターンが一致する間ループします:

```valen
while let Some(item) = iter.next() {
    process(item);
}
```

## バリアントショートハンド

型から enum が明らかな場合、`.Variant` 構文で enum 名を省略できます。

### 式での使用

```valen
let c: Color = .Red;           // Color::Red と同じ
let b: Color = .Blue(42);      // Color::Blue(42) と同じ

fn makeRed() -> Color {
    .Red                        // 戻り値型から推論
}
```

### パターンでの使用

`match`、`if let`、`while let`、`let-else` のパターンでも使えます。

```valen
match color {
    .Red => "red",
    .Green => "green",
    .Blue(v) => f"blue({v})",
}

if let .Some(x) = maybe_value {
    process(x);
}

let .Ok(data) = result else { return default; };
```

バリアントショートハンドは、特に `match` のアーム内で enum 名の繰り返しを減らし、コードを読みやすくします。推論できない場合は `EnumName::Variant` の完全形を使ってください。
