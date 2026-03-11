//! Example gallery — curated playground examples with difficulty tags.

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════
// Example Data Model
// ═══════════════════════════════════════════════════════════════════════

/// Difficulty level for a playground example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Difficulty {
    /// Beginner (green).
    Beginner,
    /// Intermediate (yellow).
    Intermediate,
    /// Advanced (red).
    Advanced,
}

impl fmt::Display for Difficulty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Beginner => write!(f, "Beginner"),
            Self::Intermediate => write!(f, "Intermediate"),
            Self::Advanced => write!(f, "Advanced"),
        }
    }
}

impl Difficulty {
    /// Returns the CSS color for this difficulty.
    pub fn color(&self) -> &'static str {
        match self {
            Self::Beginner => "#3fb950",
            Self::Intermediate => "#d29922",
            Self::Advanced => "#f85149",
        }
    }
}

/// A playground example.
#[derive(Debug, Clone)]
pub struct Example {
    /// Example title.
    pub title: String,
    /// Short description.
    pub description: String,
    /// URL slug.
    pub slug: String,
    /// Difficulty level.
    pub difficulty: Difficulty,
    /// Fajar Lang source code.
    pub code: String,
    /// Category tag.
    pub category: String,
}

// ═══════════════════════════════════════════════════════════════════════
// Built-in Examples (S32.3 - S32.9)
// ═══════════════════════════════════════════════════════════════════════

/// Returns all built-in playground examples.
pub fn builtin_examples() -> Vec<Example> {
    vec![
        // S32.3: Hello World
        Example {
            title: "Hello World".to_string(),
            description: "Basic hello world with variables and function calls".to_string(),
            slug: "hello-world".to_string(),
            difficulty: Difficulty::Beginner,
            category: "basics".to_string(),
            code: r#"fn main() {
    let name: str = "Fajar Lang"
    let version = 1.1
    println(f"Hello, {name} v{version}!")

    let x: i32 = 42
    let y = x * 2
    println(f"The answer is {y}")
}"#
            .to_string(),
        },
        // S32.4: Pattern Matching
        Example {
            title: "Pattern Matching".to_string(),
            description: "Enum definition and exhaustive match expressions".to_string(),
            slug: "pattern-matching".to_string(),
            difficulty: Difficulty::Beginner,
            category: "basics".to_string(),
            code: r#"enum Color {
    Red,
    Green,
    Blue,
    Custom(i32, i32, i32),
}

fn describe(c: Color) -> str {
    match c {
        Color::Red => "warm red",
        Color::Green => "lush green",
        Color::Blue => "deep blue",
        Color::Custom(r, g, b) => f"rgb({r},{g},{b})",
    }
}

fn main() {
    println(describe(Color::Red))
    println(describe(Color::Custom(255, 128, 0)))
}"#
            .to_string(),
        },
        // S32.5: Struct & Methods
        Example {
            title: "Structs & Methods".to_string(),
            description: "Point struct with impl block and method calls".to_string(),
            slug: "structs-methods".to_string(),
            difficulty: Difficulty::Beginner,
            category: "basics".to_string(),
            code: r#"struct Point {
    x: f64,
    y: f64,
}

impl Point {
    fn new(x: f64, y: f64) -> Point {
        Point { x: x, y: y }
    }

    fn distance(self, other: Point) -> f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        sqrt(dx * dx + dy * dy)
    }

    fn translate(self, dx: f64, dy: f64) -> Point {
        Point { x: self.x + dx, y: self.y + dy }
    }
}

fn main() {
    let a = Point::new(0.0, 0.0)
    let b = Point::new(3.0, 4.0)
    println(f"Distance: {a.distance(b)}")
}"#
            .to_string(),
        },
        // S32.6: Error Handling
        Example {
            title: "Error Handling".to_string(),
            description: "Result type, ? operator, and error propagation".to_string(),
            slug: "error-handling".to_string(),
            difficulty: Difficulty::Intermediate,
            category: "safety".to_string(),
            code: r#"fn parse_age(input: str) -> Result<i32, str> {
    let n = parse_int(input)
    match n {
        Ok(age) => {
            if age < 0 { Err("age cannot be negative") }
            else if age > 150 { Err("unrealistic age") }
            else { Ok(age) }
        },
        Err(_) => Err("not a valid number"),
    }
}

