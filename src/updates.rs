//! Self-update from GitHub releases, through fastframe-update.
//!
//! The previous release's helper installs the first release built on the
//! crate, so what it relies on stays the same: `--version` prints
//! `<command> <version>`, the new app accepts `--update-receipt` and
//! `--update-error` (stripped by [`fastframe_update::intercept`] before the
//! argument parser), and the handoff and receipt files keep their format.

pub use fastframe_update::{
    CHECK_INTERVAL, DownloadState, Installation, Kind, Prepared, Release, Source, Unsupported,
    Updater,
};
use fastframe_update::{MacConfig, ReqwestTransport, UpdateConfig};

/// Whether the app checks GitHub for new releases.
pub const ENABLED: bool = true;

/// Leaximy updates from its own repository's releases, tagged `v<version>`,
/// whose assets follow fastframe-update's names
/// (`leaximy-v<version>-x86_64-pc-windows-msvc-setup.exe` and
/// `checksums.txt`); .github/workflows/leaximy-windows.yml publishes them.
pub const CONFIG: UpdateConfig = UpdateConfig {
    legacy_names: &[],
    legacy_windows_installs: &[],
    macos: MacConfig {
        bundle_ids: &["rocks.leaximy.Leaximy"],
        executable_names: &["Leaximy"],
        legacy_bundle_names: &[],
    },
    // Releases are verified against checksums.txt alone until they are
    // signed. Only a version shipped after the first signed release may
    // carry the key: from then on an unsigned release is refused.
    publisher_key: None,
    ..UpdateConfig::new(
        "atakangneyy/leaximy",
        "Leaximy",
        "leaximy",
        env!("CARGO_PKG_VERSION"),
    )
};

/// An updater on Leaximy's HTTP client, through the configured proxy.
pub fn updater(proxy: &crate::settings::ProxyConfig) -> anyhow::Result<Updater> {
    let builder = crate::http::blocking_builder(proxy).map_err(anyhow::Error::msg)?;
    Ok(Updater::new(CONFIG, ReqwestTransport::new(builder)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_config_is_valid() {
        CONFIG.validate().unwrap();
        assert_eq!(CONFIG.current_version, env!("CARGO_PKG_VERSION"));
    }
}
