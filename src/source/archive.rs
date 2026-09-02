//! In-plugin archive extraction for catalog and upload installs (small release
//! archives that fit through the panel's 10MB plugin-HTTP cap). Big archives —
//! the SourceMod and Metamod drops — are downloaded and unpacked on the node.
//!
//! Both container formats the Source 1 ecosystem uses are handled: `.zip` and
//! `.tar.gz`. AlliedModders ships tarballs for linux, GitHub release automation
//! usually ships zips, and a plugin's release page often carries one of each.

use std::io::Read;

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Forward-slash relative path, sanitized (no leading /, no dot segments).
    pub path: String,
    pub data: Vec<u8>,
    /// Unix permission bits, defaulted per file type when the archive has none.
    pub mode: u32,
}

/// Total uncompressed budget: a "small plugin" archive that inflates past this
/// is refused rather than ballooning the wasm heap.
const MAX_TOTAL_UNCOMPRESSED: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 4096;

/// Extracts a zip or a gzipped tar, chosen by magic bytes. Callers pass whole
/// downloads, so sniffing beats trusting a file name.
pub fn extract(bytes: &[u8]) -> Result<Vec<ArchiveEntry>, String> {
    if is_gzip(bytes) {
        return extract_tar_gz(bytes);
    }
    if is_zip(bytes) {
        return extract_zip(bytes);
    }
    Err("unrecognized archive: expected a .zip or .tar.gz".into())
}

pub fn is_gzip(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1f, 0x8b])
}

pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06")
}

/// True for the file extensions the upload dialog and this extractor accept.
pub fn has_archive_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".tar.gz") || lower.ends_with(".tgz")
}

pub fn extract_zip(bytes: &[u8]) -> Result<Vec<ArchiveEntry>, String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("not a zip archive: {e}"))?;
    if zip.len() > MAX_ENTRIES {
        return Err(format!("archive has too many entries ({})", zip.len()));
    }

    let mut total: u64 = 0;
    let mut entries = Vec::new();
    for index in 0..zip.len() {
        let mut file = zip
            .by_index(index)
            .map_err(|e| format!("corrupt zip entry: {e}"))?;
        if file.is_dir() {
            continue;
        }
        let Some(path) = sanitize_archive_path(file.name()) else {
            return Err(format!("archive entry escapes its root: {}", file.name()));
        };
        total = total.saturating_add(file.size());
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err("archive is too large when uncompressed".into());
        }
        let mut data = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut data)
            .map_err(|e| format!("failed to inflate {}: {e}", file.name()))?;
        entries.push(ArchiveEntry {
            mode: entry_mode(file.unix_mode(), &path),
            path,
            data,
        });
    }
    Ok(entries)
}

pub fn extract_tar_gz(bytes: &[u8]) -> Result<Vec<ArchiveEntry>, String> {
    let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
    let mut tar = tar::Archive::new(decoder);
    let mut total: u64 = 0;
    let mut entries = Vec::new();
    for item in tar
        .entries()
        .map_err(|e| format!("not a tar archive: {e}"))?
    {
        let mut file = item.map_err(|e| format!("corrupt tar entry: {e}"))?;
        // Regular files only: a tarball's symlinks and device nodes have no
        // meaning once the content is written through nodefs.
        if !file.header().entry_type().is_file() {
            continue;
        }
        if entries.len() >= MAX_ENTRIES {
            return Err(format!("archive has too many entries (over {MAX_ENTRIES})"));
        }
        let raw = file
            .path()
            .map_err(|e| format!("unreadable tar entry name: {e}"))?
            .to_string_lossy()
            .into_owned();
        let Some(path) = sanitize_archive_path(&raw) else {
            return Err(format!("archive entry escapes its root: {raw}"));
        };
        let size = file.header().size().unwrap_or(0);
        total = total.saturating_add(size);
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err("archive is too large when uncompressed".into());
        }
        let mode = file.header().mode().ok();
        let mut data = Vec::with_capacity(size as usize);
        file.read_to_end(&mut data)
            .map_err(|e| format!("failed to inflate {path}: {e}"))?;
        entries.push(ArchiveEntry {
            mode: entry_mode(mode, &path),
            path,
            data,
        });
    }
    Ok(entries)
}

/// Permission bits for one entry: the archive's own, or 0644, plus the exec
/// bit for things that must stay runnable even when the archive was built
/// without meaningful unix modes (Windows CI, python zipfile).
fn entry_mode(recorded: Option<u32>, path: &str) -> u32 {
    let mut mode = recorded
        .filter(|m| *m & 0o777 != 0)
        .map(|m| m & 0o777)
        .unwrap_or(0o644);
    if needs_exec(path) {
        mode |= 0o111;
    }
    mode
}

