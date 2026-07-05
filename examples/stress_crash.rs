//! # Crash / stress harness
//!
//! A deliberately punishing battery of tests aimed squarely at the reader's
//! trust boundary. Where [`examples/hostile.rs`](hostile.rs) hand-picks a
//! handful of malformed archives, this harness *generates* adversarial input at
//! volume — random byte mutations, exhaustive truncation, targeted field
//! corruption, decompression bombs, and large-scale stress builds — and asserts
//! the single invariant the crate promises on untrusted input:
//!
//! > No archive — however malformed, hostile, or enormous — may make the reader
//! > panic, read out of bounds, loop forever, or allocate unboundedly. It must
//! > return an [`Error`] instead.
//!
//! Every operation runs inside [`std::panic::catch_unwind`]; a caught panic is a
//! *bug* and makes the process exit non-zero. Run it with:
//!
//! ```text
//! cargo run --example stress_crash
//! cargo run --release --example stress_crash   # for the heavier stress pass
//! ```
//!
//! The generators are seeded from a fixed constant so a failure is always
//! reproducible: the printed seed can be fed back in via `QUIVER_SEED=<n>`.

use std::panic;

use quiver::codec;
use quiver::integrity;
use quiver::reader::Archive;
use quiver::types::Compression;
use quiver::util::Limits;
use quiver::writer::{Builder, CompressionPolicy};
use quiver::Pack;

// ============================================================================
// Deterministic PRNG
// ============================================================================

/// A tiny xorshift64* generator. Not cryptographic — just a fast, portable,
/// fully-deterministic source of bytes so every run is reproducible from a
/// seed. std's `rand` is intentionally avoided to keep the crate dep-free.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Rng {
        // Avoid the zero fixed-point; xorshift is stuck at 0.
        Rng {
            state: seed ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn byte(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }

    /// Uniform in `[0, n)`; returns 0 when `n == 0`.
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    /// A `bool` that is true with probability `1 / n`.
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n.max(1)) == 0
    }

    /// A fresh vector of `len` pseudo-random bytes.
    fn bytes(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.byte()).collect()
    }
}

// ============================================================================
// Verdict accounting
// ============================================================================

/// Outcome of a single probe. `Panic` is the only failing verdict — accepting
/// or rejecting an input are both fine, we only forbid crashing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Accepted,
    Rejected,
    Panic,
}

impl Verdict {
    fn tag(self) -> &'static str {
        match self {
            Verdict::Accepted => "accept",
            Verdict::Rejected => "reject",
            Verdict::Panic => "PANIC!",
        }
    }
}

/// Running totals across the whole harness.
#[derive(Default)]
struct Tally {
    probes: u64,
    accepted: u64,
    rejected: u64,
    panics: u64,
}

impl Tally {
    fn record(&mut self, v: Verdict) {
        self.probes += 1;
        match v {
            Verdict::Accepted => self.accepted += 1,
            Verdict::Rejected => self.rejected += 1,
            Verdict::Panic => self.panics += 1,
        }
    }

    fn merge(&mut self, other: &Tally) {
        self.probes += other.probes;
        self.accepted += other.accepted;
        self.rejected += other.rejected;
        self.panics += other.panics;
    }
}

/// Run `f`, converting a `Result` into a non-panicking verdict and trapping any
/// panic. `AssertUnwindSafe` is sound here because we discard everything the
/// closure produced on the panic path — we only care *that* it panicked.
fn probe<T>(f: impl FnOnce() -> Result<T, ()>) -> Verdict {
    match panic::catch_unwind(panic::AssertUnwindSafe(f)) {
        Ok(Ok(_)) => Verdict::Accepted,
        Ok(Err(())) => Verdict::Rejected,
        Err(_) => Verdict::Panic,
    }
}

/// Open an archive under the given limits and, if it parses, touch every entry
/// so decode paths are exercised too. Any error collapses to `Err(())`.
fn open_and_drain(bytes: &[u8], limits: Limits) -> Result<usize, ()> {
    let archive = Archive::open_with(bytes, limits).map_err(|_| ())?;
    let mut drained = 0usize;
    for i in 0..archive.len() {
        // A record may legitimately fail to decode (corrupt block); that is a
        // rejection, not a crash, so propagate it as Err.
        let data = archive.read_index(i).map_err(|_| ())?;
        drained = drained.wrapping_add(data.len());
    }
    // Integrity verification must also stay panic-free on whatever survived.
    let _ = integrity::verify(&archive);
    Ok(drained)
}

