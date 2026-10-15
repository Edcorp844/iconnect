use zbus::interface;

use crate::workers::connection_worker::bluez_client::ANCS_UUID;

pub struct AncsAdvertisement;

impl AncsAdvertisement {
    pub fn new() -> Self {
        Self
    }
}

#[interface(name = "org.bluez.LEAdvertisement1")]
impl AncsAdvertisement {
    fn release(&self) {
        // BlueZ called Release – nothing special to do
    }

    #[zbus(property)]
    fn type_(&self) -> &str {
        "peripheral"
    }

    #[zbus(property)]
    fn solicit_uuids(&self) -> Vec<String> {
        vec![ANCS_UUID.to_string()]
    }

    #[zbus(property)]
    fn manufacturer_data(&self) -> std::collections::HashMap<u16, Vec<u8>> {
        let mut map = std::collections::HashMap::new();
        // Same inert payload BlueFerry uses
        map.insert(0xffff, vec![0x50, 0xb0, 0x13, 0xf0]);
        map
    }

    #[zbus(property)]
    fn discoverable(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn discoverable_timeout(&self) -> u16 {
        180
    }

    #[zbus(property)]
    fn local_name(&self) -> &str {
        "BlueFerry"
    }

    // Optional – helps some controllers
    #[zbus(property)]
    fn includes(&self) -> Vec<String> {
        vec!["tx-power".into()]
    }
}
