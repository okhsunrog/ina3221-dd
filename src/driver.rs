use super::{I2c, RegisterInterface, bisync, only_async, only_sync};
use crate::{
    AlertFlags, AveragingMode, BUS_VOLTAGE_LSB_MV, ChannelId, ConversionTime, Ina3221Error,
    Ina3221Interface, Ina3221LowLevel, OperatingMode, SHUNT_VOLTAGE_LSB_UV,
};

#[bisync]
impl<I2CBus, E> RegisterInterface for Ina3221Interface<I2CBus>
where
    I2CBus: I2c<Error = E>,
    E: core::fmt::Debug,
{
    type AddressType = u8;
    type Error = Ina3221Error<E>;
    async fn read_register(
        &mut self,
        address: u8,
        _size_bits: u32,
        data: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.i2c_bus
            .write_read(self.address, &[address], data)
            .await
            .map_err(Ina3221Error::I2c)
    }
    async fn write_register(
        &mut self,
        address: u8,
        _size_bits: u32,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let mut buffer = [0u8; 5];
        if (1 + data.len()) > buffer.len() {
            return Err(Ina3221Error::NotSupported(
                "Write data length exceeds buffer",
            ));
        }
        buffer[0] = address;
        buffer[1..1 + data.len()].copy_from_slice(data);
        self.i2c_bus
            .write(self.address, &buffer[..1 + data.len()])
            .await
            .map_err(Ina3221Error::I2c)
    }
}

pub struct Ina3221<
    I2CImpl: RegisterInterface<AddressType = u8, Error = Ina3221Error<I2CBusErr>>,
    I2CBusErr: core::fmt::Debug,
> {
    pub ll: Ina3221LowLevel<I2CImpl>,
    _marker: core::marker::PhantomData<I2CBusErr>,
}

impl<I2CBus, E> Ina3221<Ina3221Interface<I2CBus>, E>
where
    I2CBus: I2c<Error = E>,
    E: core::fmt::Debug,
{
    pub fn new(i2c: I2CBus, address: u8) -> Self {
        Self {
            ll: Ina3221LowLevel::new(Ina3221Interface::new(i2c, address)),
            _marker: core::marker::PhantomData,
        }
    }
}

include!("bisync_helpers.rs");

