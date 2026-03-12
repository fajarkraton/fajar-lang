# Debugger

Fajar Lang includes a Debug Adapter Protocol (DAP) debugger with time-travel capabilities.

## VS Code Integration

Add to `.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "fajar",
            "request": "launch",
            "name": "Debug Fajar Program",
            "program": "${workspaceFolder}/examples/hello.fj",
            "stopOnEntry": false
        }
    ]
}
```

## Breakpoints

| Type | Description | Example |
|------|-------------|---------|
| Line | Break at line number | Click gutter in VS Code |
| Conditional | Break when condition is true | `break if x > 100` |
| Hit count | Break after N hits | `break hit 5` |
| Log point | Print message without stopping | `log f"x = {x}"` |

## Stepping

| Command | Action |
|---------|--------|
| Continue | Run to next breakpoint |
| Step Over | Execute current line, skip into functions |
| Step Into | Enter function call |
| Step Out | Run until current function returns |

## Variable Inspection

The debugger shows:
- **Local variables** — all variables in current scope
- **Watch expressions** — evaluate expressions on each step
- **Call stack** — full call chain with source locations

## Time-Travel Debugging

Record execution and replay it forward or backward:

```bash
fj debug --record examples/buggy.fj
```

| Command | Action |
|---------|--------|
| Reverse Step | Go back one step |
| Reverse Continue | Run backwards to previous breakpoint |
| Watchpoint | Break when a variable changes (forward or backward) |

### Root Cause Analysis

```bash
fj debug --root-cause examples/crash.fj
```

Automatically traces backwards from a crash to find the root cause.

## Memory Visualization

### Heap Map

```bash
fj debug --heap-map
```

Shows allocated memory blocks, fragmentation level, and allocation sites.

### Reference Graph

Visualizes reference relationships between objects, detecting cycles and potential leaks.

## CPU Profiler

```bash
fj debug --profile examples/slow.fj
```

Generates flame graphs showing where time is spent. Provides PGO (profile-guided optimization) hints for the compiler.
