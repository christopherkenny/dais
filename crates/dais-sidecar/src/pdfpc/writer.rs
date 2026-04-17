use std::fmt::Write as _;
use std::path::Path;

use crate::format::SidecarError;
use crate::types::PresentationMetadata;

/// Write presentation metadata to a `.pdfpc` file.
pub fn write_pdfpc(path: &Path, metadata: &PresentationMetadata) -> Result<(), SidecarError> {
    let mut output = String::new();

    // [file] section
    if let Some(title) = &metadata.title {
        output.push_str("[file]\n");
        output.push_str(title);
        output.push('\n');
    }

    // [duration] section
    if let Some(minutes) = metadata.last_minutes {
        output.push_str("[duration]\n");
        output.push_str(&minutes.to_string());
        output.push('\n');
    }

    // [end_slide] section
    if let Some(end) = metadata.end_slide {
        output.push_str("[end_slide]\n");
        output.push_str(&(end + 1).to_string());
        output.push('\n');
    }

    // [overlay] section
    if !metadata.groups.is_empty() {
        output.push_str("[overlay]\n");
        for group in &metadata.groups {
            let _ = writeln!(output, "{} {}", group.start_page + 1, group.end_page + 1);
        }
    }

    // [notes] section
    if !metadata.notes.is_empty() {
        output.push_str("[notes]\n");
        let mut pages: Vec<_> = metadata.notes.keys().collect();
        pages.sort();
        for &page in &pages {
            let _ = writeln!(output, "### {}", page + 1);
            output.push_str(&metadata.notes[page]);
            output.push('\n');
        }
    }

    std::fs::write(path, output)?;
    Ok(())
}
