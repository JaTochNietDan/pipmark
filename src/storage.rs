use crate::model::Settings;
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const MAX_SETTINGS_BYTES: usize = 4 * 1024 * 1024;
pub fn config_path() -> PathBuf {
    std::env::var_os("PIPMARK_CONFIG").or_else(|| std::env::var_os("BADGEIFY_CONFIG"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let support = PathBuf::from(std::env::var_os("HOME").expect("HOME is required"))
                .join("Library/Application Support");
            let current = support.join("Pipmark/settings.json");
            let legacy = support.join("Badgeify Native/settings.json");
            // Preserve existing settings and absolute icon references during the rebrand.
            if !current.exists() && legacy.exists() { legacy } else { current }
        })
}
/// Strict reads are used for imports/backups: a missing file is never an empty config.
pub fn read(path: &Path) -> Result<Settings, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| {
            file.take((MAX_SETTINGS_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err("Settings file exceeds 4 MiB".into());
    }
    let settings: Settings = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    settings.validate()?;
    Ok(settings)
}
pub fn load(path: &Path) -> Result<Settings, String> {
    if !path.try_exists().map_err(|e| e.to_string())? {
        return Ok(Settings::default());
    }
    read(path)
}
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = path.with_extension(format!("tmp-{}-{suffix}", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
fn encode(settings: &Settings) -> Result<Vec<u8>, String> {
    settings.validate()?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_SETTINGS_BYTES {
        return Err("Settings file exceeds 4 MiB".into());
    }
    Ok(bytes)
}
pub fn export(path: &Path, settings: &Settings) -> Result<(), String> {
    write_atomic(path, &encode(settings)?).map_err(|e| e.to_string())
}
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    // Reject unwritable settings before rotating the previous backup.
    let bytes = encode(settings)?;
    if path.exists() {
        // Never rotate a corrupt primary over a known-good backup.
        read(path)?;
        let old = fs::read(path).map_err(|e| e.to_string())?;
        write_atomic(&path.with_extension("json.backup"), &old).map_err(|e| e.to_string())?;
    }
    write_atomic(path, &bytes).map_err(|e| e.to_string())
}
pub fn restore_backup(path: &Path) -> Result<Settings, String> {
    let settings = read(&path.with_extension("json.backup"))?;
    if path.exists() {
        let old = fs::read(path).map_err(|e| e.to_string())?;
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        write_atomic(
            &path.with_extension(format!("json.recovered-{suffix}")),
            &old,
        )
        .map_err(|e| e.to_string())?;
    }
    export(path, &settings)?;
    Ok(settings)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn oversized_save_preserves_primary_and_previous_backup() {
        let dir = std::env::temp_dir().join(format!("badgeify-oversized-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        save(&path, &settings).unwrap();
        settings.show_on_start = false;
        save(&path, &settings).unwrap();
        let current = fs::read(&path).unwrap();
        let previous = fs::read(path.with_extension("json.backup")).unwrap();
        settings.items.push(crate::model::Item {
            id: "large".into(),
            name: "x".repeat(MAX_SETTINGS_BYTES),
            enabled: true,
            appearance: Default::default(),
            kind: crate::model::ItemKind::Group { children: vec![] },
        });
        assert!(save(&path, &settings).is_err());
        assert_eq!(fs::read(&path).unwrap(), current);
        assert_eq!(
            fs::read(path.with_extension("json.backup")).unwrap(),
            previous
        );
        fs::write(dir.join("large.json"), vec![b' '; MAX_SETTINGS_BYTES + 1]).unwrap();
        assert_eq!(
            read(&dir.join("large.json")).unwrap_err(),
            "Settings file exceeds 4 MiB"
        );
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_primary_cannot_destroy_good_backup() {
        let dir = std::env::temp_dir().join(format!("badgeify-recovery-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let settings = Settings::default();
        save(&path, &settings).unwrap();
        save(&path, &settings).unwrap();
        let backup = fs::read(path.with_extension("json.backup")).unwrap();
        fs::write(&path, b"broken settings").unwrap();
        assert!(save(&path, &settings).is_err());
        assert_eq!(
            fs::read(path.with_extension("json.backup")).unwrap(),
            backup
        );
        assert_eq!(fs::read(&path).unwrap(), b"broken settings");
        restore_backup(&path).unwrap();
        assert!(read(&path).is_ok());
        let preserved = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|e| e.file_name().to_string_lossy().contains("recovered-"))
            .unwrap();
        assert_eq!(fs::read(preserved.path()).unwrap(), b"broken settings");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn missing_import_and_backup_do_not_become_empty_settings() {
        let dir = std::env::temp_dir().join(format!("badgeify-missing-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        save(&path, &Settings::default()).unwrap();
        let old = fs::read(&path).unwrap();
        assert!(read(&dir.join("nonexistent.json")).is_err());
        assert!(restore_backup(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), old);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn saves_atomically_and_keeps_previous_version() {
        let dir = std::env::temp_dir().join(format!("badgeify-storage-{}", std::process::id()));
        let p = dir.join("settings.json");
        let mut s = Settings::default();
        save(&p, &s).unwrap();
        s.show_on_start = false;
        save(&p, &s).unwrap();
        assert!(!load(&p).unwrap().show_on_start);
        assert!(
            load(&p.with_extension("json.backup"))
                .unwrap()
                .show_on_start
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
