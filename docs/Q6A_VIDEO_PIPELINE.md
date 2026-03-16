# Dragon Q6A Video Processing Pipeline

> Hardware video decode/encode + NPU inference on Radxa Dragon Q6A (QCS6490).

---

## Hardware Capabilities

| Feature | Specification |
|---------|--------------|
| **Codec Engine** | Spectra 580L ISP + Adreno video |
| **Decode** | H.264 4K@60, H.265 4K@60, VP9 4K@30 |
| **Encode** | H.264 4K@30, H.265 4K@30 |
| **Camera** | 3× MIPI-CSI (1× 4-lane, 2× 2-lane) |
| **ISP** | Spectra ISP — auto exposure, HDR10, noise reduction |
| **Display** | HDMI 2.0 4K@60 + MIPI-DSI |
| **NPU** | Hexagon 770 — 12 TOPS INT8 |

## Pipeline Architecture

```
Camera (MIPI-CSI)
    │ V4L2 /dev/video*
    ▼
Frame Capture (NV12/YUYV)
    │
    ▼
Decode (V4L2 M2M, H.264/H.265)
    │
    ▼
Preprocess (resize → normalize → CHW)
    │
    ▼
NPU Inference (Hexagon 770, INT8)
    │
    ▼
Post-process (NMS, bounding boxes)
    │
    ▼
Overlay (bbox + label on frame)
    │
    ▼
Encode (H.264/H.265) → RTSP/file
```

## Quick Start

```fajar
// Simulated video detection pipeline
fn detect_objects(frame: i64) -> i64 {
    let input = tensor_randn(1, 512)
    let w = tensor_xavier(512, 20)
    let scores = tensor_softmax(tensor_matmul(input, w))
    tensor_argmax(scores)
}

let mut frame = 0
while frame < 30 {
    let class_id = detect_objects(frame)
    println(f"Frame {frame}: class {class_id}")
    frame = frame + 1
}
```

## V4L2 Camera Access

```bash
# List video devices
v4l2-ctl --list-devices

# Capture a frame (CSI0)
v4l2-ctl -d /dev/video0 --set-fmt-video=width=1920,height=1080,pixelformat=NV12
v4l2-ctl -d /dev/video0 --stream-mmap --stream-count=1 --stream-to=frame.raw

# List supported formats
v4l2-ctl -d /dev/video0 --list-formats-ext
```

## Performance Targets

| Pipeline | Target | Notes |
|----------|--------|-------|
| Decode (1080p H.264) | < 5ms/frame | V4L2 M2M hardware |
| Preprocess (1080p→224×224) | < 2ms | CPU NEON or GPU |
| NPU Inference (MobileNetV2) | < 4ms | Hexagon 770, INT8 |
| Post-process (NMS) | < 1ms | CPU |
| Encode (1080p H.264) | < 8ms/frame | V4L2 M2M hardware |
| **End-to-end** | **< 33ms** | **30 FPS target** |
| Glass-to-glass | < 50ms | Including display latency |

## Multi-Camera Setup

The Q6A has 3 MIPI-CSI ports:

| Port | Lanes | Max Resolution | Use Case |
|------|-------|---------------|----------|
| CSI0 | 4-lane | 4K@30 / 1080p@60 | Primary camera |
| CSI1 | 2-lane | 1080p@30 | Secondary / stereo |
| CSI2 | 2-lane | 1080p@30 | Rear / auxiliary |

```bash
# Multi-camera simultaneous capture
v4l2-ctl -d /dev/video0 --stream-mmap &
v4l2-ctl -d /dev/video2 --stream-mmap &
v4l2-ctl -d /dev/video4 --stream-mmap &
```

## RTSP Server Pattern

```fajar
// Pseudo-code for RTSP streaming with inference overlay
fn video_server_loop() -> i64 {
    let mut frame_id = 0
    let max_frames = 900  // 30 FPS × 30 seconds
    while frame_id < max_frames {
        // 1. Capture frame from CSI0
        // 2. Decode (if compressed input)
        // 3. Run NPU inference
        // 4. Draw bounding boxes
        // 5. Encode to H.264
        // 6. Send via RTSP
        frame_id = frame_id + 1
    }
    frame_id
}
```

## HDR10 Support

The Spectra ISP supports HDR10 capture:

```bash
# Check HDR support
v4l2-ctl -d /dev/video0 --list-ctrls | grep hdr

# Enable HDR capture (if supported by camera module)
v4l2-ctl -d /dev/video0 --set-ctrl=wide_dynamic_range=1
```

## Troubleshooting

| Issue | Solution |
|-------|---------|
| `/dev/video*` not found | `sudo modprobe v4l2-loopback` or check camera cable |
| Decode latency > 10ms | Use M2M device, not software decode |
| NPU timeout during video | Reduce input resolution or model complexity |
| Frame drops at 30 FPS | Check thermal throttling: `cpu_temp()` |
| RTSP stream lag | Increase encode bitrate or reduce resolution |
| Multi-camera fails | Check CSI lane allocation in device tree |

## Example Programs

| Example | Description |
|---------|-------------|
| `q6a_video_detect.fj` | Video object detection with bounding boxes |
| `q6a_camera_classify.fj` | Live camera classification (pending camera) |
| `q6a_camera_detect.fj` | Live object detection (pending camera) |

---

*Dragon Q6A Video Pipeline — Fajar Lang v2.0 "Dawn"*
