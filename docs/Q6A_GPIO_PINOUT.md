# Dragon Q6A GPIO Pinout — Tested Reference

> Verified on Radxa Dragon Q6A (QCS6490), kernel 6.18.2-3-qcom

## GPIO Chips

| Chip | Controller | Lines | Usage |
|------|-----------|-------|-------|
| gpiochip0 | c440000.spmi:pmic@8:gpio@c000 | 12 | PMIC GPIO |
| gpiochip1 | c440000.spmi:pmic@1:gpio@8800 | 10 | PMIC GPIO |
| gpiochip2 | c440000.spmi:pmic@2:gpio@8800 | 9 | PMIC GPIO |
| gpiochip3 | c440000.spmi:pmic@0:gpio@b000 | 4 | PMIC GPIO |
| gpiochip4 | f100000.pinctrl | 176 | **Main SoC GPIO (40-pin header)** |
| gpiochip5 | 33c0000.pinctrl | 15 | Secondary pinctrl |

## 40-Pin Header (gpiochip4)

| Pin | Name | GPIO Line | Function | Status |
|-----|------|-----------|----------|--------|
| 1 | 3V3 | — | Power 3.3V | — |
| 2 | 5V | — | Power 5V | — |
| 3 | SDA | GPIO24 (line 0) | I2C6_SDA | kernel input [used] |
| 4 | 5V | — | Power 5V | — |
| 5 | SCL | GPIO25 (line 1) | I2C6_SCL | kernel input [used] |
| 6 | GND | — | Ground | — |
| 7 | — | GPIO96 | MCLK / General | available |
| 8 | TXD | GPIO22 | UART5_TX | — |
| 9 | GND | — | Ground | — |
| 10 | RXD | GPIO23 | UART5_RX | — |
| 11 | — | GPIO25 | General | blink verified |
| 13 | PIN_13 | GPIO0 (line 0) | General | kernel input [used] |
| 14 | GND | — | Ground | — |
| 15 | PIN_15 | GPIO1 (line 1) | General | kernel input [used] |
| 27 | PIN_27 | GPIO8 (line 8) | I2C2_SDA | kernel input [used] |
| 28 | PIN_28 | GPIO9 (line 9) | I2C2_SCL | kernel input [used] |

## I2C Buses

| Bus | Device | Pins | Status |
|-----|--------|------|--------|
| i2c-0 | /dev/i2c-0 | Internal | available |
| i2c-2 | /dev/i2c-2 | Header 27/28 | available |
| i2c-6 | /dev/i2c-6 | Header 3/5 | available |
| i2c-10 | /dev/i2c-10 | Internal | available |
| i2c-13 | /dev/i2c-13 | Internal | available |
| i2c-20 | /dev/i2c-20 | Internal | available |

## Verified Operations (2026-03-20)

1. **GPIO blink** — `q6a_gpio_blink.fj`: Toggle GPIO96 (simulated), runs on Q6A
2. **GPIO input** — `q6a_gpio_input.fj`: Read/write/debounce/edge detection, runs on Q6A
3. **Thermal sensors** — `q6a_thermal_monitor.fj`: 34 real thermal zones read via sysfs
4. **Sensor logging** — `q6a_sensor_logger.fj`: CSV logging of CPU/GPU/NPU/DDR/memory
5. **Hardware info** — `q6a_hw_info.fj`: CPU freq, memory, NVMe, RTC, GPIO/I2C enumeration

## Thermal Zones (34 total)

| Zone | Type | Typical Temp |
|------|------|-------------|
| 0 | aoss0-thermal | 60°C |
| 1-4 | cpu0-3 (A55) | 58-61°C |
| 5-6 | cpuss0/1 | 61-62°C |
| 7-14 | cpu4-11 (A78) | 59-65°C |
| 16-17 | gpuss0/1 (Adreno 643) | 58-60°C |
| 18-19 | nspss0/1 (Hexagon 770) | 58-59°C |
| 20 | video | 59°C |
| 21 | ddr | 60°C |
| 27 | pm8350c | 37°C |

## Notes

- SPI: Not available (`/dev/spi*` absent)
- PWM: Not available (`/sys/class/pwm/` empty)
- ADC: Not available
- Camera: `/dev/video0`, `/dev/video1` present (needs v4l2-utils)
- RTC: DS1307 at i2c-10 address 0x68 (`rtc-ds1307 10-0068`)
- NVMe: Samsung PM9C1a 256GB (PCIe Gen3 x2)
