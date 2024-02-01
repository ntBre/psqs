use crate::geom::{zmat_to_xyz, Geom};
use symm::Atom;

#[test]
fn test_from_zmat() {
    let s = "H
O 1 OH
H 2 OH 1 HOH

OH = 1.0
HOH = 109.5";
    let got = s.parse::<Geom>().unwrap();
    assert_eq!(got, Geom::Zmat(s.to_string()));
}

#[test]
fn test_from_cart() {
    let got = "
3
water geometry
 H          0.0000000000        0.7574590974        0.5217905143
 O          0.0000000000        0.0000000000       -0.0657441568
 H          0.0000000000       -0.7574590974        0.5217905143
"
    .parse::<Geom>()
    .unwrap();
    assert_eq!(
        got,
        Geom::Xyz(vec![
            Atom::new(1, 0.0000000000, 0.7574590974, 0.5217905143),
            Atom::new(8, 0.0000000000, 0.0000000000, -0.0657441568),
            Atom::new(1, 0.0000000000, -0.7574590974, 0.5217905143),
        ])
    );
}

#[test]
fn test_zmat_to_xyz() {
    let s = "H
O 1 OH
H 2 OH 1 HOH

OH = 1.0
HOH = 180.0";
    let got = zmat_to_xyz(s);
    let want = vec![
        Atom::new(1, 0.0, 0.0, 0.0),
        Atom::new(8, 1.0, 0.0, 0.0),
        Atom::new(1, -1.0, 0.0, 0.0),
    ];
    assert_eq!(
        got,
        want,
        "got =\n{}\nwant =\n{}",
        Geom::Xyz(got.clone()),
        Geom::Xyz(want.clone())
    );
}
