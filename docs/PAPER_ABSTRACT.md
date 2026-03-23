# Context-Enforced Effect Safety for Mixed OS/ML Systems

## Abstract

We present Fajar Lang, a statically-typed systems programming language that provides compiler-enforced safety guarantees for mixed operating system and machine learning codebases. Through a novel combination of context annotations (`@kernel`, `@device`, `@safe`) and an algebraic effect system, Fajar Lang ensures at compile time that: (1) kernel-mode code cannot perform heap allocation or tensor operations, (2) device-mode code cannot access hardware registers or interrupt handlers, and (3) safe-mode code is restricted to pure application logic. These guarantees are enforced without runtime overhead.

We formalize the context-effect interaction as a lattice of permitted effects per context, prove that well-typed programs cannot violate domain boundaries, and demonstrate the system's practicality through a 639-line autonomous drone controller that integrates flight control, ML inference, and mission planning in a single source file.

Our evaluation shows that:
- The effect system adds zero runtime overhead (all checks at compile time)
- Compilation speed is competitive: <10ms for simple programs, <3s for 10K LOC
- The approach catches real classes of embedded AI bugs that existing languages miss
- A self-hosted compiler frontend (1,268 LOC) validates the language's expressiveness

## Keywords

Programming languages, effect systems, embedded systems, machine learning, safety-critical software, context annotations, compile-time verification

## Conference Targets

- PLDI (Programming Language Design and Implementation)
- OOPSLA (Object-Oriented Programming, Systems, Languages & Applications)
- EMSOFT (Embedded Software)
- LCTES (Languages, Compilers, Tools and Theory for Embedded Systems)

## Key Contributions

1. A context annotation system that partitions code into hardware, compute, and application domains with compiler-enforced isolation
2. An algebraic effect system that formalizes the permitted side effects per context annotation
3. A practical demonstration that OS kernels and neural networks can safely coexist in a single compilation unit
4. A self-hosted compiler proving the language is expressive enough to implement itself
