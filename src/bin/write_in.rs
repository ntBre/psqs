use std::rc::Rc;

use psqs::{
    geom::Geom,
    program::{mopac::Mopac, Procedure, Program, Template},
};

fn test_mopac() -> Mopac {
    Mopac::new_full(
        String::from("/tmp/test"),
        Rc::new(Geom::Xyz(Vec::new())),
        0,
        Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
    )
}

fn main() {
    let mut tm = test_mopac();
    tm.param_dir = Some("/tmp".to_string());
    let mut res = Vec::new();
    for _ in 0..1000 {
        tm.write_input(Procedure::SinglePt);
        res.push(());
    }
}
