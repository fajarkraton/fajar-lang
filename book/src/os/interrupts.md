# Interrupts

## Registering Handlers

```fajar
@kernel fn init_interrupts() {
    irq_register(0x20, timer_handler)      // timer
    irq_register(0x21, keyboard_handler)   // keyboard
    irq_enable(0x20)
    irq_enable(0x21)
}
```

## Interrupt Handler

```fajar
@kernel fn keyboard_handler() {
    let scancode = port_read(0x60)
    // Process scancode...
}

@kernel fn timer_handler() {
    // Called on each timer tick
}
```

## Enabling/Disabling

```fajar
@kernel fn critical_section() {
    irq_disable(0x21)    // disable keyboard interrupt
    // ... do critical work ...
    irq_enable(0x21)     // re-enable
}
```

## Unregistering

```fajar
@kernel fn cleanup() {
    irq_disable(0x21)
    irq_unregister(0x21)
}
```