// ============================================================================
// Corpus of valid archives (seeds for mutation)
// ============================================================================

/// Build a family of valid archives with different shapes. These are the
/// starting points that the mutators corrupt — a diverse corpus finds more
/// reader bugs than a single hand-written blob.
fn valid_corpus(rng: &mut Rng) -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();

    // 1. A single tiny stored file.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::None);
        b.add_file("a", vec![1, 2, 3, 4]).unwrap();
        out.push(("tiny-store", b.finish()));
    }

    // 2. Highly compressible content under Auto.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        b.add_file("zeros.bin", vec![0u8; 4096]).unwrap();
        b.add_file("runs.bin", vec![0xabu8; 4096]).unwrap();
        out.push(("compressible-auto", b.finish()));
    }

    // 3. Incompressible pseudo-random content, each fixed method.
    for (name, policy) in [
        ("rand-rle", CompressionPolicy::Fixed(Compression::Rle)),
        ("rand-lz", CompressionPolicy::Fixed(Compression::Lz)),
        ("rand-store", CompressionPolicy::Fixed(Compression::Store)),
    ] {
        let mut b = Builder::new().with_policy(policy);
        let blob = rng.bytes(2000);
        b.add_file("payload", blob).unwrap();
        out.push((name, b.finish()));
    }

    // 4. A directory tree with nested paths, dirs, and a symlink.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        b.add_directory("src").unwrap();
        b.add_directory("src/deep").unwrap();
        b.add_file("src/deep/mod.rs", vec![b'x'; 300]).unwrap();
        b.add_file("README.md", b"# hi\n".to_vec()).unwrap();
        b.add_symlink("latest", "src/deep/mod.rs").unwrap();
        out.push(("tree-with-symlink", b.finish()));
    }

    // 5. Many small entries — stresses the index region.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        for i in 0..64 {
            let name = format!("entry/{i:04}.dat");
            b.add_file(&name, vec![(i & 0xff) as u8; 16]).unwrap();
        }
        out.push(("many-small", b.finish()));
    }

    out
}

// ============================================================================
// Pass 1 — bit-flip mutation fuzzing
// ============================================================================

/// For each valid seed, apply increasingly aggressive random mutations and open
/// the result. The reader must never panic, whatever bytes it is handed.
fn pass_mutation(rng: &mut Rng, rounds_per_seed: usize) -> Tally {
    let mut tally = Tally::default();
    let corpus = valid_corpus(rng);

    println!(
        "== Pass 1: bit-flip mutation fuzzing ({} seeds x {} rounds) ==",
        corpus.len(),
        rounds_per_seed
    );

    for (name, base) in &corpus {
        let mut worst: Option<Verdict> = None;
        for _ in 0..rounds_per_seed {
            let mut buf = base.clone();
            // Between 1 and 8 independent mutations of varying severity.
            let mutations = 1 + rng.below(8);
            for _ in 0..mutations {
                if buf.is_empty() {
                    break;
                }
                let pos = rng.below(buf.len());
                match rng.below(4) {
                    0 => buf[pos] ^= 1 << rng.below(8), // flip a single bit
                    1 => buf[pos] = rng.byte(),         // random replace
                    2 => buf[pos] = buf[pos].wrapping_add(1),
                    _ => buf[pos] = 0xff,               // saturate
                }
            }
            // Occasionally also change the length to stress bounds checks.
            if rng.one_in(4) && !buf.is_empty() {
                let cut = rng.below(buf.len());
                buf.truncate(cut);
            }

            let limits = pick_limits(rng);
            let verdict = probe(|| open_and_drain(&buf, limits));
            tally.record(verdict);
            worst = Some(match worst {
                Some(Verdict::Panic) => Verdict::Panic,
                _ => verdict,
            });
        }
        let w = worst.unwrap_or(Verdict::Rejected);
        println!("  [{:>6}] {name}", w.tag());
    }

    tally
}

/// Pick one of the standard limit budgets at random so mutations are tested
/// against both the permissive default and the strict profile.
fn pick_limits(rng: &mut Rng) -> Limits {
    match rng.below(3) {
        0 => Limits::STRICT,
        1 => Limits::DEFAULT,
        _ => Limits::DEFAULT
            .with_max_decoded_block(1 << 16)
            .with_max_entries(256),
    }
}

// ============================================================================
// Pass 2 — exhaustive truncation
// ============================================================================

