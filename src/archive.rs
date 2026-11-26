//! Portable settings envelope. Imported asset identifiers are never filesystem paths.
use crate::{
    model::{Appearance, Item, ItemKind, Settings},
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    format: String,
    version: u32,
    pub settings: Settings,
    assets: HashMap<String, Asset>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    template: bool,
    png: Vec<u8>,
}
/// Own only the fresh asset paths created by this import. Existing icons and
/// assets referenced by backups must never be included in rollback cleanup.
#[derive(Default)]
struct PendingAssets(Vec<PathBuf>);
impl Drop for PendingAssets {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}
fn visit(
    settings: &mut Settings,
    mut f: impl FnMut(&mut Appearance) -> Result<(), String>,
) -> Result<(), String> {
    fn walk(
        items: &mut [Item],
        f: &mut impl FnMut(&mut Appearance) -> Result<(), String>,
    ) -> Result<(), String> {
        for item in items {
            f(&mut item.appearance)?;
            if let ItemKind::Group { children } = &mut item.kind {
                walk(children, f)?;
            }
        }
        Ok(())
    }
    f(&mut settings.defaults)?;
    walk(&mut settings.items, &mut f)
}
fn validate_png(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > 128 * 1024
        || bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
        || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != 64
        || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != 64
    {
        return Err("Icon assets must be 64 × 64 PNG images".into());
    }
    resvg::tiny_skia::Pixmap::decode_png(bytes).map_err(|e| format!("Invalid icon PNG: {e}"))?;
    Ok(())
}
impl Archive {
    pub fn from_settings(settings: &Settings) -> Result<Self, String> {
        settings.validate()?;
        let mut archive = Self {
            format: "badgeify-native".into(),
            version: 1,
            settings: settings.clone(),
            assets: HashMap::new(),
        };
        let mut references = HashMap::<String, String>::new();
        visit(&mut archive.settings, |a| {
            let Some(path) = a.custom_icon.clone() else {
                return Ok(());
            };
            let key = if let Some(key) = references.get(&path) {
                key.clone()
            } else {
                if std::fs::metadata(&path)
                    .map_err(|e| format!("Cannot export icon {path}: {e}"))?
                    .len()
                    > 128 * 1024
                {
                    return Err("Icon asset is too large".into());
                }
                let png = std::fs::read(&path).map_err(|e| e.to_string())?;
                validate_png(&png)?;
                let key = format!("icon-{}", archive.assets.len());
                let template = Path::new(&path)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("template-"));
                archive.assets.insert(key.clone(), Asset { template, png });
                references.insert(path, key.clone());
                key
            };
            a.custom_icon = Some(format!("asset:{key}"));
            Ok(())
        })?;
        Ok(archive)
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
            return Err("Import exceeds 32 MiB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if value.get("format").is_none() {
            // Older local settings files can still be imported; normalize their icons.
            return Self::from_settings(&storage::read(path)?);
        }
        let mut archive: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if archive.format != "badgeify-native" || archive.version != 1 {
            return Err("Unsupported settings archive".into());
        }
        archive.settings.validate()?;
        for asset in archive.assets.values() {
            validate_png(&asset.png)?;
        }
        visit(&mut archive.settings, |a| {
            if let Some(reference) = &a.custom_icon {
                let key = reference
                    .strip_prefix("asset:")
                    .ok_or("Archive icon is not an embedded asset")?;
                if !archive.assets.contains_key(key) {
                    return Err("Archive is missing an icon asset".into());
                }
            }
            Ok(())
        })?;
        Ok(archive)
    }
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err("Export exceeds 32 MiB".into());
        }
        storage::write_atomic(path, &bytes).map_err(|e| e.to_string())
    }
    /// Called only after the user accepts the import preview.
    pub fn install(mut self, config: &Path) -> Result<Settings, String> {
        let directory = config
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("icons");
        let mut pending = PendingAssets::default();
        let mut paths = HashMap::new();
        for (key, asset) in self.assets {
            let name = format!(
                "{}-{}.png",
                if asset.template { "template" } else { "color" },
                objc2_foundation::NSUUID::UUID().UUIDString()
            );
            let path = directory.join(name);
            storage::write_atomic(&path, &asset.png).map_err(|e| e.to_string())?;
            paths.insert(key, path.to_string_lossy().into_owned());
            pending.0.push(path);
        }
        visit(&mut self.settings, |a| {
            if let Some(reference) = &a.custom_icon {
                let key = reference
                    .strip_prefix("asset:")
                    .ok_or("Invalid asset reference")?;
                a.custom_icon = Some(paths.get(key).ok_or("Missing asset")?.clone());
            }
            Ok(())
        })?;
        storage::save(config, &self.settings)?;
        // The configuration now references these assets; ownership passes to it.
        pending.0.clear();
        Ok(self.settings)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_import_removes_only_new_assets_and_preserves_current_settings() {
        let dir =
            std::env::temp_dir().join(format!("badgeify-import-rollback-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("icons")).unwrap();
        let old_icon = dir.join("icons/template-existing.png");
        let png = crate::icons::rasterize(include_bytes!("../examples/assets/monochrome.svg"))
            .unwrap()
            .png;
        std::fs::write(&old_icon, &png).unwrap();
        let mut settings = Settings::default();
        settings.defaults.custom_icon = Some(old_icon.to_string_lossy().into_owned());
        let config = dir.join("settings.json");
        storage::save(&config, &settings).unwrap();
        let original = std::fs::read(&config).unwrap();
        // Force failure after materialization when rotating the previous settings.
        std::fs::create_dir(config.with_extension("json.backup")).unwrap();
        let mut archive = Archive::from_settings(&settings).unwrap();
        archive.assets.insert(
            "extra".into(),
            Asset {
                template: false,
                png: png.clone(),
            },
        );
        assert!(archive.install(&config).is_err());
        assert_eq!(std::fs::read(&config).unwrap(), original);
        assert_eq!(std::fs::read(&old_icon).unwrap(), png);
        assert_eq!(std::fs::read_dir(dir.join("icons")).unwrap().count(), 1);
        // A reference resolution error after writing assets must roll back as well.
        let mut archive = Archive::from_settings(&settings).unwrap();
        archive.settings.defaults.custom_icon = Some("asset:missing".into());
        assert!(archive.install(&config).is_err());
        assert_eq!(std::fs::read_dir(dir.join("icons")).unwrap().count(), 1);
        assert_eq!(std::fs::read(&config).unwrap(), original);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn archive_survives_deleted_source_and_deduplicates_icons() {
        let dir = std::env::temp_dir().join(format!("badgeify-archive-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("template-source.png");
        let png = crate::icons::rasterize(include_bytes!("../examples/assets/monochrome.svg"))
            .unwrap()
            .png;
        std::fs::write(&source, &png).unwrap();
        let mut settings = Settings::default();
        settings.defaults.custom_icon = Some(source.to_string_lossy().into_owned());
        settings.items.push(Item {
            id: "group".into(),
            name: "Group".into(),
            enabled: true,
            appearance: settings.defaults.clone(),
            kind: ItemKind::Group { children: vec![] },
        });
        let archive = Archive::from_settings(&settings).unwrap();
        assert_eq!(archive.assets.len(), 1);
        let file = dir.join("export.json");
        archive.write(&file).unwrap();
        std::fs::remove_file(source).unwrap();
        let imported = Archive::read(&file)
            .unwrap()
            .install(&dir.join("new-mac/settings.json"))
            .unwrap();
        let path = imported.defaults.custom_icon.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), png);
        assert_eq!(
            imported.items[0].appearance.custom_icon.as_deref(),
            Some(path.as_str())
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_bad_png_before_decoding_large_dimensions() {
        assert!(validate_png(b"bad").is_err());
        let mut png = vec![0; 24];
        png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[12..16].copy_from_slice(b"IHDR");
        png[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(validate_png(&png).is_err());
    }
}
