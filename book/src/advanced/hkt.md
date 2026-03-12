# Higher-Kinded Types

Higher-kinded types (HKT) allow you to abstract over type constructors — types that take other types as parameters. This enables powerful abstractions like Functor, Monad, and Applicative.

## Kind System

Every type has a *kind*:
- `i64`, `str`, `bool` have kind `*` (concrete types)
- `Option`, `Array`, `Result` have kind `* -> *` (take one type, produce a type)
- `Result` has kind `* -> * -> *` (takes two types)

```fajar
// Kind::Star       — concrete type
// Kind::Arrow(a,b) — type constructor

type Option: * -> *       // Option takes a type, returns a type
type Result: * -> * -> *  // Result takes two types
type i64: *               // i64 is already concrete
```

## Functor

A Functor lets you map over a container:

```fajar
trait Functor<F: * -> *> {
    fn fmap<A, B>(fa: F<A>, f: fn(A) -> B) -> F<B>
}

impl Functor<Option> {
    fn fmap<A, B>(fa: Option<A>, f: fn(A) -> B) -> Option<B> {
        match fa {
            Some(a) => Some(f(a)),
            None => None,
        }
    }
}
```

## Monad

```fajar
trait Monad<M: * -> *>: Functor<M> {
    fn pure<A>(a: A) -> M<A>
    fn bind<A, B>(ma: M<A>, f: fn(A) -> M<B>) -> M<B>
}

impl Monad<Option> {
    fn pure<A>(a: A) -> Option<A> { Some(a) }
    fn bind<A, B>(ma: Option<A>, f: fn(A) -> Option<B>) -> Option<B> {
        match ma {
            Some(a) => f(a),
            None => None,
        }
    }
}
```

## Monad Transformers

Stack monadic effects with `MonadTransformer`:

```fajar
// OptionT<IO> combines optional values with I/O
type OptionT<M: * -> *, A> = M<Option<A>>

let result: OptionT<IO, i64> = lift(read_file("config.txt"))
    |> bind(fn(content) { parse_int(content) })
```

## Type Lambdas

Create anonymous type-level functions:

```fajar
type Pair<A> = (A, A)          // * -> *
type Triple<A> = (A, A, A)     // * -> *

// Apply a type constructor to a concrete type
type IntPair = Pair<i64>       // (i64, i64)
```

## Type Families

Compute types from types:

```fajar
type family Element<C> {
    Element<Array<T>> = T,
    Element<Option<T>> = T,
    Element<str> = char,
}

fn first<C>(container: C) -> Element<C> { ... }
```
