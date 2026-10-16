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
