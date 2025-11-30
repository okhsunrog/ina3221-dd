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

/// INA3221 default I2C address (A0 pin = GND)
/// Other addresses: 0x41 (A0=VS+), 0x42 (A0=SDA), 0x43 (A0=SCL)
pub const INA3221_I2C_ADDR_GND: u8 = 0x40;
pub const INA3221_I2C_ADDR_VS: u8 = 0x41;
pub const INA3221_I2C_ADDR_SDA: u8 = 0x42;
pub const INA3221_I2C_ADDR_SCL: u8 = 0x43;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ChannelId {
    Channel1,
    Channel2,
    Channel3,
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
