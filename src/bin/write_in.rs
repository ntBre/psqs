use psqs::{
    geom::Geom,
    program::{mopac::Mopac, Procedure, Program, Template},
};

fn test_mopac() -> Mopac {
    // let names = vec![
    //     "USS", "ZS", "BETAS", "GSS", "USS", "UPP", "ZS", "ZP", "BETAS",
    //     "BETAP", "GSS", "GPP", "GSP", "GP2", "HSP",
    // ];
    // let atoms = vec![
    //     "H", "H", "H", "H", "C", "C", "C", "C", "C", "C", "C", "C", "C", "C",
    //     "C",
    // ];
    // #[rustfmt::skip]
    //     let values = vec![
    //         -11.246958000000, 1.268641000000, -8.352984000000,
    //         14.448686000000, -51.089653000000, -39.937920000000,
    //         2.047558000000, 1.702841000000, -15.385236000000,
    //         -7.471929000000, 13.335519000000, 10.778326000000,
    //         11.528134000000, 9.486212000000, 0.717322000000,
    //     ];
    Mopac::new(
        String::from("/tmp/test"),
        Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
        0,
        Geom::Xyz(Vec::new()),
    )
}

fn main() {
    let mut tm = test_mopac();
    let mut res = Vec::new();
    for _ in 0..1000 {
        tm.write_input(Procedure::SinglePt);
        res.push(());
    }
}
