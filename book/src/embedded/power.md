# Power Management

Fajar Lang provides fine-grained power control for battery-powered embedded devices.

## Power Modes

| Mode | CPU | RAM | Wake Time | Use Case |
|------|-----|-----|-----------|----------|
| Run | Active | Active | - | Normal operation |
| Sleep | WFI | Active | ~1us | Short idle periods |
| Stop | Off | Retained | ~5us | Between sensor readings |
| Standby | Off | Lost | ~50us | Long idle periods |
| Shutdown | Off | Off | ~1ms | Minimal power draw |

```fajar
use os::power

@kernel
fn enter_low_power() {
    power::set_mode(PowerMode::Stop)
    power::set_wake_source(WakeSource::RtcAlarm(30_000))  // Wake in 30s
    power::enter()  // Execution resumes here after wake
}
```

## Wake Sources

```fajar
// Wake on GPIO pin (e.g., button press)
power::set_wake_source(WakeSource::GpioPin(13, Edge::Rising))

// Wake on RTC alarm
power::set_wake_source(WakeSource::RtcAlarm(60_000))  // 60 seconds

// Wake on interrupt
power::set_wake_source(WakeSource::Interrupt(IrqNumber::UART1))

// Wake on timer
power::set_wake_source(WakeSource::WakeupTimer(5_000))  // 5 seconds
```

## Clock Gating

Disable peripheral clocks to save power:

```fajar
@kernel
fn disable_unused() {
    power::clock_disable(Peripheral::SPI2)
    power::clock_disable(Peripheral::I2C3)
    power::clock_disable(Peripheral::USART3)
}
```

## Battery Life Estimation

```fajar
let budget = PowerBudget {
    battery_mah: 3000,
    active_ma: 50.0,
    sleep_ma: 0.01,
    duty_cycle: 0.01,  // Active 1% of the time
}

let hours = power::estimate_battery_life(budget)
println(f"Estimated battery life: {hours} hours")
```

## Voltage Scaling

```fajar
power::set_voltage_scale(VoltageScale::VOS3)  // Lowest voltage, lowest speed
power::set_clock_speed(48_000_000)  // Reduce from 168MHz to 48MHz
```