/// Shared objects — Metamod plugins, SourceMod extensions and the SourceMod
/// binaries themselves — are all `.so`/`.dll` and must be executable.
fn needs_exec(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".so") || lower.ends_with(".dylib")
}

fn sanitize_archive_path(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let trimmed = normalized.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for segment in trimmed.split('/') {
        match segment {
            "" | "." => continue,
            ".." => return None,
            other => {
                if other.contains('\0') || other.contains(':') {
                    return None;
                }
                parts.push(other);
            }
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Where a release archive's content should land, judged by its layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallRoot {
    /// Entries already rooted at the mod dir (`addons/…`, `cfg/…`): extract
    /// as-is. By far the most common shape — SourceMod plugins ship as a full
    /// overlay so their gamedata, translations and configs land with them.
    ModDir,
    /// Entries rooted at the SourceMod dir (`plugins/…`, `configs/…`):
    /// extract into addons/sourcemod.
    SourcemodDir,
    /// Loose `.smx` files at the top: extract into addons/sourcemod/plugins.
    PluginsDir,
}

/// The mod-dir prefixes a full-overlay release uses.
const MOD_DIR_ROOTS: &[&str] = &["addons", "cfg", "maps", "materials", "sound", "models", "scripts"];
/// The prefixes that sit directly inside addons/sourcemod.
const SM_DIR_ROOTS: &[&str] = &[
    "plugins",
    "configs",
    "gamedata",
    "translations",
    "extensions",
    "scripting",
    "data",
];

pub fn detect_install_root(entries: &[ArchiveEntry]) -> Result<InstallRoot, String> {
    if entries.is_empty() {
        return Err("archive is empty".into());
    }
    let mut top_levels: Vec<&str> = Vec::new();
    for entry in entries {
        let top = entry.path.split('/').next().unwrap_or("");
        if !top_levels.contains(&top) {
            top_levels.push(top);
        }
    }

    if top_levels
        .iter()
        .any(|top| MOD_DIR_ROOTS.contains(&top.to_ascii_lowercase().as_str()))
    {
        return Ok(InstallRoot::ModDir);
    }

    if top_levels
        .iter()
        .any(|top| SM_DIR_ROOTS.contains(&top.to_ascii_lowercase().as_str()))
    {
        return Ok(InstallRoot::SourcemodDir);
    }

    // Bare .smx files with nothing around them.
    if entries
        .iter()
        .any(|entry| !entry.path.contains('/') && is_smx(&entry.path))
    {
        return Ok(InstallRoot::PluginsDir);
    }

    Err(
        "unrecognized archive layout: expected addons/…, plugins/… or a top-level .smx"
            .into(),
    )
}

fn is_smx(path: &str) -> bool {
    path.to_ascii_lowercase()
        .ends_with(crate::source::SMX_EXT)
}

/// The plugin file stems an extracted archive installs, in the order the
/// archive lists them. Only files that actually land in the plugins dir count —
/// a `scripting/*.sp` source file or a disabled/ entry is not a new plugin.
pub fn plugin_names(entries: &[ArchiveEntry], root: &InstallRoot) -> Vec<String> {
    let prefix = match root {
        InstallRoot::ModDir => "addons/sourcemod/plugins/",
        InstallRoot::SourcemodDir => "plugins/",
        InstallRoot::PluginsDir => "",
    };
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let lower = entry.path.to_ascii_lowercase();
        if !lower.starts_with(prefix) || !is_smx(&entry.path) {
            continue;
        }
        let rest = &entry.path[prefix.len()..];
        // Only plugins directly in the plugins dir load; anything nested
        // (disabled/, optional/) is deliberately parked.
        if rest.contains('/') {
            continue;
        }
        let stem = crate::source::paths::file_stem(rest).to_string();
        if !stem.is_empty() && !names.iter().any(|known| known.eq_ignore_ascii_case(&stem)) {
            names.push(stem);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn build_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            for (name, data) in files {
                writer
                    .start_file(*name, SimpleFileOptions::default())
                    .expect("start file");
                writer.write_all(data).expect("write");
            }
            writer.finish().expect("finish");
        }
        cursor.into_inner()
    }

    fn build_tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        for (name, data) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, *name, *data)
                .expect("append");
        }
        builder
            .into_inner()
            .expect("finish tar")
            .finish()
            .expect("finish gz")
    }

    #[test]
    fn extracts_and_sanitizes_zip() {
        let bytes = build_zip(&[
            ("addons/sourcemod/plugins/rtd.smx", b"smx"),
            ("addons/sourcemod/translations/rtd.phrases.txt", b"{}"),
        ]);
        let entries = extract(&bytes).expect("extracts");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "addons/sourcemod/plugins/rtd.smx");
        assert_eq!(entries[0].mode, 0o644);
    }

    #[test]
    fn extracts_tar_gz_the_same_way() {
        let bytes = build_tar_gz(&[
            ("addons/sourcemod/plugins/rtd.smx", b"smx"),
            ("addons/sourcemod/extensions/steamworks.ext.so", b"elf"),
        ]);
        assert!(is_gzip(&bytes));
        let entries = extract(&bytes).expect("extracts");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "addons/sourcemod/plugins/rtd.smx");
        // Shared objects come back executable even from a mode-less archive.
        assert_eq!(entries[1].mode, 0o755);
        assert_eq!(detect_install_root(&entries).expect("root"), InstallRoot::ModDir);
    }

    #[test]
    fn rejects_traversal_and_junk() {
        assert!(extract(&build_zip(&[("../evil.smx", b"x")])).is_err());
        assert!(extract(b"not an archive at all").is_err());
        assert!(extract(b"").is_err());
    }

    /// The traversal defense both extractors share. Exercised directly for the
    /// tar side because the tar crate refuses to *write* a `..` entry, so a
    /// hostile tarball cannot be built here — only received.
    #[test]
    fn the_path_sanitizer_is_the_shared_defense() {
        assert_eq!(
            sanitize_archive_path("addons/sourcemod/plugins/rtd.smx").as_deref(),
            Some("addons/sourcemod/plugins/rtd.smx")
        );
        // Leading slashes and dot segments are stripped, not trusted.
        assert_eq!(
            sanitize_archive_path("/addons/./sourcemod/plugins/rtd.smx").as_deref(),
            Some("addons/sourcemod/plugins/rtd.smx")
        );
        assert_eq!(
            sanitize_archive_path("addons\\sourcemod\\plugins\\rtd.smx").as_deref(),
            Some("addons/sourcemod/plugins/rtd.smx")
        );
        for hostile in [
            "../evil.smx",
            "addons/../../evil.smx",
            "C:/windows/system32/evil.dll",
            "/",
            "",
        ] {
            assert_eq!(sanitize_archive_path(hostile), None, "{hostile}");
        }
    }

    #[test]
    fn detects_roots() {
        let overlay = extract(&build_zip(&[
            ("addons/sourcemod/plugins/sbpp_main.smx", b"x"),
            ("addons/sourcemod/configs/sourcebans/config.cfg", b"x"),
        ]))
        .expect("zip");
        assert_eq!(detect_install_root(&overlay).expect("root"), InstallRoot::ModDir);

        let sm_rooted = extract(&build_zip(&[
            ("plugins/rtd.smx", b"x"),
            ("translations/rtd.phrases.txt", b"x"),
        ]))
        .expect("zip");
        assert_eq!(
            detect_install_root(&sm_rooted).expect("root"),
            InstallRoot::SourcemodDir
        );

        let loose = extract(&build_zip(&[("funcommands.smx", b"x")])).expect("zip");
        assert_eq!(detect_install_root(&loose).expect("root"), InstallRoot::PluginsDir);

        let junk = extract(&build_zip(&[("readme.txt", b"x")])).expect("zip");
        assert!(detect_install_root(&junk).is_err());
    }

    #[test]
    fn names_only_the_plugins_that_load() {
        let entries = extract(&build_zip(&[
            ("addons/sourcemod/plugins/sbpp_main.smx", b"x"),
            ("addons/sourcemod/plugins/sbpp_checker.smx", b"x"),
            // Shipped but deliberately parked, and the source it was built from.
            ("addons/sourcemod/plugins/disabled/sbpp_debug.smx", b"x"),
            ("addons/sourcemod/scripting/sbpp_main.sp", b"x"),
            ("addons/sourcemod/gamedata/sbpp.txt", b"x"),
        ]))
        .expect("zip");
        let root = detect_install_root(&entries).expect("root");
        assert_eq!(
            plugin_names(&entries, &root),
            vec!["sbpp_main".to_string(), "sbpp_checker".to_string()]
        );
    }

    #[test]
    fn names_loose_and_sm_rooted_plugins() {
        let loose = extract(&build_zip(&[("rtd.smx", b"x"), ("readme.txt", b"x")])).expect("zip");
        assert_eq!(
            plugin_names(&loose, &InstallRoot::PluginsDir),
            vec!["rtd".to_string()]
        );
        let sm_rooted = extract(&build_zip(&[
            ("plugins/rtd.smx", b"x"),
            ("configs/rtd.cfg", b"x"),
        ]))
        .expect("zip");
        assert_eq!(
            plugin_names(&sm_rooted, &InstallRoot::SourcemodDir),
            vec!["rtd".to_string()]
        );
    }
}
