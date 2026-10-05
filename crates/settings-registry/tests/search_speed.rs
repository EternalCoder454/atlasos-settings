//! Search per keystroke against DESIGN.md's 16 ms budget (the Rust search
//! only; the QML model rebuild on top is part of the window's measures). Timing depends on
//! the machine, so it is ignored in the normal run; scripts/bench.sh's job
//! runs it on a release build:
//!   cargo test --release -p settings-registry --test search_speed -- --ignored --nocapture

use settings_registry::search;
use std::time::{Duration, Instant};

#[test]
#[ignore]
fn search_per_keystroke_is_within_budget() {
    // What people type, one key at a time, plus a long query and one that
    // matches nothing.
    let words = [
        "wifi password",
        "bluetooth",
        "night light",
        "dark mode",
        "keyboard layout",
        "time zone",
        "zzzzqq",
        &"a".repeat(search::MAX_QUERY + 50),
    ];
    // The first call builds the index; a real window pays it once.
    let first = Instant::now();
    search::search("w", 50);
    let first = first.elapsed();

    let mut times: Vec<Duration> = Vec::new();
    for _ in 0..20 {
        for w in &words {
            for end in 1..=w.len() {
                if !w.is_char_boundary(end) {
                    continue;
                }
                let t = Instant::now();
                std::hint::black_box(search::search(&w[..end], 50));
                times.push(t.elapsed());
            }
        }
    }
    // The 99th percentile: one slow sample on a busy machine isn't the
    // search being slow.
    times.sort();
    let n = times.len() as u32;
    let mean = times.iter().sum::<Duration>() / n;
    let p99 = times[times.len() * 99 / 100];
    let worst = times[times.len() - 1];
    println!(
        "search: first call {first:?}; {n} keystrokes, mean {mean:?}, p99 {p99:?}, worst {worst:?}; budget 16 ms"
    );
    assert!(p99 < Duration::from_millis(16), "p99 keystroke {p99:?}");
}
