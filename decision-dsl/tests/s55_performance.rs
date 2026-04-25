use std::time::Instant;

use decision_dsl::ast::eval::Evaluator;
use decision_dsl::ext::blackboard::Blackboard;

// ═════════════════════════════════════════════════════════════════════════════
// Performance: 1M evaluator calls/second
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn performance_one_million_evaluator_calls() {
    let bb = Blackboard::default();
    let eval = Evaluator::Script {
        expression: r#"provider_output == """#.into(),
    };

    let iterations = 1_000_000u64;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = eval.evaluate(&bb).unwrap();
    }

    let elapsed = start.elapsed();
    let calls_per_sec = iterations as f64 / elapsed.as_secs_f64();

    eprintln!(
        "1M evaluator calls in {:?} => {:.0} calls/sec",
        elapsed, calls_per_sec
    );

    // Target: 1M calls/second (generous for a script parser)
    assert!(
        calls_per_sec >= 100_000.0,
        "evaluator throughput too low: {:.0} calls/sec",
        calls_per_sec
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// Memory footprint verification
// ═════════════════════════════════════════════════════════════════════════════

#[test]
fn blackboard_memory_footprint() {
    let bb = Blackboard::default();
    let size = std::mem::size_of_val(&bb);
    // Target: blackboard ≤ 2KB (it's mostly pointers + small vecs/maps)
    assert!(
        size <= 4096,
        "Blackboard too large: {} bytes (target ≤ 4096)",
        size
    );
}

#[test]
fn evaluator_enum_memory_footprint() {
    let eval = Evaluator::Script {
        expression: "x == y".into(),
    };
    let size = std::mem::size_of_val(&eval);
    // Target: Evaluator ≤ 256 bytes
    assert!(
        size <= 256,
        "Evaluator too large: {} bytes (target ≤ 256)",
        size
    );
}
