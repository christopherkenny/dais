use std::time::Instant;

use dais_document::page::RenderSize;
use dais_document::pdf_hayro::HayroDocument;
use dais_document::source::DocumentSource;

fn main() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("example.pdf");

    let doc = HayroDocument::open(&path).expect("should load example PDF");
    let page_count = doc.page_count();
    println!("example.pdf: {page_count} pages");

    let sizes = [
        ("200x150 (overview)", RenderSize { width: 200, height: 150 }),
        ("640x360 (next preview)", RenderSize { width: 640, height: 360 }),
        ("1280x720 (current)", RenderSize { width: 1280, height: 720 }),
        ("1920x1080 (audience)", RenderSize { width: 1920, height: 1080 }),
    ];

    for (label, size) in &sizes {
        let start = Instant::now();
        let _ = doc.render_page(0, *size).expect("render failed");
        let cold = start.elapsed();

        let start = Instant::now();
        let _ = doc.render_page(0, *size).expect("render failed");
        let warm = start.elapsed();

        println!("{label}: cold={cold:?} warm={warm:?}");
    }

    println!("\n--- Sequential render all {page_count} pages at 1280x720 ---");
    let start = Instant::now();
    for i in 0..page_count {
        let _ = doc.render_page(i, RenderSize { width: 1280, height: 720 }).expect("render failed");
    }
    let total = start.elapsed();
    let avg = total / page_count as u32;
    println!("Total: {total:?}, avg per page: {avg:?}");

    // Simulate what happens on slide navigation
    println!("\n--- Slide navigation (current@1280x720 + next@640x360 + audience@1920x1080) ---");
    let start = Instant::now();
    let _ = doc.render_page(0, RenderSize { width: 1280, height: 720 }).unwrap();
    let t1 = start.elapsed();
    let _ = doc.render_page(1, RenderSize { width: 640, height: 360 }).unwrap();
    let t2 = start.elapsed();
    let _ = doc.render_page(0, RenderSize { width: 1920, height: 1080 }).unwrap();
    let t3 = start.elapsed();
    println!("current: {t1:?}, +next: {t2:?}, +audience: {t3:?}");

    // Single-size: render current + next at same size
    println!("\n--- Single-size (current@1280x720 + next@1280x720) ---");
    let start = Instant::now();
    let _ = doc.render_page(0, RenderSize { width: 1280, height: 720 }).unwrap();
    let _ = doc.render_page(1, RenderSize { width: 1280, height: 720 }).unwrap();
    let single_total = start.elapsed();
    println!("Total: {single_total:?}");
}
