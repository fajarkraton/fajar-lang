# Real-Time IoT Dashboard

Build a sensor data pipeline: MQTT telemetry → regex validation → SQLite storage → anomaly alerts.

## What You'll Build

An IoT edge device application that:
- Connects to an MQTT broker for sensor data
- Validates data format with regex
- Stores readings in SQLite
- Detects temperature anomalies

## Step 1: Connect to MQTT Broker

```fajar
fn main() {
    // Connect to MQTT broker (in-memory simulation without --features mqtt)
    let client = mqtt_connect("mqtt://127.0.0.1:1883")
    mqtt_subscribe(client, "sensors/temperature")
    println("[mqtt] Connected and subscribed")
```

With `--features mqtt`, this connects to a real Mosquitto broker. Without the feature, it uses an in-memory broker for testing.

## Step 2: Validate Sensor Data with Regex

```fajar
fn validate(payload: str) -> bool {
    // Format: "sensor_id:value" (e.g., "temp_01:23.5")
    regex_match("^\\w+:[\\-]?\\d+\\.?\\d*$", payload)
}

fn parse_value(payload: str) -> f64 {
    let parts = payload.split(":")
    to_float(parts[1])
}
```

The `regex_match` builtin uses a compiled regex cache — patterns are only compiled once.

## Step 3: Store in SQLite

```fajar
    let db = db_open(":memory:")
    db_execute(db, "CREATE TABLE readings (sensor TEXT, value REAL, status TEXT)")
```

## Step 4: Process Pipeline

```fajar
    // Publish test data
    let data = ["temp_01:23.5", "temp_02:45.1", "temp_03:-15.0", "invalid!"]
    let mut i = 0
    while i < len(data) {
        mqtt_publish(client, "sensors/temperature", data[i])
        i = i + 1
    }

    // Process received messages
    let mut done = false
    while !done {
        let msg = mqtt_recv(client)
        if msg == null {
            done = true
        } else {
            let payload = to_string(msg)
            if validate(payload) {
                let value = parse_value(payload)
                let status = if value > 40.0 || value < -10.0 { "anomaly" } else { "normal" }
                println(f"  {status}: {payload}")
                db_execute(db, f"INSERT INTO readings VALUES ('{payload}', {value}, '{status}')")
            } else {
                println(f"  SKIP: invalid format")
            }
        }
    }

    mqtt_disconnect(client)
    println("Pipeline complete")
}
```

## Key Concepts

| Feature | Builtin |
|---------|---------|
| MQTT connect | `mqtt_connect(broker)` |
| MQTT publish | `mqtt_publish(client, topic, payload)` |
| MQTT subscribe | `mqtt_subscribe(client, topic)` |
| MQTT receive | `mqtt_recv(client)` → Map or null |
| Regex validation | `regex_match(pattern, text)` |
| SQLite storage | `db_open`, `db_execute`, `db_query` |

## Running with Real MQTT

```bash
# Start Mosquitto broker
mosquitto -d

# Run with real networking
cargo run --features mqtt -- run examples/iot_mqtt_pipeline.fj
```

## Full Source

See [`examples/iot_mqtt_pipeline.fj`](https://github.com/fajarkraton/fajar-lang/blob/main/examples/iot_mqtt_pipeline.fj)