/// Truncate each valid archive at *every* possible length and open the prefix.
/// Truncation is the single most common real-world corruption (a partial
/// download, a killed write) and every prefix must be rejected cleanly.
fn pass_truncation(rng: &mut Rng) -> Tally {
    let mut tally = Tally::default();
    let corpus = valid_corpus(rng);

    println!("\n== Pass 2: exhaustive truncation sweep ==");
    for (name, base) in &corpus {
        let mut panicked = false;
        let mut accepted_prefixes = 0usize;
        for cut in 0..=base.len() {
            let prefix = &base[..cut];
            let verdict = probe(|| open_and_drain(prefix, Limits::DEFAULT));
            if verdict == Verdict::Panic {
                panicked = true;
            }
            if verdict == Verdict::Accepted {
                accepted_prefixes += 1;
            }
            tally.record(verdict);
        }
        // Only the full-length archive should ever be accepted; every strict
        // prefix is missing its footer/index and must be rejected.
        let tag = if panicked {
            "PANIC!"
        } else if accepted_prefixes <= 1 {
            "clean"
        } else {
            "LOOSE?"
        };
        println!(
            "  [{tag:>6}] {name} ({} prefixes, {accepted_prefixes} accepted)",
            base.len() + 1
        );
    }

    tally
}

// ============================================================================
// Pass 3 — targeted header/index field corruption
// ============================================================================

/// Overwrite specific structural fields with pathological values. Random
/// mutation rarely lands a `u64::MAX` in exactly the offset slot; this pass
/// does it deliberately to probe the arithmetic-overflow and giant-allocation
/// guards.
fn pass_field_corruption(rng: &mut Rng) -> Tally {
    let mut tally = Tally::default();

    // A fresh, reasonably sized valid archive to deface.
    let base = {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        b.add_file("one.bin", rng.bytes(500)).unwrap();
        b.add_file("two.bin", vec![0u8; 500]).unwrap();
        b.finish()
    };

    println!("\n== Pass 3: targeted field corruption ==");

    // The header is a fixed 20-byte prefix: magic[4] version[..] entry_count[..]
    // index_offset[12..20]. We spray extreme values across the whole header and
    // a few index positions.
    let poison: [u64; 4] = [0, 1, u64::MAX, u64::MAX / 2];
    let targets: &[(&str, usize)] = &[
        ("entry_count field", 4),
        ("entry_count high", 8),
        ("index_offset (be u64)", 12),
        ("mid-index", base.len().saturating_sub(24)),
        ("footer-adjacent", base.len().saturating_sub(8)),
    ];

    for (label, off) in targets {
        let mut worst = Verdict::Rejected;
        for &value in &poison {
            if *off + 8 > base.len() {
                continue;
            }
            let mut buf = base.clone();
            buf[*off..*off + 8].copy_from_slice(&value.to_be_bytes());
            let verdict = probe(|| open_and_drain(&buf, Limits::DEFAULT));
            if verdict == Verdict::Panic {
                worst = Verdict::Panic;
            }
            tally.record(verdict);
        }
        println!("  [{:>6}] {label}", worst.tag());
    }

    tally
}

// ============================================================================
// Pass 4 — codec round-trip and decompression-bomb resistance
// ============================================================================

