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
    let tests = [
        // basic linear water
        (
            "H\nO 1 OH\nH 2 OH 1 HOH\n\nOH = 1.0\nHOH = 180.0",
            vec![
                Atom::new(1, 0.0, 0.0, 0.0),
                Atom::new(8, 0.0, 0.0, 1.0),
                Atom::new(1, 0.0, 0.0, 2.0),
            ],
        ),
        // more realistic water
        (
            "H\nO 1 OH\nH 2 OH 1 HOH\n\nOH = 1.0\nHOH = 109.5",
            vec![
                Atom::new(1, 0.0, 0.0, 0.0),
                Atom::new(8, 0.0, 0.0, 1.0),
                Atom::new(1, 0.0, 0.942641491, 1.333806859),
            ],
        ),
        // formaldehyde, tests all four atoms
        (
            "O\nC 1 CO\nH 2 CH 1 OCH\nH 2 CH 1 OCH 3 D\n\n\
            CO = 1.2\nCH = 1.0\nOCH = 109.5\nD = 180.0",
            vec![
                Atom::new(8, 0.000000000, 0.000000000, 0.000000000),
                Atom::new(6, 0.000000000, 0.000000000, 1.200000000),
                Atom::new(1, 0.000000000, 0.942641491, 1.533806859),
                Atom::new(1, 0.000000000, -0.942641491, 1.533806859),
            ],
        ),
        (
            "
X
H 1 1.0
C 2 CH1 1 90.0
C 2 CH2 1 90.0 3 0.0
C 4 CC 2 CCH 1 90.0
C 4 CC 2 CCH 5 180.0
H 5 HC 4 HCC 2 0.0
H 6 HC 4 HCC 2 0.0

CH1 =                 1.07255623
CH2 =                 2.30676989
CC =                  1.51392942
CCH =               155.12551628
HC =                  1.07782362
HCC =               133.02614172
",
            vec![
                Atom::new(1, 0.000000000, 0.000000000, 1.000000000),
                Atom::new(6, 0.000000000, 1.072556230, 1.000000000),
                Atom::new(6, 0.000000000, 2.306769890, 1.000000000),
                Atom::new(6, -0.636806896, 3.680254242, 1.000000000),
                Atom::new(6, 0.636806896, 3.680254242, 1.000000000),
                Atom::new(1, -1.660992881, 4.016032181, 1.000000000),
                Atom::new(1, 1.660992881, 4.016032181, 1.000000000),
            ],
        ),
    ];

    for (s, want) in tests {
        let got = zmat_to_xyz(s);
        assert_eq!(
            got,
            want,
            "got =\n{}\nwant =\n{}",
            Geom::Xyz(got.clone()),
            Geom::Xyz(want.clone())
        );
    }
}
