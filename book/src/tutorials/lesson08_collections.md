# Lesson 8: Collections

## Objectives

By the end of this lesson, you will be able to:

- Create and manipulate arrays with built-in methods
- Use HashMap for key-value storage
- Chain iterator operations (map, filter, collect)
- Choose the right collection for the task

## Arrays

Arrays are ordered, growable collections of values.

```fajar
fn main() {
    let numbers = [1, 2, 3, 4, 5]
    println(len(numbers))     // 5
    println(numbers[0])       // 1
    println(numbers[4])       // 5
}
```

### Mutable Arrays

```fajar
fn main() {
    let mut items = [10, 20, 30]

    // Add elements
    items.push(40)
    println(items)       // [10, 20, 30, 40]

    // Remove last element
    let last = items.pop()
    println(last)        // 40
    println(items)       // [10, 20, 30]

    // Modify by index
    items[1] = 25
    println(items)       // [10, 25, 30]
}
```

### Array Methods

```fajar
fn main() {
    let data = [3, 1, 4, 1, 5, 9, 2, 6]

    println(len(data))           // 8
    println(data.contains(5))    // true
    println(data.contains(7))    // false

    // Slicing
    let slice = data[1..4]
    println(slice)               // [1, 4, 1]

    // Sorting
    let mut sortable = [3, 1, 4, 1, 5]
    sortable.sort()
    println(sortable)            // [1, 1, 3, 4, 5]

    // Reverse
    sortable.reverse()
    println(sortable)            // [5, 4, 3, 1, 1]
}
```

## Iterators

Iterators let you process collections element by element. Chain operations for expressive data pipelines.

### map: Transform Each Element

```fajar
fn main() {
    let numbers = [1, 2, 3, 4, 5]
    let doubled = numbers.iter().map(|x| x * 2).collect()
    println(doubled)   // [2, 4, 6, 8, 10]
}
```

### filter: Keep Matching Elements

```fajar
fn main() {
    let numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let evens = numbers.iter().filter(|x| x % 2 == 0).collect()
    println(evens)   // [2, 4, 6, 8, 10]
}
```

### Chaining Operations

```fajar
fn main() {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]

    // Get squares of even numbers
    let result = data.iter()
        .filter(|x| x % 2 == 0)
        .map(|x| x * x)
        .collect()

    println(result)   // [4, 16, 36, 64, 100]
}
```

### Fold: Reduce to a Single Value

```fajar
fn main() {
    let numbers = [1, 2, 3, 4, 5]
    let sum = numbers.iter().fold(0, |acc, x| acc + x)
    println(sum)   // 15

    let product = numbers.iter().fold(1, |acc, x| acc * x)
    println(product)   // 120
}
```

## HashMap

HashMap stores key-value pairs with fast lookup by key.

```fajar
fn main() {
    let mut scores = HashMap::new()

    // Insert entries
    scores.insert("Alice", 95)
    scores.insert("Bob", 87)
    scores.insert("Carol", 92)

    // Lookup
    println(scores["Alice"])     // 95

    // Check existence
    println(scores.contains_key("Bob"))     // true
    println(scores.contains_key("Dave"))    // false

    // Size
    println(len(scores))        // 3
}
```

### Iterating Over a HashMap

```fajar
fn main() {
    let mut capitals = HashMap::new()
    capitals.insert("Indonesia", "Jakarta")
    capitals.insert("Japan", "Tokyo")
    capitals.insert("France", "Paris")

    for key in capitals.keys() {
        println(f"{key}: {capitals[key]}")
    }
}
```

### Updating Values

```fajar
fn main() {
    let mut word_count = HashMap::new()
    let words = ["apple", "banana", "apple", "cherry", "banana", "apple"]

    for word in words {
        if word_count.contains_key(word) {
            word_count[word] = word_count[word] + 1
        } else {
            word_count.insert(word, 1)
        }
    }

    for key in word_count.keys() {
        println(f"{key}: {word_count[key]}")
    }
}
```

**Expected output (order may vary):**

```
apple: 3
banana: 2
cherry: 1
```

## Practical Example: Student Grades

```fajar
fn main() {
    let grades = [85, 92, 78, 95, 88, 72, 91, 83]

    let passing = grades.iter().filter(|g| g >= 80).collect()
    let average = grades.iter().fold(0, |acc, g| acc + g) / len(grades)

    println(f"All grades: {grades}")
    println(f"Passing: {passing}")
    println(f"Average: {average}")
    println(f"Highest: {grades.iter().fold(0, |best, g| if g > best { g } else { best })}")
}
```

## Exercises

### Exercise 8.1: Array Statistics (*)

Given an array `[4, 8, 15, 16, 23, 42]`, compute and print the sum, minimum, and maximum using iterator methods.

**Expected output:**

```
Sum: 108
Min: 4
Max: 42
```

### Exercise 8.2: Word Frequency (**)

Write a program that counts the frequency of each word in the sentence "the cat sat on the mat the cat" using a HashMap. Print each word and its count.

**Expected output (order may vary):**

```
the: 3
cat: 2
sat: 1
on: 1
mat: 1
```

### Exercise 8.3: Pipeline Processing (***)

Given numbers 1 through 20, use iterator chaining to: (1) filter to keep only multiples of 3, (2) square each one, (3) collect into an array. Print the result.

**Expected output:**

```
[9, 36, 81, 144, 225, 324]
```
