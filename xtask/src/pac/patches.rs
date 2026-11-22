use anyhow::Result;
use svd_rs::{Access, Device};

use super::svd::{
    Bits, Register, Window, add_field, add_register, narrow, peripheral, register, resize,
    window_peripheral,
};

pub(super) fn gd32f425(device: &mut Device) -> Result<()> {
    let fmc = peripheral(device, "FMC")?;
    add_register(
        fmc,
        &Register {
            name: "PECFG",
            description: "Page erase configuration register",
            offset: 0x20,
            size: 32,
            access: Access::ReadWrite,
            fields: &[
                Bits {
                    name: "PE_EN",
                    description: "Page erase function enable",
                    offset: 31,
                    width: 1,
                },
                Bits {
                    name: "PE_ADDR",
                    description: "Page erase address",
                    offset: 0,
                    width: 29,
                },
            ],
        },
    )?;
    add_register(
        fmc,
        &Register {
            name: "PEKEY",
            description: "Page erase unlock key register",
            offset: 0x24,
            size: 32,
            access: Access::WriteOnly,
            fields: &[Bits {
                name: "PE_KEY",
                description: "Page erase unlock key",
                offset: 0,
                width: 32,
            }],
        },
    )?;

    let global = peripheral(device, "HS_GLOBAL")?;
    add_register(
        global,
        &Register {
            name: "GSNPSID",
            description: "Core identification register",
            offset: 0x40,
            size: 32,
            access: Access::ReadOnly,
            fields: &[Bits {
                name: "ID",
                description: "Core identification",
                offset: 0,
                width: 32,
            }],
        },
    )?;

    device.peripherals.push(window_peripheral(&Window {
        peripheral: "HS_FIFO",
        description: "USBHS data FIFO access windows",
        base: 0x4004_1000,
        register: "FIFO",
        register_description: "Data FIFO access window of endpoint or channel",
        count: 6,
        stride: 0x1000,
        size: 32,
    })?);
    Ok(())
}

pub(super) fn gd32f303(device: &mut Device) -> Result<()> {
    let bkp = peripheral(device, "BKP")?;
    for register in bkp.registers_mut() {
        narrow(register, 16)?;
    }

    let usbd = peripheral(device, "USBD")?;
    for register in usbd
        .registers_mut()
        .filter(|register| register.name != "LPMCS")
    {
        narrow(register, 16)?;
    }

    let fmc = peripheral(device, "FMC")?;
    let obstat = register(fmc, "OBCTL")?;
    obstat.name = "OBSTAT".into();
    obstat.description = Some("Option byte status register".into());

    for adc in ["ADC0", "ADC1"] {
        let adc = peripheral(device, adc)?;
        resize(register(adc, "CTL0")?, "SYNCM", 4)?;
        add_field(
            register(adc, "CTL1")?,
            &Bits {
                name: "ETEIC",
                description: "External trigger enable for inserted channel",
                offset: 15,
                width: 1,
            },
        )?;
    }

    device.peripherals.push(window_peripheral(&Window {
        peripheral: "USBD_RAM",
        description: "USBD packet buffer",
        base: 0x4000_6000,
        register: "DATA",
        register_description: "Packet buffer half word",
        count: 256,
        stride: 4,
        size: 16,
    })?);
    Ok(())
}
