//! A tiny, dependency-free benchmark harness for the hot paths: block codecs
//! and archive open/read. Not statistically rigorous — it exists to catch
//! order-of-magnitude regressions. Run with `cargo bench`.

use std::time::Instant;

use quiver::codec;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::writer::Builder;

fn corpus() -> Vec<u8> {
    // A mix of runs and repetitive text — representative of real archive data.
    let mut data = Vec::new();
    for i in 0..2000u32 {
        data.extend_from_slice(format!("record {i:04} name=service-{}\n", i % 32).as_bytes());
    }
    data.extend(std::iter::repeat(b'.').take(8000));
    data
}

fn bench<F: FnMut()>(name: &str, iters: u32, mut f: F) {
    for _ in 0..iters.min(32) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let per = start.elapsed().as_nanos() as f64 / iters as f64;
    println!("{name:<26} {per:>12.1} ns/iter  ({iters} iters)");
}

fn main() {
    let data = corpus();
    let lz = codec::encode(Compression::Lz, &data);
    let rle = codec::encode(Compression::Rle, &data);

    println!("quiver micro-benchmarks");
    println!(
        "corpus {} bytes  ->  lz {} ({:.0}%), rle {} ({:.0}%)\n",
        data.len(),
        lz.len(),
        100.0 * lz.len() as f64 / data.len() as f64,
        rle.len(),
        100.0 * rle.len() as f64 / data.len() as f64,
    );

    bench("lz_encode", 2_000, || {
        std::hint::black_box(codec::encode(Compression::Lz, &data));
    });
    bench("lz_decode", 20_000, || {
        std::hint::black_box(codec::decode(Compression::Lz, &lz, 1 << 24).unwrap());
    });
    bench("rle_encode", 5_000, || {
        std::hint::black_box(codec::encode(Compression::Rle, &data));
    });
    bench("rle_decode", 20_000, || {
        std::hint::black_box(codec::decode(Compression::Rle, &rle, 1 << 24).unwrap());
    });

    let archive_bytes = {
        let mut b = Builder::new();
        for i in 0..64 {
            b.add_file(&format!("entry/{i:03}.dat"), data.clone())
                .unwrap();
        }
        b.finish()
    };
    bench("archive_open", 10_000, || {
        std::hint::black_box(Archive::open(&archive_bytes).unwrap());
    });
    bench("archive_read_one", 20_000, || {
        let a = Archive::open(&archive_bytes).unwrap();
        std::hint::black_box(a.read("entry/000.dat").unwrap());
    });
}
