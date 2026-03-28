# Dragon Q6A Production Deployment Guide

> Deploy Fajar Lang edge AI applications on Radxa Dragon Q6A for 24/7 operation.

---

## Quick Start

```bash
# 1. Cross-compile on host
cargo build --release --target aarch64-unknown-linux-gnu

# 2. Deploy binary
scp target/aarch64-unknown-linux-gnu/release/fj radxa@192.168.100.2:/usr/local/bin/fj

# 3. Deploy application
scp examples/q6a_edge_deploy.fj radxa@192.168.100.2:/opt/fj/app.fj

# 4. Run
ssh radxa@192.168.100.2 'fj run /opt/fj/app.fj'
```

---

## System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| RAM | 1 GB free | 4 GB free |
| Storage | 100 MB | 1 GB (for logs + models) |
| OS | Ubuntu 22.04 aarch64 | Ubuntu 24.04 aarch64 |
| Network | Ethernet (static IP) | Ethernet + WiFi |

---

## Systemd Service

Install the service file for auto-start on boot:

```bash
sudo cp scripts/fj-service.service /etc/systemd/system/fj-app.service
sudo mkdir -p /opt/fj /var/log/fj
sudo systemctl daemon-reload
sudo systemctl enable fj-app
sudo systemctl start fj-app
```

Check status:
```bash
sudo systemctl status fj-app
journalctl -u fj-app -f  # follow logs
```

The service automatically restarts on failure (`Restart=on-failure`, 5s delay).

---

## Monitoring

### Built-in Builtins

Fajar Lang provides system monitoring builtins:

| Builtin | Returns | Description |
|---------|---------|-------------|
| `cpu_temp()` | `i64` | CPU temperature in millidegrees Celsius |
| `cpu_freq()` | `i64` | CPU frequency in kHz |
| `mem_usage()` | `i64` | Memory usage percentage (0-100) |
| `sys_uptime()` | `i64` | System uptime in seconds |
| `process_id()` | `i64` | Current process PID |
| `log_to_file(path, msg)` | `bool` | Append timestamped message to log |

### Shell Monitor

```bash
./scripts/q6a-monitor.sh 5  # monitor every 5 seconds
```

---

## Watchdog Timer

Use the software watchdog to detect hangs:

```fajar
let wd = watchdog_start(5000)  // 5 second timeout

while true {
    watchdog_kick(wd)         // must kick within timeout
    let result = run_inference()
    log_to_file("/var/log/fj/inference.log", result)
    sleep_ms(100)
}
```

If the application fails to kick the watchdog within the timeout, a warning is printed to stderr. Combined with systemd `Restart=on-failure`, this enables automatic recovery.

---

## Inference Caching

For repeated queries, use the built-in cache:

```fajar
let cached = cache_get("sensor_42")
if cached != "" {
    println(f"Cache hit: {cached}")
} else {
    let result = run_inference(sensor_data)
    cache_set("sensor_42", result)
}
```

Clear cache periodically: `cache_clear()`

---

## Thermal Management

Monitor temperature during inference to avoid throttling:

```fajar
let temp = cpu_temp()
if temp > 80000 {
    // 80°C — reduce workload
    sleep_ms(500)
    log_to_file("/var/log/fj/thermal.log", f"THROTTLE: {temp}mC")
}
```

Q6A thermal zones:
- Zone 0-4: CPU clusters (target < 80°C)
- Throttling begins at ~85°C
- Emergency shutdown at ~105°C

---

## Log Management

### Application Logging

```fajar
log_to_file("/var/log/fj/app.log", f"INFER class={class} conf={confidence}")
```

### Log Rotation

Use logrotate (create `/etc/logrotate.d/fj`):

```
/var/log/fj/*.log {
    daily
    rotate 7
    compress
    missingok
    notifempty
    create 0644 radxa radxa
}
```

---

## Performance Benchmarks (Q6A)

| Operation | Time | Notes |
|-----------|------|-------|
| Cold start → first inference | 4 ms | No JIT warmup needed |
| Tensor 4x4 matmul | < 1 ms | NEON accelerated |
| NPU inference (MobileNetV2) | ~ 5 ms | Hexagon 770 |
| GPU detection (OpenCL) | < 1 ms | Adreno 635 |
| Memory overhead | ~15% at idle | With interpreter |

---

## Security Checklist

- [x] Disable SSH password auth (`PasswordAuthentication no`)
- [x] Use firewall (`ufw allow 22/tcp && ufw enable`)
- [x] Run as non-root user (`radxa`)
- [x] Resource limits in systemd (`MemoryMax=4G`, `LimitNOFILE=65536`)
- [x] Log rotation enabled
- [x] No exposed development ports

---

## File Layout

```
/usr/local/bin/fj           # Fajar Lang binary
/opt/fj/
├── app.fj                  # Main application
├── models/                 # ML model files (.fjml, .fjmq)
└── config.toml             # Application config
/var/log/fj/
├── app.log                 # Application log
├── error.log               # Error log
└── thermal.log             # Thermal events
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `fj: command not found` | Check `/usr/local/bin/fj` exists and is executable |
| High CPU temperature | Reduce inference rate, add `sleep_ms()` between cycles |
| Out of memory | Check `MemoryMax` in service, reduce batch size |
| NPU not available | Verify CDSP: `cat /sys/bus/platform/drivers/fastrpc/*/subsys_state` |
| GPU not detected | Check ICD: `cat /etc/OpenCL/vendors/adreno.icd` |
| Service won't start | `journalctl -u fj-app -n 50` for details |

---

## Hardware BOM (Edge AI Kit)

| Item | Model | Qty | Notes |
|------|-------|-----|-------|
| SBC | Radxa Dragon Q6A (8GB) | 1 | QCS6490, 12 TOPS NPU |
| Power | USB-C PD 12V/3A adapter | 1 | 36W recommended |
| Storage | NVMe M.2 2230 (256GB+) | 1 | PCIe Gen3 x2 |
| MicroSD | 32GB+ A2 U3 | 1 | Optional, for model storage |
| Camera | Radxa Camera 4K (IMX415) | 1 | Optional, for vision apps |
| Ethernet | Cat6 cable | 1 | For SSH + deployment |
| Heat Sink | Dragon Q6A heatsink + fan | 1 | Included with board |
| Enclosure | 3D-printed or aluminum case | 1 | Optional |
| RTC Battery | CR2032 | 1 | For DS1307 RTC |

**Total estimated cost:** ~$200 USD (board + storage + power)

---

*Q6A Production Guide v1.0 | 2026-03-16*
