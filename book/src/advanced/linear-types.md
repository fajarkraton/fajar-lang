# Linear Types

Linear types guarantee that a resource is used exactly once — no forgetting, no double-use. This is critical for hardware resources that must be properly released.

## Linear Resources

```fajar
linear struct FileHandle {
    fd: i32,
}

fn open(path: str) -> FileHandle { ... }
fn write(handle: FileHandle, data: str) -> FileHandle { ... }  // Consumes and returns
fn close(handle: FileHandle) -> void { ... }  // Consumes permanently
```

The compiler enforces:
- Every `FileHandle` must be consumed (via `write`, `close`, or another consuming function)
- Using a handle after it's consumed is a compile error
- Dropping a handle without consuming it is a compile error

## Hardware Handles

```fajar
linear struct GpioPin {
    port: u8,
    pin: u8,
}

fn configure(pin: GpioPin, mode: PinMode) -> GpioPin { ... }
fn digital_write(pin: GpioPin, value: bool) -> GpioPin { ... }
fn release(pin: GpioPin) -> void { ... }

// Correct usage:
let pin = GpioPin { port: 0, pin: 13 }
let pin = configure(pin, Output)
let pin = digital_write(pin, true)
release(pin)  // Must release — can't just drop it

// Error: pin used after consumed
// digital_write(pin, false)  // LN001: use after linear consumption
```

## Borrowing Protocol

Sometimes you need to temporarily access a linear resource without consuming it. Use `&borrow`:

```fajar
fn read_status(handle: &borrow FileHandle) -> Status {
    // Can read from handle but not consume it
    query_fd(handle.fd)
}

let handle = open("data.bin")
let status = read_status(&handle)  // Borrows temporarily
close(handle)  // handle is still available
```

## Linear vs Affine

| Type | Rule | Use Case |
|------|------|----------|
| Linear | Must be used exactly once | Hardware pins, DMA buffers |
| Affine | May be used at most once | File handles, GPU buffers |

Affine types can be dropped without explicit consumption. Linear types cannot.

## Error Codes

| Code | Meaning |
|------|---------|
| LN001 | Use after linear consumption |
| LN002 | Linear resource not consumed |
| LN003 | Linear resource consumed twice |
| LN004 | Linear resource escaped scope |
| LN005 | Borrow of consumed resource |
| LN006 | Linear resource in non-linear context |
| LN007 | Must-use resource ignored |
| LN008 | Pin protocol violation |
