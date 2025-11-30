# INA3221 Triple-Channel Current/Voltage Monitor Driver (ina3221-dd)

[![Crates.io](https://img.shields.io/crates/v/ina3221-dd.svg)](https://crates.io/crates/ina3221-dd)
[![Docs.rs](https://docs.rs/ina3221-dd/badge.svg)](https://docs.rs/ina3221-dd)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses)
[![Build Status](https://img.shields.io/github/actions/workflow/status/okhsunrog/ina3221-dd/rust_ci.yml?logo=github)](https://github.com/okhsunrog/ina3221-dd/actions/workflows/rust_ci.yml)

A `no_std` Rust driver for the Texas Instruments INA3221 triple-channel, high-side current and bus voltage monitor. This driver leverages the `device-driver` crate with a declarative YAML manifest (`device.yaml`) for a robust, type-safe register map definition. The low-level API covers 100% of the INA3221's registers, with `device.yaml` providing a comprehensive and accurate description of all registers and their fields verified against the official datasheet.

## Overview

The `ina3221-dd` driver offers:

- **Declarative Configuration:** The INA3221 register map is defined in [`device.yaml`](device.yaml), enabling `device-driver` to generate a type-safe, low-level register access API.
- **Unified Async/Blocking API:** Uses the [`bisync`](https://github.com/JM4ier/bisync) crate to provide both asynchronous (`Ina3221Async`) and blocking (`Ina3221`) drivers from the same codebase, with no feature flags required.
- **High-Level and Low-Level APIs:**
  - High-level methods simplify tasks like reading bus/shunt voltages and calculating current.
  - Low-level API (via the `ll` field) offers direct, type-safe access to all registers defined in `device.yaml`.
- **Triple-Channel Monitoring:** Monitor three independent power rails simultaneously.
- **`no_std` and `no-alloc`:** Optimized for bare-metal and RTOS environments.
- **Optional Logging:** Supports `defmt` and the `log` facade for debugging.

## Features

- **Three Independent Channels:** Monitor voltage and current on three separate power rails.
- **Bus Voltage Measurement:** 0V to 26V range with 8mV resolution.
- **Shunt Voltage Measurement:** ±163.8mV range with 40µV resolution.
- **Current Calculation:** Derive current from shunt voltage and known shunt resistor value.
- **Configurable Averaging:** 1 to 1024 samples for noise reduction.
- **Configurable Conversion Time:** 140µs to 8.244ms per channel.
- **Multiple Operating Modes:** Continuous, single-shot, or power-down.
- **Alert Functions:**
  - Critical and warning alert limits per channel.
  - Summation alert for combined channel monitoring.
  - Power-valid detection for supply sequencing.
- **Four I2C Addresses:** 0x40, 0x41, 0x42, 0x43 (configurable via A0 pin).
- **Device Identification:** Read manufacturer ID (0x5449 = "TI") and die ID (0x3220).

## Getting Started

1. **Add `ina3221-dd` to `Cargo.toml`:**

   ```toml
   [dependencies]
   ina3221-dd = "0.1.0"
   # For blocking usage (Ina3221):
   embedded-hal = "1.0.0"
   # For async usage (Ina3221Async):
   embedded-hal-async = "1.0.0"
   ```

2. **Instantiate the driver with your I2C bus:**

   - **Blocking:**
     ```rust
     use ina3221_dd::{Ina3221, ChannelId, INA3221_I2C_ADDR_GND};

     let i2c_bus = /* your I2C bus */;
     let mut ina = Ina3221::new(i2c_bus, INA3221_I2C_ADDR_GND);

     // Read bus voltage (in millivolts)
     let bus_voltage = ina.get_bus_voltage_mv(ChannelId::Channel1)?;

     // Read shunt voltage (in microvolts)
     let shunt_voltage = ina.get_shunt_voltage_uv(ChannelId::Channel1)?;

     // Calculate current (with 100mΩ shunt resistor)
     let current_ma = ina.get_current_ma(ChannelId::Channel1, 100.0)?;

     // Verify device identity
     let manufacturer_id = ina.get_manufacturer_id()?; // Should be 0x5449
     let die_id = ina.get_die_id()?; // Should be 0x3220
     ```

   - **Async:**
     ```rust
     use ina3221_dd::{Ina3221Async, ChannelId, INA3221_I2C_ADDR_GND};

     let i2c_bus = /* your async I2C bus */;
     let mut ina = Ina3221Async::new(i2c_bus, INA3221_I2C_ADDR_GND);

     // Read bus voltage (in millivolts)
     let bus_voltage = ina.get_bus_voltage_mv(ChannelId::Channel1).await?;

     // Read shunt voltage (in microvolts)
     let shunt_voltage = ina.get_shunt_voltage_uv(ChannelId::Channel1).await?;

     // Calculate current (with 100mΩ shunt resistor)
     let current_ma = ina.get_current_ma(ChannelId::Channel1, 100.0).await?;
     ```

## I2C Addresses

The INA3221 supports four I2C addresses based on the A0 pin connection:

| A0 Pin Connection | I2C Address | Constant |
|-------------------|-------------|----------|
| GND | 0x40 | `INA3221_I2C_ADDR_GND` |
| VS | 0x41 | `INA3221_I2C_ADDR_VS` |
| SDA | 0x42 | `INA3221_I2C_ADDR_SDA` |
| SCL | 0x43 | `INA3221_I2C_ADDR_SCL` |

## Low-Level API Usage

The driver provides direct access to all INA3221 registers through the low-level API via `ina.ll`. This API is automatically generated from [`device.yaml`](device.yaml) and provides type-safe access to all register fields.

### Reading Registers

```rust
// Read configuration register
let config = ina.ll.configuration().read()?;
let ch1_enabled = config.ch_1_enable();
let averaging = config.averaging_mode();
let operating_mode = config.operating_mode();

// Read raw shunt voltage register
let shunt = ina.ll.channel_1_shunt_voltage().read()?;
let sign = shunt.sign();
let data = shunt.shunt_data();

// Read manufacturer ID
let mfr_id = ina.ll.manufacturer_id().read()?;
```

### Writing Registers

```rust
// Configure the device
ina.ll.configuration().write(|w| {
    w.set_ch_1_enable(true);
    w.set_ch_2_enable(true);
    w.set_ch_3_enable(false);
    w.set_averaging_mode(2); // 16 samples
    w.set_vbus_conversion_time(4); // 1.1ms
    w.set_vshunt_conversion_time(4); // 1.1ms
    w.set_operating_mode(7); // Continuous shunt and bus
})?;

// Set alert limits
ina.ll.channel_1_critical_alert_limit().write(|w| {
    w.set_limit_data(0x1000);
})?;
```

### Modifying Registers

Use `.modify()` to read-modify-write, preserving other fields:

```rust
// Change only the averaging mode, preserve other settings
ina.ll.configuration().modify(|w| {
    w.set_averaging_mode(3); // 64 samples
})?;
```

### Async Low-Level API

Append `_async` to method names for async usage:

```rust
let config = ina.ll.configuration().read_async().await?;

ina.ll.configuration().modify_async(|w| {
    w.set_averaging_mode(4); // 128 samples
}).await?;
```

## Register Map

The complete INA3221 register map is defined in [`device.yaml`](device.yaml):

| Address | Register | Description |
|---------|----------|-------------|
| 0x00 | Configuration | Operating modes, conversion times, averaging |
| 0x01-0x06 | Channel Voltages | Shunt and bus voltage for channels 1-3 |
| 0x07-0x0C | Alert Limits | Critical and warning limits per channel |
| 0x0D | Shunt Voltage Sum | Sum of enabled channel shunt voltages |
| 0x0E | Shunt Voltage Sum Limit | Limit for summed shunt voltage |
| 0x0F | Mask/Enable | Alert configuration and status flags |
| 0x10-0x11 | Power Valid Limits | Upper and lower limits for power-valid detection |
| 0xFE | Manufacturer ID | Returns 0x5449 ("TI") |
| 0xFF | Die ID | Returns 0x3220 |

## Examples

Examples for ESP32 using `esp-hal` are included. Both examples demonstrate high-level convenience methods and low-level register API usage.

- **Async Example:** [`examples/test_ina3221_async.rs`](examples/test_ina3221_async.rs)
  ```bash
  cargo run --release --example test_ina3221_async --features defmt
  ```
- **Blocking Example:** [`examples/test_ina3221_blocking.rs`](examples/test_ina3221_blocking.rs)
  ```bash
  cargo run --release --example test_ina3221_blocking --features defmt
  ```

## Feature Flags

- **`default = []`**: No default features; async and blocking drivers are always available.
- **`std`**: Enables `std` features for `thiserror`.
- **`log`**: Enables `log` facade logging.
- **`defmt`**: Enables `defmt` logging for embedded debugging.

## Current Calculation

The INA3221 measures shunt voltage across an external shunt resistor. To calculate current:

```
Current (A) = Shunt Voltage (V) / Shunt Resistance (Ω)
```

The driver's `get_current_ma()` method handles this calculation:

```rust
// With a 100mΩ (0.1Ω) shunt resistor
let current_ma = ina.get_current_ma(ChannelId::Channel1, 100.0)?;

// With a 10mΩ (0.01Ω) shunt resistor for higher currents
let current_ma = ina.get_current_ma(ChannelId::Channel1, 10.0)?;
```

## Contributing

Contributions are welcome! You can contribute by:

- Adding high-level convenience methods for additional features.
- Enhancing documentation with examples or clarifications.
- Reporting issues or suggesting improvements.
- Testing on different hardware platforms.

Please submit issues, fork the repository, and create pull requests.

## License

This project is dual-licensed under the [MIT License](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE), at your option.
