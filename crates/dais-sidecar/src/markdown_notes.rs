use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::format::SidecarError;

/// A parsed Markdown notes file that can be updated without rewriting headings.
#[derive(Debug, Clone)]
pub struct MarkdownNotesDocument {
    path: PathBuf,
    preamble: String,
    sections: Vec<MarkdownNoteSection>,
}

#[derive(Debug, Clone)]
struct MarkdownNoteSection {
    heading: String,
    slide_index: usize,
    body: String,
}

impl MarkdownNotesDocument {
    /// Read and parse a Markdown notes file.
    pub fn read(path: &Path, total_logical_slides: usize) -> Result<Self, SidecarError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(path.to_path_buf(), &content, total_logical_slides)
    }

    /// Return notes keyed by zero-based logical slide index.
    pub fn notes_by_slide(&self) -> HashMap<usize, String> {
        self.sections
            .iter()
            .filter_map(|section| {
                let body = section.body.trim().to_string();
                (!body.is_empty()).then_some((section.slide_index, body))
            })
            .collect()
    }

    /// Write updated notes back to the original Markdown file.
    pub fn write_notes(&self, notes: &HashMap<usize, String>) -> Result<(), SidecarError> {
        let content = self.render_with_notes(notes);
        std::fs::write(&self.path, content)?;
        Ok(())
    }

    fn parse(
        path: PathBuf,
        content: &str,
        total_logical_slides: usize,
    ) -> Result<Self, SidecarError> {
        let mut preamble = String::new();
        let mut sections = Vec::new();
        let mut current_heading: Option<(usize, String, usize)> = None;
        let mut current_body = String::new();
        let mut next_slide_index = 0usize;
        let mut seen = HashSet::new();

        for (line_index, line) in content.split_inclusive('\n').enumerate() {
            if is_top_level_heading(line) {
                if let Some((slide_index, heading, heading_line)) = current_heading.take() {
                    push_section(
                        &mut sections,
                        &mut seen,
                        slide_index,
                        heading,
                        std::mem::take(&mut current_body),
                        heading_line,
                        total_logical_slides,
                    )?;
                } else {
                    preamble.clone_from(&current_body);
                    current_body.clear();
                }

                let parsed = parse_heading_slide(line, next_slide_index, line_index + 1)?;
                next_slide_index = parsed.slide_index + 1;
                current_heading = Some((parsed.slide_index, line.to_string(), line_index + 1));
            } else {
                current_body.push_str(line);
            }
        }

        if let Some((slide_index, heading, heading_line)) = current_heading {
            push_section(
                &mut sections,
                &mut seen,
                slide_index,
                heading,
                current_body,
                heading_line,
                total_logical_slides,
            )?;
        } else {
            preamble = current_body;
        }

        Ok(Self { path, preamble, sections })
    }

    fn render_with_notes(&self, notes: &HashMap<usize, String>) -> String {
        let mut output = self.preamble.clone();
        let mut written = HashSet::new();

        for section in &self.sections {
            output.push_str(&section.heading);
            if !section.heading.ends_with('\n') {
                output.push('\n');
            }

            if let Some(note) = notes.get(&section.slide_index) {
                push_note_body(&mut output, note);
            }

            written.insert(section.slide_index);
        }

        let mut added: Vec<_> = notes
            .iter()
            .filter(|(slide, note)| !written.contains(slide) && !note.trim().is_empty())
            .collect();
        added.sort_by_key(|(slide, _)| **slide);

        for (slide, note) in added {
            ensure_blank_line_before_appended_section(&mut output);
            let _ = writeln!(output, "# Slide {} {{slide={}}}", slide + 1, slide + 1);
            push_note_body(&mut output, note);
        }

        output
    }
}

struct ParsedHeading {
    slide_index: usize,
}

fn is_top_level_heading(line: &str) -> bool {
    line.strip_prefix("# ").is_some()
}

