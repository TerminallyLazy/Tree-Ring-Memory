use std::env;
use std::time::Instant;
use tempfile::tempdir;
use tree_ring_memory_core::MemoryEvent;
use tree_ring_memory_sqlite::{MemoryRetriever, SQLiteMemoryStore};

const MIN_INSERTS_PER_SECOND: f64 = 500.0;
const MAX_RECALL_MS: f64 = 250.0;

fn main() {
    let count = env::args()
        .nth(1)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(10_000);

    let dir = tempdir().expect("temp dir");
    let db_path = dir.path().join("memory.sqlite");
    let mut store = SQLiteMemoryStore::open(&db_path).expect("open store");

    let started = Instant::now();
    let mut events = Vec::with_capacity(count);
    for index in 0..count {
        let mut event = if index % 50 == 0 {
            let mut event = MemoryEvent::new(
                format!("Avoid stale deployment rollback cache failure {index}."),
                "warning",
            )
            .expect("memory event");
            event.tags = vec!["deployment".to_string(), "cache".to_string()];
            event
        } else {
            let mut event = MemoryEvent::new(
                format!("Implementation note {index} for subsystem {}.", index % 17),
                "lesson",
            )
            .expect("memory event");
            event.tags = vec![format!("subsystem-{}", index % 17)];
            event
        };
        event.project = Some("bench".to_string());
        events.push(event);
    }
    store.put_many(&events).expect("put memories");
    let insert_elapsed = started.elapsed();

    let retriever = MemoryRetriever::new(&store);
    let mut latencies = Vec::new();
    let mut query_metrics = Vec::new();
    for query in [
        "deployment rollback cache",
        "subsystem 7",
        "implementation note",
    ] {
        let started = Instant::now();
        let results = retriever
            .recall(
                query,
                Some("bench"),
                None,
                None,
                None,
                None,
                false,
                false,
                8,
                false,
            )
            .expect("recall");
        let elapsed = started.elapsed();
        assert!(
            !results.is_empty(),
            "performance smoke recall returned no results for {query:?}"
        );
        let latency_ms = elapsed.as_secs_f64() * 1000.0;
        latencies.push(latency_ms);
        query_metrics.push(serde_json::json!({
            "query": query,
            "latency_ms": latency_ms,
            "first_summary": results[0].memory.summary.clone(),
        }));
        println!(
            "QUERY={query:?} LATENCY_MS={:.3} FIRST_SUMMARY={:?}",
            latency_ms,
            results
                .first()
                .map(|result| result.memory.summary.as_str())
                .unwrap_or("<none>")
        );
    }

    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let max = latencies
        .iter()
        .copied()
        .fold(0.0_f64, |current, item| current.max(item));
    let insert_total_ms = insert_elapsed.as_secs_f64() * 1000.0;
    let inserts_per_second = count as f64 / insert_elapsed.as_secs_f64();

    assert!(
        inserts_per_second >= MIN_INSERTS_PER_SECOND,
        "insert throughput {inserts_per_second:.1}/s is below threshold {MIN_INSERTS_PER_SECOND:.1}/s"
    );
    assert!(
        max <= MAX_RECALL_MS,
        "recall max latency {max:.3}ms is above threshold {MAX_RECALL_MS:.3}ms"
    );

    println!("INSERTED={count}");
    println!("INSERT_TOTAL_MS={insert_total_ms:.1}");
    println!("INSERTS_PER_SECOND={inserts_per_second:.1}");
    println!("RECALL_AVG_MS={avg:.3}");
    println!("RECALL_MAX_MS={max:.3}");
    println!(
        "METRICS_JSON={}",
        serde_json::json!({
            "inserted": count,
            "insert_total_ms": insert_total_ms,
            "inserts_per_second": inserts_per_second,
            "recall_avg_ms": avg,
            "recall_max_ms": max,
            "thresholds": {
                "min_inserts_per_second": MIN_INSERTS_PER_SECOND,
                "max_recall_ms": MAX_RECALL_MS,
            },
            "queries": query_metrics,
        })
    );
}
