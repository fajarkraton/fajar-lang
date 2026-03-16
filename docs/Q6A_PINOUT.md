# Dragon Q6A 40-Pin Header Reference

> GPIO chip: `/dev/gpiochip4` | Voltage: 3.3V | 27 usable GPIO pins

---

## Pin Map

```
                    Dragon Q6A 40-Pin Header
                    ========================

         3.3V [ 1] [ 2] 5V
   I2C3_SDA/6 [ 3] [ 4] 5V
   I2C3_SCL/7 [ 5] [ 6] GND
       GPIO/8 [ 7] [ 8] UART7_TX/0    (GPIO 0)
          GND [ 9] [10] UART7_RX/1    (GPIO 1)
      GPIO/13 [11] [12] I2S0_SCLK/15
      GPIO/14 [13] [14] GND
      GPIO/16 [15] [16] GPIO/17
         3.3V [17] [18] GPIO/18
SPI1_MOSI/19  [19] [20] GND
SPI1_MISO/20  [21] [22] GPIO/21
SPI1_CLK/22   [23] [24] SPI1_CS0/23
          GND [25] [26] GPIO/24
 I2C2_SDA/25  [27] [28] I2C2_SCL/26
      GPIO/27 [29] [30] GND
      GPIO/28 [31] [32] GPIO/29
      GPIO/30 [33] [34] GND
I2S0_LRCK/31  [35] [36] GPIO/32
      GPIO/33 [37] [38] I2S0_SDI/34
          GND [39] [40] I2S0_SDO/35
```

## Function Groups

### UART (7 available)

| UART | TX Pin | RX Pin | Notes |
|------|--------|--------|-------|
| UART7 | Pin 8 (GPIO 0) | Pin 10 (GPIO 1) | Default console |
| UART1-6 | Various | Various | See device tree |

### I2C (2 available on header)

| Bus | SDA Pin | SCL Pin | Speed |
|-----|---------|---------|-------|
| I2C2 | Pin 27 (GPIO 25) | Pin 28 (GPIO 26) | 400 kHz |
| I2C3 | Pin 3 (GPIO 6) | Pin 5 (GPIO 7) | 400 kHz |

### SPI (1 available on header)

| Bus | MOSI | MISO | CLK | CS0 |
|-----|------|------|-----|-----|
| SPI1 | Pin 19 (GPIO 19) | Pin 21 (GPIO 20) | Pin 23 (GPIO 22) | Pin 24 (GPIO 23) |

### I2S Audio

| Signal | Pin |
|--------|-----|
| SCLK | Pin 12 (GPIO 15) |
| LRCK | Pin 35 (GPIO 31) |
| SDI | Pin 38 (GPIO 34) |
| SDO | Pin 40 (GPIO 35) |

### General Purpose GPIO

| Pin | GPIO # | Default | Notes |
|-----|--------|---------|-------|
| 7 | 8 | Input | General purpose |
| 11 | 13 | Input | General purpose |
| 13 | 14 | Input | General purpose |
| 15 | 16 | Input | General purpose |
| 16 | 17 | Input | General purpose |
| 18 | 18 | Input | General purpose |
| 22 | 21 | Input | General purpose |
| 29 | 27 | Input | General purpose |
| 31 | 28 | Input | General purpose |
| 32 | 29 | Input | General purpose |
| 33 | 30 | Input | General purpose |
| 36 | 32 | Input | General purpose |
| 37 | 33 | Input | General purpose |

## Usage from Fajar Lang

```fajar
// Set GPIO 8 (pin 7) as output, drive high
gpio_setup(8, 1)        // 1 = output
gpio_write(8, 1)        // drive high
sleep_ms(1000)
gpio_write(8, 0)        // drive low

// Read GPIO 13 (pin 11) as input
gpio_setup(13, 0)       // 0 = input
let level = gpio_read(13)
println(f"Pin 11 level: {level}")
```

## Shell Commands

```bash
# List GPIO lines
gpioinfo gpiochip4

# Set GPIO 8 high
gpioset gpiochip4 8=1

# Read GPIO 13
gpioget gpiochip4 13
```

---

*Dragon Q6A Pinout Reference v1.0 | 2026-03-16*
