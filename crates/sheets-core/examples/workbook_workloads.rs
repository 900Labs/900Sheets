use sheets_core::Sheet;
use std::hint::black_box;
use std::time::{Duration, Instant};

const QUICK_SPARSE_CELLS: u32 = 2_000;
const QUICK_DENSE_SIDE: u32 = 100;
const DEFAULT_SPARSE_CELLS: u32 = 50_000;
const DEFAULT_DENSE_SIDE: u32 = 500;

fn timed(mut operation: impl FnMut()) -> Duration {
    let start = Instant::now();
    operation();
    start.elapsed()
}

fn sparse_workload(cell_count: u32) -> (Duration, Duration, usize) {
    let mut sheet = Sheet::new("Sparse workload");
    let write = timed(|| {
        for index in 0..cell_count {
            let row = index.saturating_mul(19) % 1_000_000;
            let col = index.saturating_mul(97) % 16_384;
            sheet.set_cell_value(row, col, index.to_string());
        }
    });
    let scan = timed(|| {
        let checksum: usize = sheet
            .iter_cells()
            .map(|((row, col), cell)| row as usize + col as usize + cell.raw.len())
            .sum();
        black_box(checksum);
    });
    (write, scan, sheet.cell_count())
}

fn dense_workload(side: u32) -> (Duration, Duration, usize) {
    let mut sheet = Sheet::new("Dense workload");
    let write = timed(|| {
        for row in 0..side {
            for col in 0..side {
                sheet.set_cell_value(
                    row,
                    col,
                    (row as u64 * side as u64 + col as u64).to_string(),
                );
            }
        }
    });
    let scan = timed(|| {
        let checksum: usize = sheet
            .iter_cells()
            .map(|((row, col), cell)| row as usize + col as usize + cell.raw.len())
            .sum();
        black_box(checksum);
    });
    (write, scan, sheet.cell_count())
}

fn main() {
    let quick = std::env::args().any(|argument| argument == "--quick");
    let (sparse_cells, dense_side) = if quick {
        (QUICK_SPARSE_CELLS, QUICK_DENSE_SIDE)
    } else {
        (DEFAULT_SPARSE_CELLS, DEFAULT_DENSE_SIDE)
    };

    let (sparse_write, sparse_scan, sparse_stored) = sparse_workload(sparse_cells);
    let (dense_write, dense_scan, dense_stored) = dense_workload(dense_side);

    println!("900Sheets workbook workload benchmark");
    println!("mode={}", if quick { "quick" } else { "default" });
    println!(
        "sparse requested={sparse_cells} stored={sparse_stored} write_ms={} scan_ms={}",
        sparse_write.as_millis(),
        sparse_scan.as_millis()
    );
    println!(
        "dense side={dense_side} stored={dense_stored} write_ms={} scan_ms={}",
        dense_write.as_millis(),
        dense_scan.as_millis()
    );
}
