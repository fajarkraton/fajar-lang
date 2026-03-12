# Radxa Dragon Q6A — Application Development Reference

> Comprehensive reference dari https://docs.radxa.com/en/dragon/q6a/app-dev
> Untuk integrasi Fajar Lang pada edge AI SBC Qualcomm QCS6490.

---

## 1. Documentation Map

```
Dragon Q6A docs
├── Getting Started
├── System Usage
│   ├── Serial/SSH/RDP/VNC/NoMachine Login
│   ├── Performance Setting (CPU/GPU governor)
│   ├── Video Codec (H.264/H.265 HW decode via V4L2)
│   ├── Hardware-accelerated mpv
│   ├── OpenCL Usage (Mesa RustiCL + Freedreno)
│   ├── Vulkan Usage (Mesa Turnip driver)
│   ├── WebGL Usage
│   ├── KVM / Waydroid
│   └── Rsetup / System Update / apt
├── Hardware Usage
│   ├── Hardware Info
│   ├── Power Interface (12V USB-C PD)
│   ├── MicroSD Card Slot
│   ├── M.2 M-Key 2230 NVMe
│   ├── eMMC/UFS Module
│   ├── Ethernet (GbE + PoE)
│   ├── USB Type-A/C
│   ├── HDMI Interface (4K@30)
│   ├── 40-Pin GPIO Interface ← CRITICAL
│   ├── MIPI CSI (3 cameras)
│   ├── MIPI DSI (4-lane display)
│   ├── Antenna (WiFi/BT)
│   ├── Power Button
│   ├── Headphone Jack
│   ├── RTC Interface
│   └── EDL Button
├── Application Development ← THIS DOCUMENT
│   ├── NPU Development (20 items)
│   ├── CasaOS (AI apps)
│   ├── Virtual Environment (Docker, Conda)
│   ├── ROS Development (ROS1/ROS2)
│   ├── OpenCV Development
│   └── VS Code Remote SSH
├── Low-level Development
│   ├── BIOS Introduction
│   ├── Build System
│   ├── EDL Mode
│   └── SPI Boot Firmware
└── Other OS / Accessories / FAQ / Downloads
```

---

## 2. NPU Development (Hexagon 770 V68)

### 2.1 Enable NPU (FastRPC)

NPU runtime pre-installed pada system image R2+. Manual install:

```bash
sudo apt update
sudo apt install fastrpc libcdsprpc1
```

**Required device nodes:**
- `/dev/fastrpc-adsp`
- `/dev/fastrpc-cdsp`
- `/dev/fastrpc-cdsp-secure`

**DSP libraries:** `/usr/lib/dsp`

**Verification:**
```bash
sudo apt install fastrpc-test
fastrpc_test -a v68    # QCS6490 = V68 architecture
```

Expected: `RESULT: All applicable tests PASSED`

### 2.2 QAIRT SDK (Qualcomm AI Runtime)

**Version:** v2.37.1.250807
**Download:** https://softwarecenter.qualcomm.com/api/download/software/sdks/Qualcomm_AI_Runtime_Community/All/2.37.1.250807/v2.37.1.250807.zip

**Components:**
| Component | Purpose |
|-----------|---------|
| **QAIRT SDK** | Model porting & deployment |
| **AIMET** | Model quantization (PTQ + QAT) |
| **QAI AppBuilder** | Simplified deployment API (Python + C++) |
| **QAI Hub** | Online model conversion service |

**Supported backends:**
| Backend | Library | Best for |
|---------|---------|----------|
| CPU | `libQnnCpu.so` | Any model, fallback |
| GPU | `libQnnGpu.so` | FP16/FP32 |
| HTP (NPU) | `libQnnHtp.so` | INT8 quantized, fastest |

**Model formats:**
| Format | Backend | Cross-OS | Cross-Chip | Notes |
|--------|---------|----------|------------|-------|
| Library (.so) | CPU/GPU/NPU | No | Yes | Standard |
| DLC | CPU/GPU/NPU | Yes | Yes | Flexible |
| Context Binary (.bin) | NPU only | Yes | No | **Optimal: best memory + perf** |

**QCS6490 identifiers:**
- `dsp_arch`: v68
- `soc_id`: 35

### 2.3 QAIRT SDK Installation (Host x86_64)

```bash
# Download & extract
unzip v2.37.1.250807.zip && cd qairt/2.37.1.250807

# Python env
conda create -n qairt python=3.10
conda activate qairt

# Environment setup
source bin/envsetup.sh

# Dependency checks
sudo ${QAIRT_SDK_ROOT}/bin/check-linux-dependency.sh
${QAIRT_SDK_ROOT}/bin/envcheck -c
python3 "${QAIRT_SDK_ROOT}/bin/check-python-dependency"
```

