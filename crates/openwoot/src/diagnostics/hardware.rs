use openwoot_protocol::diagnostics::Bus;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Hardware {
    pub scan_overruns: Option<u32>,
    pub led_bus: Bus,
    pub flash_bus: Bus,
}
