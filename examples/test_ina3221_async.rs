#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    Async,
    i2c::master::{Config as I2cConfig, Error as I2cError, I2c},
    interrupt::software::SoftwareInterruptControl,
    time::Rate,
    timer::timg::TimerGroup,
};
use ina3221_dd::{ChannelId, INA3221_I2C_ADDR_GND, Ina3221Async, Ina3221Error};
use panic_rtt_target as _;
use rtt_target::rtt_init_defmt;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    rtt_init_defmt!();
    info!("Init!");

    let p = esp_hal::init(esp_hal::Config::default());

    let timg0 = TimerGroup::new(p.TIMG0);
    let sw_ints = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_ints.software_interrupt0);

    let config: I2cConfig = I2cConfig::default().with_frequency(Rate::from_khz(400));
    let i2c = I2c::new(p.I2C0, config)
        .unwrap()
        .with_sda(p.GPIO6)
        .with_scl(p.GPIO7)
        .into_async();

    test_ina3221(i2c).await.unwrap();

    loop {
        info!("Hello world!");
        Timer::after(Duration::from_secs(1)).await;
    }
}

#[rustfmt::skip]
async fn test_ina3221(i2c: I2c<'_, Async>) -> Result<(), Ina3221Error<I2cError>> {
    // Create INA3221 instance with default I2C address (0x40)
    let mut ina = Ina3221Async::new(i2c, INA3221_I2C_ADDR_GND);

    info!("=== High-Level API Examples ===");

    // Verify device identity
    let manufacturer_id = ina.get_manufacturer_id().await?;
    info!("Manufacturer ID: 0x{:04X} (should be 0x5449 = 'TI')", manufacturer_id);

    let die_id = ina.get_die_id().await?;
    info!("Die ID: 0x{:04X} (should be 0x3220)", die_id);

    // Enable all three channels
    ina.set_channel_enable(ChannelId::Channel1, true).await?;
    ina.set_channel_enable(ChannelId::Channel2, true).await?;
    ina.set_channel_enable(ChannelId::Channel3, true).await?;
    info!("All channels enabled");

    // Read bus voltages from all channels
    let bus_v1 = ina.get_bus_voltage_mv(ChannelId::Channel1).await?;
    let bus_v2 = ina.get_bus_voltage_mv(ChannelId::Channel2).await?;
    let bus_v3 = ina.get_bus_voltage_mv(ChannelId::Channel3).await?;
    info!("Bus voltages: Ch1={} mV, Ch2={} mV, Ch3={} mV", bus_v1, bus_v2, bus_v3);

    // Read shunt voltages from all channels
    let shunt_v1 = ina.get_shunt_voltage_uv(ChannelId::Channel1).await?;
    let shunt_v2 = ina.get_shunt_voltage_uv(ChannelId::Channel2).await?;
    let shunt_v3 = ina.get_shunt_voltage_uv(ChannelId::Channel3).await?;
    info!("Shunt voltages: Ch1={} µV, Ch2={} µV, Ch3={} µV", shunt_v1, shunt_v2, shunt_v3);

    // Calculate current (assuming 100 mΩ shunt resistors)
    const SHUNT_RESISTOR_MOHMS: f32 = 100.0;
    let current1 = ina.get_current_ma(ChannelId::Channel1, SHUNT_RESISTOR_MOHMS).await?;
    let current2 = ina.get_current_ma(ChannelId::Channel2, SHUNT_RESISTOR_MOHMS).await?;
    let current3 = ina.get_current_ma(ChannelId::Channel3, SHUNT_RESISTOR_MOHMS).await?;
    info!("Currents: Ch1={} mA, Ch2={} mA, Ch3={} mA", current1, current2, current3);

    info!("=== Low-Level API Examples ===");

    // Read configuration register using low-level API
    let config = ina.ll.configuration().read_async().await?;
    info!("Channel enables: Ch1={}, Ch2={}, Ch3={}",
          config.ch_1_enable(), config.ch_2_enable(), config.ch_3_enable());
    info!("Operating mode: {}", config.operating_mode());

    // Read raw bus voltage register using low-level API
    let bus_raw = ina.ll.channel_1_bus_voltage().read_async().await?;
    info!("Channel 1 raw bus voltage: sign={}, data={}",
          bus_raw.sign(), bus_raw.bus_data());

    // Read raw shunt voltage register using low-level API
    let shunt_raw = ina.ll.channel_1_shunt_voltage().read_async().await?;
    info!("Channel 1 raw shunt voltage: sign={}, data={}",
          shunt_raw.sign(), shunt_raw.shunt_data());

    // Modify configuration using low-level API
    ina.ll.configuration().modify_async(|w| {
        w.set_averaging_mode(1); // 4 samples averaging
    }).await?;
    info!("Averaging mode set to 4 samples via LL API");

    Ok(())
}