**Docker alternative (QCS6490):**
```bash
docker pull radxazifeng278/qairt-npu:v1.0
```

### 2.4 QAIRT CLI Tools

| Tool | Purpose |
|------|---------|
| `qairt-converter` | Convert ONNX/TF/TFLite/PyTorch → DLC |
| `qairt-quantizer` | Quantize DLC (INT8/INT16) |
| `qnn-context-binary-generator` | DLC → Context Binary (.bin) for NPU |
| `qnn-net-run` | Run inference (DLC or Context Binary) |
| `qnn-context-binary-utility` | Inspect context binary metadata |

### 2.5 Complete NPU Pipeline (ResNet50 Example)

```bash
# 1. Export to ONNX
python3 export_onnx.py  # → resnet50.onnx

# 2. Convert to DLC
qairt-converter --input_network ./resnet50.onnx -d 'input' 1,3,224,224
# → resnet50.dlc

# 3. Quantize to INT8
qairt-quantizer --input_dlc ./resnet50.dlc \
  --input_list ./calib_list.txt \
  --output_dlc resnet50_quantized.dlc

# 4. Create config_backend.json (QCS6490)
cat > config_backend.json << 'EOF'
{
  "graphs": [{"graph_names": ["resnet50"], "vtcm_mb": 0}],
  "devices": [{"dsp_arch": "v68", "soc_id": 35}]
}
EOF

cat > config_file.json << 'EOF'
{
  "backend_extensions": {
    "shared_library_path": "libQnnHtpNetRunExtensions.so",
    "config_file_path": "config_backend.json"
  }
}
EOF

# 5. Generate Context Binary
qnn-context-binary-generator \
  --model libQnnModelDlc.so \
  --backend libQnnHtp.so \
  --dlc_path resnet50_quantized.dlc \
  --output_dir output --binary_file resnet50_quantized \
  --config_file config_file.json
# → output/resnet50_quantized.bin

# 6. Deploy to Q6A
scp resnet50_quantized.bin radxa@<ip>:/opt/models/
scp qnn-net-run libQnnHtp.so libQnnHtpV68Stub.so libQnnHtpV68Skel.so \
    radxa@<ip>:/opt/models/

# 7. Run inference on Q6A
ssh radxa@<ip>
cd /opt/models
./qnn-net-run --backend ./libQnnHtp.so \
  --retrieve_context ./resnet50_quantized.bin \
  --input_list ./test_list.txt --output_dir output_bin
```

### 2.6 Quick NPU Validation (On-Device)

```bash
pip3 install modelscope
modelscope download --model radxa/resnet50_qairt --local ./resnet50_qairt
export PRODUCT_SOC=6490
cd resnet50_qairt/${PRODUCT_SOC}
chmod +x qnn-net-run
./qnn-net-run --backend ./libQnnHtp.so \
  --retrieve_context ./resnet50_aimet_quantized_6490.bin \
  --input_list ./test_list.txt --output_dir output_bin
```

### 2.7 AIMET Quantization

**Install:**
```bash
conda create -n aimet python=3.10
conda activate aimet
pip3 install aimet-onnx    # PTQ (Post-Training Quantization)
pip3 install aimet-torch    # QAT (Quantization-Aware Training)
```

**Output:** quantized `.onnx` + `.encodings` → feed to `qairt-converter` with `--quantization_overrides`

### 2.8 ONNX Runtime QNN Execution Provider

**Install (on Q6A):**
```bash
pip3 install https://github.com/ZIFENG278/onnxruntime/releases/download/v1.23.2/onnxruntime_qnn-1.23.2-cp312-cp312-linux_aarch64.whl
```

**Python API:**
```python
import onnxruntime
import numpy as np

options = onnxruntime.SessionOptions()
options.add_session_config_entry("session.disable_cpu_ep_fallback", "1")

session = onnxruntime.InferenceSession(
    "model.onnx",
    sess_options=options,
    providers=["QNNExecutionProvider"],
    provider_options=[{"backend_path": "libQnnHtp.so"}]
)

input0 = np.ones((1, 3, 224, 224), dtype=np.uint8)
result = session.run(None, {"image_tensor": input0})
```

### 2.9 TFLite Delegate

```bash
modelscope download --model radxa/Inception_v3_qairt_tflite_delegate \
  --local ./tflite_demo
cd tflite_demo
chmod +x qtld-net-run
export LD_LIBRARY_PATH=$(pwd)/libs:$LD_LIBRARY_PATH
export ADSP_LIBRARY_PATH=$(pwd)/libs
./qtld-net-run --model inception_v3_quant.tflite \
  --input input_list.txt --output outputs --backend htp
```

