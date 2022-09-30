use std::rc::Rc;

use psqs::{
    geom::Geom,
    program::{mopac::Mopac, Program, Template},
};

fn main() {
    let mp = Mopac::new(
        String::from("testfiles/job"),
        None,
        Rc::new(Geom::Xyz(Vec::new())),
        0,
        Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
    );
    let mut res = Vec::new();
    for _ in 0..1000 {
        res.push(mp.read_output());
    }
}