fn main() {
    let inputs = ["25", "-3", "abc", "200"]
    for input in inputs {
        match parse_age(input) {
            Ok(age) => println(f"{input} -> valid age: {age}"),
            Err(msg) => println(f"{input} -> error: {msg}"),
        }
    }
}"#
            .to_string(),
        },
        // S32.7: Tensor Operations
        Example {
            title: "Tensor Operations".to_string(),
            description: "Create tensors, matmul, activation, shape manipulation".to_string(),
            slug: "tensor-ops".to_string(),
            difficulty: Difficulty::Intermediate,
            category: "ml".to_string(),
            code: r#"@device
fn tensor_demo() {
    let a = zeros(3, 3)
    let b = ones(3, 3)
    let c = randn(3, 3)

    let sum = a + b
    let product = matmul(b, c)

    let activated = relu(product)
    let normalized = softmax(activated)

    println(f"Shape: {normalized.shape()}")
    println(f"Sum: {normalized.sum()}")
}"#
            .to_string(),
        },
        // S32.8: ML Training
        Example {
            title: "ML Training".to_string(),
            description: "Simple linear regression with autograd and optimizer".to_string(),
            slug: "ml-training".to_string(),
            difficulty: Difficulty::Advanced,
            category: "ml".to_string(),
            code: r#"@device
fn train_linear() {
    // y = 2x + 1 (target)
    let x = from_data([[1.0], [2.0], [3.0], [4.0]])
    let y = from_data([[3.0], [5.0], [7.0], [9.0]])

    let w = randn(1, 1)
    set_requires_grad(w, true)
    let b = zeros(1, 1)
    set_requires_grad(b, true)

    let lr = 0.01
    for epoch in 0..100 {
        let pred = matmul(x, w) + b
        let loss = mse_loss(pred, y)

        backward(loss)

        // Gradient descent
        let w_grad = grad(w)
        let b_grad = grad(b)
    }

    println("Training complete")
}"#
            .to_string(),
        },
        // S32.9: Pipeline Operator
        Example {
            title: "Pipeline Operator".to_string(),
            description: "Chain transformations with |> for functional style".to_string(),
            slug: "pipeline".to_string(),
            difficulty: Difficulty::Beginner,
            category: "basics".to_string(),
            code: r#"fn double(x: i32) -> i32 { x * 2 }
fn add_one(x: i32) -> i32 { x + 1 }
fn square(x: i32) -> i32 { x * x }
fn to_string(x: i32) -> str { f"result = {x}" }

fn main() {
    // Without pipeline
    let result1 = to_string(square(add_one(double(5))))

    // With pipeline — reads left-to-right
    let result2 = 5 |> double |> add_one |> square |> to_string

    println(result1)
    println(result2)
}"#
            .to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Guided Tutorial (S32.10)
// ═══════════════════════════════════════════════════════════════════════

/// A step in the guided calculator tutorial.
#[derive(Debug, Clone)]
pub struct TutorialStep {
    /// Step number.
    pub step: u32,
    /// Step title.
    pub title: String,
    /// Description of what this step teaches.
    pub description: String,
    /// Code for this step.
    pub code: String,
}

/// Returns the guided calculator tutorial (5 incremental steps).
pub fn calculator_tutorial() -> Vec<TutorialStep> {
    vec![
        TutorialStep {
            step: 1,
            title: "Basic Addition".to_string(),
            description: "Start with a simple add function.".to_string(),
            code: "fn add(a: f64, b: f64) -> f64 { a + b }\n\nfn main() {\n    println(f\"2 + 3 = {add(2.0, 3.0)}\")\n}".to_string(),
        },
        TutorialStep {
            step: 2,
            title: "Four Operations".to_string(),
            description: "Add subtract, multiply, and divide.".to_string(),
            code: "fn add(a: f64, b: f64) -> f64 { a + b }\nfn sub(a: f64, b: f64) -> f64 { a - b }\nfn mul(a: f64, b: f64) -> f64 { a * b }\nfn div(a: f64, b: f64) -> f64 { a / b }\n\nfn main() {\n    println(f\"10 / 3 = {div(10.0, 3.0)}\")\n}".to_string(),
        },
        TutorialStep {
            step: 3,
            title: "Pattern Matching".to_string(),
            description: "Use match to dispatch operations.".to_string(),
            code: "fn calc(op: str, a: f64, b: f64) -> f64 {\n    match op {\n        \"+\" => a + b,\n        \"-\" => a - b,\n        \"*\" => a * b,\n        \"/\" => a / b,\n        _ => 0.0,\n    }\n}\n\nfn main() {\n    println(f\"5 * 3 = {calc(\"*\", 5.0, 3.0)}\")\n}".to_string(),
        },
        TutorialStep {
            step: 4,
            title: "Error Handling".to_string(),
            description: "Handle division by zero with Result.".to_string(),
            code: "fn safe_calc(op: str, a: f64, b: f64) -> Result<f64, str> {\n    match op {\n        \"+\" => Ok(a + b),\n        \"-\" => Ok(a - b),\n        \"*\" => Ok(a * b),\n        \"/\" => {\n            if b == 0.0 { Err(\"division by zero\") }\n            else { Ok(a / b) }\n        },\n        _ => Err(f\"unknown op: {op}\"),\n    }\n}\n\nfn main() {\n    match safe_calc(\"/\", 10.0, 0.0) {\n        Ok(r) => println(f\"Result: {r}\"),\n        Err(e) => println(f\"Error: {e}\"),\n    }\n}".to_string(),
        },
        TutorialStep {
            step: 5,
            title: "Pipeline Composition".to_string(),
            description: "Chain calculations with the pipeline operator.".to_string(),
            code: "fn add_ten(x: f64) -> f64 { x + 10.0 }\nfn double(x: f64) -> f64 { x * 2.0 }\nfn negate(x: f64) -> f64 { 0.0 - x }\n\nfn main() {\n    let result = 5.0 |> add_ten |> double |> negate\n    println(f\"5 |> +10 |> *2 |> negate = {result}\")\n}".to_string(),
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════════
// Gallery Page Generation (S32.1, S32.2)
// ═══════════════════════════════════════════════════════════════════════

/// Generates the example gallery as an HTML page.
pub fn gallery_html(examples: &[Example]) -> String {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>Examples — Fajar Lang Playground</title>
<style>
.gallery { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 16px; padding: 24px; }
.card { border: 1px solid #30363d; border-radius: 12px; padding: 16px; background: #161b22; }
.card h3 { margin: 0 0 8px 0; }
.card p { color: #8b949e; font-size: 0.9rem; }
.badge { display: inline-block; padding: 2px 8px; border-radius: 12px; font-size: 0.75rem; font-weight: 600; color: #fff; }
</style>
</head>
<body>
<h1>Example Gallery</h1>
<div class="gallery">
"#,
    );

    for ex in examples {
        html.push_str(&format!(
            r#"<div class="card">
<span class="badge" style="background:{}">{}</span>
<h3>{}</h3>
<p>{}</p>
<a href="/playground?example={}">Open in Playground</a>
</div>
"#,
            ex.difficulty.color(),
            ex.difficulty,
            ex.title,
            ex.description,
            ex.slug,
        ));
    }

    html.push_str("</div>\n</body>\n</html>");
    html
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // S32.1: Gallery page
    #[test]
    fn s32_1_gallery_html_generation() {
        let examples = builtin_examples();
        let html = gallery_html(&examples);
        assert!(html.contains("Example Gallery"));
        assert!(html.contains("Hello World"));
        assert!(html.contains("gallery"));
    }

    // S32.2: Difficulty levels
    #[test]
    fn s32_2_difficulty_levels() {
        assert_eq!(format!("{}", Difficulty::Beginner), "Beginner");
        assert_eq!(Difficulty::Beginner.color(), "#3fb950");
        assert_eq!(Difficulty::Intermediate.color(), "#d29922");
        assert_eq!(Difficulty::Advanced.color(), "#f85149");
    }

    // S32.3: Hello World example
    #[test]
    fn s32_3_hello_world_example() {
        let examples = builtin_examples();
        let hello = examples.iter().find(|e| e.slug == "hello-world").unwrap();
        assert_eq!(hello.difficulty, Difficulty::Beginner);
        assert!(hello.code.contains("println"));
        assert!(hello.code.contains("fn main"));
    }

    // S32.4: Pattern matching example
    #[test]
    fn s32_4_pattern_matching_example() {
        let examples = builtin_examples();
        let pm = examples
            .iter()
            .find(|e| e.slug == "pattern-matching")
            .unwrap();
        assert!(pm.code.contains("enum"));
        assert!(pm.code.contains("match"));
    }

    // S32.5: Struct & methods example
    #[test]
    fn s32_5_structs_methods_example() {
        let examples = builtin_examples();
        let sm = examples
            .iter()
            .find(|e| e.slug == "structs-methods")
            .unwrap();
        assert!(sm.code.contains("struct Point"));
        assert!(sm.code.contains("impl Point"));
    }

    // S32.6: Error handling example
    #[test]
    fn s32_6_error_handling_example() {
        let examples = builtin_examples();
        let eh = examples
            .iter()
            .find(|e| e.slug == "error-handling")
            .unwrap();
        assert_eq!(eh.difficulty, Difficulty::Intermediate);
        assert!(eh.code.contains("Result"));
    }

    // S32.7: Tensor operations example
    #[test]
    fn s32_7_tensor_ops_example() {
        let examples = builtin_examples();
        let to = examples.iter().find(|e| e.slug == "tensor-ops").unwrap();
        assert_eq!(to.category, "ml");
        assert!(to.code.contains("@device"));
        assert!(to.code.contains("matmul"));
    }

    // S32.8: ML training example
    #[test]
    fn s32_8_ml_training_example() {
        let examples = builtin_examples();
        let ml = examples.iter().find(|e| e.slug == "ml-training").unwrap();
        assert_eq!(ml.difficulty, Difficulty::Advanced);
        assert!(ml.code.contains("backward"));
    }

    // S32.9: Pipeline operator example
    #[test]
    fn s32_9_pipeline_example() {
        let examples = builtin_examples();
        let pp = examples.iter().find(|e| e.slug == "pipeline").unwrap();
        assert!(pp.code.contains("|>"));
    }

    // S32.10: Guided tutorial
    #[test]
    fn s32_10_calculator_tutorial() {
        let steps = calculator_tutorial();
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].step, 1);
        assert_eq!(steps[4].step, 5);
        // Each step should have non-empty code
        for step in &steps {
            assert!(!step.code.is_empty());
        }
    }

    // All examples should have unique slugs
    #[test]
    fn s32_10_unique_slugs() {
        let examples = builtin_examples();
        let mut slugs: Vec<&str> = examples.iter().map(|e| e.slug.as_str()).collect();
        let orig_len = slugs.len();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), orig_len);
    }

    // All examples should have valid categories
    #[test]
    fn s32_10_valid_categories() {
        let valid = ["basics", "safety", "ml", "os", "advanced"];
        let examples = builtin_examples();
        for ex in &examples {
            assert!(
                valid.contains(&ex.category.as_str()),
                "invalid category: {}",
                ex.category
            );
        }
    }
}
