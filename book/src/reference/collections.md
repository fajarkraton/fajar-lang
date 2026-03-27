# Collections

Fajar Lang provides two primary collection types: `Array` and `HashMap`.

## Array

Arrays are ordered, growable sequences of values.

```fajar
let nums = [1, 2, 3, 4, 5]
let names: [str] = ["Alice", "Bob"]
let mut items = [10, 20, 30]
```

### Array Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `push` | `push(value: T) -> void` | Append element to end |
| `pop` | `pop() -> T` | Remove and return last element |
| `len` | `len() -> i64` | Number of elements |
| `contains` | `contains(value: T) -> bool` | Check if element exists |
| `remove` | `remove(index: i64) -> T` | Remove element at index |
| `insert` | `insert(index: i64, value: T) -> void` | Insert element at index |
| `sort` | `sort() -> void` | Sort in ascending order (mutates) |
| `reverse` | `reverse() -> void` | Reverse order (mutates) |
| `map` | `map(fn: T -> U) -> [U]` | Transform each element |
| `filter` | `filter(fn: T -> bool) -> [T]` | Keep matching elements |
| `collect` | `collect() -> [T]` | Materialize iterator into array |
| `iter` | `iter() -> Iterator<T>` | Create lazy iterator |
| `join` | `join(sep: str) -> str` | Join elements with separator |
| `first` | `first() -> Option<T>` | First element or None |
| `last` | `last() -> Option<T>` | Last element or None |
| `slice` | `slice(start: i64, end: i64) -> [T]` | Sub-array (start inclusive, end exclusive) |

### Array Examples

```fajar
let mut arr = [3, 1, 4, 1, 5]

// push / pop
arr.push(9)           // [3, 1, 4, 1, 5, 9]
let last = arr.pop()  // 9, arr is [3, 1, 4, 1, 5]

// len / contains
println(arr.len())          // 5
println(arr.contains(4))    // true

// insert / remove
arr.insert(0, 0)     // [0, 3, 1, 4, 1, 5]
arr.remove(2)         // removes 1 -> [0, 3, 4, 1, 5]

// sort / reverse
arr.sort()            // [0, 1, 3, 4, 5]
arr.reverse()         // [5, 4, 3, 1, 0]

// map / filter
let doubled = [1, 2, 3].map(|x| x * 2)       // [2, 4, 6]
let evens = [1, 2, 3, 4].filter(|x| x % 2 == 0)  // [2, 4]

// join
let csv = ["a", "b", "c"].join(", ")  // "a, b, c"

// first / last
let f = [10, 20, 30].first()  // Some(10)
let l = [10, 20, 30].last()   // Some(30)

// slice
let sub = [1, 2, 3, 4, 5].slice(1, 4)  // [2, 3, 4]
```

### Iterator Protocol

Arrays support lazy iteration with chaining:

```fajar
let result = [1, 2, 3, 4, 5]
    .iter()
    .filter(|x| x > 2)
    .map(|x| x * 10)
    .collect()
// result: [30, 40, 50]
```

## HashMap

Hash maps store key-value pairs with O(1) average lookup.

```fajar
let mut scores = {}
scores.insert("Alice", 95)
scores.insert("Bob", 87)
```

### HashMap Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `insert` | `insert(key: K, value: V) -> void` | Add or update entry |
| `get` | `get(key: K) -> Option<V>` | Retrieve value by key |
| `contains_key` | `contains_key(key: K) -> bool` | Check if key exists |
| `remove` | `remove(key: K) -> Option<V>` | Remove entry by key |
| `keys` | `keys() -> [K]` | All keys as array |
| `values` | `values() -> [V]` | All values as array |
| `len` | `len() -> i64` | Number of entries |
| `is_empty` | `is_empty() -> bool` | True if no entries |

### HashMap Examples

```fajar
let mut config = {}
config.insert("host", "localhost")
config.insert("port", "8080")

// get
match config.get("host") {
    Some(h) => println(f"Host: {h}"),
    None => println("No host configured"),
}

// contains_key
if config.contains_key("port") {
    println("Port is configured")
}

// remove
config.remove("port")

// keys / values
let all_keys = config.keys()      // ["host"]
let all_vals = config.values()    // ["localhost"]

// len / is_empty
println(config.len())        // 1
println(config.is_empty())   // false
```

### Iterating Over Maps

```fajar
let scores = {}
scores.insert("Alice", 95)
scores.insert("Bob", 87)

for entry in scores {
    println(f"{entry.0}: {entry.1}")
}
```
