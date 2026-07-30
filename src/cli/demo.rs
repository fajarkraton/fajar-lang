//! Demo and playground commands.
//!
//! Extracted from `main.rs` (REFACTOR_2026_07 Phase 2); pure code motion.

use crate::EXIT_USAGE;
use std::process::ExitCode;

/// All built-in demos: (name, aliases, description, source).
pub(crate) fn demo_registry() -> Vec<(
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static str,
)> {
    vec![
        (
            "drone",
            &[],
            "PID controller + thrust simulation",
            include_str!("../../examples/drone_demo.fj"),
        ),
        (
            "os",
            &[],
            "Kernel boot, process table, scheduler",
            include_str!("../../examples/mini_os_demo.fj"),
        ),
        (
            "network",
            &["net"],
            "HTTP echo server, DNS, TCP",
            include_str!("../../examples/http_echo_server.fj"),
        ),
        (
            "ffi",
            &[],
            "FFI to libc (getpid, abs)",
            include_str!("../../examples/ffi_libc.fj"),
        ),
        (
            "database",
            &["db"],
            "Connection pool, query builder, schema",
            include_str!("../../examples/database_demo.fj"),
        ),
        (
            "web",
            &["fullstack"],
            "HTTP routing, templates, REST API",
            include_str!("../../examples/web_demo.fj"),
        ),
        (
            "embedded-ml",
            &["ml", "embedded"],
            "Sensor → tensor → inference pipeline",
            include_str!("../../examples/embedded_ml_demo.fj"),
        ),
        (
            "cli",
            &[],
            "Arg parser, table formatter, progress bar",
            include_str!("../../examples/cli_tool_demo.fj"),
        ),
        (
            "macros",
            &["macro"],
            "User macro_rules! with metavariables",
            include_str!("../../examples/macros.fj"),
        ),
        (
            "pattern-match",
            &["match", "patterns"],
            "Ok/Err/Some/None destructuring",
            include_str!("../../examples/pattern_match.fj"),
        ),
        (
            "async",
            &[],
            "async_sleep, async_spawn, async_join",
            include_str!("../../examples/async_demo.fj"),
        ),
        (
            "test-framework",
            &["test"],
            "@test, @should_panic, @ignore annotations",
            include_str!("../../examples/test_framework.fj"),
        ),
        (
            "iterator",
            &["iter"],
            "Iterators, map, filter, fold",
            include_str!("../../examples/iterator_demo.fj"),
        ),
    ]
}

pub(crate) fn cmd_demo_list() -> ExitCode {
    println!("Available demos:\n");
    for (name, aliases, desc, _) in demo_registry() {
        if aliases.is_empty() {
            println!("  {name:<16} {desc}");
        } else {
            let alias_str = aliases.join(", ");
            println!("  {name:<16} {desc}  (aliases: {alias_str})");
        }
    }
    println!("\nUsage: fj demo <name>");
    ExitCode::SUCCESS
}

/// V18 3.8: Run a built-in demo by name.
pub(crate) fn cmd_demo(name: &str) -> ExitCode {
    for (demo_name, aliases, _, source) in demo_registry() {
        if name == demo_name || aliases.contains(&name) {
            let mut interp = fajar_lang::interpreter::Interpreter::new();
            return match interp.eval_source(source) {
                Ok(_) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            };
        }
    }
    eprintln!("Unknown demo: '{name}'");
    eprintln!("Run 'fj demo --list' to see available demos.");
    ExitCode::from(EXIT_USAGE)
}

