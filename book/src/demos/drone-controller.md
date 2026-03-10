# Demo: Drone Flight Controller

A real-time sensor-to-actuator pipeline demonstrating Fajar Lang's cross-domain bridge pattern.

## Architecture

```
@kernel (IMU/Baro)  →  @safe (bridge)  →  @device (ML inference)  →  @safe  →  @kernel (motors)
```

## Key Features

- **@kernel** reads raw IMU + barometer data via `port_read`
- **@device** runs attitude estimation using tensor `matmul` + `relu`
- **@safe** bridges the two domains with type-safe data conversion
- PID controller computes motor adjustments
- Motor mixing distributes thrust to 4 motors

## How It Works

1. `read_imu_sensor()` returns raw [f32; 6] from hardware ports
2. Bridge converts raw data to `Tensor` via `from_data`
3. `estimate_attitude()` runs a 6→16→4 neural network
4. PID controller computes error-correction per axis
5. `mix_motors()` maps pitch/roll/yaw/thrust to 4 PWM outputs

## Running

```bash
fj run examples/drone_control.fj
```

## Source

See `examples/drone_control.fj` for the full implementation.
