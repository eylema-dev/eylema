#[path = "fixtures/numeric.rs"]
mod f;
use eylema_journal::Storage;
use eylema_retrieval::*;
use std::{
    cell::RefCell,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};
#[derive(Clone)]
struct Mem(Rc<RefCell<Cursor<Vec<u8>>>>);
impl Read for Mem {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.borrow_mut().read(b)
    }
}
impl Write for Mem {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(b)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for Mem {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.0.borrow_mut().seek(p)
    }
}
impl Storage for Mem {
    fn byte_len(&mut self) -> io::Result<u64> {
        Ok(self.0.borrow().get_ref().len() as u64)
    }
    fn synchronize(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn complete_generations_empty_delete_and_corrupt_tail() {
    let m = Mem(Rc::new(RefCell::new(Cursor::new(vec![]))));
    let s = Snapshot::build(
        f::p(),
        f::binding(),
        1,
        f::docs(),
        &mut f::Hash,
        &mut f::c(),
    )
    .unwrap();
    let mut i = IndexStore::create(m.clone(), f::Hash, f::Hash, s, &mut f::c()).unwrap();
    assert_eq!(
        i.current()
            .search(&f::q(), &mut f::c())
            .unwrap()
            .hit_count(),
        2
    );
    for generation in 2..=4 {
        let s = Snapshot::build(
            f::p(),
            f::binding(),
            generation,
            vec![],
            &mut f::Hash,
            &mut f::c(),
        )
        .unwrap();
        i.append(s, generation - 1, &mut f::c()).unwrap();
    }
    let s = Snapshot::build(f::p(), f::binding(), 5, vec![], &mut f::Hash, &mut f::c()).unwrap();
    assert!(i.append(s, 4, &mut f::c()).is_err());
    drop(i);
    let i = IndexStore::open(
        m.clone(),
        f::Hash,
        f::Hash,
        &f::p(),
        &f::binding(),
        &mut f::c(),
    )
    .unwrap();
    assert_eq!(i.current().generation(), 4);
    assert!(i.current().documents().is_empty());
    drop(i);
    m.0.borrow_mut().get_mut().pop();
    assert!(IndexStore::open(m, f::Hash, f::Hash, &f::p(), &f::binding(), &mut f::c()).is_err());
}
#[test]
fn low_budget_is_rejected_without_history_read() {
    struct NoRead;
    impl Read for NoRead {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("budget denial must precede read")
        }
    }
    impl Write for NoRead {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!()
        }
        fn flush(&mut self) -> io::Result<()> {
            panic!()
        }
    }
    impl Seek for NoRead {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            panic!()
        }
    }
    impl Storage for NoRead {
        fn byte_len(&mut self) -> io::Result<u64> {
            Ok(512)
        }
        fn synchronize(&mut self) -> io::Result<()> {
            panic!()
        }
    }
    let mut control = Control::new(
        RetrievalBudget {
            max_work: 1,
            max_live_bytes: 1,
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(1),
        },
        CancelToken::new(),
    )
    .unwrap();
    assert!(matches!(
        IndexStore::open(
            NoRead,
            f::Hash,
            f::Hash,
            &f::p(),
            &f::binding(),
            &mut control
        ),
        Err(RetrievalError::LimitExceeded)
    ));
}
#[test]
fn checksummed_semantic_mutation_does_not_fall_back() {
    use eylema_contracts::codec::Value;
    use eylema_journal::{Journal, Limits};
    let m = Mem(Rc::new(RefCell::new(Cursor::new(vec![]))));
    let mut j = Journal::create_new(
        m.clone(),
        f::Hash,
        Limits {
            records: 4,
            payload_bytes: 1024 * 1024,
            file_bytes: 5 * 1024 * 1024,
        },
    )
    .unwrap();
    let s = Snapshot::build(
        f::p(),
        f::binding(),
        1,
        f::docs(),
        &mut f::Hash,
        &mut f::c(),
    )
    .unwrap();
    j.append(&s.encode(&mut f::c()).unwrap()).unwrap();
    let mut bad = s.encode(&mut f::c()).unwrap();
    let Value::List(e) = &mut bad else { panic!() };
    e[4] = Value::Unsigned(2);
    let Value::List(d) = &mut e[5] else { panic!() };
    let Value::List(d) = &mut d[0] else { panic!() };
    let Value::List(chunks) = &mut d[3] else {
        panic!()
    };
    let Value::List(ch) = &mut chunks[0] else {
        panic!()
    };
    ch[0] = Value::Unsigned(1);
    j.append(&bad).unwrap();
    drop(j);
    assert!(matches!(
        IndexStore::open(m, f::Hash, f::Hash, &f::p(), &f::binding(), &mut f::c()),
        Err(RetrievalError::CorruptSnapshot)
    ));
}
#[derive(Clone)]
struct Fault {
    m: Mem,
    stage: Rc<std::cell::Cell<u8>>,
}
impl Read for Fault {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.m.read(b)
    }
}
impl Write for Fault {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.stage.get() == 1 {
            Err(io::ErrorKind::Other.into())
        } else {
            self.m.write(b)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.stage.get() == 2 {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
impl Seek for Fault {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.m.seek(p)
    }
}
impl Storage for Fault {
    fn byte_len(&mut self) -> io::Result<u64> {
        if self.stage.get() == 4 {
            Err(io::ErrorKind::Other.into())
        } else {
            self.m.byte_len()
        }
    }
    fn synchronize(&mut self) -> io::Result<()> {
        if self.stage.get() == 3 {
            Err(io::ErrorKind::Other.into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn uncertain_writes_require_reopen_no_automatic_retry() {
    for stage in 1..=4 {
        let m = Mem(Rc::new(RefCell::new(Cursor::new(vec![]))));
        let trigger = Rc::new(std::cell::Cell::new(0));
        let storage = Fault {
            m,
            stage: trigger.clone(),
        };
        let s = Snapshot::build(
            f::p(),
            f::binding(),
            1,
            f::docs(),
            &mut f::Hash,
            &mut f::c(),
        )
        .unwrap();
        let mut index = IndexStore::create(storage, f::Hash, f::Hash, s, &mut f::c()).unwrap();
        let next =
            Snapshot::build(f::p(), f::binding(), 2, vec![], &mut f::Hash, &mut f::c()).unwrap();
        trigger.set(stage);
        assert!(index.append(next, 1, &mut f::c()).is_err());
        assert!(index.is_reopen_required());
        trigger.set(0);
        let next =
            Snapshot::build(f::p(), f::binding(), 2, vec![], &mut f::Hash, &mut f::c()).unwrap();
        assert!(matches!(
            index.append(next, 1, &mut f::c()),
            Err(RetrievalError::NeedsReopen)
        ));
    }
}
