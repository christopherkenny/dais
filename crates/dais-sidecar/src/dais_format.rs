use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::format::{SidecarError, SidecarFormat};
use crate::types::{InkStrokeMeta, PresentationMetadata, SlideGroupMeta, TextBoxMeta};

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
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    slide_annotations: HashMap<String, Vec<DaisInkStroke>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    whiteboard_annotations: Vec<DaisInkStroke>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    slide_text_boxes: HashMap<String, Vec<DaisTextBox>>,
}

#[derive(Serialize, Deserialize)]
struct DaisGroup {
    start_page: usize,
    end_page: usize,
}

#[derive(Serialize, Deserialize)]
struct DaisInkStroke {
    points: Vec<(f32, f32)>,
    color: [u8; 4],
    width: f32,
}

#[derive(Serialize, Deserialize)]
struct DaisTextBox {
    id: u64,
    rect: (f32, f32, f32, f32),
    content: String,
    font_size: f32,
    color: [u8; 4],
    #[serde(skip_serializing_if = "Option::is_none")]
    background: Option<[u8; 4]>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    typst_prelude: String,
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
            slide_annotations: meta
                .slide_annotations
                .iter()
                .filter(|(_, strokes)| !strokes.is_empty())
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.iter()
                            .map(|s| DaisInkStroke {
                                points: s.points.clone(),
                                color: s.color,
                                width: s.width,
                            })
                            .collect(),
                    )
                })
                .collect(),
            whiteboard_annotations: meta
                .whiteboard_annotations
                .iter()
                .map(|s| DaisInkStroke { points: s.points.clone(), color: s.color, width: s.width })
                .collect(),
            slide_text_boxes: meta
                .slide_text_boxes
                .iter()
                .filter(|(_, boxes)| !boxes.is_empty())
                .map(|(k, v)| {
                    (
                        k.to_string(),
                        v.iter()
                            .map(|tb| DaisTextBox {
                                id: tb.id,
                                rect: tb.rect,
                                content: tb.content.clone(),
                                font_size: tb.font_size,
                                color: tb.color,
                                background: tb.background,
                                typst_prelude: tb.typst_prelude.clone(),
                            })
                            .collect(),
                    )
                })
                .collect(),
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
            slide_annotations: self
                .slide_annotations
                .into_iter()
                .filter_map(|(k, v)| {
                    k.parse::<usize>().ok().map(|idx| {
                        (
                            idx,
                            v.into_iter()
                                .map(|s| InkStrokeMeta {
                                    points: s.points,
                                    color: s.color,
                                    width: s.width,
                                })
                                .collect(),
                        )
                    })
                })
                .collect(),
            whiteboard_annotations: self
                .whiteboard_annotations
                .into_iter()
                .map(|s| InkStrokeMeta { points: s.points, color: s.color, width: s.width })
                .collect(),
            slide_text_boxes: self
                .slide_text_boxes
                .into_iter()
                .filter_map(|(k, v)| {
                    k.parse::<usize>().ok().map(|idx| {
                        (
                            idx,
                            v.into_iter()
                                .map(|tb| TextBoxMeta {
                                    id: tb.id,
                                    rect: tb.rect,
                                    content: tb.content,
                                    font_size: tb.font_size,
                                    color: tb.color,
                                    background: tb.background,
                                    typst_prelude: tb.typst_prelude,
                                })
                                .collect(),
                        )
                    })
                })
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
            slide_annotations: HashMap::new(),
            whiteboard_annotations: Vec::new(),
            slide_text_boxes: HashMap::new(),
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

    #[test]
    fn roundtrip_slide_annotations() {
        let dir = test_dir();
        let path = dir.join("annotations.dais");
        let format = DaisFormat;

        let mut slide_annotations = HashMap::new();
        slide_annotations.insert(
            0,
            vec![InkStrokeMeta {
                points: vec![(0.1, 0.2), (0.3, 0.4)],
                color: [255, 0, 0, 255],
                width: 3.0,
            }],
        );
        slide_annotations.insert(
            5,
            vec![
                InkStrokeMeta { points: vec![(0.5, 0.5)], color: [0, 255, 0, 255], width: 2.0 },
                InkStrokeMeta {
                    points: vec![(0.7, 0.8), (0.9, 0.1)],
                    color: [0, 0, 255, 128],
                    width: 5.0,
                },
            ],
        );

        let original = PresentationMetadata { slide_annotations, ..Default::default() };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        assert_eq!(loaded.slide_annotations.len(), 2);
        assert_eq!(loaded.slide_annotations[&0].len(), 1);
        assert_eq!(loaded.slide_annotations[&0][0].points, vec![(0.1, 0.2), (0.3, 0.4)]);
        assert_eq!(loaded.slide_annotations[&0][0].color, [255, 0, 0, 255]);
        assert!((loaded.slide_annotations[&0][0].width - 3.0).abs() < f32::EPSILON);
        assert_eq!(loaded.slide_annotations[&5].len(), 2);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn roundtrip_whiteboard_annotations() {
        let dir = test_dir();
        let path = dir.join("whiteboard.dais");
        let format = DaisFormat;

        let original = PresentationMetadata {
            whiteboard_annotations: vec![InkStrokeMeta {
                points: vec![(0.1, 0.1), (0.9, 0.9)],
                color: [0, 0, 0, 255],
                width: 4.0,
            }],
            ..Default::default()
        };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        assert_eq!(loaded.whiteboard_annotations.len(), 1);
        assert_eq!(loaded.whiteboard_annotations[0].points, vec![(0.1, 0.1), (0.9, 0.9)]);
        assert_eq!(loaded.whiteboard_annotations[0].color, [0, 0, 0, 255]);
        assert!((loaded.whiteboard_annotations[0].width - 4.0).abs() < f32::EPSILON);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_roundtrips_for_slide_annotations() {
        let dir = test_dir();
        let path = dir.join("alpha_slide.dais");
        let format = DaisFormat;

        let mut slide_annotations = HashMap::new();
        slide_annotations.insert(
            0,
            vec![InkStrokeMeta {
                points: vec![(0.1, 0.2)],
                color: [255, 128, 0, 77], // non-opaque alpha
                width: 3.0,
            }],
        );
        let original = PresentationMetadata { slide_annotations, ..Default::default() };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        let stroke = &loaded.slide_annotations[&0][0];
        assert_eq!(stroke.color, [255, 128, 0, 77], "RGBA including alpha must roundtrip exactly");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn alpha_roundtrips_for_whiteboard_annotations() {
        let dir = test_dir();
        let path = dir.join("alpha_whiteboard.dais");
        let format = DaisFormat;

        let original = PresentationMetadata {
            whiteboard_annotations: vec![InkStrokeMeta {
                points: vec![(0.5, 0.5)],
                color: [0, 200, 255, 51], // 20% alpha — highlighter-like
                width: 8.0,
            }],
            ..Default::default()
        };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        let stroke = &loaded.whiteboard_annotations[0];
        assert_eq!(stroke.color, [0, 200, 255, 51], "Whiteboard RGBA + alpha must roundtrip");
        assert!((stroke.width - 8.0).abs() < f32::EPSILON, "Whiteboard width must roundtrip");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn width_roundtrips_for_non_default_values() {
        let dir = test_dir();
        let path = dir.join("width.dais");
        let format = DaisFormat;

        let mut slide_annotations = HashMap::new();
        slide_annotations.insert(
            2,
            vec![InkStrokeMeta { points: vec![(0.0, 0.0)], color: [0, 0, 0, 255], width: 12.5 }],
        );
        let original = PresentationMetadata { slide_annotations, ..Default::default() };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        let stroke = &loaded.slide_annotations[&2][0];
        assert!((stroke.width - 12.5).abs() < f32::EPSILON, "Non-default width must roundtrip");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_annotation_fields_parse_cleanly() {
        let dir = test_dir();
        let path = dir.join("no_annotations.dais");

        // A .dais file with no annotation fields — older format
        let content = "version: 1\ntitle: \"No annotations\"\n";
        std::fs::write(&path, content).unwrap();

        let format = DaisFormat;
        let loaded = format.read(&path).unwrap();
        assert_eq!(loaded.title.as_deref(), Some("No annotations"));
        assert!(loaded.slide_annotations.is_empty());
        assert!(loaded.whiteboard_annotations.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn text_box_typst_prelude_roundtrips() {
        let dir = test_dir();
        let path = dir.join("text_box_prelude.dais");
        let format = DaisFormat;

        let mut slide_text_boxes = HashMap::new();
        slide_text_boxes.insert(
            0,
            vec![TextBoxMeta {
                id: 7,
                rect: (0.1, 0.2, 0.3, 0.4),
                content: "$pi r^2$".to_string(),
                font_size: 24.0,
                color: [0, 0, 0, 255],
                background: None,
                typst_prelude: "#set align(horizon)".to_string(),
            }],
        );
        let original = PresentationMetadata { slide_text_boxes, ..Default::default() };

        format.write(&path, &original).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("typst_prelude"));

        let loaded = format.read(&path).unwrap();
        assert_eq!(loaded.slide_text_boxes[&0][0].typst_prelude, "#set align(horizon)");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn text_box_without_typst_prelude_parses_as_empty() {
        let dir = test_dir();
        let path = dir.join("text_box_no_prelude.dais");
        let format = DaisFormat;

        let mut slide_text_boxes = HashMap::new();
        slide_text_boxes.insert(
            0,
            vec![TextBoxMeta {
                id: 1,
                rect: (0.1, 0.2, 0.3, 0.4),
                content: "Hello".to_string(),
                font_size: 20.0,
                color: [0, 0, 0, 255],
                background: None,
                typst_prelude: String::new(),
            }],
        );
        let original = PresentationMetadata { slide_text_boxes, ..Default::default() };
        format.write(&path, &original).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("typst_prelude"));

        let loaded = format.read(&path).unwrap();
        assert_eq!(loaded.slide_text_boxes[&0][0].typst_prelude, "");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn existing_fields_roundtrip_with_annotations() {
        let dir = test_dir();
        let path = dir.join("full_with_annotations.dais");
        let format = DaisFormat;

        let mut slide_annotations = HashMap::new();
        slide_annotations.insert(
            0,
            vec![InkStrokeMeta { points: vec![(0.1, 0.2)], color: [255, 0, 0, 255], width: 3.0 }],
        );

        let original = PresentationMetadata {
            title: Some("With Annotations".to_string()),
            end_slide: Some(10),
            last_minutes: Some(15),
            groups: vec![SlideGroupMeta { start_page: 0, end_page: 2 }],
            notes: {
                let mut n = HashMap::new();
                n.insert(0, "Note".to_string());
                n
            },
            slide_timings: {
                let mut t = HashMap::new();
                t.insert(0, 5.0);
                t
            },
            slide_annotations,
            whiteboard_annotations: vec![InkStrokeMeta {
                points: vec![(0.5, 0.5)],
                color: [0, 0, 255, 255],
                width: 2.0,
            }],
            slide_text_boxes: HashMap::new(),
        };

        format.write(&path, &original).unwrap();
        let loaded = format.read(&path).unwrap();

        // Existing fields preserved
        assert_eq!(loaded.title.as_deref(), Some("With Annotations"));
        assert_eq!(loaded.end_slide, Some(10));
        assert_eq!(loaded.last_minutes, Some(15));
        assert_eq!(loaded.groups.len(), 1);
        assert_eq!(loaded.notes.len(), 1);
        assert_eq!(loaded.slide_timings.len(), 1);

        // Annotations also preserved
        assert_eq!(loaded.slide_annotations.len(), 1);
        assert_eq!(loaded.whiteboard_annotations.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
