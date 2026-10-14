use zbus::proxy;

// ---------- Messages1 (commands + snapshots) ----------
#[proxy(
    interface = "io.weirdware.BlueFerry.Messages1",
    default_service = "io.weirdware.BlueFerry",
    default_path = "/io/weirdware/BlueFerry"
)]
pub trait Messages {
    fn get_status(&self) -> zbus::Result<String>;
    fn list_threads(&self, limit: u32) -> zbus::Result<String>;
    fn send_to_thread_checked(
        &self,
        key: &str,
        body: &str,
        confirm_group: bool,
        expected_group_token: &str,
    ) -> zbus::Result<String>;
    fn mark_thread_read(&self, key: &str) -> zbus::Result<u32>;

    // keep this just in case some versions still emit it
    #[zbus(signal)]
    fn message_received(&self) -> zbus::Result<()>;
}

// ---------- Events1 (live content-free notifications) ----------
#[proxy(
    interface = "io.weirdware.BlueFerry.Events1",
    default_service = "io.weirdware.BlueFerry",
    default_path = "/io/weirdware/BlueFerry"
)]
pub trait Events {
    #[zbus(signal)]
    fn history_changed(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn status_changed(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn open_message_requested(&self) -> zbus::Result<()>;
}
