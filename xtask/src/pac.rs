use std::{fs, io, path::Path, process::Command};

use anyhow::{Context, Result, bail, ensure};
use svd_rs::{
    Access, AddressBlock, AddressBlockUsage, BitRange, Device, DimElement, Field, FieldInfo,
    Peripheral, PeripheralInfo, RegisterCluster, RegisterInfo, ValidateLevel,
};

const SVD2RUST: Tool = Tool {
    program: "svd2rust",
    version: "svd2rust 0.37.1",
};

const FORM: Tool = Tool {
    program: "form",
    version: "form 0.13.0",
};

const CHIPS: [Chip; 2] = [
    Chip {
        name: "gd32f425",
        svd_file: "GD32F4xx.svd",
        patch: patch_gd32f425,
    },
    Chip {
        name: "gd32f303",
        svd_file: "GD32F30x_XD.svd",
        patch: patch_gd32f303,
    },
];

pub(crate) fn run(root: &Path) -> Result<()> {
    SVD2RUST.require()?;
    FORM.require()?;
    for chip in &CHIPS {
        chip.generate(root)
            .with_context(|| format!("could not generate the `{}` PAC", chip.name))?;
    }
    Ok(())
}

struct Tool {
    program: &'static str,
    version: &'static str,
}

impl Tool {
    fn require(&self) -> Result<()> {
        let output = Command::new(self.program)
            .arg("--version")
            .output()
            .with_context(|| format!("could not run `{} --version`", self.program))?;
        let reported = String::from_utf8_lossy(&output.stdout);
        ensure!(
            reported.starts_with(self.version),
            "`{}` reports `{}`, the PACs are generated with `{}`",
            self.program,
            reported.trim(),
            self.version
        );
        Ok(())
    }
}

struct Chip {
    name: &'static str,
    svd_file: &'static str,
    patch: fn(&mut Device) -> Result<()>,
}

impl Chip {
    fn generate(&self, root: &Path) -> Result<()> {
        let package = root.join("crates").join(format!("{}-pac", self.name));
        let work = root.join("target").join("pac").join(self.name);
        recreate(&work)?;

        let source = package.join("svd").join(self.svd_file);
        let xml = fs::read_to_string(&source)
            .with_context(|| format!("could not read `{}`", source.display()))?;
        let mut device = svd_parser::parse(&with_standard_access(&xml))
            .with_context(|| format!("could not parse `{}`", source.display()))?;
        (self.patch)(&mut device)?;
        let patched = work.join(format!("{}.svd", self.name));
        fs::write(&patched, svd_encoder::encode(&device)?)
            .with_context(|| format!("could not write `{}`", patched.display()))?;

        execute(
            Command::new(SVD2RUST.program)
                .args([
                    "--target",
                    "cortex-m",
                    "--edition",
                    "2024",
                    "--generic-mod",
                    "--reexport-interrupt",
                    "--log",
                    "warn",
                ])
                .arg("-i")
                .arg(&patched)
                .arg("--output-dir")
                .arg(&work),
        )?;

        let src = package.join("src");
        recreate(&src)?;
        execute(
            Command::new(FORM.program)
                .env("RUST_LOG", "warn")
                .arg("--input")
                .arg(work.join("lib.rs"))
                .arg("--outdir")
                .arg(&src),
        )?;
        copy(&work.join("generic.rs"), &src.join("generic.rs"))?;
        for file in ["build.rs", "device.x"] {
            copy(&work.join(file), &package.join(file))?;
        }

        execute(
            Command::new("rustfmt")
                .args(["+nightly", "--edition", "2024"])
                .arg(src.join("lib.rs"))
                .arg(package.join("build.rs"))
                .current_dir(root),
        )
    }
}

fn patch_gd32f425(device: &mut Device) -> Result<()> {
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

fn patch_gd32f303(device: &mut Device) -> Result<()> {
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

struct Register<'a> {
    name: &'a str,
    description: &'a str,
    offset: u32,
    size: u32,
    access: Access,
    fields: &'a [Bits<'a>],
}

impl Register<'_> {
    fn build(&self) -> Result<RegisterInfo> {
        let fields = self
            .fields
            .iter()
            .map(Bits::build)
            .collect::<Result<Vec<_>>>()?;
        Ok(RegisterInfo::builder()
            .name(self.name.into())
            .description(Some(self.description.into()))
            .address_offset(self.offset)
            .size(Some(self.size))
            .access(Some(self.access))
            .reset_value(Some(0))
            .reset_mask(Some(mask(self.size)))
            .fields(Some(fields))
            .build(ValidateLevel::Strict)?)
    }
}

struct Bits<'a> {
    name: &'a str,
    description: &'a str,
    offset: u32,
    width: u32,
}

impl Bits<'_> {
    fn build(&self) -> Result<Field> {
        Ok(FieldInfo::builder()
            .name(self.name.into())
            .description(Some(self.description.into()))
            .bit_range(BitRange::from_offset_width(self.offset, self.width))
            .build(ValidateLevel::Strict)?
            .single())
    }
}

