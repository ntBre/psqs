use std::rc::Rc;

use criterion::{criterion_group, criterion_main, Criterion};
use psqs::{
    geom::Geom,
    program::{mopac::Mopac, Template},
};

pub fn read_aux(c: &mut Criterion) {
    let mp = Mopac::new_full(
        String::from("testfiles/job"),
        None,
        Rc::new(Geom::Xyz(Vec::new())),
        0,
        Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
    );
    c.bench_function("read aux", |b| b.iter(|| mp.read_aux()));
}

criterion_group!(benches, read_aux);
criterion_main!(benches);
