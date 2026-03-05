use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontMeta {
    pub version: u8,
    pub width: u32,
    pub height: u32,
    pub num_glyphs: u32,
    pub flags: u32,
}

impl FontMeta {
    pub fn load(path: &Path) -> Result<Self, String> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {}", path.display(), e))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("invalid JSON in {}: {}", path.display(), e))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("JSON error: {}", e))?;
        std::fs::write(path, data)
            .map_err(|e| format!("cannot write {}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_json() {
        let meta = FontMeta { version: 2, width: 8, height: 16, num_glyphs: 512, flags: 0 };
        let tmp = std::env::temp_dir().join("psftool_test_meta.json");
        meta.save(&tmp).unwrap();
        let loaded = FontMeta::load(&tmp).unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.width, 8);
        assert_eq!(loaded.height, 16);
        assert_eq!(loaded.num_glyphs, 512);
        assert_eq!(loaded.flags, 0);
        std::fs::remove_file(&tmp).ok();
    }
}