### 2.10 QAI AppBuilder Models (QCS6490)

| Category | Models |
|----------|--------|
| **Classification** | ConvNeXt, EfficientNet, VisionTransformer |
| **Detection** | YOLOv8-det |
| **Segmentation** | FCN-ResNet50 |
| **Super-Resolution** | Real-ESRGAN, QuickSRNet |
| **LLM** | Llama 3.2-1B, Qwen 2.5-0.5B |

### 2.11 NPU Inference Examples

| Model | Inference Time | Notes |
|-------|---------------|-------|
| ResNet50 (INT8) | ~few ms | Image classification, 1000 classes |
| YOLOv8-det | ~33ms | Object detection |
| Inception v3 | ~few ms | TFLite delegate |
| GoogLeNet | ~few ms | Image classification |
| FCN-ResNet50 | ~few ms | Semantic segmentation |
| Real-ESRGAN | ~few ms | 4x image super-resolution |

### 2.12 LLM on NPU

**Llama 3.2-1B:**
```bash
modelscope download --model radxa/Llama3.2-1B-1024-qairt-v68 \
  --local ./Llama3.2-1B
cd Llama3.2-1B
export LD_LIBRARY_PATH=$(pwd)
chmod +x genie-t2t-run
./genie-t2t-run -c ./htp-model-config-llama32-1b-gqa.json \
  -p '<|begin_of_text|><|start_header_id|>system<|end_header_id|>
You are a helpful assistant<|eot_id|><|start_header_id|>user<|end_header_id|>
What is AI?<|eot_id|><|start_header_id|>assistant<|end_header_id|>'
```

| Metric | CTX 1024 | CTX 4096 |
|--------|----------|----------|
| Prompt rate | 171.67 tok/s | 110.04 tok/s |
| Generation rate | 12.16 tok/s | 9.30 tok/s |
| Time-to-first-token | 192ms | 300ms |

**Qwen 2.5-0.5B:**
```bash
modelscope download --model radxa/Qwen2.5-0.5B-v68 --local ./Qwen2.5
cd Qwen2.5
export LD_LIBRARY_PATH=$(pwd)
chmod +x genie-t2t-run
./genie-t2t-run -c qwen2.5-0.5B-1k-htp.json \
  -p '<|im_start|>system
You are a helpful assistant.<|im_end|>
<|im_start|>user
What is AI?<|im_end|>
<|im_start|>assistant
'
```

| Metric | Value |
|--------|-------|
| Prompt rate | ~309 tok/s |
| Generation rate | ~24.3 tok/s |
| Time-to-first-token | 93.8ms |

---

## 3. GPU Compute

### 3.1 OpenCL (Mesa RustiCL + Freedreno)

```bash
sudo apt update
sudo apt install mesa-opencl-icd -y
RUSTICL_ENABLE=freedreno clinfo
```

**Key specs:**
| Property | Value |
|----------|-------|
| Device | FD643 (Qualcomm) |
| OpenCL Version | 3.0 (FULL_PROFILE) |
| OpenCL C | 1.2 |
| Max Work Group Size | 1024 |
| Max Work Item Sizes | 1024 x 1024 x 64 |
| Global Memory | ~7.4 GiB |
| Max Alloc | 2 GiB |
| FP32 | Supported |
| FP64 | Not available |
| INT dot product | Supported |

### 3.2 Vulkan (Mesa Turnip Driver)

```bash
sudo apt install mesa-vulkan-drivers vulkan-tools
vulkaninfo
```

**Benchmark (vkpeak):**
| Metric | Performance |
|--------|------------|
| **FP32 scalar** | **773 GFLOPS** |
| **FP16 vec4** | **1,581 GFLOPS** |
| **INT8 dotprod** | **1,176 GIOPS** |
| **Memory bandwidth** | 9.06 GB/s |
| Device | Turnip Adreno 643 |

---

## 4. GPIO (40-Pin Header)

### 4.1 Pin Control via python-periphery

```bash
sudo apt update
sudo apt install -y python3-periphery
```

GPIO access via `/dev/gpiochip4`:

```python
from periphery import GPIO

# Output: Toggle LED on GPIO25
led = GPIO("/dev/gpiochip4", 25, "out")
led.write(True)   # HIGH
led.write(False)   # LOW
led.close()

# Input: Read button on GPIO96
button = GPIO("/dev/gpiochip4", 96, "in")
state = button.read()
button.close()
```

### 4.2 Supported Protocols (via Device Tree Overlays)

