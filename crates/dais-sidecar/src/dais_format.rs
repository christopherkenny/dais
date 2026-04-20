use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::format::{SidecarError, SidecarFormat};
use crate::types::{PresentationMetadata, SlideGroupMeta};

/// The native `.dais` sidecar format, stored as EON.
pub struct DaisFormat;

#[derive(Serialize, Deserialize)]
struct DaisFile {
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_slide: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_minutes: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    groups: Vec<DaisGroup>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    notes: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    slide_timings: HashMap<String, f64>,
}

#[derive(Serialize, Deserialize)]
struct DaisGroup {
    start_page: usize,
    end_page: usize,
}

impl DaisFile {
    fn from_metadata(meta: &PresentationMetadata) -> Self {
        Self {
            version: 1,
            title: meta.title.clone(),
            end_slide: meta.end_slide,
            last_minutes: meta.last_minutes,
            groups: meta
                .groups
                .iter()
                .map(|g| DaisGroup { start_page: g.start_page, end_page: g.end_page })
                .collect(),
            notes: meta.notes.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            slide_timings: meta.slide_timings.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }

    fn into_metadata(self) -> PresentationMetadata {
        PresentationMetadata {
            title: self.title,
            end_slide: self.end_slide,
            last_minutes: self.last_minutes,
            groups: self
                .groups
                .into_iter()
                .map(|g| SlideGroupMeta { start_page: g.start_page, end_page: g.end_page })
                .collect(),
            notes: self
                .notes
                .into_iter()
                .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v)))
                .collect(),
            slide_timings: self
                .slide_timings
                .into_iter()
                .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v)))
                .collect(),
        }
    }
}

impl SidecarFormat for DaisFormat {
    fn read(&self, path: &Path) -> Result<PresentationMetadata, SidecarError> {
        let content = std::fs::read_to_string(path)?;
        let file: DaisFile = eon::from_str(&content)
            .map_err(|err| SidecarError::Parse { line: 0, message: err.to_string() })?;
        Ok(file.into_metadata())
    }

    fn write(&self, path: &Path, metadata: &PresentationMetadata) -> Result<(), SidecarError> {
        let file = DaisFile::from_metadata(metadata);
        let options = eon::FormatOptions::default();
        let content = eon::to_string(&file, &options)
            .map_err(|err| SidecarError::Parse { line: 0, message: err.to_string() })?;
        std::fs::write(path, content)?;
        Ok(())
    }

    fn file_extension(&self) -> &'static str {
        "dais"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("dais_test_dais_format");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn roundtrip_empty_metadata() {
        let dir = test_dir();
        let path = dir.join("empty.dais");
        let format = DaisFormat;

        let original = PresentationMetadata::default();
        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        assert!(loaded.title.is_none());
        assert!(loaded.groups.is_empty());
        assert!(loaded.notes.is_empty());
        assert!(loaded.end_slide.is_none());
        assert!(loaded.last_minutes.is_none());
        assert!(loaded.slide_timings.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_with_all_fields() {
        let dir = test_dir();
        let path = dir.join("full.dais");
        let format = DaisFormat;

        let original = PresentationMetadata {
            title: Some("My Presentation".to_string()),
            end_slide: Some(25),
            last_minutes: Some(20),
            groups: vec![
                SlideGroupMeta { start_page: 0, end_page: 2 },
                SlideGroupMeta { start_page: 3, end_page: 3 },
            ],
            notes: {
                let mut n = HashMap::new();
                n.insert(0, "Welcome everyone".to_string());
                n.insert(5, "Key point here".to_string());
                n
            },
            slide_timings: {
                let mut t = HashMap::new();
                t.insert(0, 12.5);
                t.insert(1, 45.0);
                t
            },
        };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        assert_eq!(loaded.title.as_deref(), Some("My Presentation"));
        assert_eq!(loaded.end_slide, Some(25));
        assert_eq!(loaded.last_minutes, Some(20));
        assert_eq!(loaded.groups.len(), 2);
        assert_eq!(loaded.groups[0].start_page, 0);
        assert_eq!(loaded.groups[0].end_page, 2);
        assert_eq!(loaded.groups[1].start_page, 3);
        assert_eq!(loaded.groups[1].end_page, 3);
        assert_eq!(loaded.notes.len(), 2);
        assert_eq!(loaded.notes[&0], "Welcome everyone");
        assert_eq!(loaded.notes[&5], "Key point here");
        assert_eq!(loaded.slide_timings.len(), 2);
        assert!((loaded.slide_timings[&0] - 12.5).abs() < f64::EPSILON);
        assert!((loaded.slide_timings[&1] - 45.0).abs() < f64::EPSILON);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn version_field_is_present() {
        let dir = test_dir();
        let path = dir.join("version_check.dais");
        let format = DaisFormat;

        format.write(&path, &PresentationMetadata::default()).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("version: 1"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unknown_version_still_parses() {
        let dir = test_dir();
        let path = dir.join("future_version.dais");

        let content = "version: 2\ntitle: \"Future talk\"\n";
        std::fs::write(&path, content).unwrap();

        let format = DaisFormat;
        let loaded = format.read(&path).unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Future talk"));

        let _ = std::fs::remove_file(&path);
    }
}
