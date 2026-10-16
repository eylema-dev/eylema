#[path = "fixtures/numeric.rs"]
mod f;
use eylema_contracts::codec::{self, Value};
use eylema_retrieval::*;
#[test]
fn canonical_roundtrip_rank_ties_and_allowlist() {
    let s = Snapshot::build(
        f::p(),
        f::binding(),
        1,
        f::docs(),
        &mut f::Hash,
        &mut f::c(),
    )
    .unwrap();
    let value = s.encode(&mut f::c()).unwrap();
    let again = Snapshot::decode(&value, &mut f::Hash, &mut f::c()).unwrap();
    let mut q = f::q();
    assert_eq!(again.search(&q, &mut f::c()).unwrap().hit_count(), 2);
    q.max_hits = 1;
    assert!(matches!(
        s.search(&q, &mut f::c()),
        Err(RetrievalError::TieLimit)
    ));
    q.max_hits = 2;
    q.sources = vec![f::r("a")];
    let result = s.search(&q, &mut f::c()).unwrap();
    assert_eq!(result.hit_count(), 1);
    let bytes = result.encode(65536, &mut f::c()).unwrap();
    let Value::List(envelope) = codec::decode(&bytes).unwrap() else {
        panic!()
    };
    let Value::List(hits) = &envelope[7] else {
        panic!()
    };
    let Value::List(hit) = &hits[0] else { panic!() };
    assert_eq!(hit[1], Value::Text("a".into()));
    assert_eq!(hit[9], Value::Signed(1_000_000));
    assert!(result.encode(1, &mut f::c()).is_err());
}
#[test]
fn every_mode_has_literal_full_scores_and_recall() {
    let s = Snapshot::build(
        f::p(),
        f::binding(),
        1,
        f::docs(),
        &mut f::Hash,
        &mut f::c(),
    )
    .unwrap();
    for (mode, wants) in [
        (0, vec![1_000_000, 1_000_000, 0, -1_000_000]),
        (1, vec![1_000_000, 1_000_000, 333333, 0]),
        (2, vec![1_000_000, 1_000_000, 166666, -500000]),
    ] {
        let mut q = f::q();
        q.top_k = 4;
        q.max_hits = 4;
        let mut v = vec![0.; 1024];
        v[0] = 1.;
        q.input = match mode {
            0 => QueryInput::Vector(v),
            1 => QueryInput::Lexical("apple red".into()),
            _ => QueryInput::Hybrid {
                text: "apple red".into(),
                vector: v,
                vector_weight: 500000,
            },
        };
        let bytes = s
            .search(&q, &mut f::c())
            .unwrap()
            .encode(65536, &mut f::c())
            .unwrap();
        let Value::List(e) = codec::decode(&bytes).unwrap() else {
            panic!()
        };
        let Value::List(h) = &e[7] else { panic!() };
        for (i, (h, w)) in h.iter().zip(wants).enumerate() {
            let Value::List(h) = h else { panic!() };
            assert_eq!(h[1], Value::Text(["a", "d", "b", "c"][i].into()));
            assert_eq!(h[9], Value::Signed(w));
        }
    }
}
#[test]
fn mutation_and_binding_rejected() {
    let s = Snapshot::build(
        f::p(),
        f::binding(),
        1,
        f::docs(),
        &mut f::Hash,
        &mut f::c(),
    )
    .unwrap();
    let mut v = s.encode(&mut f::c()).unwrap();
    let Value::List(e) = &mut v else { panic!() };
    e[4] = Value::Unsigned(0);
    assert!(Snapshot::decode(&v, &mut f::Hash, &mut f::c()).is_err());
    let mut q = f::q();
    q.binding.artifact_sha256 = [0; 32];
    assert!(s.search(&q, &mut f::c()).is_err());
    q = f::q();
    q.generation = 2;
    assert!(matches!(
        s.search(&q, &mut f::c()),
        Err(RetrievalError::StaleGeneration)
    ));
    q = f::q();
    q.sources = vec![f::r("missing")];
    assert!(matches!(
        s.search(&q, &mut f::c()),
        Err(RetrievalError::NotIndexed)
    ));
}
