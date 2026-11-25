use embedded_hal_async::spi::SpiDevice;
use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_drivers::spi_nor::SpiNor;

use super::{
    device_info::{DeviceInfo, DeviceInfoSource},
    format::Opened,
    layout_variant::{LayoutOverride, LayoutVariantSource},
    nor_identity::NorIdentity,
    nor_layout::SettingsRegion,
    partition::Partition,
    settings::{Error, Settings},
};

#[derive(Debug)]
pub struct Storage<'s, F: MultiwriteNorFlash> {
    pub settings: Option<&'s mut Settings<F>>,
    pub opened: Result<Opened, Error>,
    pub device_info: Option<DeviceInfo>,
    pub layout_variant: u8,
    pub factory_layout_variant: u8,
    pub nor_identity: Option<NorIdentity>,
}

impl<'s, D: SpiDevice> Storage<'s, SpiNor<D>> {
    pub async fn open(
        mut nor: SpiNor<D>,
        cell: &'s mut Settings<SpiNor<D>>,
        settings: SettingsRegion,
        device_info: impl DeviceInfoSource,
        layout_variant: impl LayoutVariantSource,
        format_requested: bool,
    ) -> Self {
        let nor_identity = match (nor.identification().await, nor.unique_id().await) {
            (Ok(identification), Ok(unique_id)) => Some(NorIdentity {
                identification,
                unique_id,
            }),
            _ => None,
        };
        let device_info = device_info.read(&mut nor).await.ok();
        let factory_layout_variant = layout_variant.variant(device_info.as_ref());
        let opened = cell
            .open(Partition::new(nor, settings), format_requested)
            .await;
        let mut settings = opened.is_ok().then_some(cell);
        let layout_variant = match settings.as_deref_mut() {
            Some(stored) => LayoutOverride::variant_or(stored, factory_layout_variant).await,
            None => factory_layout_variant,
        };
        Self {
            settings,
            opened,
            device_info,
            layout_variant,
            factory_layout_variant,
            nor_identity,
        }
    }
}
