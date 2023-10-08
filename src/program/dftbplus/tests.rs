use std::fs::read_to_string;
use std::str::FromStr;

use symm::Atom;

use super::*;

use crate::check;
use crate::program::ProgramResult;

#[test]
fn write_input() {
    let template = Template::from(
        "
Geometry = xyzFormat {
{{.geom}}
}

Hamiltonian = DFTB {
  Scc = Yes
  SlaterKosterFiles = Type2FileNames {
    Prefix = \"/opt/dftb+/slako/mio/mio-1-1/\"
    Separator = \"-\"
    Suffix = \".skf\"
  }
  MaxAngularMomentum {
    O = \"p\"
    H = \"s\"
  }
  Charge = {{.charge}}
}

Options {
}

Analysis {
  CalculateForces = Yes
}

ParserOptions {
  ParserVersion = 12
}
",
    );

    let mut d = DFTBPlus {
        filename: "/tmp/dftb_in.hsd".into(),
        template,
        charge: 0,
        geom: Geom::from_str(
            "    3
Geometry Step: 9
    O      0.00000000     -0.71603315      0.00000000
    H      0.00000000     -0.14200298      0.77844804
    H     -0.00000000     -0.14200298     -0.77844804
",
        )
        .unwrap(),
    };

    d.write_input(Procedure::Opt);
    check!("testfiles/dftb+/single_opt.want", "/tmp/dftb_in.hsd");

    d.write_input(Procedure::SinglePt);
    check!("testfiles/dftb+/single_single.want", "/tmp/dftb_in.hsd");
}

#[test]
fn read_opt_output() {
    let got = DFTBPlus::read_output("testfiles/dftb+/opt/out").unwrap();
    let want = ProgramResult {
        energy: -4.0779379326,
        cart_geom: Some(vec![
            Atom::new_from_label("O", 0.00000000, -0.71603315, 0.00000000),
            Atom::new_from_label("H", 0.00000000, -0.14200298, 0.77844804),
            Atom::new_from_label("H", -0.00000000, -0.14200298, -0.77844804),
        ]),
        time: 0.05,
    };
    assert_eq!(got, want);
}

#[test]
fn read_single_output() {
    let got = DFTBPlus::read_output("testfiles/dftb+/single/out").unwrap();
    let want = ProgramResult {
        energy: -3.9798793068,
        cart_geom: None,
        time: 0.03,
    };
    assert_eq!(got, want);
}
