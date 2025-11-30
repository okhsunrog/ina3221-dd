use super::{I2c, RegisterInterface, bisync, only_async, only_sync};
use crate::{ChannelId, Ina3221Error, Ina3221Interface, Ina3221LowLevel};

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
            return Err(Ina3221Error::NotSupported("Write data length exceeds buffer"));
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
    /// Get bus voltage for a specific channel in millivolts
    /// INA3221 measures 0-26V range with 8mV LSB resolution
    #[bisync]
    pub async fn get_bus_voltage_mv(&mut self, channel: ChannelId) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = match channel {
            ChannelId::Channel1 => self.ll.channel_1_bus_voltage(),
            ChannelId::Channel2 => self.ll.channel_2_bus_voltage(),
            ChannelId::Channel3 => self.ll.channel_3_bus_voltage(),
        };

        let bus_data = read_internal(&mut op).await?;

        // Extract 12-bit value (bits 14-3), LSB is 8mV
        let raw_value = bus_data.bus_data() as u16;
        let sign = bus_data.sign();

        let voltage_mv = if sign {
            -(raw_value as f32 * 8.0)
        } else {
            raw_value as f32 * 8.0
        };

        Ok(voltage_mv)
    }

    /// Get shunt voltage for a specific channel in microvolts
    /// INA3221 measures ±163.8mV range with 40µV LSB resolution
    #[bisync]
    pub async fn get_shunt_voltage_uv(&mut self, channel: ChannelId) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let mut op = match channel {
            ChannelId::Channel1 => self.ll.channel_1_shunt_voltage(),
            ChannelId::Channel2 => self.ll.channel_2_shunt_voltage(),
            ChannelId::Channel3 => self.ll.channel_3_shunt_voltage(),
        };

        let shunt_data = read_internal(&mut op).await?;

        // Extract 12-bit value (bits 14-3), LSB is 40µV
        let raw_value = shunt_data.shunt_data() as u16;
        let sign = shunt_data.sign();

        let voltage_uv = if sign {
            -(raw_value as f32 * 40.0)
        } else {
            raw_value as f32 * 40.0
        };

        Ok(voltage_uv)
    }

    /// Calculate current in milliamps for a specific channel
    /// shunt_resistor_mohms: Shunt resistor value in milliohms
    #[bisync]
    pub async fn get_current_ma(
        &mut self,
        channel: ChannelId,
        shunt_resistor_mohms: f32
    ) -> Result<f32, Ina3221Error<I2CBusErr>> {
        let shunt_voltage_uv = self.get_shunt_voltage_uv(channel).await?;

        // I = V / R
        // V is in µV, R is in mΩ
        // I = (V_µV / 1000) / (R_mΩ / 1000) = V_µV / R_mΩ mA
        let current_ma = shunt_voltage_uv / shunt_resistor_mohms;

        Ok(current_ma)
    }

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
