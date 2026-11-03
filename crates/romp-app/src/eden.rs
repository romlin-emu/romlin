pub mod input;
pub mod saves;

use crate::external::{Asset, Emulator, Running};
use std::path::{Path, PathBuf};

pub const PLATFORM: &str = "switch";
const VERSION: &str = "v0.2.1";

// shortcut: Forgejo has no asset digests; update these verified pins when adopting a newer stable release.
fn download() -> Option<(&'static str, &'static str)> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some((
            "Eden-Linux-v0.2.1-legacy-gcc-standard.AppImage",
            "2003a408753b9663a3cb72080085df7fe5f09b7825f542c1a01ad123bdb7c8e2",
        ))
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some((
            "Eden-Linux-v0.2.1-aarch64-gcc-standard.AppImage",
            "494014f67401bbbe7d48da7128d769734cf97863bfb2349d48cd2a118173e769",
        ))
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        Some((
            "Eden-Windows-v0.2.1-amd64-msvc-standard.zip",
            "ff498e5da9630216926ac3cbe9fb493b14930665c728a40a8f5b59507fdd7ebf",
        ))
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some((
            "Eden-Windows-v0.2.1-arm64-clang-standard.zip",
            "7df16b64bb0a0225acddcb498e524aa7854c24708022ecbea599cc1aebf010dc",
        ))
    } else {
        None
    }
}

pub fn available() -> bool {
    download().is_some()
}

pub fn emulator(dir: PathBuf) -> Emulator {
    Emulator {
        name: "Eden",
        dir,
        pick: |_| None,
        exe_in: |dir, asset| dir.join(if cfg!(windows) { "eden.exe" } else { asset }),
    }
}

pub async fn install(emulator: &Emulator, http: &reqwest::Client) -> Result<PathBuf, String> {
    let (name, hash) =
        download().ok_or_else(|| "Eden has no download for this computer".to_string())?;
    migrate_users(&emulator.dir)?;
    let exe = emulator
        .install_asset(
            http,
            VERSION,
            &Asset {
                name: name.into(),
                browser_download_url: format!("https://stable.eden-emu.dev/{VERSION}/{name}"),
                digest: Some(format!("sha256:{hash}")),
            },
        )
        .await?;
    ready(exe)
}

pub fn missing(source: &Path) -> Vec<String> {
    [
        (!source.join("prod.keys").is_file(), "prod.keys"),
        (find_firmware(source).is_none(), "firmware.zip"),
    ]
    .into_iter()
    .filter(|(absent, _)| *absent)
    .map(|(_, name)| name.to_string())
    .collect()
}

pub fn is_firmware_archive(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
}

pub(crate) fn firmware_version(name: &str) -> Option<Vec<u64>> {
    name.split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter_map(|candidate| {
            let parts = candidate.trim_matches('.').split('.').collect::<Vec<_>>();
            if parts.len() < 2 {
                return None;
            }
            let mut version = parts
                .iter()
                .map(|part| part.parse().ok())
                .collect::<Option<Vec<u64>>>()?;
            while version.last() == Some(&0) {
                version.pop();
            }
            Some(version)
        })
        .max()
}

fn find_firmware(source: &Path) -> Option<PathBuf> {
    std::fs::read_dir(source)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() || !is_firmware_archive(&name) {
                return None;
            }
            Some((
                (
                    firmware_version(&name),
                    metadata.modified().ok(),
                    path.clone(),
                ),
                path,
            ))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(_, path)| path)
}

fn valid_keys(text: &str) -> bool {
    let mut count = 0;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let Some((name, value)) = line.split_once('=') else {
            return false;
        };
        let value = value.trim();
        if name.trim().is_empty()
            || value.is_empty()
            || value.len() % 2 != 0
            || !value.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return false;
        }
        count += 1;
    }
    count > 0
}

pub fn user_dir(exe: &Path) -> PathBuf {
    let dir = exe.parent().expect("installed executable has a parent");
    let version = if dir.file_name().is_some_and(|n| n == "squashfs-root") {
        dir.parent().expect("AppImage has a version directory")
    } else {
        dir
    };
    #[cfg(windows)]
    if version.file_name().is_some_and(|n| n == "runtime") {
        return version.join("user");
    }
    let root = version.parent().expect("Eden has an install directory");
    if cfg!(windows) {
        root.join("runtime/user")
    } else {
        root.join("user")
    }
}

