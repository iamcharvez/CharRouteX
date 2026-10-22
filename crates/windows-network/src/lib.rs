//! Windows implementation of network discovery.

#[cfg(windows)]
mod connectivity;
#[cfg(windows)]
mod inventory;
#[cfg(windows)]
mod watcher;
#[cfg(windows)]
pub use inventory::WindowsAdapterInventory;
#[cfg(windows)]
pub use watcher::NetworkChangeWatcher;
#[cfg(windows)]
pub fn detect_ipv4_conflict(
    target: std::net::Ipv4Addr,
    source: Option<std::net::Ipv4Addr>,
) -> bool {
    connectivity::detect_ipv4_conflict(target, source)
}

#[cfg(not(windows))]
pub struct WindowsAdapterInventory;
#[cfg(not(windows))]
impl WindowsAdapterInventory {
    pub fn new() -> Self {
        Self
    }
}
#[cfg(not(windows))]
impl Default for WindowsAdapterInventory {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(not(windows))]
impl network_core::AdapterInventory for WindowsAdapterInventory {
    fn snapshot(&self) -> Result<network_core::AdapterSnapshot, network_core::NetworkError> {
        Err(network_core::NetworkError::UnsupportedPlatform)
    }
}