fn parse_heading_slide(
    line: &str,
    default_slide_index: usize,
    line_number: usize,
) -> Result<ParsedHeading, SidecarError> {
    let Some(anchor_start) = line.find("{slide") else {
        return Ok(ParsedHeading { slide_index: default_slide_index });
    };

    let anchor = line[anchor_start..].trim();
    let Some(anchor_body) = anchor.strip_prefix("{slide=") else {
        return Err(parse_error(line_number, "Malformed slide anchor"));
    };
    let Some(anchor_end) = anchor_body.find('}') else {
        return Err(parse_error(line_number, "Malformed slide anchor"));
    };

    let number = anchor_body[..anchor_end]
        .trim()
        .parse::<usize>()
        .map_err(|_| parse_error(line_number, "Slide anchor must contain a positive integer"))?;
    let slide_index = number
        .checked_sub(1)
        .ok_or_else(|| parse_error(line_number, "Slide anchor must use one-based slide numbers"))?;

    Ok(ParsedHeading { slide_index })
}

fn push_section(
    sections: &mut Vec<MarkdownNoteSection>,
    seen: &mut HashSet<usize>,
    slide_index: usize,
    heading: String,
    body: String,
    heading_line: usize,
    total_logical_slides: usize,
) -> Result<(), SidecarError> {
    if slide_index >= total_logical_slides {
        return Err(parse_error(
            heading_line,
            format!(
                "Slide {} is outside this presentation's {} logical slides",
                slide_index + 1,
                total_logical_slides
            ),
        ));
    }
    if !seen.insert(slide_index) {
        return Err(parse_error(
            heading_line,
            format!("Duplicate notes section for slide {}", slide_index + 1),
        ));
    }
    sections.push(MarkdownNoteSection { heading, slide_index, body });
    Ok(())
}

fn push_note_body(output: &mut String, note: &str) {
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output.push('\n');
    output.push_str(trimmed);
    output.push('\n');
}

fn ensure_blank_line_before_appended_section(output: &mut String) {
    if output.is_empty() {
        return;
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }
    if !output.ends_with("\n\n") {
        output.push('\n');
    }
}

fn parse_error(line: usize, message: impl Into<String>) -> SidecarError {
    SidecarError::Parse { line, message: message.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_headings_in_order() {
        let doc = MarkdownNotesDocument::parse(
            PathBuf::from("notes.md"),
            "# Motivation\n\nOpening.\n\n# Prior Work\n\nCompare.\n",
            3,
        )
        .unwrap();

        let notes = doc.notes_by_slide();
        assert_eq!(notes[&0], "Opening.");
        assert_eq!(notes[&1], "Compare.");
    }

    #[test]
    fn explicit_slide_anchor_skips_and_resets_counter() {
        let doc = MarkdownNotesDocument::parse(
            PathBuf::from("notes.md"),
            "# Motivation\n\nOpening.\n\n# Main Result {slide=4}\n\nSlow down.\n\n# Extra\n\nMore.\n",
            5,
        )
        .unwrap();

        let notes = doc.notes_by_slide();
        assert_eq!(notes[&0], "Opening.");
        assert_eq!(notes[&3], "Slow down.");
        assert_eq!(notes[&4], "More.");
        assert!(!notes.contains_key(&2));
    }

    #[test]
    fn lower_level_headings_stay_in_body() {
        let doc = MarkdownNotesDocument::parse(
            PathBuf::from("notes.md"),
            "# Main {slide=2}\n\n## Setup\n\nExplain.\n",
            2,
        )
        .unwrap();

        assert_eq!(doc.notes_by_slide()[&1], "## Setup\n\nExplain.");
    }

    #[test]
    fn duplicate_slide_is_error() {
        let err = MarkdownNotesDocument::parse(
            PathBuf::from("notes.md"),
            "# A {slide=1}\n\nA\n\n# B {slide=1}\n\nB\n",
            2,
        )
        .unwrap_err();

        assert!(err.to_string().contains("Duplicate"));
    }

    #[test]
    fn render_updates_existing_and_appends_new_sections() {
        let doc = MarkdownNotesDocument::parse(
            PathBuf::from("notes.md"),
            "---\ntitle: Talk\n---\n\n# Motivation\n\nOld.\n",
            3,
        )
        .unwrap();
        let notes = HashMap::from([(0, "New.".to_string()), (2, "Third.".to_string())]);

        let rendered = doc.render_with_notes(&notes);

        assert!(rendered.contains("---\ntitle: Talk\n---"));
        assert!(rendered.contains("# Motivation\n\nNew.\n"));
        assert!(rendered.contains("# Slide 3 {slide=3}\n\nThird.\n"));
        assert!(!rendered.contains("Old."));
    }
}
