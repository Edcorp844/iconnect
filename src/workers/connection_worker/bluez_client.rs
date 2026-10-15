//! BlueZ system-bus client. Owns discovery, pairing, device properties,
//! and battery reads. One connection to the system bus.
use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::Connection;

use crate::workers::connection_worker::ancs_advertisment::AncsAdvertisement;

pub const BLUEZ_SERVICE: &str = "org.bluez";
pub const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
pub const DEVICE_IFACE: &str = "org.bluez.Device1";
pub const OBJECT_MANAGER_IFACE: &str = "org.freedesktop.DBus.ObjectManager";
pub const PROPERTIES_IFACE: &str = "org.freedesktop.DBus.Properties";
pub const GATT_CHAR_IFACE: &str = "org.bluez.GattCharacteristic1";
pub const AGENT_MANAGER_IFACE: &str = "org.bluez.AgentManager1";
pub const ANCS_UUID: &str = "7905f431-b5ce-4e99-a40f-4b1e122d00d0";
pub const BATTERY_LEVEL_UUID: &str = "00002a19-0000-1000-8000-00805f9b34fb";
pub const AGENT_OBJECT_PATH: &str = "/io/weirdware/iconnect/agent";
pub const AGENT_MANAGER_PATH: &str = "/org/bluez";
pub const ADVERT_OBJECT_PATH: &str = "/io/weirdware/iconnect/advertisement0";
pub const COD_MAJOR: u8 = 4; // A/V
pub const COD_MINOR: u8 = 8; // Hands-Free

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatterySource {
    Gatt,
    Bluez,
}

#[derive(Debug, Clone, Copy)]
pub struct PhoneBattery {
    pub percent: u8,
    pub source: BatterySource,
}

#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub path: String,
    pub address: String,
    pub alias: String,
    pub powered: bool,
    pub discovering: bool,
}

#[derive(Debug, Clone)]
pub struct BluetoothDevice {
    pub path: String,
    pub address: String,
    pub name: String,
    pub icon: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    pub uuids: Vec<String>,
}

impl BluetoothDevice {
    pub fn likely_iphone(&self) -> bool {
        let icon = self.icon.to_lowercase();
        let name = self.name.to_lowercase();
        icon == "phone"
            || icon == "smartphone"
            || icon == "phone-apple-iphone"
            || name.contains("iphone")
            || name.contains("ipad")
    }

    pub fn ancs_bonded(&self) -> bool {
        self.uuids.iter().any(|u| u.to_lowercase() == ANCS_UUID)
    }

    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.address
        } else {
            &self.name
        }
    }
}

pub struct BluezClient {
    pub(crate) conn: Connection,
}

