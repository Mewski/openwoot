use anyhow::{Context, Result, bail, ensure};
use svd_rs::{
    Access, AddressBlock, AddressBlockUsage, BitRange, Device, DimElement, Field, FieldInfo,
    Peripheral, PeripheralInfo, RegisterCluster, RegisterInfo, ValidateLevel,
};

pub(super) struct Register<'a> {
    pub(super) name: &'a str,
    pub(super) description: &'a str,
    pub(super) offset: u32,
    pub(super) size: u32,
    pub(super) access: Access,
    pub(super) fields: &'a [Bits<'a>],
}

pub(super) struct Bits<'a> {
    pub(super) name: &'a str,
    pub(super) description: &'a str,
    pub(super) offset: u32,
    pub(super) width: u32,
}

pub(super) struct Window<'a> {
    pub(super) peripheral: &'a str,
    pub(super) description: &'a str,
    pub(super) base: u64,
    pub(super) register: &'a str,
    pub(super) register_description: &'a str,
    pub(super) count: u32,
    pub(super) stride: u32,
    pub(super) size: u32,
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

pub(super) fn window_peripheral(window: &Window) -> Result<Peripheral> {
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

pub(super) fn peripheral<'a>(device: &'a mut Device, name: &str) -> Result<&'a mut PeripheralInfo> {
    device
        .get_mut_peripheral(name)
        .map(|peripheral| &mut **peripheral)
        .with_context(|| format!("the SVD file has no peripheral `{name}`"))
}

pub(super) fn register<'a>(
    peripheral: &'a mut PeripheralInfo,
    name: &str,
) -> Result<&'a mut RegisterInfo> {
    let owner = peripheral.name.clone();
    peripheral
        .get_mut_register(name)
        .map(|register| &mut **register)
        .with_context(|| format!("the SVD file has no register `{owner}.{name}`"))
}

pub(super) fn add_register(peripheral: &mut PeripheralInfo, register: &Register) -> Result<()> {
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

pub(super) fn add_field(register: &mut RegisterInfo, bits: &Bits) -> Result<()> {
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

pub(super) fn resize(register: &mut RegisterInfo, name: &str, width: u32) -> Result<()> {
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

pub(super) fn narrow(register: &mut RegisterInfo, size: u32) -> Result<()> {
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

pub(super) fn with_standard_access(xml: &str) -> String {
    xml.replace("<access>read</access>", "<access>read-only</access>")
        .replace("<access>write</access>", "<access>write-only</access>")
}

fn field<'a>(register: &'a mut RegisterInfo, name: &str) -> Result<&'a mut FieldInfo> {
    let owner = register.name.clone();
    register
        .get_mut_field(name)
        .map(|field| &mut **field)
        .with_context(|| format!("the SVD file has no field `{name}` in `{owner}`"))
}

const fn mask(size: u32) -> u64 {
    match 1_u64.checked_shl(size) {
        Some(bit) => bit - 1,
        None => u64::MAX,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_covers_the_low_bits_up_to_all_64() {
        assert_eq!(mask(0), 0);
        assert_eq!(mask(8), 0xFF);
        assert_eq!(mask(32), 0xFFFF_FFFF);
        assert_eq!(mask(63), u64::MAX >> 1);
        assert_eq!(mask(64), u64::MAX);
        assert_eq!(mask(65), u64::MAX);
    }
}
