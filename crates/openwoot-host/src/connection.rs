use async_hid::DeviceInfo;
use openwoot_protocol::{Endpoint, PROTOCOL_VERSION, Version};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    channel,
    hid::{Hid, Interface},
    keyboard,
};

pub struct Connection {
    interface: Interface,
    version: Version,
}

impl Connection {
    pub async fn open(hid: &Hid) -> Result<Self, keyboard::Error> {
        let (_, mut interface) = keyboard::open_running(hid).await?;
        let version = channel::firmware_version(&mut interface)
            .await
            .map_err(keyboard::Error::from_request)?;
        if version.protocol != PROTOCOL_VERSION {
            return Err(keyboard::Error::OtherProtocol {
                firmware: version.protocol,
                tool: PROTOCOL_VERSION,
            });
        }
        Ok(Self { interface, version })
    }

    #[must_use]
    pub const fn info(&self) -> &DeviceInfo {
        self.interface.info()
    }

    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    pub async fn request<E>(&mut self, request: &E::Request) -> Result<E::Response, channel::Error>
    where
        E: Endpoint,
        E::Request: Serialize,
        E::Response: DeserializeOwned,
    {
        channel::request::<E>(&mut self.interface, request).await
    }
}