impl BluezClient {
    pub async fn new() -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("system bus unavailable")?;
        Ok(Self { conn })
    }

    pub async fn primary_adapter(&self) -> Result<AdapterInfo> {
        let managed = self.managed_objects().await?;
        let mut adapters: Vec<(String, HashMap<String, OwnedValue>)> = managed
            .into_iter()
            .filter_map(|(path, ifaces)| {
                ifaces
                    .into_iter()
                    .find(|(name, _)| name == ADAPTER_IFACE)
                    .map(|(_, props)| (path, props))
            })
            .collect();
        adapters.sort_by(|a, b| a.0.cmp(&b.0));
        let (path, props) = adapters
            .into_iter()
            .find(|(p, _)| p.ends_with("/hci0"))
            .ok_or_else(|| anyhow!("no Bluetooth adapter found"))?;
        Ok(AdapterInfo {
            path: path.clone(),
            address: str_prop(&props, "Address"),
            alias: str_prop(&props, "Alias"),
            powered: bool_prop(&props, "Powered"),
            discovering: bool_prop(&props, "Discovering"),
        })
    }

    pub async fn devices_on(&self, adapter_path: &str) -> Result<Vec<BluetoothDevice>> {
        let managed = self.managed_objects().await?;
        let prefix = format!("{adapter_path}/");
        let mut out = Vec::new();
        for (path, ifaces) in managed {
            if !path.starts_with(&prefix) {
                continue;
            }
            let Some(props) = ifaces.get(DEVICE_IFACE) else {
                continue;
            };
            out.push(BluetoothDevice {
                path,
                address: str_prop(props, "Address"),
                name: str_prop(props, "Name"),
                icon: str_prop(props, "Icon"),
                paired: bool_prop(props, "Paired"),
                connected: bool_prop(props, "Connected"),
                trusted: bool_prop(props, "Trusted"),
                uuids: str_vec_prop(props, "UUIDs"),
            });
        }
        out.sort_by(|a, b| a.address.cmp(&b.address));
        Ok(out)
    }

    pub async fn find_iphone(&self, adapter_path: &str) -> Result<Option<BluetoothDevice>> {
        let devices = self.devices_on(adapter_path).await?;
        let mut candidates: Vec<BluetoothDevice> = devices
            .into_iter()
            .filter(|d| d.paired && (d.ancs_bonded() || d.likely_iphone()))
            .collect();
        candidates.sort_by_key(|d| !d.ancs_bonded());
        Ok(candidates.into_iter().next())
    }

    pub async fn start_discovery(&self, adapter_path: &str) -> Result<()> {
        self.conn
            .call_method(
                Some(BLUEZ_SERVICE),
                adapter_path,
                Some(ADAPTER_IFACE),
                "StartDiscovery",
                &(),
            )
            .await
            .context("StartDiscovery failed")?;
        Ok(())
    }

    pub async fn stop_discovery(&self, adapter_path: &str) -> Result<()> {
        self.conn
            .call_method(
                Some(BLUEZ_SERVICE),
                adapter_path,
                Some(ADAPTER_IFACE),
                "StopDiscovery",
                &(),
            )
            .await
            .context("StopDiscovery failed")?;
        Ok(())
    }

    /// Connect-first pairing. iOS initiates pairing when a profile connect
    /// is refused on an unpaired device. The Agent receives the passkey.
    pub async fn connect_device(&self, device_path: &str) -> Result<()> {
        self.conn
            .call_method(
                Some(BLUEZ_SERVICE),
                device_path,
                Some(DEVICE_IFACE),
                "Connect",
                &(),
            )
            .await
            .context("Device1.Connect failed")?;
        Ok(())
    }

    pub async fn device_property(&self, device_path: &str, name: &str) -> Result<OwnedValue> {
        let reply = self
            .conn
            .call_method(
                Some(BLUEZ_SERVICE),
                device_path,
                Some(PROPERTIES_IFACE),
                "Get",
                &(DEVICE_IFACE, name),
            )
            .await?;
        let value: OwnedValue = reply
            .body()
            .deserialize()
            .context("property reply malformed")?;
        Ok(value)
    }

    pub async fn set_trusted(&self, device_path: &str, trusted: bool) -> Result<()> {
        let mut props = HashMap::new();
        props.insert("Trusted", Value::Bool(trusted));
        self.conn
            .call_method(
                Some(BLUEZ_SERVICE),
                device_path,
                Some(PROPERTIES_IFACE),
                "Set",
                &(DEVICE_IFACE, "Trusted", Value::Bool(trusted)),
            )
            .await
            .context("Set Trusted failed")?;
        Ok(())
    }

    /// Read the iPhone's battery. Prefers the GATT Battery Level
    /// characteristic (fresh), falls back to Battery1.Percentage.
    pub async fn phone_battery(&self, device_path: &str) -> Result<Option<PhoneBattery>> {
        let managed = self.managed_objects().await?;
        let prefix = format!("{device_path}/");
        let mut char_path: Option<String> = None;
        for (path, ifaces) in &managed {
            if !path.starts_with(&prefix) {
                continue;
            }
            let Some(char) = ifaces.get(GATT_CHAR_IFACE) else {
                continue;
            };
            if str_prop(char, "UUID").to_lowercase() == BATTERY_LEVEL_UUID {
                char_path = Some(path.clone());
                break;
            }
        }

        if let Some(path) = char_path {
            match self.read_gatt_battery(&path).await {
                Ok(Some(level)) => {
                    return Ok(Some(PhoneBattery {
                        percent: level,
                        source: BatterySource::Gatt,
                    }));
                }
                Ok(None) => {}
                Err(e) => eprintln!("GATT battery read failed: {e:#}"),
            }
        }

        match self.device_property(device_path, "Percentage").await {
            Ok(v) => {
                let percent = u8::try_from(&v).ok().filter(|p| *p <= 100);
                Ok(percent.map(|percent| PhoneBattery {
                    percent,
                    source: BatterySource::Bluez,
                }))
            }
            Err(_) => Ok(None),
        }
    }

    async fn read_gatt_battery(&self, char_path: &str) -> Result<Option<u8>> {
        let opts: HashMap<&str, Value<'_>> = Default::default();
        let reply = self
            .conn
            .call_method(
                Some(BLUEZ_SERVICE),
                char_path,
                Some(GATT_CHAR_IFACE),
                "ReadValue",
                &(opts,),
            )
            .await
            .context("GATT ReadValue failed")?;
        let data: Vec<u8> = reply
            .body()
            .deserialize()
            .context("GATT battery value malformed")?;
        if data.len() != 1 || data[0] > 100 {
            return Ok(None);
        }
        Ok(Some(data[0]))
    }

    pub async fn unpair(&self, adapter_path: &str, device_path: &str) -> Result<()> {
        let device_op = ObjectPath::try_from(device_path).context("invalid device path")?;
        self.conn
            .call_method(
                Some(BLUEZ_SERVICE),
                adapter_path,
                Some(ADAPTER_IFACE),
                "RemoveDevice",
                &(device_op,),
            )
            .await
            .context("RemoveDevice failed")?;
        Ok(())
    }

    pub(crate) async fn managed_objects(
        &self,
    ) -> Result<HashMap<String, HashMap<String, HashMap<String, OwnedValue>>>> {
        let reply = self
            .conn
            .call_method(
                Some(BLUEZ_SERVICE),
                "/",
                Some(OBJECT_MANAGER_IFACE),
                "GetManagedObjects",
                &(),
            )
            .await
            .context("GetManagedObjects failed")?;
        let body: HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> =
            reply.body().deserialize().context("malformed object map")?;
        Ok(body
            .into_iter()
            .map(|(p, i)| (p.as_str().to_string(), i))
            .collect())
    }

    /// Set Class-of-Device to A/V Hands-Free (Major=4, Minor=8)
    pub async fn set_cod(&self, adapter: &str) -> anyhow::Result<()> {
        let index = adapter.trim_start_matches("hci");
        let status = tokio::process::Command::new("btmgmt")
            .args(["--index", index, "class", "4", "8"])
            .stdin(std::process::Stdio::null())
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("btmgmt class 4 8 failed");
        }
        Ok(())
    }

    /// Register the ANCS solicitation advertisement
    pub async fn start_ancs_advert(&self, adapter_path: &str) -> anyhow::Result<()> {
        // Export the LEAdvertisement1 object
        let advert = AncsAdvertisement::new();
        let object_server = self.conn.object_server();
        object_server.at(ADVERT_OBJECT_PATH, advert).await?;

        let adv_path = ObjectPath::try_from(ADVERT_OBJECT_PATH)?;

        // Register it
        self.conn
            .call_method(
                Some("org.bluez"),
                adapter_path,
                Some("org.bluez.LEAdvertisingManager1"),
                "RegisterAdvertisement",
                &(
                    adv_path,
                    std::collections::HashMap::<String, zbus::zvariant::Value<'_>>::new(),
                ),
            )
            .await?;

        Ok(())
    }

    pub async fn stop_ancs_advert(&self, adapter_path: &str) -> anyhow::Result<()> {
        let adv_path = ObjectPath::try_from(ADVERT_OBJECT_PATH)?;
        let _ = self
            .conn
            .call_method(
                Some("org.bluez"),
                adapter_path,
                Some("org.bluez.LEAdvertisingManager1"),
                "UnregisterAdvertisement",
                &(adv_path,),
            )
            .await;
        // Best-effort remove from object server
        let _ = self
            .conn
            .object_server()
            .remove::<AncsAdvertisement, _>(ADVERT_OBJECT_PATH)
            .await;
        Ok(())
    }
}

impl Clone for BluezClient {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}

// ---- property helpers ----

fn str_prop(props: &HashMap<String, OwnedValue>, name: &str) -> String {
    match props.get(name).map(|v| &**v) {
        Some(Value::Str(s)) => s.to_string(),
        Some(Value::ObjectPath(p)) => p.to_string(),
        _ => String::new(),
    }
}

fn bool_prop(props: &HashMap<String, OwnedValue>, name: &str) -> bool {
    matches!(props.get(name).map(|v| &**v), Some(Value::Bool(true)))
}

fn str_vec_prop(props: &HashMap<String, OwnedValue>, name: &str) -> Vec<String> {
    let Some(value) = props.get(name) else {
        return Vec::new();
    };
    let Value::Array(arr) = &**value else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|item| match item {
            Value::Str(s) => Some(s.to_string()),
            Value::ObjectPath(p) => Some(p.to_string()),
            _ => None,
        })
        .collect()
}