pub(crate) fn cmd_playground(output_dir: &str) -> ExitCode {
    let dir = std::path::Path::new(output_dir);
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("error: cannot create directory '{output_dir}': {e}");
        return ExitCode::from(EXIT_USAGE);
    }

    // Generate playground HTML
    let html = generate_playground_html();
    let html_path = dir.join("index.html");
    if let Err(e) = std::fs::write(&html_path, &html) {
        eprintln!("error: cannot write {}: {e}", html_path.display());
        return ExitCode::from(EXIT_USAGE);
    }

    // Generate examples JSON
    let examples = generate_examples_json();
    let examples_path = dir.join("examples.json");
    if let Err(e) = std::fs::write(&examples_path, &examples) {
        eprintln!("error: cannot write {}: {e}", examples_path.display());
        return ExitCode::from(EXIT_USAGE);
    }

    println!("Playground generated in: {output_dir}/");
    println!("  index.html    — main playground page");
    println!("  examples.json — pre-loaded examples");
    println!(
        "\nOpen {}/index.html in a browser to use the playground.",
        output_dir
    );

    ExitCode::SUCCESS
}

/// Generates the main playground HTML page.
pub(crate) fn generate_playground_html() -> String {
    let examples = get_playground_examples();
    let example_options: String = examples
        .iter()
        .map(|(name, _, _)| format!("            <option value=\"{name}\">{name}</option>"))
        .collect::<Vec<_>>()
        .join("\n");
    let first_code = examples
        .first()
        .map_or("// Welcome to Fajar Lang!", |e| e.1);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Fajar Lang Playground</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: #1e1e2e; color: #cdd6f4; }}
  .header {{ background: #181825; padding: 12px 24px; display: flex; align-items: center; gap: 16px; border-bottom: 1px solid #313244; }}
  .header h1 {{ font-size: 18px; color: #89b4fa; }}
  .header select, .header button {{ background: #313244; color: #cdd6f4; border: 1px solid #45475a; padding: 6px 12px; border-radius: 4px; cursor: pointer; }}
  .header button:hover {{ background: #45475a; }}
  .header .run-btn {{ background: #a6e3a1; color: #1e1e2e; font-weight: bold; }}
  .header .share-btn {{ background: #89b4fa; color: #1e1e2e; }}
  .container {{ display: flex; height: calc(100vh - 48px); }}
  .editor {{ flex: 1; padding: 16px; }}
  .output {{ flex: 1; padding: 16px; background: #11111b; border-left: 1px solid #313244; }}
  textarea {{ width: 100%; height: 100%; background: #1e1e2e; color: #cdd6f4; border: none; font-family: 'JetBrains Mono', monospace; font-size: 14px; resize: none; outline: none; padding: 8px; tab-size: 4; }}
  .output pre {{ font-family: 'JetBrains Mono', monospace; font-size: 13px; white-space: pre-wrap; color: #a6e3a1; }}
  .output .error {{ color: #f38ba8; }}
  .output h3 {{ color: #89b4fa; margin-bottom: 8px; font-size: 14px; }}
</style>
</head>
<body>
<div class="header">
    <h1>Fajar Lang Playground</h1>
    <select id="examples" onchange="loadExample()">
        <option value="">-- Select Example --</option>
{example_options}
    </select>
    <button class="run-btn" onclick="runCode()">Run</button>
    <button class="share-btn" onclick="shareCode()">Share</button>
</div>
<div class="container">
    <div class="editor">
        <textarea id="code" spellcheck="false">{first_code}</textarea>
    </div>
    <div class="output">
        <h3>Output</h3>
        <pre id="output">Click "Run" to execute your code.</pre>
    </div>
</div>
<script>
const EXAMPLES = {{}};
{examples_js}
function loadExample() {{
    const sel = document.getElementById('examples').value;
    if (EXAMPLES[sel]) document.getElementById('code').value = EXAMPLES[sel];
}}
function runCode() {{
    document.getElementById('output').textContent = 'Compiling...\\n(Note: server-side execution required for full compilation)';
}}
function shareCode() {{
    const code = document.getElementById('code').value;
    const encoded = encodeURIComponent(code);
    const url = location.href.split('#')[0] + '#code=' + encoded;
    navigator.clipboard.writeText(url).then(() => alert('Link copied!'));
}}
// Load shared code from URL
if (location.hash.startsWith('#code=')) {{
    document.getElementById('code').value = decodeURIComponent(location.hash.slice(6));
}}
</script>
</body>
</html>"#,
        examples_js = examples
            .iter()
            .map(|(name, code, _)| {
                format!(
                    "EXAMPLES[\"{name}\"] = `{}`;",
                    code.replace('`', "\\`").replace("${", "\\${")
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Returns playground examples: (name, code, description).
pub(crate) fn get_playground_examples() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "Hello World",
            "fn main() {\n    println(\"Hello, Fajar Lang!\")\n}",
            "Basic hello world program",
        ),
        (
            "Fibonacci",
            "fn fib(n: i64) -> i64 {\n    if n <= 1 { n } else { fib(n-1) + fib(n-2) }\n}\nfn main() {\n    println(fib(20))\n}",
            "Recursive Fibonacci",
        ),
        (
            "Effect System",
            "effect Logger {\n    fn log(msg: str) -> void\n}\n\nfn greet() with IO {\n    println(\"Hello with effects!\")\n}\n\nfn main() { greet() }",
            "Formal effect system",
        ),
        (
            "Comptime",
            "comptime fn factorial(n: i64) -> i64 {\n    if n <= 1 { 1 } else { n * factorial(n - 1) }\n}\n\nfn main() {\n    let result = comptime { factorial(10) }\n    println(result)\n}",
            "Compile-time evaluation",
        ),
        (
            "Pattern Matching",
            "fn describe(n: i64) -> str {\n    match n {\n        0 => \"zero\",\n        1 => \"one\",\n        _ => \"other\"\n    }\n}\nfn main() {\n    println(describe(0))\n    println(describe(42))\n}",
            "Pattern matching",
        ),
        (
            "Macros",
            "fn main() {\n    let arr = vec![1, 2, 3, 4, 5]\n    let msg = concat!(\"length: \", len(arr))\n    println(msg)\n}",
            "Built-in macros",
        ),
        (
            "Structs",
            "struct Point {\n    x: f64,\n    y: f64\n}\n\nfn distance(p: Point) -> f64 {\n    sqrt(p.x * p.x + p.y * p.y)\n}\n\nfn main() {\n    let p = Point { x: 3.0, y: 4.0 }\n    println(distance(p))\n}",
            "Struct types",
        ),
        (
            "Context Safety",
            "@kernel fn read_hw() -> i64 {\n    // Only hardware ops allowed here\n    0\n}\n\n@device fn inference() -> i64 {\n    // Only tensor ops allowed here\n    42\n}\n\n@safe fn bridge() -> i64 {\n    // Safest: no hardware, no tensor\n    0\n}\n\nfn main() {\n    println(\"Context annotations enforce safety!\")\n}",
            "Compiler-enforced context isolation",
        ),
    ]
}

/// Generates examples JSON for external tools.
///
/// Merges inline examples with the playground gallery from
/// [`fajar_lang::playground::examples`].
pub(crate) fn generate_examples_json() -> String {
    let examples = get_playground_examples();
    // Also include the rich playground gallery examples from the playground module.
    let gallery = fajar_lang::playground::examples::builtin_examples();
    let mut json = String::from("[\n");
    for (i, (name, code, desc)) in examples.iter().enumerate() {
        if i > 0 {
            json.push_str(",\n");
        }
        json.push_str(&format!(
            "  {{\"name\": \"{name}\", \"description\": \"{desc}\", \"code\": {}}}",
            serde_json::json!(code),
        ));
    }
    // Append gallery examples (richer metadata: difficulty, category).
    for ex in &gallery {
        json.push_str(",\n");
        json.push_str(&format!(
            "  {{\"name\": {:?}, \"description\": {:?}, \"code\": {}, \"difficulty\": {:?}, \"category\": {:?}}}",
            ex.title,
            ex.description,
            serde_json::json!(&ex.code),
            ex.difficulty.to_string(),
            ex.category,
        ));
    }
    json.push_str("\n]\n");
    json
}
