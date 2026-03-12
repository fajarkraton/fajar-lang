# WebAssembly Backend

Fajar Lang can compile to WebAssembly for browser-based and edge deployments.

## Usage

```bash
# Compile to Wasm
fj build --wasm examples/hello.fj -o hello.wasm

# Run with Wasm (local runtime)
fj run --wasm examples/hello.fj
```

## WASI Support

Fajar Lang targets WASI (WebAssembly System Interface) for system access:

```fajar
fn main() {
    let content = read_file("input.txt")
    match content {
        Ok(s) => println(f"Read {len(s)} bytes"),
        Err(e) => println(f"Error: {e}"),
    }
}
```

WASI provides: file I/O, environment variables, clocks, random numbers.

## Wasm-on-MCU

For resource-constrained embedded targets, Fajar Lang provides a size-optimized Wasm runtime:

```bash
fj build --wasm --size-opt examples/sensor.fj
```

This produces minimal Wasm binaries suitable for microcontrollers with limited memory.

## Component Model

Fajar Lang generates WIT (WebAssembly Interface Types) for the component model:

```fajar
// Exported as a Wasm component
pub fn process(input: str) -> str {
    f"Processed: {input}"
}
```

Generates WIT interface:
```wit
interface processor {
    process: func(input: string) -> string
}
```

## Browser Integration

Compile Fajar Lang to Wasm and load in the browser:

```html
<script type="module">
  const { instance } = await WebAssembly.instantiateStreaming(
    fetch('program.wasm')
  );
  const result = instance.exports.main();
</script>
```
