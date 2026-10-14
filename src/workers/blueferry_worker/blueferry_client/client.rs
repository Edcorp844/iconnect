use zbus::proxy;

// ---------- Messages1 ----------
#[proxy(
    interface = "io.weirdware.BlueFerry.Messages1",
    default_service = "io.weirdware.BlueFerry",
    default_path = "/io/weirdware/BlueFerry"
)]
pub trait Messages {
    // ---- status & health ----
    fn get_status(&self) -> zbus::Result<String>;
    fn is_healthy(&self) -> zbus::Result<bool>;

    // ---- threads & messages ----
    fn list_threads(&self, limit: u32) -> zbus::Result<String>;
    fn mark_thread_read(&self, key: &str) -> zbus::Result<u32>;
    fn set_thread_starred(&self, key: &str, starred: bool) -> zbus::Result<bool>;
    fn set_group_participants(&self, key: &str, recipients: &[&str]) -> zbus::Result<String>;
    fn delete_threads(&self, keys: &[&str], _arg1: bool) -> zbus::Result<u32>;
    fn clear_history(&self, _arg0: bool) -> zbus::Result<()>;

    // ---- sending ----
    fn send(&self, recipient: &str, body: &str) -> zbus::Result<String>;
    fn send_to_thread(&self, key: &str, body: &str, confirm_group: bool) -> zbus::Result<String>;
    fn send_to_thread_checked(
        &self,
        key: &str,
        body: &str,
        confirm_group: bool,
        expected_group_token: &str,
    ) -> zbus::Result<String>;

    // ---- contacts ----
    fn find_contacts(&self, query: &str) -> zbus::Result<String>;
    fn list_contacts(&self, offset: u32, limit: u32) -> zbus::Result<String>;
    fn sync_contacts(&self) -> zbus::Result<u32>;

    // ---- events & recents ----
    fn list_events(&self, kinds: &[&str], limit: u32) -> zbus::Result<String>;
    fn list_recent(&self, folder: &str, limit: u32) -> zbus::Result<String>;

    // ---- policies ----
    fn get_notification_policy(&self) -> zbus::Result<String>;
    fn set_notification_policy(&self, policy: &str) -> zbus::Result<String>;
    fn get_contacts_only_notifications(&self) -> zbus::Result<bool>;
    fn set_contacts_only_notifications(&self, enabled: bool) -> zbus::Result<bool>;
    fn get_storage_policy(&self) -> zbus::Result<String>;
    fn set_storage_policy(&self, policy: &str) -> zbus::Result<String>;
    fn unlock_storage(&self) -> zbus::Result<String>;

    // ---- legacy Gtk client handoff ----
    fn open_legacy_gtk_message(&self, _arg0: &str, _arg1: &str) -> zbus::Result<bool>;
}

// ---------- Events1 ----------
#[proxy(
    interface = "io.weirdware.BlueFerry.Events1",
    default_service = "io.weirdware.BlueFerry",
    default_path = "/io/weirdware/BlueFerry"
)]
pub trait Events {
    /// Signal: message history changed. Carries a dict of metadata.
    #[zbus(signal)]
    fn history_changed(
        &self,
        payload: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
    ) -> zbus::Result<()>;

    /// Signal: backend status changed (connectivity, storage, etc.).
    #[zbus(signal)]
    fn status_changed(&self) -> zbus::Result<()>;

    /// Signal: a specific message should be opened in the UI. Carries its key.
    #[zbus(signal)]
    fn open_message_requested(&self, key: String) -> zbus::Result<()>;
}
