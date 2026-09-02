//! Binary Metamod plugins registered by `addons/metamod/<name>.vdf` alias files.
//!
//! Metamod:Source loads every `*.vdf` in its own directory. Renaming one to
//! `<name>.vdf.disabled` is the persistent off switch — the single-file
//! equivalent of commenting a metaplugins.ini line out.
//!
//! Note the two different vdf files this plugin deals with: `addons/metamod.vdf`
//! (the engine's loader for Metamod itself, see `gameinfo.rs`) and these, which
//! sit one level deeper and register plugins with an already-running Metamod.

use crate::host_api::{DirEntry, HostApi, HostApiError};

pub const VDF_EXT: &str = ".vdf";
/// SourceMod registers itself with Metamod through this alias, so it shows up
/// in the same list as ordinary binary plugins — but switching it off unloads
/// the whole platform this tab manages, and every `sm` console command with it.
pub const PLATFORM_VDF_STEM: &str = "sourcemod";
pub const DISABLED_SUFFIX: &str = ".vdf.disabled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VdfPlugin {
    pub name: String,
    pub enabled: bool,
}

/// Whether a vdf stem is the SourceMod registration itself.
pub fn is_platform(name: &str) -> bool {
    name.eq_ignore_ascii_case(PLATFORM_VDF_STEM)
}

/// Classifies one metamod-dir entry; `None` for non-VDF files (metaplugins.ini,
/// the metamod binaries dir, README leftovers).
pub fn classify(entry: &DirEntry) -> Option<VdfPlugin> {
    if entry.is_dir {
        return None;
    }
    let lower = entry.name.to_ascii_lowercase();
    if let Some(stem) = lower
        .ends_with(DISABLED_SUFFIX)
        .then(|| &entry.name[..entry.name.len() - DISABLED_SUFFIX.len()])
    {
        return (!stem.is_empty()).then(|| VdfPlugin {
            name: stem.to_string(),
            enabled: false,
        });
    }
    if let Some(stem) = lower
        .ends_with(VDF_EXT)
        .then(|| &entry.name[..entry.name.len() - VDF_EXT.len()])
    {
        return (!stem.is_empty()).then(|| VdfPlugin {
            name: stem.to_string(),
            enabled: true,
        });
    }
    None
}

/// Lists VDF plugins in the metamod dir, sorted by name. A name present in
/// both live and disabled form yields two entries, mirroring the plugin list.
pub fn list<H: HostApi>(
    host: &mut H,
    node_id: u64,
    metamod_abs: &str,
) -> Result<Vec<VdfPlugin>, HostApiError> {
    let Some(entries) = host.read_dir(node_id, metamod_abs)? else {
        return Ok(Vec::new());
    };
    let mut plugins: Vec<VdfPlugin> = entries.iter().filter_map(classify).collect();
    plugins.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then(b.enabled.cmp(&a.enabled))
    });
    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool) -> DirEntry {
        DirEntry {
            name: name.into(),
            is_dir,
        }
    }

    #[test]
    fn classifies_vdf_states() {
        assert_eq!(
            classify(&entry("sourcemod.vdf", false)),
            Some(VdfPlugin {
                name: "sourcemod".into(),
                enabled: true
            })
        );
        assert_eq!(
            classify(&entry("stripper.vdf.disabled", false)),
            Some(VdfPlugin {
                name: "stripper".into(),
                enabled: false
            })
        );
        assert_eq!(classify(&entry("metaplugins.ini", false)), None);
        assert_eq!(classify(&entry("bin", true)), None);
        assert_eq!(classify(&entry(".vdf", false)), None);
    }

    #[test]
    fn platform_alias_is_sourcemod() {
        assert!(is_platform("sourcemod"));
        assert!(is_platform("SourceMod"));
        assert!(!is_platform("stripper"));
    }
}
