# ジェネリクス

## 基本構文

Valen のジェネリクスは `<T>` の形式で型パラメータを宣言します。Java や Kotlin と同じ山括弧の構文です。

```valen
fn identity<T>(x: T) -> T {
    x
}
```

この関数はあらゆる型 `T` に対して動作します。呼び出し側では型推論が働くため、多くの場合は型パラメータの明示は不要です。

```valen
let a = identity(42);       // T = Int と推論される
let b = identity("hello");  // T = String と推論される
```

## 関数のジェネリクス

関数名の直後に型パラメータを宣言します。

```valen
fn first<T>(items: List<T>) -> Option<T> {
    if items.is_empty() {
        None
    } else {
        Some(items.get(0))
    }
}

fn swap<A, B>(pair: Pair<A, B>) -> Pair<B, A> {
    Pair(first = pair.second, second = pair.first)
}
```

複数の型パラメータが必要な場合は、カンマ区切りで列挙します。

## class のジェネリクス

class 名の直後に型パラメータを宣言します。class 内のメソッドや associated function からその型パラメータを参照できます。

```valen
class Box<T>(pub value: T) {
    fn map<U>(self, f: fn(T) -> U) -> Box<U> {
        Box(value = f(self.value))
    }

    fn get(self) -> T {
        self.value
    }
}
```

```valen
let box = Box(value = 42);
let mapped = box.map(|x| f"{x}");  // Box<String>
```

メソッド自体に追加の型パラメータ（上の例では `U`）を持たせることもできます。

## data class のジェネリクス

data class でも同じ構文でジェネリクスを使えます。`equals`/`hashCode`/`toString`/`copy` は型パラメータを考慮して自動生成されます。

```valen
data class Pair<A, B>(pub first: A, pub second: B);

data class Triple<A, B, C>(pub first: A, pub second: B, pub third: C);
```

```valen
let pair = Pair(first = "name", second = 42);
println(pair);  // Pair(first=name, second=42)

let other = Pair(first = "name", second = 42);
pair == other   // true — data class の構造比較
```

## trait のジェネリクス

trait にも型パラメータを宣言できます。

```valen
trait Mapper<T> {
    fn map<U>(self, f: fn(T) -> U) -> Self;
}

trait Convertible<From, To> {
    fn convert(self, value: From) -> To;
}
```

trait を実装する際には、具体的な型を指定します。

```valen
impl Mapper<Int> for Box<Int> {
    fn map<U>(self, f: fn(Int) -> U) -> Box<Int> {
        // ...
    }
}
```

## 型パラメータの bounds

型パラメータに制約を付けるには `:` の後に trait 名を指定します。これにより、その型パラメータが特定の trait を実装していることを要求できます。

```valen
trait Display {
    fn display(self) -> String;
}

fn print_value<T: Display>(value: T) {
    println(value.display());
}
```

`T: Display` は「`T` は `Display` trait を実装していなければならない」という制約です。この制約がない場合、`value.display()` の呼び出しはコンパイルエラーになります。

### 複数の bounds

型パラメータに複数の trait を要求する場合は `+` で繋ぎます。

```valen
trait Display {
    fn display(self) -> String;
}

trait Debug {
    fn debug(self) -> String;
}

fn log<T: Display + Debug>(value: T) {
    println(f"display: {value.display()}");
    println(f"debug: {value.debug()}");
}
```

### bounds 付きの class

class の型パラメータにも bounds を指定できます。

```valen
class SortedList<T: Comparable>(mut items: List<T>) {
    fn add(mut self, item: T) {
        self.items.add(item);
        self.items.sort();
    }

    fn first(self) -> Option<T> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items.get(0))
        }
    }
}
```

## Erasure（型消去）

Valen のジェネリクスは JVM の型消去（erasure）に従います。これは Java/Kotlin と同じ動作です。

- コンパイル時には型パラメータの情報でフル型チェックが行われます
- 実行時には型パラメータの情報は消えます
- `reified`（実行時に型情報を保持する）型パラメータは Phase 2 以降で導入予定です

```valen
// コンパイル時: Box<Int> と Box<String> は別の型
let a: Box<Int> = Box(value = 42);
let b: Box<String> = Box(value = "hello");

// 実行時: 両方とも Box として扱われる（型パラメータは消去済み）
```

Java 開発者の方へ: erasure の挙動は Java のジェネリクスと同じです。Kotlin の `reified` に慣れている方は注意してください。Valen の MVP ではまだ `reified` をサポートしていません。

## Variance（変位指定）

ジェネリクスの型パラメータに `in`/`out` を指定することで、サブタイピングの方向を制御できます。

| 指定 | 意味 | Kotlin での相当 | Java での相当 |
|------|------|-----------------|---------------|
| `out T` | 共変（生産者） | `out T` | `? extends T` |
| `in T` | 反変（消費者） | `in T` | `? super T` |
| 指定なし | 不変 | `T` | `T` |

```valen
// out: T を返す（生産する）だけの型
class Producer<out T>(value: T) {
    fn get(self) -> T { self.value }
}

// in: T を受け取る（消費する）だけの型
class Consumer<in T> {
    fn accept(self, value: T) { /* ... */ }
}
```

`out T` を指定すると、`Producer<Dog>` を `Producer<Animal>` として扱えます（Dog が Animal のサブタイプである場合）。`in T` はその逆方向です。

variance を正しく指定すると、型安全を保ちながら柔軟な代入が可能になります。迷ったときは指定なし（不変）にしておくのが安全です。

## 型推論との組み合わせ

ジェネリクスの型パラメータは多くの場合、引数や戻り値から推論されます。

```valen
// 型パラメータを明示しなくても推論される
let box = Box(value = 42);           // Box<Int>
let id = identity("hello");          // String
let pair = Pair(first = 1, second = "a"); // Pair<Int, String>

// 推論できない場合は型注釈が必要
let empty: List<Int> = List();       // 引数がないので推論不可
```

関数シグネチャでは型パラメータの宣言が必要ですが、呼び出し側での型引数の指定は推論に任せるのが一般的です。

## 次のステップ

- [クラスとデータクラス](04-classes.md) — class, data class, 継承