impl<I2CImpl, I2CBusErr> Ina3221<I2CImpl, I2CBusErr>
where
    I2CImpl: RegisterInterface<AddressType = u8, Error = Ina3221Error<I2CBusErr>>,
    I2CBusErr: core::fmt::Debug,
{
    // =========================================================================
    // Voltage and Current Measurements
    // =========================================================================

    /// Get bus voltage for a specific channel in millivolts
    /// INA3221 measures 0-26V range with 8mV LSB resolution
    #[bisync]
    pub async fn get_bus_voltage_mv(
        &mut self,
        channel: ChannelId,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = match channel {
            ChannelId::Channel1 => self.ll.channel_1_bus_voltage(),
            ChannelId::Channel2 => self.ll.channel_2_bus_voltage(),
            ChannelId::Channel3 => self.ll.channel_3_bus_voltage(),
        };

        let bus_data = read_internal(&mut op).await?;

        let raw_value = bus_data.bus_data() as u16;
        let sign = bus_data.sign();

        let voltage_mv = if sign {
            -(raw_value as f32 * BUS_VOLTAGE_LSB_MV)
        } else {
            raw_value as f32 * BUS_VOLTAGE_LSB_MV
        };

        Ok(voltage_mv)
    }

    /// Get shunt voltage for a specific channel in microvolts
    /// INA3221 measures ±163.8mV range with 40µV LSB resolution
    #[bisync]
    pub async fn get_shunt_voltage_uv(
        &mut self,
        channel: ChannelId,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = match channel {
            ChannelId::Channel1 => self.ll.channel_1_shunt_voltage(),
            ChannelId::Channel2 => self.ll.channel_2_shunt_voltage(),
            ChannelId::Channel3 => self.ll.channel_3_shunt_voltage(),
        };

        let shunt_data = read_internal(&mut op).await?;

        let raw_value = shunt_data.shunt_data() as u16;
        let sign = shunt_data.sign();

        let voltage_uv = if sign {
            -(raw_value as f32 * SHUNT_VOLTAGE_LSB_UV)
        } else {
            raw_value as f32 * SHUNT_VOLTAGE_LSB_UV
        };

        Ok(voltage_uv)
    }

    /// Calculate current in milliamps for a specific channel
    /// shunt_resistor_mohms: Shunt resistor value in milliohms
    #[bisync]
    pub async fn get_current_ma(
        &mut self,
        channel: ChannelId,
        shunt_resistor_mohms: f32,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let shunt_voltage_uv = self.get_shunt_voltage_uv(channel).await?;
        // I = V / R; V in µV, R in mΩ → I in mA
        Ok(shunt_voltage_uv / shunt_resistor_mohms)
    }

    /// Calculate power in milliwatts for a specific channel
    /// shunt_resistor_mohms: Shunt resistor value in milliohms
    #[bisync]
    pub async fn get_power_mw(
        &mut self,
        channel: ChannelId,
        shunt_resistor_mohms: f32,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let bus_voltage_mv = self.get_bus_voltage_mv(channel).await?;
        let current_ma = self.get_current_ma(channel, shunt_resistor_mohms).await?;
        // P = V * I; V in mV, I in mA → P in µW, divide by 1000 for mW
        Ok((bus_voltage_mv * current_ma) / 1000.0)
    }

    // =========================================================================
    // Channel Configuration
    // =========================================================================

    /// Enable or disable a specific channel
    #[bisync]
    pub async fn set_channel_enable(
        &mut self,
        channel: ChannelId,
        enable: bool,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| match channel {
            ChannelId::Channel1 => r.set_ch_1_enable(enable),
            ChannelId::Channel2 => r.set_ch_2_enable(enable),
            ChannelId::Channel3 => r.set_ch_3_enable(enable),
        })
        .await
    }

    /// Check if a specific channel is enabled
    #[bisync]
    pub async fn is_channel_enabled(
        &mut self,
        channel: ChannelId,
    ) -> Result<bool, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        let config = read_internal(&mut op).await?;
        Ok(match channel {
            ChannelId::Channel1 => config.ch_1_enable(),
            ChannelId::Channel2 => config.ch_2_enable(),
            ChannelId::Channel3 => config.ch_3_enable(),
        })
    }

    /// Enable or disable all channels at once
    #[bisync]
    pub async fn set_all_channels_enable(
        &mut self,
        ch1: bool,
        ch2: bool,
        ch3: bool,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| {
            r.set_ch_1_enable(ch1);
            r.set_ch_2_enable(ch2);
            r.set_ch_3_enable(ch3);
        })
        .await
    }

    // =========================================================================
    // Operating Mode Configuration
    // =========================================================================

    /// Get the current operating mode
    #[bisync]
    pub async fn get_operating_mode(&mut self) -> Result<OperatingMode, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        let config = read_internal(&mut op).await?;
        Ok(OperatingMode::from_raw(config.operating_mode() as u8))
    }

    /// Set the operating mode
    #[bisync]
    pub async fn set_operating_mode(
        &mut self,
        mode: OperatingMode,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| r.set_operating_mode(mode as u8)).await
    }

    /// Get the averaging mode
    #[bisync]
    pub async fn get_averaging_mode(&mut self) -> Result<AveragingMode, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        let config = read_internal(&mut op).await?;
        Ok(AveragingMode::from_raw(config.averaging_mode() as u8))
    }

    /// Set the averaging mode (number of samples to average)
    #[bisync]
    pub async fn set_averaging_mode(
        &mut self,
        mode: AveragingMode,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| r.set_averaging_mode(mode as u8)).await
    }

    /// Get the bus voltage conversion time
    #[bisync]
    pub async fn get_vbus_conversion_time(
        &mut self,
    ) -> Result<ConversionTime, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        let config = read_internal(&mut op).await?;
        Ok(ConversionTime::from_raw(config.vbus_conversion_time() as u8))
    }

    /// Set the bus voltage conversion time
    #[bisync]
    pub async fn set_vbus_conversion_time(
        &mut self,
        time: ConversionTime,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| r.set_vbus_conversion_time(time as u8)).await
    }

    /// Get the shunt voltage conversion time
    #[bisync]
    pub async fn get_vshunt_conversion_time(
        &mut self,
    ) -> Result<ConversionTime, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        let config = read_internal(&mut op).await?;
        Ok(ConversionTime::from_raw(
            config.vshunt_conversion_time() as u8
        ))
    }

    /// Set the shunt voltage conversion time
    #[bisync]
    pub async fn set_vshunt_conversion_time(
        &mut self,
        time: ConversionTime,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| r.set_vshunt_conversion_time(time as u8)).await
    }

    // =========================================================================
    // Alert Limits
    // =========================================================================

    /// Get the critical alert limit for a channel in microvolts
    #[bisync]
    pub async fn get_critical_alert_limit_uv(
        &mut self,
        channel: ChannelId,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let raw = match channel {
            ChannelId::Channel1 => {
                let mut op = self.ll.channel_1_critical_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
            ChannelId::Channel2 => {
                let mut op = self.ll.channel_2_critical_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
            ChannelId::Channel3 => {
                let mut op = self.ll.channel_3_critical_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
        };
        Ok(raw as f32 * SHUNT_VOLTAGE_LSB_UV)
    }

    /// Set the critical alert limit for a channel in microvolts
    #[bisync]
    pub async fn set_critical_alert_limit_uv(
        &mut self,
        channel: ChannelId,
        limit_uv: f32,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let raw = (limit_uv / SHUNT_VOLTAGE_LSB_UV) as u16;
        match channel {
            ChannelId::Channel1 => {
                let mut op = self.ll.channel_1_critical_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
            ChannelId::Channel2 => {
                let mut op = self.ll.channel_2_critical_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
            ChannelId::Channel3 => {
                let mut op = self.ll.channel_3_critical_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
        }
    }

    /// Get the warning alert limit for a channel in microvolts
    #[bisync]
    pub async fn get_warning_alert_limit_uv(
        &mut self,
        channel: ChannelId,
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let raw = match channel {
            ChannelId::Channel1 => {
                let mut op = self.ll.channel_1_warning_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
            ChannelId::Channel2 => {
                let mut op = self.ll.channel_2_warning_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
            ChannelId::Channel3 => {
                let mut op = self.ll.channel_3_warning_alert_limit();
                read_internal(&mut op).await?.limit_data()
            }
        };
        Ok(raw as f32 * SHUNT_VOLTAGE_LSB_UV)
    }

    /// Set the warning alert limit for a channel in microvolts
    #[bisync]
    pub async fn set_warning_alert_limit_uv(
        &mut self,
        channel: ChannelId,
        limit_uv: f32,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let raw = (limit_uv / SHUNT_VOLTAGE_LSB_UV) as u16;
        match channel {
            ChannelId::Channel1 => {
                let mut op = self.ll.channel_1_warning_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
            ChannelId::Channel2 => {
                let mut op = self.ll.channel_2_warning_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
            ChannelId::Channel3 => {
                let mut op = self.ll.channel_3_warning_alert_limit();
                modify_internal(&mut op, |r| r.set_limit_data(raw)).await
            }
        }
    }

    /// Get the power-valid upper and lower limits in millivolts
    #[bisync]
    pub async fn get_power_valid_limits_mv(
        &mut self,
    ) -> Result<(f32, f32), Ina3221Error<I2CBusErr>> {
        let mut op_upper = self.ll.power_valid_upper_limit();
        let upper = read_internal(&mut op_upper).await?.limit_data();

        let mut op_lower = self.ll.power_valid_lower_limit();
        let lower = read_internal(&mut op_lower).await?.limit_data();

        Ok((
            lower as f32 * BUS_VOLTAGE_LSB_MV,
            upper as f32 * BUS_VOLTAGE_LSB_MV,
        ))
    }

    /// Set the power-valid upper and lower limits in millivolts
    #[bisync]
    pub async fn set_power_valid_limits_mv(
        &mut self,
        lower_mv: f32,
        upper_mv: f32,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let lower_raw = (lower_mv / BUS_VOLTAGE_LSB_MV) as u16;
        let upper_raw = (upper_mv / BUS_VOLTAGE_LSB_MV) as u16;

        let mut op_lower = self.ll.power_valid_lower_limit();
        modify_internal(&mut op_lower, |r| r.set_limit_data(lower_raw)).await?;

        let mut op_upper = self.ll.power_valid_upper_limit();
        modify_internal(&mut op_upper, |r| r.set_limit_data(upper_raw)).await
    }

    // =========================================================================
    // Alert Configuration
    // =========================================================================

    /// Enable or disable critical alert latch mode
    #[bisync]
    pub async fn set_critical_alert_latch(
        &mut self,
        enable: bool,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.mask_enable();
        modify_internal(&mut op, |r| r.set_critical_alert_enable(enable)).await
    }

    /// Enable or disable warning alert latch mode
    #[bisync]
    pub async fn set_warning_alert_latch(
        &mut self,
        enable: bool,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.mask_enable();
        modify_internal(&mut op, |r| r.set_warning_alert_enable(enable)).await
    }

    /// Read alert flags (clears flags on read)
    #[bisync]
    pub async fn get_alert_flags(&mut self) -> Result<AlertFlags, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.mask_enable();
        let reg = read_internal(&mut op).await?;
        Ok(AlertFlags {
            conversion_ready: reg.conversion_ready_flag(),
            timing_control: reg.timing_control_flag(),
            power_valid: reg.power_valid_flag(),
            warning_ch1: reg.warning_flag_ch_1(),
            warning_ch2: reg.warning_flag_ch_2(),
            warning_ch3: reg.warning_flag_ch_3(),
            summation: reg.summation_flag(),
            critical_ch1: reg.critical_flag_ch_1(),
            critical_ch2: reg.critical_flag_ch_2(),
            critical_ch3: reg.critical_flag_ch_3(),
        })
    }

    /// Check if conversion is ready
    #[bisync]
    pub async fn is_conversion_ready(&mut self) -> Result<bool, Ina3221Error<I2CBusErr>> {
        let flags = self.get_alert_flags().await?;
        Ok(flags.conversion_ready)
    }

    // =========================================================================
    // Summation Control
    // =========================================================================

    /// Enable or disable a channel in the shunt voltage summation
    #[bisync]
    pub async fn set_summation_channel_enable(
        &mut self,
        channel: ChannelId,
        enable: bool,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.mask_enable();
        modify_internal(&mut op, |r| match channel {
            ChannelId::Channel1 => r.set_sum_control_ch_1(enable),
            ChannelId::Channel2 => r.set_sum_control_ch_2(enable),
            ChannelId::Channel3 => r.set_sum_control_ch_3(enable),
        })
        .await
    }

    /// Get the shunt voltage sum in microvolts
    #[bisync]
    pub async fn get_shunt_voltage_sum_uv(&mut self) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.shunt_voltage_sum();
        let data = read_internal(&mut op).await?;
        let raw = data.sum_data() as u16;
        let sign = data.sign();

        let voltage_uv = if sign {
            -(raw as f32 * SHUNT_VOLTAGE_LSB_UV)
        } else {
            raw as f32 * SHUNT_VOLTAGE_LSB_UV
        };
        Ok(voltage_uv)
    }

    /// Get the shunt voltage sum limit in microvolts
    #[bisync]
    pub async fn get_shunt_voltage_sum_limit_uv(&mut self) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.shunt_voltage_sum_limit();
        let data = read_internal(&mut op).await?;
        Ok(data.limit_data() as f32 * SHUNT_VOLTAGE_LSB_UV)
    }

    /// Set the shunt voltage sum limit in microvolts
    #[bisync]
    pub async fn set_shunt_voltage_sum_limit_uv(
        &mut self,
        limit_uv: f32,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let raw = (limit_uv / SHUNT_VOLTAGE_LSB_UV) as u16;
        let mut op = self.ll.shunt_voltage_sum_limit();
        modify_internal(&mut op, |r| r.set_limit_data(raw)).await
    }

    // =========================================================================
    // Device Info and Control
    // =========================================================================

    /// Get the manufacturer ID (should be 0x5449 = "TI")
    #[bisync]
    pub async fn get_manufacturer_id(&mut self) -> Result<u16, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.manufacturer_id();
        let id = read_internal(&mut op).await?;
        Ok(id.manufacturer_id() as u16)
    }

    /// Get the die ID (should be 0x3220)
    #[bisync]
    pub async fn get_die_id(&mut self) -> Result<u16, Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.die_id();
        let id = read_internal(&mut op).await?;
        Ok(id.die_id() as u16)
    }

    /// Perform software reset
    #[bisync]
    pub async fn reset(&mut self) -> Result<(), Ina3221Error<I2CBusErr>> {
        let mut op = self.ll.configuration();
        modify_internal(&mut op, |r| r.set_reset(true)).await
    }
}

// =============================================================================
// UOM-based API (behind feature flag)
// =============================================================================

#[cfg(feature = "uom")]
impl<I2CImpl, I2CBusErr> Ina3221<I2CImpl, I2CBusErr>
where
    I2CImpl: RegisterInterface<AddressType = u8, Error = Ina3221Error<I2CBusErr>>,
    I2CBusErr: core::fmt::Debug,
{
    /// Get bus voltage for a specific channel as ElectricPotential
    #[bisync]
    pub async fn get_bus_voltage(
        &mut self,
        channel: ChannelId,
    ) -> Result<crate::ElectricPotential, Ina3221Error<I2CBusErr>> {
        let mv = self.get_bus_voltage_mv(channel).await?;
        Ok(crate::ElectricPotential::new::<crate::millivolt>(mv))
    }

    /// Get shunt voltage for a specific channel as ElectricPotential
    #[bisync]
    pub async fn get_shunt_voltage(
        &mut self,
        channel: ChannelId,
    ) -> Result<crate::ElectricPotential, Ina3221Error<I2CBusErr>> {
        let uv = self.get_shunt_voltage_uv(channel).await?;
        Ok(crate::ElectricPotential::new::<crate::microvolt>(uv))
    }

    /// Get current for a specific channel as ElectricCurrent
    #[bisync]
    pub async fn get_current(
        &mut self,
        channel: ChannelId,
        shunt_resistor: crate::ElectricalResistance,
    ) -> Result<crate::ElectricCurrent, Ina3221Error<I2CBusErr>> {
        let shunt_mohms = shunt_resistor.get::<crate::milliohm>();
        let ma = self.get_current_ma(channel, shunt_mohms).await?;
        Ok(crate::ElectricCurrent::new::<crate::milliampere>(ma))
    }

    /// Set critical alert limit for a channel using ElectricPotential
    #[bisync]
    pub async fn set_critical_alert_limit(
        &mut self,
        channel: ChannelId,
        limit: crate::ElectricPotential,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let uv = limit.get::<crate::microvolt>();
        self.set_critical_alert_limit_uv(channel, uv).await
    }

    /// Set warning alert limit for a channel using ElectricPotential
    #[bisync]
    pub async fn set_warning_alert_limit(
        &mut self,
        channel: ChannelId,
        limit: crate::ElectricPotential,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let uv = limit.get::<crate::microvolt>();
        self.set_warning_alert_limit_uv(channel, uv).await
    }

    /// Set power-valid limits using ElectricPotential
    #[bisync]
    pub async fn set_power_valid_limits(
        &mut self,
        lower: crate::ElectricPotential,
        upper: crate::ElectricPotential,
    ) -> Result<(), Ina3221Error<I2CBusErr>> {
        let lower_mv = lower.get::<crate::millivolt>();
        let upper_mv = upper.get::<crate::millivolt>();
        self.set_power_valid_limits_mv(lower_mv, upper_mv).await
    }
}
