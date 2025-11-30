#![cfg_attr(not(any(test, feature = "std")), no_std)]
//! # INA3221 Triple-Channel Current/Voltage Monitor Interface
//!
//! This crate provides a bisync-based driver for the INA3221 triple-channel, high-side
//! current and bus voltage monitor, built upon the `device-driver` crate for robust,
//! declarative register definitions via a YAML manifest. It supports both asynchronous
//! (`async`) and blocking operation through a unified API, using the [`bisync`](https://docs.rs/bisync)
//! crate for seamless compatibility with both `embedded-hal` and `embedded-hal-async` traits.
//!
//! ## Features
//!
//! *   **Declarative Register Map:** Full device configuration defined in `device.yaml`.
//! *   **Unified Async/Blocking Support:** Write your code once and use it in both async and blocking contexts via bisync.
//! *   **Type-Safe API:** High-level functions for reading voltages and currents
//!     and a generated low-level API (`ll`) for direct register access.
//! *   **Triple-Channel Monitoring:** Simultaneously monitor 3 independent power rails.
//! *   **`defmt` and `log` Integration:** Optional support for logging and debugging.
//!
//! ## Getting Started
//!
//! To use the driver, instantiate `Ina3221` (blocking) or `Ina3221Async` (async) with your I2C bus implementation:
//!
//! ```rust,no_run
//! # use embedded_hal::i2c::I2c;
//! # use ina3221_dd::{Ina3221, ChannelId};
//! let i2c_bus = todo!();
//! let mut ina = Ina3221::new(i2c_bus);
//!
//! let bus_voltage = ina.get_bus_voltage(ChannelId::Channel1)?;
//! # Ok::<(), ina3221_dd::Ina3221Error<std::io::Error>>(())
//! ```
//!
//! For async environments, use `Ina3221Async` (re-exported from the `asynchronous` module):
//!
//! ```rust,no_run
//! # use embedded_hal_async::i2c::I2c;
//! # use ina3221_dd::{Ina3221Async, ChannelId};
//! let i2c_bus = todo!();
//! let mut ina = Ina3221Async::new(i2c_bus);
//!
//! let bus_voltage = ina.get_bus_voltage(ChannelId::Channel1).await?;
//! # Ok::<(), ina3221_dd::Ina3221Error<std::io::Error>>(())
//! ```
//!
//! For a detailed register map, please refer to the `device.yaml` file in the
//! [repository](https://github.com/okhsunrog/ina3221-dd).
//!
//! ## Supported Devices
//!
//! The INA3221 is found in various embedded devices for power monitoring,
//! including NVIDIA Jetson boards and other development platforms.
//!

#[macro_use]
pub(crate) mod fmt;

use thiserror::Error;

device_driver::create_device!(device_name: Ina3221LowLevel, manifest: "device.yaml");

// Re-export uom types when feature is enabled
#[cfg(feature = "uom")]
pub use uom::si::electric_current::milliampere;
#[cfg(feature = "uom")]
pub use uom::si::electric_potential::{microvolt, millivolt};
#[cfg(feature = "uom")]
pub use uom::si::electrical_resistance::milliohm;
#[cfg(feature = "uom")]
pub use uom::si::f32::{ElectricCurrent, ElectricPotential, ElectricalResistance};

/// INA3221 default I2C address (A0 pin = GND)
/// Other addresses: 0x41 (A0=VS+), 0x42 (A0=SDA), 0x43 (A0=SCL)
pub const INA3221_I2C_ADDR_GND: u8 = 0x40;
pub const INA3221_I2C_ADDR_VS: u8 = 0x41;
pub const INA3221_I2C_ADDR_SDA: u8 = 0x42;
pub const INA3221_I2C_ADDR_SCL: u8 = 0x43;

/// Shunt voltage LSB: 40µV
pub const SHUNT_VOLTAGE_LSB_UV: f32 = 40.0;
/// Bus voltage LSB: 8mV
pub const BUS_VOLTAGE_LSB_MV: f32 = 8.0;

#[derive(Debug, Error)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Ina3221Error<I2cErr> {
    #[error("I2C error")]
    I2c(I2cErr),
    #[error("Invalid channel")]
    InvalidChannel,
    #[error("Feature or specific mode not supported/implemented: {0}")]
    NotSupported(&'static str),
}

/// Channel identifier for the INA3221's three monitoring channels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelId {
    Channel1,
    Channel2,
    Channel3,
}

/// Operating mode for the INA3221
///
/// Controls whether the device performs continuous measurements, single-shot (triggered),
/// or enters power-down mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum OperatingMode {
    /// Power-down mode - minimal power consumption
    PowerDown = 0,
    /// Shunt voltage only, single-shot (triggered)
    ShuntTriggered = 1,
    /// Bus voltage only, single-shot (triggered)
    BusTriggered = 2,
    /// Shunt and bus voltage, single-shot (triggered)
    ShuntBusTriggered = 3,
    /// Shunt voltage only, continuous
    ShuntContinuous = 5,
    /// Bus voltage only, continuous
    BusContinuous = 6,
    /// Shunt and bus voltage, continuous (default)
    #[default]
    ShuntBusContinuous = 7,
}

