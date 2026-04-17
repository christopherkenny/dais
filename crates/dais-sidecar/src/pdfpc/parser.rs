use std::path::Path;

use crate::format::{SidecarError, SidecarFormat};
use crate::types::PresentationMetadata;

use super::PdfpcFormat;

impl SidecarFormat for PdfpcFormat {
    fn read(&self, path: &Path) -> Result<PresentationMetadata, SidecarError> {
        let content = std::fs::read_to_string(path)?;
        Ok(parse_pdfpc(&content))
    }

    fn write(&self, _path: &Path, _metadata: &PresentationMetadata) -> Result<(), SidecarError> {
        todo!("pdfpc writer not yet implemented")
    }

    fn file_extension(&self) -> &'static str {
        "pdfpc"
    }
}

/// Parse `.pdfpc` file content into presentation metadata.
fn parse_pdfpc(content: &str) -> PresentationMetadata {
    let mut metadata = PresentationMetadata::default();
    let mut current_section: Option<String> = None;
    let mut current_note_page: Option<usize> = None;
    let mut current_note_text = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Section headers
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            // Flush any pending note
            if let Some(page) = current_note_page.take() {
                let note = current_note_text.trim().to_string();
                if !note.is_empty() {
                    metadata.notes.insert(page, note);
                }
                current_note_text.clear();
            }

            let section = trimmed[1..trimmed.len() - 1].to_string();
            current_section = Some(section);
            continue;
        }

        match current_section.as_deref() {
            Some("file") if !trimmed.is_empty() => {
                metadata.title = Some(trimmed.to_string());
            }
            Some("file") => {}
            Some("notes") => {
                if let Some(rest) = trimmed.strip_prefix("### ") {
                    // Flush previous note
                    if let Some(page) = current_note_page.take() {
                        let note = current_note_text.trim().to_string();
                        if !note.is_empty() {
                            metadata.notes.insert(page, note);
                        }
                        current_note_text.clear();
                    }
                    // Parse page number (1-based in file, 0-based internally)
                    if let Ok(page_num) = rest.trim().parse::<usize>() {
                        current_note_page = Some(page_num.saturating_sub(1));
                    }
                } else if current_note_page.is_some() {
                    if !current_note_text.is_empty() {
                        current_note_text.push('\n');
                    }
                    current_note_text.push_str(line);
                }
            }
            Some("overlay") => {
                // Format: "start_page end_page" (1-based)
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() == 2
                    && let (Ok(start), Ok(end)) =
                        (parts[0].parse::<usize>(), parts[1].parse::<usize>())
                {
                    metadata.groups.push(crate::types::SlideGroupMeta {
                        start_page: start.saturating_sub(1),
                        end_page: end.saturating_sub(1),
                    });
                }
            }
            Some("duration") => {
                if let Ok(minutes) = trimmed.parse::<u32>() {
                    metadata.last_minutes = Some(minutes);
                }
            }
            Some("end_slide" | "end_user_slide") => {
                if let Ok(page) = trimmed.parse::<usize>() {
                    metadata.end_slide = Some(page.saturating_sub(1));
                }
            }
            _ => {
                // Unknown section — ignore gracefully (forward compatibility)
            }
        }
    }

    // Flush final pending note
    if let Some(page) = current_note_page {
        let note = current_note_text.trim().to_string();
        if !note.is_empty() {
            metadata.notes.insert(page, note);
        }
    }

    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_pdfpc() {
        let content = "\
[file]
test.pdf
[notes]
### 1
First slide notes
### 2
Second slide notes
with multiple lines
[overlay]
1 3
4 5
[duration]
20
";
        let meta = parse_pdfpc(content);
        assert_eq!(meta.title.as_deref(), Some("test.pdf"));
        assert_eq!(meta.notes.len(), 2);
        assert_eq!(meta.notes[&0], "First slide notes");
        assert_eq!(meta.notes[&1], "Second slide notes\nwith multiple lines");
        assert_eq!(meta.groups.len(), 2);
        assert_eq!(meta.groups[0].start_page, 0);
        assert_eq!(meta.groups[0].end_page, 2);
        assert_eq!(meta.groups[1].start_page, 3);
        assert_eq!(meta.groups[1].end_page, 4);
        assert_eq!(meta.last_minutes, Some(20));
    }

    #[test]
    fn parse_empty_pdfpc() {
        let meta = parse_pdfpc("");
        assert!(meta.title.is_none());
        assert!(meta.notes.is_empty());
        assert!(meta.groups.is_empty());
    }

    #[test]
    fn unknown_sections_ignored() {
        let content = "\
[file]
test.pdf
[unknown_future_section]
some data
[notes]
### 1
Note here
";
        let meta = parse_pdfpc(content);
        assert_eq!(meta.title.as_deref(), Some("test.pdf"));
        assert_eq!(meta.notes.len(), 1);
    }
}