/// Encode random and structured data with every method and confirm a byte-exact
/// round-trip, then confirm that adversarial "bombs" (streams that decode to far
/// more than the output cap) are rejected rather than exhausting memory.
fn pass_codec(rng: &mut Rng) -> Tally {
    let mut tally = Tally::default();
    println!("\n== Pass 4: codec round-trip + bomb resistance ==");

    let methods = [Compression::Store, Compression::Rle, Compression::Lz];

    // --- 4a. Round-trip fuzzing ---------------------------------------------
    let mut roundtrip_ok = true;
    for _ in 0..400 {
        // Mix of shapes: pure random, long runs, and short blobs — each class
        // exercises a different branch of the RLE/LZ encoders.
        let data = match rng.below(4) {
            0 => {
                let n = rng.below(3000);
                rng.bytes(n)
            }
            1 => {
                let n = rng.below(3000);
                vec![rng.byte(); n]
            }
            2 => {
                // Alternating runs.
                let mut v = Vec::new();
                while v.len() < 2000 {
                    let b = rng.byte();
                    let run = 1 + rng.below(40);
                    v.extend(std::iter::repeat(b).take(run));
                }
                v
            }
            _ => {
                let n = rng.below(64);
                rng.bytes(n)
            }
        };

        for &method in &methods {
            let encoded = codec::encode(method, &data);
            // Cap generously so a valid stream is never rejected for size.
            let cap = data.len() + 16;
            let verdict = probe(|| {
                let decoded = codec::decode(method, &encoded, cap).map_err(|_| ())?;
                if decoded == data {
                    Ok(())
                } else {
                    // A mismatch is a real correctness bug — surface it as a
                    // rejection so the pass fails loudly below.
                    Err(())
                }
            });
            if verdict != Verdict::Accepted {
                roundtrip_ok = false;
            }
            tally.record(verdict);
        }
    }
    println!(
        "  [{:>6}] round-trip: 400 payloads x {} methods",
        if roundtrip_ok { "ok" } else { "FAIL!" },
        methods.len()
    );

    // --- 4b. Decompression bombs --------------------------------------------
    // A megabyte of zeros compresses to a handful of bytes; decoding it under a
    // 1 KiB output cap must fail with an error, never allocate the megabyte.
    let bomb_source = vec![0u8; 1 << 20];
    let mut bombs_contained = true;
    for &method in &[Compression::Rle, Compression::Lz] {
        let encoded = codec::encode(method, &bomb_source);
        let verdict = probe(|| match codec::decode(method, &encoded, 1024) {
            Ok(_) => Ok(()),   // accepted — the cap did NOT hold
            Err(_) => Err(()), // rejected — correct, the cap held
        });
        // For a bomb, "Rejected" is the desired outcome.
        if verdict != Verdict::Rejected {
            bombs_contained = false;
        }
        tally.record(verdict);
    }
    println!(
        "  [{:>6}] decompression bombs contained under 1 KiB cap",
        if bombs_contained { "ok" } else { "LEAK!" }
    );

    // --- 4c. Random bytes as compressed input -------------------------------
    // Random bytes are almost never a valid RLE/LZ stream; decoding must reject
    // or return bounded output, never panic.
    for _ in 0..300 {
        let junk_len = rng.below(256);
        let junk = rng.bytes(junk_len);
        for &method in &methods {
            let verdict = probe(|| {
                let _ = codec::decode(method, &junk, 1 << 16).map_err(|_| ())?;
                Ok(())
            });
            tally.record(verdict);
        }
    }
    println!("  [   ok ] 300 junk streams x {} methods decoded", methods.len());

    tally
}

// ============================================================================
// Pass 5 — large-scale stress builds
// ============================================================================

/// Build genuinely large archives and confirm they round-trip and verify. This
/// is less about crashing and more about catching quadratic blowups, index
/// overflow, and off-by-one errors that only appear at scale.
fn pass_stress(rng: &mut Rng, heavy: bool) -> Tally {
    let mut tally = Tally::default();
    println!("\n== Pass 5: large-scale stress builds ==");

    let entry_count = if heavy { 5000 } else { 800 };
    let big_blob = if heavy { 1 << 20 } else { 1 << 16 };

    // 5a. Many entries.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        for i in 0..entry_count {
            let name = format!("dir{}/sub{}/file{:06}.bin", i % 16, i % 128, i);
            let body = vec![(i % 251) as u8; 8 + (i % 40)];
            b.add_file(&name, body).unwrap();
        }
        let bytes = b.finish();
        let verdict = probe(|| {
            let archive = Archive::open_with(&bytes, Limits::DEFAULT).map_err(|_| ())?;
            if archive.len() != entry_count {
                return Err(());
            }
            let report = integrity::verify(&archive);
            if report.is_clean() {
                Ok(())
            } else {
                Err(())
            }
        });
        tally.record(verdict);
        println!("  [{:>6}] {entry_count} entries build+open+verify", verdict.tag());
    }

    // 5b. A few very large blocks.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::Auto);
        for i in 0..4 {
            // Mix compressible and incompressible large blocks.
            let body = if i % 2 == 0 {
                vec![0u8; big_blob]
            } else {
                rng.bytes(big_blob)
            };
            b.add_file(&format!("big{i}.bin"), body).unwrap();
        }
        let bytes = b.finish();
        let verdict = probe(|| {
            // Needs a raised block cap to admit the large decoded blocks.
            let limits = Limits::DEFAULT.with_max_decoded_block(big_blob * 2);
            let archive = Archive::open_with(&bytes, limits).map_err(|_| ())?;
            for i in 0..archive.len() {
                let _ = archive.read_index(i).map_err(|_| ())?;
            }
            Ok(())
        });
        tally.record(verdict);
        println!(
            "  [{:>6}] 4 x {} KiB blocks build+drain",
            verdict.tag(),
            big_blob / 1024
        );
    }

    // 5c. Deeply nested paths.
    {
        let mut b = Builder::new().with_policy(CompressionPolicy::None);
        let mut path = String::from("d0");
        for i in 1..24 {
            path.push_str(&format!("/d{i}"));
        }
        path.push_str("/leaf.txt");
        b.add_file(&path, b"deep".to_vec()).unwrap();
        let bytes = b.finish();
        // The default depth limit may reject this — either way, no panic.
        let verdict = probe(|| open_and_drain(&bytes, Limits::DEFAULT));
        tally.record(verdict);
        println!("  [{:>6}] 24-level deep path", verdict.tag());
    }

    tally
}