impl OperatingMode {
    /// Create from raw register value (bits 2-0)
    pub fn from_raw(value: u8) -> Self {
        match value & 0x07 {
            0 | 4 => Self::PowerDown,
            1 => Self::ShuntTriggered,
            2 => Self::BusTriggered,
            3 => Self::ShuntBusTriggered,
            5 => Self::ShuntContinuous,
            6 => Self::BusContinuous,
            7 => Self::ShuntBusContinuous,
            _ => Self::PowerDown,
        }
    }
}

/// Averaging mode - number of samples to average for each measurement
///
/// Higher averaging reduces noise but increases conversion time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum AveragingMode {
    /// 1 sample (no averaging, default)
    #[default]
    Samples1 = 0,
    /// 4 samples averaged
    Samples4 = 1,
    /// 16 samples averaged
    Samples16 = 2,
    /// 64 samples averaged
    Samples64 = 3,
    /// 128 samples averaged
    Samples128 = 4,
    /// 256 samples averaged
    Samples256 = 5,
    /// 512 samples averaged
    Samples512 = 6,
    /// 1024 samples averaged
    Samples1024 = 7,
}

impl AveragingMode {
    /// Create from raw register value (bits 11-9)
    pub fn from_raw(value: u8) -> Self {
        match value & 0x07 {
            0 => Self::Samples1,
            1 => Self::Samples4,
            2 => Self::Samples16,
            3 => Self::Samples64,
            4 => Self::Samples128,
            5 => Self::Samples256,
            6 => Self::Samples512,
            7 => Self::Samples1024,
            _ => Self::Samples1,
        }
    }

    /// Get the number of samples for this mode
    pub fn sample_count(&self) -> u16 {
        match self {
            Self::Samples1 => 1,
            Self::Samples4 => 4,
            Self::Samples16 => 16,
            Self::Samples64 => 64,
            Self::Samples128 => 128,
            Self::Samples256 => 256,
            Self::Samples512 => 512,
            Self::Samples1024 => 1024,
        }
    }
}

/// Conversion time for ADC measurements
///
/// Longer conversion times provide better accuracy but slower measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum ConversionTime {
    /// 140 µs
    Us140 = 0,
    /// 204 µs
    Us204 = 1,
    /// 332 µs
    Us332 = 2,
    /// 588 µs
    Us588 = 3,
    /// 1.1 ms (default)
    #[default]
    Ms1_1 = 4,
    /// 2.116 ms
    Ms2_116 = 5,
    /// 4.156 ms
    Ms4_156 = 6,
    /// 8.244 ms
    Ms8_244 = 7,
}

impl ConversionTime {
    /// Create from raw register value
    pub fn from_raw(value: u8) -> Self {
        match value & 0x07 {
            0 => Self::Us140,
            1 => Self::Us204,
            2 => Self::Us332,
            3 => Self::Us588,
            4 => Self::Ms1_1,
            5 => Self::Ms2_116,
            6 => Self::Ms4_156,
            7 => Self::Ms8_244,
            _ => Self::Ms1_1,
        }
    }

    /// Get the conversion time in microseconds
    pub fn as_micros(&self) -> u32 {
        match self {
            Self::Us140 => 140,
            Self::Us204 => 204,
            Self::Us332 => 332,
            Self::Us588 => 588,
            Self::Ms1_1 => 1100,
            Self::Ms2_116 => 2116,
            Self::Ms4_156 => 4156,
            Self::Ms8_244 => 8244,
        }
    }
}

/// Alert flags from the Mask/Enable register
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AlertFlags {
    /// Conversion ready flag
    pub conversion_ready: bool,
    /// Timing control alert flag
    pub timing_control: bool,
    /// Power valid alert flag
    pub power_valid: bool,
    /// Channel 1 warning alert flag
    pub warning_ch1: bool,
    /// Channel 2 warning alert flag
    pub warning_ch2: bool,
    /// Channel 3 warning alert flag
    pub warning_ch3: bool,
    /// Summation alert flag
    pub summation: bool,
    /// Channel 1 critical alert flag
    pub critical_ch1: bool,
    /// Channel 2 critical alert flag
    pub critical_ch2: bool,
    /// Channel 3 critical alert flag
    pub critical_ch3: bool,
}

pub struct Ina3221Interface<I2CBus> {
    i2c_bus: I2CBus,
    address: u8,
}

impl<I2CBus> Ina3221Interface<I2CBus> {
    pub fn new(i2c_bus: I2CBus, address: u8) -> Self {
        Self { i2c_bus, address }
    }
}

#[path = "."]
mod asynchronous {
    use bisync::asynchronous::*;
    use device_driver::AsyncRegisterInterface as RegisterInterface;
    use embedded_hal_async::i2c::I2c;
    mod driver;
    pub use driver::*;
}
pub use asynchronous::Ina3221 as Ina3221Async;

#[path = "."]
mod blocking {
    use bisync::synchronous::*;
    use device_driver::RegisterInterface;
    use embedded_hal::i2c::I2c;
    #[allow(clippy::duplicate_mod)]
    mod driver;
    pub use driver::*;
}
pub use blocking::Ina3221;
