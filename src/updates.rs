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

/// Whether the app checks GitHub for new releases. Off in Leaximy: the
/// configuration below still names upstream's releases, which would replace
/// this fork with the upstream app.
pub const ENABLED: bool = false;

pub const CONFIG: UpdateConfig = UpdateConfig {
    // Leaximy_Legacy, the name before the rename: its marker files, `leaximy_legacy
    // <version>` answers, and its Windows setup program's location.
    legacy_names: &["leaximy_legacy"],
    legacy_windows_installs: &["Programs/Leaximy_Legacy/fastpotify.exe"],
    macos: MacConfig {
        bundle_ids: &["rocks.leaximy.Leaximy", "me.paolino.leaximy_legacy"],
        // 0.9.1 kept "leaximy_legacy" for older clients' validation; later
        // releases may rename it to "Leaximy" (#538).
        executable_names: &["leaximy_legacy", "Leaximy"],
        legacy_bundle_names: &["Leaximy_Legacy.app"],
    },
    // Releases are verified against checksums.txt alone until they are
    // signed. Only a version shipped after the first signed release may
    // carry the key: from then on an unsigned release is refused.
    publisher_key: None,
    ..UpdateConfig::new(
        "crmne/spotifast",
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
