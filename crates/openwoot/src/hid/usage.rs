pub(crate) mod page {
    pub(crate) const GENERIC_DESKTOP: u16 = 0x01;
    pub(crate) const SIMULATION: u16 = 0x02;
    pub(crate) const KEYBOARD: u16 = 0x07;
    pub(crate) const LED: u16 = 0x08;
    pub(crate) const BUTTON: u16 = 0x09;
    pub(crate) const CONSUMER: u16 = 0x0C;
    pub(crate) const VENDOR_PLACEHOLDER: u16 = 0xFF00;
    pub(crate) const VENDOR_KEY_STATE: u16 = 0xFF53;
    pub(crate) const VENDOR_CHANNEL: u16 = 0xFF55;
}

pub(crate) mod desktop {
    pub(crate) const POINTER: u16 = 0x01;
    pub(crate) const MOUSE: u16 = 0x02;
    pub(crate) const GAMEPAD: u16 = 0x05;
    pub(crate) const KEYBOARD: u16 = 0x06;
    pub(crate) const X_AXIS: u16 = 0x30;
    pub(crate) const Y_AXIS: u16 = 0x31;
    pub(crate) const Z_AXIS: u16 = 0x32;
    pub(crate) const RX: u16 = 0x33;
    pub(crate) const RY: u16 = 0x34;
    pub(crate) const RZ: u16 = 0x35;
    pub(crate) const WHEEL: u16 = 0x38;
    pub(crate) const HAT_SWITCH: u16 = 0x39;
    pub(crate) const SYSTEM_CONTROL: u16 = 0x80;
    pub(crate) const SYSTEM_POWER_DOWN: u16 = 0x81;
    pub(crate) const SYSTEM_WAKE_UP: u16 = 0x83;
}

pub(crate) mod simulation {
    pub(crate) const ELEVATOR: u16 = 0xB8;
    pub(crate) const RUDDER: u16 = 0xBA;
    pub(crate) const THROTTLE: u16 = 0xBB;
    pub(crate) const TRIGGER: u16 = 0xC0;
    pub(crate) const ACCELERATOR: u16 = 0xC4;
}

pub(crate) mod keyboard {
    pub(crate) const LEFT_CONTROL: u16 = 0xE0;
    pub(crate) const RIGHT_GUI: u16 = 0xE7;
    pub(crate) const LAST_KEY: u16 = 0xDD;
}

pub(crate) mod led {
    pub(crate) const NUM_LOCK: u16 = 0x01;
    pub(crate) const KANA: u16 = 0x05;
}

pub(crate) mod consumer {
    pub(crate) const CONSUMER_CONTROL: u16 = 0x01;
    pub(crate) const AC_PAN: u16 = 0x0238;
    pub(crate) const LAST: u16 = 0x02A2;
}

pub(crate) mod unit {
    pub(crate) const NONE: u32 = 0x00;
    pub(crate) const DEGREES: u32 = 0x14;
}
