# GUI Calculator App

Build an interactive calculator with Fajar Lang's GUI toolkit using `winit` windowing.

## What You'll Build

A graphical calculator with:
- Window with title and custom size
- Number buttons with click callbacks
- Display label for results
- Color rectangles for visual layout

## Prerequisites

Build with GUI support:
```bash
cargo build --release --features gui
```

## Step 1: Define Callback Functions

Each button has a callback function that runs when clicked:

```fajar
fn on_btn_1() { println("[calc] pressed: 1") }
fn on_btn_2() { println("[calc] pressed: 2") }
fn on_btn_3() { println("[calc] pressed: 3") }
fn on_btn_add() { println("[calc] pressed: +") }
fn on_btn_eq() { println("[calc] pressed: =") }
fn on_btn_clear() { println("[calc] pressed: C") }
```

## Step 2: Create the Window

```fajar
fn main() {
    // Configure window
    gui_window("Fajar Calculator", 320, 480)

    // Background panel
    gui_rect(0, 0, 320, 80, 0x1A1A2E)   // display area
    gui_rect(0, 80, 320, 400, 0x16213E)  // button area
```

## Step 3: Add Display and Buttons

```fajar
    // Display
    gui_label("0", 20, 30)

    // Number buttons (text, x, y, w, h, callback)
    gui_button("1", 10, 100, 70, 50, "on_btn_1")
    gui_button("2", 85, 100, 70, 50, "on_btn_2")
    gui_button("3", 160, 100, 70, 50, "on_btn_3")
    gui_button("+", 235, 100, 70, 50, "on_btn_add")

    // More rows...
    gui_button("C", 10, 160, 70, 50, "on_btn_clear")
    gui_button("=", 235, 160, 70, 50, "on_btn_eq")

    println("[calc] 8 widgets created")
}
```

## Step 4: Run It

```bash
# Launch the GUI
cargo run --features gui -- gui examples/tutorial_calculator.fj
```

A real OS window opens. Buttons show text (bitmap font), change color on hover (blue → lighter blue), and print to console when clicked.

## Step 5: Auto-Layout (Optional)

Instead of manual x/y positioning, use flex layout:

```fajar
    gui_layout("row", 10, 20)  // horizontal layout, 10px gap, 20px padding
```

This automatically positions all widgets in a row using the FlexLayout engine.

## How GUI Works

1. **`gui_window(title, w, h)`** — configures the window (title, dimensions)
2. **`gui_button(text, x, y, w, h, callback)`** — creates a button with click callback
3. **`gui_label(text, x, y)`** — creates a text label (bitmap 5x7 font)
4. **`gui_rect(x, y, w, h, color)`** — draws a colored rectangle
5. **`fj gui file.fj`** — runs the program, then opens the window with all widgets
6. **Callbacks** — on button click, the interpreter invokes the named function

### Rendering Pipeline

```
.fj program → gui_* builtins → GuiState { widgets }
                                     ↓
cmd_gui() → run_windowed_interactive()
                ↓
Canvas::draw_text() → bitmap font → softbuffer → OS window
                ↓
Mouse events → button hover/click → invoke callback function
```

## Key Builtins

| Builtin | Signature |
|---------|-----------|
| `gui_window` | `(title: str, width: i64, height: i64)` |
| `gui_label` | `(text: str, x: i64, y: i64)` |
| `gui_button` | `(text: str, x: i64, y: i64, w: i64, h: i64, on_click: str)` |
| `gui_rect` | `(x: i64, y: i64, w: i64, h: i64, color: i64)` |
| `gui_layout` | `(mode: str, gap: i64, padding: i64)` |

## Full Source

See [`examples/gui_hello.fj`](https://github.com/fajarkraton/fajar-lang/blob/main/examples/gui_hello.fj) for a complete GUI demo with multiple widgets.
