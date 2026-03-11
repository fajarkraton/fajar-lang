# Iterators

Fajar Lang provides a lazy iterator protocol for arrays, strings, ranges, and maps.

## Creating Iterators

Call `.iter()` on a collection:

```fajar
let nums = [1, 2, 3, 4, 5]
let it = nums.iter()
```

Strings, maps, and ranges also produce iterators.

## Combinators

Iterators support lazy transformations:

| Method | Description |
|--------|-------------|
| `.map(f)` | Transform each element |
| `.filter(f)` | Keep elements where `f` returns true |
| `.take(n)` | Take first `n` elements |
| `.enumerate()` | Produce `(index, value)` pairs |

```fajar
let doubled = [1, 2, 3].iter().map(|x| x * 2).collect()
// [2, 4, 6]

let evens = [1, 2, 3, 4].iter().filter(|x| x % 2 == 0).collect()
// [2, 4]

let first2 = [10, 20, 30].iter().take(2).collect()
// [10, 20]
```

## Consuming Methods

| Method | Description |
|--------|-------------|
| `.collect()` | Consume iterator into array |
| `.sum()` | Sum all elements |
| `.count()` | Count elements |
| `.fold(init, f)` | Accumulate with custom function |
| `.next()` | Get next element (`Some(v)` or `None`) |

```fajar
let total = [1, 2, 3, 4, 5].iter().sum()     // 15
let n = [10, 20, 30].iter().count()            // 3
let product = [1, 2, 3, 4].iter().fold(1, |acc, x| acc * x)  // 24
```

## For-In Loops

`for x in collection` works with any iterable:

```fajar
for x in [1, 2, 3] {
    println(x)
}

for i in 0..10 {
    println(i)
}

for ch in "hello".iter() {
    println(ch)
}
```

## Chaining

Combinators can be chained for powerful data pipelines:

```fajar
let result = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    .iter()
    .filter(|x| x % 3 == 0)
    .map(|x| x * x)
    .collect()
// [9, 36, 81]
```