// Copy missing files atomically and retain the source until the whole migration succeeds.
fn merge_user(source: &Path, target: &Path) -> std::io::Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let metadata = source.symlink_metadata()?;
    if !metadata.is_dir() && !metadata.is_file() {
        return Err(std::io::Error::other(
            "Eden user data contains a link or unsupported file type",
        ));
    }
    if source.is_dir() {
        std::fs::create_dir_all(target)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            merge_user(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if !target.exists() {
        let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
        std::io::copy(&mut std::fs::File::open(source)?, &mut temp)?;
        temp.as_file().sync_all()?;
        match temp.persist_noclobber(target) {
            Ok(_) => {}
            Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.error),
        }
    }
    Ok(())
}

fn migrate_users(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let target = if cfg!(windows) {
        root.join("runtime/user")
    } else {
        root.join("user")
    };
    let mut versions = std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    versions.sort_by_key(|entry| {
        std::cmp::Reverse(firmware_version(&entry.file_name().to_string_lossy()))
    });
    for entry in versions {
        if !entry.file_name().to_string_lossy().starts_with('v') {
            continue;
        }
        let source = entry.path().join("user");
        if !source.exists() {
            continue;
        }
        let backups = root.join("legacy-users");
        std::fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
        let backup = backups.join(entry.file_name());
        if backup.exists() {
            return Err("An Eden user migration backup already exists".into());
        }
        merge_user(&source, &target).map_err(|e| e.to_string())?;
        std::fs::rename(&source, backup).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Upgrade an existing AppImage once, without FUSE or per-launch extraction.
pub fn ready(exe: PathBuf) -> Result<PathBuf, String> {
    let parent = exe.parent().ok_or("Missing Eden directory")?;
    let version = if parent.file_name().is_some_and(|n| n == "squashfs-root") {
        parent.parent().unwrap()
    } else {
        parent
    };
    let root = version
        .parent()
        .ok_or("Missing Eden install directory")?
        .to_path_buf();
    migrate_users(&root)?;
    #[cfg(target_os = "linux")]
    if exe.extension().is_some_and(|ext| ext == "AppImage") {
        let version = exe.parent().unwrap();
        let extracted = version.join("squashfs-root");
        if !extracted.join("AppRun").is_file() {
            let stage = tempfile::tempdir_in(version).map_err(|e| e.to_string())?;
            let output = std::process::Command::new(&exe)
                .arg("--appimage-extract")
                .current_dir(stage.path())
                .env_remove("APPIMAGE_EXTRACT_AND_RUN")
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err(format!(
                    "Could not extract Eden: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            // Eden's uruntime extracts squashfs-root as a link to AppDir.
            let tree = stage
                .path()
                .join("squashfs-root")
                .canonicalize()
                .map_err(|e| e.to_string())?;
            std::fs::rename(tree, &extracted).map_err(|e| e.to_string())?;
        }
        return Ok(extracted.join("AppRun"));
    }
    #[cfg(windows)]
    {
        let runtime = root.join("runtime");
        if exe.parent() == Some(runtime.as_path()) {
            return Ok(exe);
        }
        prepare_runtime(version, &runtime)?;
        return Ok(runtime.join("eden.exe"));
    }
    #[cfg(not(windows))]
    Ok(exe)
}

#[cfg(windows)]
fn prepare_runtime(source: &Path, runtime: &Path) -> Result<(), String> {
    let version = source
        .file_name()
        .ok_or("Missing Eden version")?
        .to_string_lossy();
    let marker = runtime.join("romlin-version");
    if runtime.join("eden.exe").is_file()
        && std::fs::read_to_string(&marker).ok().as_deref() == Some(version.as_ref())
    {
        return Ok(());
    }
    let stage = tempfile::tempdir_in(runtime.parent().ok_or("Missing Eden directory")?)
        .map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name() == "user" {
            continue;
        }
        copy_runtime(&entry.path(), &stage.path().join(entry.file_name()))
            .map_err(|e| e.to_string())?;
    }
    if !stage.path().join("eden.exe").is_file() {
        return Err("Eden runtime has no executable".into());
    }
    std::fs::create_dir_all(runtime).map_err(|e| e.to_string())?;
    // Keep portable data while removing files no longer shipped by the new version.
    for entry in std::fs::read_dir(runtime).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name() == "user" {
            continue;
        }
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            std::fs::remove_dir_all(entry.path())
        } else {
            std::fs::remove_file(entry.path())
        }
        .map_err(|e| e.to_string())?;
    }
    for entry in std::fs::read_dir(stage.path()).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        std::fs::rename(entry.path(), runtime.join(entry.file_name()))
            .map_err(|e| e.to_string())?;
    }
    crate::saves::replace_file(&marker, version.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn copy_runtime(source: &Path, target: &Path) -> std::io::Result<()> {
    if source.is_dir() {
        std::fs::create_dir_all(target)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            copy_runtime(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap())?;
        std::io::copy(&mut std::fs::File::open(source)?, &mut temp)?;
        temp.persist(target).map_err(|e| e.error)?;
    }
    Ok(())
}

/// Portable mode keeps Eden's keys, firmware and saves separate from any existing Eden installation.
pub fn prepare(exe: &Path, source: &Path) -> Result<(), String> {
    let home = user_dir(exe);
    let keys = home.join("keys");
    std::fs::create_dir_all(&keys).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&keys, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    let mut imports = Vec::new();
    for name in ["prod.keys", "title.keys"] {
        let path = source.join(name);
        if name == "title.keys" && !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        if !valid_keys(&text) {
            return Err(format!("{name} is not a valid key file"));
        }
        imports.push((name, text));
    }
    let firmware = find_firmware(source).ok_or("Switch firmware.zip is missing")?;
    tracing::info!(firmware_name = ?firmware.file_name(), "Switch firmware selected");
    install_firmware(&firmware, &home)
        .map_err(|e| format!("Could not install Switch firmware: {e}"))?;
    for (name, text) in imports {
        let path = keys.join(name);
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, text).map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn install_firmware(archive: &Path, home: &Path) -> Result<(), String> {
    if std::fs::metadata(archive).map_err(|e| e.to_string())?.len() > 1024 * 1024 * 1024 {
        return Err("Firmware archive is too large".into());
    }
    let bytes = std::fs::read(archive).map_err(|e| e.to_string())?;
    let hash = crate::cores::sha256_hex(&bytes);
    let content = home.join("nand/system/Contents");
    let registered = content.join("registered");
    let stamp = home.join("firmware.sha256");
    let firmware_name = archive.file_name().unwrap_or_default().to_string_lossy();
    if registered.is_dir() && std::fs::read_to_string(&stamp).ok().as_deref() == Some(&hash) {
        tracing::info!(%firmware_name, "Switch firmware already installed");
        return Ok(());
    }
    // Keep staging names so interrupted upstream installations remain recoverable.
    let stage = content.join("romp-firmware");
    if stage.exists() {
        std::fs::remove_dir_all(&stage).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    unpack_firmware(&bytes, &stage)?;
    let backup = content.join("romp-firmware-backup");
    // Never discard a backup left by an interrupted installation.
    if backup.exists() {
        return Err("A firmware backup already exists; restore it before retrying".into());
    }
    let had_old = registered.exists();
    if had_old {
        std::fs::rename(&registered, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(e) = std::fs::rename(&stage, &registered) {
        if had_old {
            let _ = std::fs::rename(&backup, &registered);
        }
        return Err(e.to_string());
    }
    if had_old {
        std::fs::remove_dir_all(&backup).map_err(|e| e.to_string())?;
    }
    std::fs::write(stamp, hash).map_err(|e| e.to_string())?;
    tracing::info!(%firmware_name, "Switch firmware installed");
    Ok(())
}

fn unpack_firmware(bytes: &[u8], stage: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut count = 0;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let path = entry
            .enclosed_name()
            .ok_or("Unsafe firmware archive path")?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid firmware filename")?;
        let filename = name.to_ascii_lowercase();
        let Some(stem) = filename.strip_suffix(".nca") else {
            continue;
        };
        let id = stem.strip_suffix(".cnmt").unwrap_or(stem);
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("Invalid firmware NCA filename: {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Firmware too large")?;
        if entry.size() == 0 || total > 1024 * 1024 * 1024 {
            return Err("Invalid firmware size".into());
        }
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.join(filename))
            .map_err(|e| e.to_string())?;
        let size = entry.size();
        let copied = std::io::copy(&mut std::io::Read::take(entry, size + 1), &mut out)
            .map_err(|e| e.to_string())?;
        if copied != size {
            return Err("Invalid firmware entry size".into());
        }
        count += 1;
    }
    if count == 0 {
        return Err("The firmware zip contains no NCA files".into());
    }
    Ok(())
}

pub fn launch(
    exe: &Path,
    rom: &Path,
    fullscreen: bool,
    steam_virtual: bool,
    on_exit: impl FnOnce(Option<i32>, Vec<String>) + Send + 'static,
) -> std::io::Result<Running> {
    let mut command = std::process::Command::new(exe);
    let home = user_dir(exe);
    std::fs::create_dir_all(&home)?;
    command.current_dir(home.parent().unwrap());
    #[cfg(windows)]
    command.current_dir(exe.parent().unwrap());
    command.env_remove("APPIMAGE_EXTRACT_AND_RUN");
    command.env_remove("APPIMAGE");
    command.env_remove("ARGV0");
    if steam_virtual {
        let ignored = std::env::var("SDL_GAMECONTROLLER_IGNORE_DEVICES").unwrap_or_default();
        command.env(
            "SDL_GAMECONTROLLER_IGNORE_DEVICES",
            format!("{ignored},0x28de/0x1205"),
        );
    }
    if fullscreen {
        command.arg("-f");
    }
    command.arg("-g").arg(rom);
    crate::external::launch(command, "Eden", on_exit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[cfg(windows)]
    #[test]
    fn runtime_is_cached_by_version_and_updates_remove_stale_files() {
        let dir = tempfile::tempdir().unwrap();
        let v1 = dir.path().join("v1");
        let v2 = dir.path().join("v2");
        let runtime = dir.path().join("runtime");
        std::fs::create_dir_all(&v1).unwrap();
        std::fs::create_dir_all(&v2).unwrap();
        std::fs::write(v1.join("eden.exe"), b"v1").unwrap();
        std::fs::write(v1.join("old.dll"), b"old").unwrap();
        std::fs::write(v2.join("eden.exe"), b"v2").unwrap();
        std::fs::create_dir_all(runtime.join("user")).unwrap();
        std::fs::write(runtime.join("user/save"), b"keep").unwrap();
        prepare_runtime(&v1, &runtime).unwrap();
        std::fs::write(v1.join("eden.exe"), b"must not recopy").unwrap();
        prepare_runtime(&v1, &runtime).unwrap();
        assert_eq!(std::fs::read(runtime.join("eden.exe")).unwrap(), b"v1");
        prepare_runtime(&v2, &runtime).unwrap();
        assert_eq!(std::fs::read(runtime.join("eden.exe")).unwrap(), b"v2");
        assert!(!runtime.join("old.dll").exists());
        assert_eq!(
            std::fs::read_to_string(runtime.join("romlin-version")).unwrap(),
            "v2"
        );
        assert_eq!(std::fs::read(runtime.join("user/save")).unwrap(), b"keep");
        let bad = dir.path().join("v3");
        std::fs::create_dir(&bad).unwrap();
        assert!(prepare_runtime(&bad, &runtime).is_err());
        assert_eq!(
            std::fs::read_to_string(runtime.join("romlin-version")).unwrap(),
            "v2"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn extracts_once_including_uruntimes_appdir_link() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("v1/eden.AppImage");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "#!/bin/sh\nmkdir AppDir\nprintf '#!/bin/sh\\nexit 0\\n' > AppDir/AppRun\nchmod +x AppDir/AppRun\nln -s AppDir squashfs-root\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let extracted = ready(exe.clone()).unwrap();
        assert!(extracted.is_file());
        assert_eq!(user_dir(&extracted), user_dir(&exe));
        std::fs::write(&exe, "broken executable").unwrap();
        assert_eq!(ready(exe).unwrap(), extracted);
    }

    #[test]
    fn migrates_without_overwriting_and_keeps_backups_outside_versions() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("v0.2.0/user");
        std::fs::create_dir_all(old.join("nand/user/save")).unwrap();
        std::fs::write(old.join("nand/user/save/progress"), b"save").unwrap();
        std::fs::write(old.join("settings"), b"old").unwrap();
        let home = user_dir(&dir.path().join("v0.2.1/eden"));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("settings"), b"new").unwrap();
        migrate_users(dir.path()).unwrap();
        assert_eq!(std::fs::read(home.join("settings")).unwrap(), b"new");
        assert_eq!(
            std::fs::read(home.join("nand/user/save/progress")).unwrap(),
            b"save"
        );
        assert_eq!(
            std::fs::read(dir.path().join("legacy-users/v0.2.0/settings")).unwrap(),
            b"old"
        );
        std::fs::remove_dir_all(dir.path().join("v0.2.0")).unwrap();
        migrate_users(dir.path()).unwrap();
        assert_eq!(
            std::fs::read(home.join("nand/user/save/progress")).unwrap(),
            b"save"
        );
    }

    #[cfg(unix)]
    #[test]
    fn failed_migration_retains_original_data() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("v1/user");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("save"), b"keep").unwrap();
        std::os::unix::fs::symlink("save", old.join("link")).unwrap();
        assert!(migrate_users(dir.path()).is_err());
        assert_eq!(std::fs::read(old.join("save")).unwrap(), b"keep");
    }

    fn firmware(name: &str, data: &[u8]) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn prepares_a_private_portable_install_and_optional_title_keys() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let exe = dir.path().join("v1/eden");
        std::fs::write(source.join("prod.keys"), "master_key_00 = 0123abcd").unwrap();
        std::fs::write(source.join("title.keys"), "abcd = 0123abcd").unwrap();
        std::fs::write(
            source.join("firmware.zip"),
            firmware("0123456789abcdef0123456789abcdef.nca", b"nca"),
        )
        .unwrap();
        prepare(&exe, &source).unwrap();
        let home = user_dir(&exe);
        assert_eq!(
            std::fs::read_to_string(home.join("keys/prod.keys")).unwrap(),
            "master_key_00 = 0123abcd"
        );
        assert!(home.join("keys/title.keys").is_file());
        std::fs::create_dir_all(home.join("nand/user/save")).unwrap();
        std::fs::write(home.join("nand/user/save/game"), b"save").unwrap();
        prepare(&exe, &source).unwrap();
        assert_eq!(
            std::fs::read(home.join("nand/user/save/game")).unwrap(),
            b"save"
        );
    }

    #[cfg(unix)]
    #[test]
    fn launches_the_game_in_portable_mode_with_fullscreen() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("v1/eden");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > args\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let _running = launch(
            &exe,
            Path::new("/roms/a game.nsp"),
            true,
            false,
            move |code, _| {
                tx.send(code).unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            Some(0)
        );
        let args = std::fs::read_to_string(dir.path().join("args")).unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            [cwd.to_str().unwrap(), "-f", "-g", "/roms/a game.nsp"]
        );
    }

    #[test]
    fn validates_keys_and_requires_firmware() {
        assert!(valid_keys("# comment\nmaster_key_00 = 0123abcd\n"));
        for bad in ["", "not keys", "key = zz", "key = 123", " = ab"] {
            assert!(!valid_keys(bad));
        }
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(missing(dir.path()), ["prod.keys", "firmware.zip"]);
    }

    #[test]
    fn firmware_versions_and_unversioned_zip_fallback() {
        assert_eq!(
            firmware_version("Switch Firmware v19.0.10 (Rebootless).zip"),
            Some(vec![19, 0, 10])
        );
        assert_eq!(firmware_version("Firmware_20.1.0.zip"), Some(vec![20, 1]));
        assert_eq!(firmware_version("firmware.zip"), None);
        let dir = tempfile::tempdir().unwrap();
        for (name, seconds) in [("firmware.zip", 10), ("dump.zip", 20)] {
            let path = dir.path().join(name);
            let file = std::fs::File::create(&path).unwrap();
            file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
                .unwrap();
        }
        assert_eq!(
            find_firmware(dir.path()).unwrap().file_name().unwrap(),
            "dump.zip"
        );
        std::fs::write(dir.path().join("Firmware v9.0.0.zip"), b"versioned").unwrap();
        assert_eq!(
            find_firmware(dir.path()).unwrap().file_name().unwrap(),
            "Firmware v9.0.0.zip"
        );
    }

    #[test]
    fn imports_the_newest_versioned_firmware() {
        for accepted in ["firmware.zip", "FIRMWARE.ZIP", "Firmware.18.0.1.zip"] {
            assert!(is_firmware_archive(accepted));
        }
        for rejected in ["firmware.18.zip.part", "prod.keys"] {
            assert!(!is_firmware_archive(rejected));
        }
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("prod.keys"), "master_key_00 = 0123abcd").unwrap();
        std::fs::write(source.join("unrelated.txt"), b"unrelated").unwrap();
        assert_eq!(missing(&source), ["firmware.zip"]);
        let name = "0123456789abcdef0123456789abcdef.nca";
        let archive = firmware(name, b"versioned");
        std::fs::write(source.join("Firmware.18.0.1.zip"), &archive).unwrap();
        assert!(missing(&source).is_empty());
        let exe = dir.path().join("v1/eden");
        prepare(&exe, &source).unwrap();
        let installed = user_dir(&exe)
            .join("nand/system/Contents/registered")
            .join(name);
        assert_eq!(std::fs::read(&installed).unwrap(), b"versioned");
        for version in ["9.0.0", "19.0.0", "19.0.10", "19.0.2"] {
            std::fs::write(
                source.join(format!("Firmware.{version}.zip")),
                firmware(name, version.as_bytes()),
            )
            .unwrap();
        }
        prepare(&exe, &source).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), b"19.0.10");
        std::fs::write(source.join("firmware.zip"), firmware(name, b"selected")).unwrap();
        prepare(&exe, &source).unwrap();
        assert_eq!(std::fs::read(installed).unwrap(), b"19.0.10");
    }

    #[test]
    fn imports_firmware_without_destroying_the_previous_install_on_bad_input() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("user");
        let archive = dir.path().join("firmware.zip");
        let name = "0123456789abcdef0123456789abcdef.nca";
        std::fs::write(&archive, firmware(&format!("firmware/{name}"), b"nca")).unwrap();
        install_firmware(&archive, &home).unwrap();
        let installed = home.join("nand/system/Contents/registered").join(name);
        assert_eq!(std::fs::read(&installed).unwrap(), b"nca");
        install_firmware(&archive, &home).unwrap();
        for bad in [
            "readme.txt",
            "../0123456789abcdef0123456789abcdef.nca",
            "bad.nca",
            "bad.cnmt.nca",
            "0123456789abcdef0123456789abcdef.cnmt.cnmt.nca",
        ] {
            std::fs::write(&archive, firmware(bad, b"bad")).unwrap();
            assert!(install_firmware(&archive, &home).is_err());
            assert_eq!(std::fs::read(&installed).unwrap(), b"nca");
        }
    }

    #[test]
    fn imports_metadata_ncas_with_the_filenames_eden_accepts() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("user");
        let archive = dir.path().join("Firmware.22.5.0.zip");
        for name in [
            "0123456789abcdef0123456789abcdef.cnmt.nca",
            "0123456789ABCDEF0123456789ABCDEF.NCA",
        ] {
            std::fs::write(&archive, firmware(&format!("firmware/{name}"), b"metadata")).unwrap();
            install_firmware(&archive, &home).unwrap();
            assert_eq!(
                std::fs::read(
                    home.join("nand/system/Contents/registered")
                        .join(name.to_ascii_lowercase())
                )
                .unwrap(),
                b"metadata"
            );
        }
    }
}
