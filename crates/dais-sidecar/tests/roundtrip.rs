//! Integration tests for sidecar format roundtrips using fixture files.

use std::path::Path;

use dais_sidecar::format::SidecarFormat;

#[test]
fn pdfpc_fixture_roundtrip() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/beamer-example.pdfpc");
    assert!(fixture.exists(), "Fixture file not found: {}", fixture.display());

    let format = dais_sidecar::pdfpc::PdfpcFormat;
    let meta = format.read(&fixture).expect("Failed to read pdfpc fixture");

    assert_eq!(meta.groups.len(), 3);
    assert_eq!(meta.groups[0].start_page, 0);
    assert_eq!(meta.groups[0].end_page, 1);
    assert_eq!(meta.groups[2].start_page, 4);
    assert_eq!(meta.groups[2].end_page, 4);

    assert_eq!(meta.notes.len(), 3);
    assert!(meta.notes[&0].contains("Welcome"));
    assert!(meta.notes[&2].contains("outperforms"));
    assert!(meta.notes[&4].contains("Questions"));

    assert_eq!(meta.end_slide, Some(4)); // 0-based (pdfpc file stores 1-based "5")
    assert_eq!(meta.last_minutes, Some(20));

    // Write to temp and read back
    let tmp = std::env::temp_dir().join("dais-test-beamer-roundtrip.pdfpc");
    format.write(&tmp, &meta).expect("Failed to write pdfpc");
    let meta2 = format.read(&tmp).expect("Failed to re-read pdfpc");
    let _ = std::fs::remove_file(&tmp);

    assert_eq!(meta.groups.len(), meta2.groups.len());
    assert_eq!(meta.notes.len(), meta2.notes.len());
    assert_eq!(meta.end_slide, meta2.end_slide);
    for (k, v) in &meta.notes {
        assert_eq!(meta2.notes.get(k).map(String::as_str), Some(v.as_str()));
    }
}

#[test]
fn dais_fixture_roundtrip() {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/quarto-example.dais");
    assert!(fixture.exists(), "Fixture file not found: {}", fixture.display());

    let format = dais_sidecar::dais_format::DaisFormat;
    let meta = format.read(&fixture).expect("Failed to read .dais fixture");

    assert_eq!(meta.title.as_deref(), Some("Quarto Beamer Example"));
    assert_eq!(meta.groups.len(), 3);
    assert_eq!(meta.groups[0].start_page, 0);
    assert_eq!(meta.groups[0].end_page, 1);

    assert_eq!(meta.notes.len(), 3);
    assert!(meta.notes[&0].contains("Welcome"));
    assert!(meta.notes[&2].contains("outperforms"));
    assert!(meta.notes[&4].contains("Questions"));

    assert_eq!(meta.end_slide, Some(5));
    assert_eq!(meta.last_minutes, Some(20));

    // Write to temp and read back
    let tmp = std::env::temp_dir().join("dais-test-quarto-roundtrip.dais");
    format.write(&tmp, &meta).expect("Failed to write .dais");
    let meta2 = format.read(&tmp).expect("Failed to re-read .dais");
    let _ = std::fs::remove_file(&tmp);

    assert_eq!(meta.title, meta2.title);
    assert_eq!(meta.groups.len(), meta2.groups.len());
    assert_eq!(meta.notes.len(), meta2.notes.len());
    assert_eq!(meta.end_slide, meta2.end_slide);
    assert_eq!(meta.last_minutes, meta2.last_minutes);
    for (k, v) in &meta.notes {
        assert_eq!(meta2.notes.get(k).map(String::as_str), Some(v.as_str()));
    }
}

#[test]
fn dais_format_preferred_over_pdfpc() {
    // Both fixtures describe the same 3-group, 3-note structure.
    // Verify that load_metadata picks .dais over .pdfpc when both exist.
    let tmp_dir = std::env::temp_dir().join("dais-test-priority");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let pdf_path = tmp_dir.join("test.pdf");
    std::fs::write(&pdf_path, b"fake pdf").unwrap();

    // Write a .dais with a title
    let dais_format = dais_sidecar::dais_format::DaisFormat;
    let mut meta = dais_sidecar::types::PresentationMetadata {
        title: Some("From Dais".into()),
        ..Default::default()
    };
    meta.groups.push(dais_sidecar::types::SlideGroupMeta { start_page: 0, end_page: 1 });
    dais_format.write(&pdf_path.with_extension("dais"), &meta).unwrap();

    // Write a .pdfpc with a different title
    let pdfpc_format = dais_sidecar::pdfpc::PdfpcFormat;
    let mut meta_pdfpc = dais_sidecar::types::PresentationMetadata {
        title: Some("From Pdfpc".into()),
        ..Default::default()
    };
    meta_pdfpc.groups.push(dais_sidecar::types::SlideGroupMeta { start_page: 0, end_page: 2 });
    pdfpc_format.write(&pdf_path.with_extension("pdfpc"), &meta_pdfpc).unwrap();

    // Load and verify .dais wins
    let (loaded, source) = dais_sidecar::metadata::load_metadata(&pdf_path, None);
    assert_eq!(loaded.title.as_deref(), Some("From Dais"));
    assert!(matches!(source, dais_sidecar::metadata::MetadataSource::Sidecar(_)));

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
}
