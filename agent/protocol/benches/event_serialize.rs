use agent_protocol::events::*;
use agent_protocol::state::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn make_event() -> Event {
    Event {
        seq: 1,
        payload: EventPayload::ItemDelta(ItemDeltaData {
            item_id: "item-42".to_string(),
            delta: ItemDelta::Text(
                "Lorem ipsum dolor sit amet, consectetur adipiscing elit.".to_string(),
            ),
        }),
    }
}

fn event_serialize(c: &mut Criterion) {
    let event = make_event();
    c.bench_function("event_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&event)).unwrap())
    });
}

fn event_deserialize(c: &mut Criterion) {
    let json = serde_json::to_string(&make_event()).unwrap();
    c.bench_function("event_deserialize", |b| {
        b.iter(|| {
            let ev: Event = serde_json::from_str(black_box(&json)).unwrap();
            ev
        })
    });
}

fn event_roundtrip(c: &mut Criterion) {
    let event = make_event();
    c.bench_function("event_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&event)).unwrap();
            let ev: Event = serde_json::from_str(&json).unwrap();
            ev
        })
    });
}

criterion_group!(benches, event_serialize, event_deserialize, event_roundtrip);
criterion_main!(benches);