| Protocol | Instances | Header Pins |
|----------|-----------|-------------|
| UART | 7 (UART0,2,5,6,7,12,14) | TX/RX pairs |
| I2C | 6 (I2C0,2,6,7,12,14) | SDA/SCL pairs |
| SPI | 7 (SPI0,2,5,6,7,12,14) | MOSI/MISO/CS |
| I2S | 1 (MI2S0) | MCLK/SCK/WS/DATA0/DATA1 |
| I3C | 1 (I3C0) | 2 pins |

---

## 5. Camera (MIPI CSI)

| Port | Lanes | FPC Connector |
|------|-------|---------------|
| CSI0 | 4-lane | Primary (highest bandwidth) |
| CSI1 | 2-lane | Secondary |
| CSI2 | 2-lane | Tertiary |

**Compatible cameras:** Radxa Camera 4K, 8M 219, 12M 577, 13M 214

**V4L2 devices:** `/dev/video0`, `/dev/video1`

---

## 6. Video Codec (Hardware Accelerated)

**H.264 decode:**
```bash
gst-launch-1.0 -e filesrc location="video.mp4" ! qtdemux ! queue ! \
  h264parse ! v4l2h264dec ! autovideosink
```

**H.265 decode:**
```bash
gst-launch-1.0 filesrc location="video.mp4" ! qtdemux name=demux \
  demux.video_0 ! queue ! h265parse ! v4l2h265dec \
  capture-io-mode=4 output-io-mode=4 ! video/x-raw,format=NV12 ! \
  fpsdisplaysink text-overlay=false video-sink="fakesink" sync=false
```

---

## 7. Performance Tuning

### CPU Governor

```bash
# Policy 0 (Silver, 4x A55): 300 MHz - 1.96 GHz
echo userspace > /sys/devices/system/cpu/cpufreq/policy0/scaling_governor
echo 1958400 > /sys/devices/system/cpu/cpufreq/policy0/scaling_setspeed

# Policy 4 (Gold, 3x A78): 691 MHz - 2.4 GHz
echo userspace > /sys/devices/system/cpu/cpufreq/policy4/scaling_governor
echo 2400000 > /sys/devices/system/cpu/cpufreq/policy4/scaling_setspeed

# Policy 7 (Prime, 1x A78): 806 MHz - 2.71 GHz
echo userspace > /sys/devices/system/cpu/cpufreq/policy7/scaling_governor
echo 2710000 > /sys/devices/system/cpu/cpufreq/policy7/scaling_setspeed
```

### GPU Governor

```bash
# Available: 315, 450, 550, 608, 700, 812 MHz
echo userspace > /sys/class/devfreq/3d00000.gpu/governor
echo 812000000 > /sys/class/devfreq/3d00000.gpu/userspace/set_freq
```

---

## 8. Virtual Environment

| Tool | Purpose |
|------|---------|
| **Docker** | Container isolation |
| **Conda** | Python environment management |

---

## 9. ROS Development

| Version | Status |
|---------|--------|
| ROS1 | Supported |
| ROS2 | Supported |

---

## 10. OpenCV Development

- OpenCV installation guide available
- Example programs provided

---

## 11. VS Code Remote SSH

```bash
# On Q6A: enable SSH
sudo systemctl enable --now ssh
ip a  # get IP

# On host: connect via VS Code
# Install "Remote - SSH" extension
# Ctrl+Shift+P → "Remote-SSH: Add New SSH Host..."
# ssh radxa@<ip>

# SSH key auth (recommended)
ssh-keygen -t ed25519
ssh-copy-id radxa@<ip>
```

---

## 12. Fajar Lang Integration Points

| Radxa Feature | Fajar Lang Integration | Priority |
|--------------|----------------------|----------|
| **FastRPC + NPU** | `npu_load()`, `npu_infer()` builtins via libQnnHtp.so dlopen | P0 |
| **ONNX Runtime QNN EP** | Export Fajar Lang models → ONNX → QNN EP inference | P0 |
| **GPIO /dev/gpiochip4** | `gpio_write()`, `gpio_read()` builtins | P0 |
| **python-periphery** | Reference impl for GPIO/I2C/SPI access patterns | P1 |
| **OpenCL 3.0** | GPU tensor ops via OpenCL C kernels | P1 |
| **Vulkan compute** | Compute shaders for tensor operations | P1 |
| **V4L2 camera** | Camera capture → Tensor conversion | P2 |
| **GStreamer** | Video decode pipeline integration | P2 |
| **CPU governor** | Performance tuning from .fj programs | P2 |
| **LLM (Llama/Qwen)** | On-device LLM inference from Fajar Lang | P3 |
| **Docker** | Containerized Fajar Lang deployment | P3 |
| **ROS2** | Robotics integration | P3 |

---

*Document Version: 1.0 | Created: 2026-03-12 | Source: docs.radxa.com/en/dragon/q6a/app-dev*