struct Window<'a> {
    peripheral: &'a str,
    description: &'a str,
    base: u64,
    register: &'a str,
    register_description: &'a str,
    count: u32,
    stride: u32,
    size: u32,
}

fn window_peripheral(window: &Window) -> Result<Peripheral> {
    let register = Register {
        name: &format!("{}[%s]", window.register),
        description: window.register_description,
        offset: 0,
        size: window.size,
        access: Access::ReadWrite,
        fields: &[Bits {
            name: window.register,
            description: window.register_description,
            offset: 0,
            width: window.size,
        }],
    }
    .build()?
    .array(
        DimElement::builder()
            .dim(window.count)
            .dim_increment(window.stride)
            .build(ValidateLevel::Strict)?,
    );
    let block = AddressBlock::builder()
        .offset(0)
        .size(window.count * window.stride)
        .usage(AddressBlockUsage::Registers)
        .build(ValidateLevel::Strict)?;
    Ok(PeripheralInfo::builder()
        .name(window.peripheral.into())
        .description(Some(window.description.into()))
        .base_address(window.base)
        .address_block(Some(vec![block]))
        .registers(Some(vec![RegisterCluster::Register(register)]))
        .build(ValidateLevel::Strict)?
        .single())
}

fn peripheral<'a>(device: &'a mut Device, name: &str) -> Result<&'a mut PeripheralInfo> {
    device
        .get_mut_peripheral(name)
        .map(|peripheral| &mut **peripheral)
        .with_context(|| format!("the SVD file has no peripheral `{name}`"))
}

fn register<'a>(peripheral: &'a mut PeripheralInfo, name: &str) -> Result<&'a mut RegisterInfo> {
    let owner = peripheral.name.clone();
    peripheral
        .get_mut_register(name)
        .map(|register| &mut **register)
        .with_context(|| format!("the SVD file has no register `{owner}.{name}`"))
}

fn field<'a>(register: &'a mut RegisterInfo, name: &str) -> Result<&'a mut FieldInfo> {
    let owner = register.name.clone();
    register
        .get_mut_field(name)
        .map(|field| &mut **field)
        .with_context(|| format!("the SVD file has no field `{name}` in `{owner}`"))
}

fn add_register(peripheral: &mut PeripheralInfo, register: &Register) -> Result<()> {
    ensure!(
        peripheral.get_register(register.name).is_none(),
        "register `{}.{}` exists already",
        peripheral.name,
        register.name
    );
    let built = register.build()?.single();
    peripheral
        .registers
        .get_or_insert_with(Vec::new)
        .push(RegisterCluster::Register(built));
    Ok(())
}

fn add_field(register: &mut RegisterInfo, bits: &Bits) -> Result<()> {
    let built = bits.build()?;
    ensure!(
        register.bitmask() & built.bitmask() == 0,
        "field `{}` overlaps a field of `{}`",
        bits.name,
        register.name
    );
    register.fields.get_or_insert_with(Vec::new).push(built);
    Ok(())
}

fn resize(register: &mut RegisterInfo, name: &str, width: u32) -> Result<()> {
    let others = register
        .fields()
        .filter(|field| field.name != name)
        .fold(0, |mask, field| mask | field.bitmask());
    let resized = field(register, name)?;
    resized.bit_range = BitRange::from_offset_width(resized.bit_range.offset, width);
    ensure!(
        resized.bitmask() & others == 0,
        "field `{name}` of `{}` overlaps another field",
        register.name
    );
    Ok(())
}

fn narrow(register: &mut RegisterInfo, size: u32) -> Result<()> {
    let kept = mask(size);
    if let Some(overflow) = register.fields().find(|field| field.msb() >= size) {
        bail!(
            "field `{}` of `{}` does not fit in {size} bits",
            overflow.name,
            register.name
        );
    }
    let properties = &mut register.properties;
    properties.size = Some(size);
    properties.reset_value = properties.reset_value.map(|value| value & kept);
    properties.reset_mask = properties.reset_mask.map(|value| value & kept);
    Ok(())
}

fn with_standard_access(xml: &str) -> String {
    xml.replace("<access>read</access>", "<access>read-only</access>")
        .replace("<access>write</access>", "<access>write-only</access>")
}

fn mask(size: u32) -> u64 {
    (1_u64 << size) - 1
}

fn recreate(directory: &Path) -> Result<()> {
    match fs::remove_dir_all(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not remove `{}`", directory.display()));
        }
    }
    fs::create_dir_all(directory)
        .with_context(|| format!("could not create `{}`", directory.display()))
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to)
        .with_context(|| format!("could not copy `{}` to `{}`", from.display(), to.display()))?;
    Ok(())
}

fn execute(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("could not run {command:?}"))?;
    ensure!(status.success(), "{command:?} ended with {status}");
    Ok(())
}
