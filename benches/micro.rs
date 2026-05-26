//! Micro-benchmarks for pure-Rust hot paths.
//! Run with: `cargo run --release --bin bench-micro`

use std::time::Instant;

use phone_tv::history::reappeared_packages;
use phone_tv::history::types::{CleanSession, DeviceHistory};
use phone_tv::security::bulletins::bulletins_behind;

const WARMUP: usize = 100;

fn bench<F: FnMut()>(name: &str, iters: usize, mut f: F) {
    for _ in 0..WARMUP {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per_op = elapsed.as_nanos() as f64 / iters as f64;
    let per_op_str = if per_op < 1000.0 {
        format!("{:>8.1} ns/op", per_op)
    } else if per_op < 1_000_000.0 {
        format!("{:>8.1} µs/op", per_op / 1000.0)
    } else {
        format!("{:>8.2} ms/op", per_op / 1_000_000.0)
    };
    println!(
        "{:<42} {:>10} iters  {}  ({:?} total)",
        name, iters, per_op_str, elapsed
    );
}

fn fixture_history() -> DeviceHistory {
    DeviceHistory {
        serial: "BENCH-SERIAL".into(),
        brand: "samsung".into(),
        model: "SM-G991B".into(),
        display_name: "Bench device".into(),
        first_seen: "2025-01-01".into(),
        sessions: (0..10)
            .map(|i| CleanSession {
                date: format!("2025-{:02}-15", i + 1),
                android_version: "14".into(),
                security_patch: "2025-01-05".into(),
                score_before: 50,
                score_after: 80,
                risk_score_before: 60,
                risk_score_after: 30,
                apps_removed: (0..20).map(|j| format!("com.bloat.app{}", j)).collect(),
                apps_disabled: (0..10).map(|j| format!("com.disable.app{}", j)).collect(),
                apps_failed: vec![],
                vulns_found: 5,
                vulns_patched: 3,
                profile_used: "Moderate".into(),
                ai_suggestions_accepted: 2,
            })
            .collect(),
    }
}

fn fixture_apps(n: usize) -> Vec<String> {
    let mut v: Vec<String> = (0..n).map(|i| format!("com.example.app{}", i)).collect();
    // sprinkle a few packages that were previously removed
    for i in 0..5 {
        v.push(format!("com.bloat.app{}", i));
    }
    v
}

fn main() {
    println!(
        "\nPhone-TV micro-benchmarks (warmup={} iters)\n{}",
        WARMUP,
        "-".repeat(85)
    );

    // --- bulletins lookup ---
    bench("bulletins_behind (recent patch)", 50_000, || {
        let _ = std::hint::black_box(bulletins_behind("2025-12-05"));
    });
    bench("bulletins_behind (old patch)", 50_000, || {
        let _ = std::hint::black_box(bulletins_behind("2024-01-05"));
    });
    bench("bulletins_behind (invalid)", 50_000, || {
        let _ = std::hint::black_box(bulletins_behind("not-a-date"));
    });

    // --- history diff ---
    let history = fixture_history();
    let apps_300 = fixture_apps(300);
    let apps_1000 = fixture_apps(1000);
    bench(
        "reappeared_packages (10 sessions × 300 apps)",
        10_000,
        || {
            let _ = std::hint::black_box(reappeared_packages(&history, &apps_300));
        },
    );
    bench(
        "reappeared_packages (10 sessions × 1000 apps)",
        5_000,
        || {
            let _ = std::hint::black_box(reappeared_packages(&history, &apps_1000));
        },
    );

    // --- JSON parsing (LLM verdict shape) ---
    let llm_json = serde_json::to_string(
        &(0..50)
            .map(|i| {
                serde_json::json!({
                    "package": format!("com.example.app{}", i),
                    "verdict": "bloatware",
                    "category": "tracker",
                    "profile": "moderate",
                    "explanation": "Tracker publicitaire avec collecte d'identifiants"
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap();
    bench("serde_json::from_str (50-app verdict array)", 5_000, || {
        let _: serde_json::Value = serde_json::from_str(&llm_json).unwrap();
    });

    println!();
}
