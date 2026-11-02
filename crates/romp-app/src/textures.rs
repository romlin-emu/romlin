use anyhow::{bail, Context, Result};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};

/// Install only texture images, stripping a pack's optional serial/replacements wrapper.
pub fn install(zip: &Path, save: &Path) -> Result<()> {
    fs::create_dir_all(save)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(save.join("textures.lock"))?;
    lock.try_lock()
        .context("Another texture installation is in progress")?;
    recover(save)?;
    let stage = tempfile::tempdir_in(save)?;
    let replacements = stage.path().join("textures/replacements");
    fs::create_dir_all(&replacements)?;
    let mut archive = zip::ZipArchive::new(File::open(zip)?).context("Open texture ZIP")?;
    let mut files = Vec::new();
    let mut roots = std::collections::HashSet::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let path = entry
            .enclosed_name()
            .context("Unsafe path in texture ZIP")?;
        if path
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        {
            bail!("Unsafe entry in texture ZIP");
        }
        if entry.is_dir() {
            continue;
        }
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("png") || e.eq_ignore_ascii_case("dds"))
        {
            continue;
        }
        let parts: Vec<_> = path.components().collect();
        let root = parts
            .iter()
            .position(|c| c.as_os_str().eq_ignore_ascii_case("replacements"));
        let relative = if let Some(root) = root {
            roots.insert(parts[..root].iter().collect::<std::path::PathBuf>());
            parts[root + 1..].iter().collect::<std::path::PathBuf>()
        } else {
            path.clone()
        };
        total = total
            .checked_add(entry.size())
            .context("Texture ZIP is too large")?;
        // Bound extraction of an untrusted archive, including packs with many tiny files.
        if total > 64 << 30 || files.len() >= 500_000 {
            bail!("Texture ZIP exceeds 64 GiB or 500,000 images");
        }
        files.push((i, relative, root.is_some()));
    }
    if files.is_empty() || roots.len() > 1 {
        bail!("Select a ZIP containing one texture pack with PNG or DDS images");
    }
    let wrapped = !roots.is_empty();
    for (i, relative, in_replacements) in files {
        if wrapped && !in_replacements {
            continue;
        }
        let dest = replacements.join(relative);
        fs::create_dir_all(dest.parent().unwrap())?;
        let mut output = File::options()
            .write(true)
            .create_new(true)
            .open(&dest)
            .context("Duplicate texture filename in ZIP")?;
        let entry = archive.by_index(i)?;
        let expected = entry.size();
        let copied = std::io::copy(&mut entry.take(expected + 1), &mut output)
            .context("Extract texture image")?;
        if copied != expected {
            bail!("Incorrect texture size in ZIP");
        }
    }
    let target = save.join("textures");
    let previous = save.join("textures.previous");
    // Recover an interrupted replacement before attempting another install.
    if previous.exists() {
        if target.exists() {
            fs::remove_dir_all(&previous)?;
        } else {
            fs::rename(&previous, &target)?;
        }
    }
    if target.exists() {
        fs::rename(&target, &previous)?;
    }
    if let Err(error) = fs::rename(stage.path().join("textures"), &target) {
        if previous.exists() {
            fs::rename(&previous, &target).context("Restore previous texture pack")?;
        }
        return Err(error.into());
    }
    if previous.exists() {
        fs::remove_dir_all(previous)?;
    }
    Ok(())
}

/// Finish a pack replacement interrupted between the two directory renames.
pub fn recover(save: &Path) -> Result<()> {
    let target = save.join("textures");
    let previous = save.join("textures.previous");
    if !target.exists() && previous.exists() {
        fs::rename(previous, target).context("Recover previous texture pack")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pack(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let path = dir.join("pack.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    #[test]
    fn installs_wrapped_pack_and_replaces_without_retaining_old_images() {
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("game");
        install(
            &pack(
                dir.path(),
                &[
                    ("SLUS-12345/replacements/sub/a.PNG", b"old"),
                    ("preview.png", b"preview"),
                ],
            ),
            &save,
        )
        .unwrap();
        assert_eq!(
            fs::read(save.join("textures/replacements/sub/a.PNG")).unwrap(),
            b"old"
        );
        install(&pack(dir.path(), &[("b.dds", b"new")]), &save).unwrap();
        assert!(!save.join("textures/replacements/sub/a.PNG").exists());
        assert!(!save.join("textures/replacements/preview.png").exists());
        assert_eq!(
            fs::read(save.join("textures/replacements/b.dds")).unwrap(),
            b"new"
        );
    }

    #[test]
    fn interrupted_install_can_be_recovered_and_symlinks_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("game");
        install(&pack(dir.path(), &[("a.png", b"original")]), &save).unwrap();
        fs::rename(save.join("textures"), save.join("textures.previous")).unwrap();
        recover(&save).unwrap();
        assert_eq!(
            fs::read(save.join("textures/replacements/a.png")).unwrap(),
            b"original"
        );
        let path = dir.path().join("link.zip");
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        zip.add_symlink(
            "link.png",
            "../../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.finish().unwrap();
        assert!(install(&path, &save).is_err());
        assert_eq!(
            fs::read(save.join("textures/replacements/a.png")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn a_concurrent_install_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("game");
        fs::create_dir_all(&save).unwrap();
        let lock = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(save.join("textures.lock"))
            .unwrap();
        lock.lock().unwrap();
        let zip = pack(dir.path(), &[("a.png", b"texture")]);
        assert!(install(&zip, &save).is_err());
        assert!(!save.join("textures").exists());
        drop(lock);
        install(&zip, &save).unwrap();
    }

    #[test]
    fn invalid_or_ambiguous_archives_preserve_installed_pack() {
        let dir = tempfile::tempdir().unwrap();
        let save = dir.path().join("game");
        install(&pack(dir.path(), &[("a.png", b"original")]), &save).unwrap();
        for entries in [
            vec![("../escape.png", b"bad".as_slice())],
            vec![("readme.txt", b"bad".as_slice())],
            vec![
                ("A/replacements/a.png", b"a".as_slice()),
                ("B/replacements/b.png", b"b".as_slice()),
            ],
        ] {
            assert!(install(&pack(dir.path(), &entries), &save).is_err());
            assert_eq!(
                fs::read(save.join("textures/replacements/a.png")).unwrap(),
                b"original"
            );
        }
        assert!(!dir.path().join("escape.png").exists());
    }
}