// ============================================================================
// Pass 6 — high-level Pack façade under corruption
// ============================================================================

/// The `Pack` convenience functions wrap the same reader, but each has its own
/// entry point (summary, tree, extract_all, open_verified). Make sure a corrupt
/// archive can't slip a panic through any of them.
fn pass_pack_facade(rng: &mut Rng) -> Tally {
    let mut tally = Tally::default();
    println!("\n== Pass 6: Pack façade under corruption ==");

    let base = Pack::create([
        ("a/b/c.txt", b"hello world".to_vec()),
        ("a/b/d.bin", vec![9u8; 1024]),
        ("top.md", b"# title".to_vec()),
    ])
    .unwrap();

    let mut panics = 0u64;
    for _ in 0..500 {
        let mut buf = base.clone();
        let hits = 1 + rng.below(6);
        for _ in 0..hits {
            if buf.is_empty() {
                break;
            }
            let pos = rng.below(buf.len());
            buf[pos] = rng.byte();
        }

        // Each façade entry point gets the same corrupt buffer.
        let checks: [Verdict; 5] = [
            probe(|| Pack::open(&buf).map(|_| ()).map_err(|_| ())),
            probe(|| Pack::summary(&buf).map(|_| ()).map_err(|_| ())),
            probe(|| Pack::tree(&buf).map(|_| ()).map_err(|_| ())),
            probe(|| Pack::extract_all(&buf).map(|_| ()).map_err(|_| ())),
            probe(|| Pack::open_verified(&buf).map(|_| ()).map_err(|_| ())),
        ];
        for v in checks {
            if v == Verdict::Panic {
                panics += 1;
            }
            tally.record(v);
        }
    }
    println!(
        "  [{:>6}] 500 corruptions x 5 façade entry points",
        if panics == 0 { "ok" } else { "PANIC!" }
    );

    tally
}

// ============================================================================
// Driver
// ============================================================================

fn seed_from_env() -> u64 {
    match std::env::var("QUIVER_SEED") {
        Ok(s) => s.trim().parse().unwrap_or(0x5151_2323_4242_9696),
        Err(_) => 0x5151_2323_4242_9696,
    }
}

fn main() {
    // Silence the default panic printer; a trapped panic is reported through
    // our own tally, and we don't want a wall of backtraces from the fuzz runs.
    // A genuine panic still counts and still fails the process.
    panic::set_hook(Box::new(|_info| {}));

    let heavy = std::env::args().any(|a| a == "--heavy") || cfg!(not(debug_assertions));
    let seed = seed_from_env();
    println!("quiver crash/stress harness — seed = 0x{seed:016x}, heavy = {heavy}");
    println!("(reproduce a failure with QUIVER_SEED=<seed>)\n");

    let mut rng = Rng::new(seed);
    let mut total = Tally::default();

    let rounds = if heavy { 2000 } else { 400 };

    let passes: Vec<Tally> = vec![
        pass_mutation(&mut rng, rounds),
        pass_truncation(&mut rng),
        pass_field_corruption(&mut rng),
        pass_codec(&mut rng),
        pass_stress(&mut rng, heavy),
        pass_pack_facade(&mut rng),
    ];
    for p in &passes {
        total.merge(p);
    }

    // Restore a sane hook before we print the summary.
    let _ = panic::take_hook();

    println!("\n{}", "=".repeat(60));
    println!(
        "TOTAL: {} probes — {} accepted, {} rejected, {} PANICS",
        total.probes, total.accepted, total.rejected, total.panics
    );

    if total.panics == 0 {
        println!("PASS: no panic across {} adversarial probes.", total.probes);
    } else {
        eprintln!(
            "FAIL: {} panic(s) — reproduce with QUIVER_SEED=0x{seed:016x}",
            total.panics
        );
        std::process::exit(1);
    }
}
