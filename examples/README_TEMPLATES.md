# Application Templates — Getting Started

## Web Service Template

**File:** `examples/template_web_service.fj`
**Lines:** 528 | **Features:** Router, Auth, CRUD, JSON, Logging

### Quick Start
```bash
fj run examples/template_web_service.fj
```

### What It Demonstrates
1. **HTTP Router** — Route matching with path parameters
2. **Authentication** — Token-based auth middleware
3. **CRUD Operations** — Create/Read/Update/Delete on in-memory database
4. **JSON Responses** — Structured HTTP response building
5. **Middleware Chain** — Logging + auth applied to every request
6. **Configuration** — Load from TOML file or environment defaults
7. **Error Handling** — Status codes (200, 201, 401, 404, 500)
8. **Health Check** — `/health` endpoint bypasses auth

### Output
Processes 6 simulated HTTP requests showing full request/response cycle.

---

## IoT Edge Device Template

**File:** `examples/template_iot_edge.fj`
**Lines:** 336 | **Features:** Sensors, Anomaly Detection, Telemetry

### Quick Start
```bash
fj run examples/template_iot_edge.fj
```

### What It Demonstrates
1. **4 Sensor Types** — Temperature, Humidity, Pressure, Light
2. **Sensor Calibration** — Scale + offset correction
3. **Ring Buffer** — Store readings when offline
4. **Anomaly Detection** — Threshold-based with severity levels
5. **JSON Telemetry** — Formatted output for MQTT/HTTP upload
6. **Power Management** — Active/LowPower/Sleep states
7. **Fleet Status** — Health classification (Healthy/Degraded/Critical)
8. **File Logging** — Write events to log file

### Output
Simulates 8 sensor reading ticks with anomaly injection at ticks 3 and 5.

---

## ML Training Pipeline Template

**File:** `examples/template_ml_pipeline.fj`
**Lines:** 381 | **Features:** SGD, Early Stopping, Metrics, Model Export

### Quick Start
```bash
fj run examples/template_ml_pipeline.fj
```

### What It Demonstrates
1. **Synthetic Dataset** — y = 2x + 3 + noise (50 samples)
2. **Train/Val/Test Split** — 60%/20%/20%
3. **Mini-batch SGD** — Configurable learning rate, batch size
4. **MSE/RMSE/MAE Metrics** — Computed per epoch
5. **Early Stopping** — Patience-based on validation loss
6. **Model Persistence** — Save/load weight + bias to text file
7. **Hyperparameter Config** — lr, epochs, batch_size, patience

### Output
Trains linear model, reports metrics per epoch, saves model to `/tmp/`.

---

## Iris Classification Template (Real Data)

**File:** `examples/template_ml_iris.fj`
**Data:** `examples/data/iris_sample.csv`
**Lines:** 165 | **Features:** CSV Loading, Classification, Confusion Matrix

### Quick Start
```bash
fj run examples/template_ml_iris.fj
```

### What It Demonstrates
1. **Real CSV Data** — 30 samples from Fisher's Iris dataset
2. **Data Analysis** — Per-class statistics (mean, count)
3. **Threshold Classifier** — Decision boundaries from class means
4. **Confusion Matrix** — 3×3 actual vs predicted
5. **Model Export** — Thresholds saved to text file
6. **Predictions CSV** — Per-sample results exported
7. **96.7% Accuracy** — 29/30 correct classifications

### Output
Loads real CSV, trains classifier, shows confusion matrix, exports model + predictions.
