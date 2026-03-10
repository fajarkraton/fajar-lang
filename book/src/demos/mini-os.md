# Demo: Mini OS Kernel

A minimal operating system kernel demonstrating Fajar Lang's `@kernel` context for OS development.

## Components

| Component | Feature | Port/Address |
|-----------|---------|-------------|
| VGA Driver | Text mode 80x25 | 0xB8000 |
| Keyboard | PS/2 scancode input | 0x60, 0x64 |
| Timer | PIT 100Hz tick | 0x40, 0x43 |
| Shell | Command interpreter | — |

## Key Features

- All hardware access through `@kernel` context annotations
- `mem_write`/`mem_read` for memory-mapped I/O (VGA)
- `port_write`/`port_read` for port I/O (keyboard, PIT)
- `irq_register`/`irq_enable` for interrupt handling
- Simple command shell with built-in commands

## Shell Commands

| Command | Description |
|---------|-------------|
| `help` | List available commands |
| `version` | Show OS version |
| `clear` | Clear the screen |
| `mem` | Show memory info |
| `uptime` | Show uptime |
| `halt` | Halt the system |

## Building for Bare Metal

```bash
fj build examples/mini_os.fj --target x86_64-unknown-none --no-std
```

## Source

See `examples/mini_os.fj` for the full implementation.
