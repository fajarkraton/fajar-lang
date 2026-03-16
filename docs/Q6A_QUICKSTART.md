# Dragon Q6A Quick Start (5 minutes)

> Get Fajar Lang running on your Dragon Q6A in 5 minutes.

---

## Prerequisites

- Radxa Dragon Q6A with Ubuntu 24.04
- Ethernet connection to your dev machine
- Rust toolchain on dev machine (`rustup target add aarch64-unknown-linux-gnu`)

## Step 1: Build (on your dev machine)

```bash
git clone https://github.com/primecore/fajar-lang.git
cd fajar-lang
cargo build --release --target aarch64-unknown-linux-gnu
```

## Step 2: Deploy

```bash
# Copy binary to Q6A
scp target/aarch64-unknown-linux-gnu/release/fj radxa@192.168.100.2:/usr/local/bin/fj
```

## Step 3: Hello World

SSH into your Q6A and create a file:

```bash
ssh radxa@192.168.100.2
cat > /tmp/hello.fj << 'EOF'
println("Hello from Dragon Q6A!")
println(f"CPU temp: {cpu_temp() / 1000}C")
println(f"Memory: {mem_usage()}%")
println(f"GPU: {gpu_info()}")
println(f"NPU: {npu_info()}")
EOF
fj run /tmp/hello.fj
```

Expected output:
```
Hello from Dragon Q6A!
CPU temp: 55C
Memory: 12%
GPU: QUALCOMM Adreno(TM) 635, OpenCL 3.0 ...
NPU: Hexagon 770 V68, 12 TOPS INT8, QNN SDK
```

## Step 4: Run AI Inference

```bash
cat > /tmp/infer.fj << 'EOF'
let input = tensor_randn(1, 10)
let weights = tensor_xavier(10, 3)
let output = tensor_matmul(input, weights)
let probs = tensor_softmax(output)
let class = tensor_argmax(probs)
println(f"Predicted class: {class}")
EOF
fj run /tmp/infer.fj
```

## Step 5: Deploy as Service

```bash
sudo mkdir -p /opt/fj /var/log/fj
sudo cp /tmp/infer.fj /opt/fj/app.fj
# Copy the service file and enable auto-start
sudo systemctl enable --now fj-app
```

---

## Next Steps

- **GPIO**: See `examples/q6a_blinky.fj` for LED control
- **NPU**: See `examples/q6a_npu_classify.fj` for neural network inference
- **GPU**: See `examples/q6a_system_monitor.fj` for GPU/system monitoring
- **Production**: See `docs/Q6A_PRODUCTION.md` for deployment guide
- **Pinout**: See `docs/Q6A_PINOUT.md` for 40-pin header reference

---

*Dragon Q6A Quick Start v1.0 | 2026-03-16*
