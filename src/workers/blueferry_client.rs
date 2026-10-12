use anyhow::{Context, Result};
use zbus::{Connection, proxy};

#[proxy(
    interface = "io.weirdware.BlueFerry.Messages1",
    default_service = "io.weirdware.BlueFerry",
    default_path = "/io/weirdware/BlueFerry"
)]
trait Messages {
    fn get_status(&self) -> zbus::Result<String>;
    fn list_threads(&self, limit: u32) -> zbus::Result<String>;
    fn is_healthy(&self) -> zbus::Result<bool>;
    fn send_to_thread_checked(
        &self,
        key: &str,
        body: &str,
        confirm_group: bool,
        expected_group_token: &str,
    ) -> zbus::Result<String>;
}
