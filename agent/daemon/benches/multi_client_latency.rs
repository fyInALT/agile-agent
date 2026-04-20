use agent_daemon::broadcaster::EventBroadcaster;
use agent_protocol::events::*;
use agent_protocol::state::AgentSlotStatus;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn make_event(seq: u64) -> Event {
    Event {
        seq,
        payload: EventPayload::AgentStatusChanged(AgentStatusChangedData {
            agent_id: "a1".to_string(),
            status: AgentSlotStatus::Running,
        }),
    }
}

fn broadcast_latency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    for client_count in [1, 3, 10] {
        c.bench_function(&format!("broadcast_{}_clients", client_count), |b| {
            b.to_async(&rt).iter(|| async {
                let broadcaster = EventBroadcaster::new();
                let mut receivers = Vec::new();
                for i in 0..client_count {
                    receivers.push(broadcaster.register(format!("conn-{}", i)).await);
                }

                let event = make_event(1);
                broadcaster.broadcast(black_box(event.clone())).await;

                // Wait for all receivers to confirm delivery.
                for mut rx in receivers {
                    let _ = tokio::time::timeout(
                        tokio::time::Duration::from_secs(1),
                        rx.recv(),
                    )
                    .await;
                }
            });
        });
    }
}

criterion_group!(benches, broadcast_latency);
criterion_main!(benches);
