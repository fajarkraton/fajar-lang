# IoT Connectivity

Fajar Lang provides integrated IoT protocol support for connected embedded devices.

## WiFi

```fajar
use iot::wifi

@device
fn connect_wifi() {
    wifi::init(WifiMode::Station)
    wifi::connect("MyNetwork", "password123")

    // HTTP client
    let response = wifi::http_get("http://api.example.com/data")
    match response {
        Ok(body) => process_data(body),
        Err(e) => println(f"HTTP error: {e}"),
    }
}
```

## BLE (Bluetooth Low Energy)

```fajar
use iot::ble

fn setup_ble() {
    ble::init("FajarSensor")

    // GATT server
    let service = ble::service_create(0x180F)  // Battery Service
    let char = ble::characteristic_add(service, 0x2A19, Read | Notify)

    ble::start_advertising()

    // Update characteristic value
    ble::characteristic_set(char, battery_level())
}
```

## MQTT

```fajar
use iot::mqtt

fn telemetry_loop() {
    let client = mqtt::connect("broker.example.com", 1883)

    mqtt::subscribe(client, "device/commands", QoS::AtLeastOnce)

    loop {
        let temp = read_temperature()
        mqtt::publish(client, "device/telemetry", f"{temp}", QoS::AtMostOnce)

        // Check for incoming commands
        match mqtt::poll(client) {
            Some(msg) => handle_command(msg),
            None => {},
        }

        delay_ms(5000)
    }
}
```

## LoRaWAN

For long-range, low-power IoT:

```fajar
use iot::lorawan

@kernel
fn sensor_node() {
    let config = LoRaConfig {
        region: FreqPlan::EU868,
        class: DeviceClass::A,
        data_rate: DataRate::SF7BW125,
    }

    lorawan::init(config)
    lorawan::join_otaa(dev_eui, app_eui, app_key)

    loop {
        let payload = encode_sensor_data(read_sensors())
        lorawan::send(1, payload, false)  // Port 1, unconfirmed
        lorawan::sleep_until_next_window()
    }
}
```

LoRaWAN features: OTAA join, Class A/B/C, adaptive data rate, 6 frequency plans, duty cycle enforcement, MAC commands.

## OTA Updates

```fajar
use iot::ota

fn check_update() {
    let update = ota::check("https://firmware.example.com/latest")
    match update {
        Some(firmware) => {
            if ota::verify_signature(firmware) {
                ota::apply(firmware)  // Reboots into new firmware
            }
        },
        None => {},  // No update available
    }
}
```
