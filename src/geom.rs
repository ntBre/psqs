use std::{fmt::Display, str::FromStr};

use symm::atom::Atom;

#[derive(Debug, PartialEq)]
pub enum Geom {
    Xyz(Vec<Atom>),
    Zmat(String),
}

impl Display for Geom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Geom::Xyz(atoms) => {
                for atom in atoms {
                    writeln!(
                        f,
                        "{:5}{:15.10}{:15.10}{:15.10}",
                        atom.label(),
                        atom.x,
                        atom.y,
                        atom.z,
                    )?
                }
            }
            Geom::Zmat(_) => todo!(),
        }
        Ok(())
    }
}

impl From<symm::Molecule> for Geom {
    fn from(mol: symm::Molecule) -> Self {
        Geom::Xyz(mol.atoms)
    }
}

impl FromStr for Geom {
    type Err = std::string::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut atoms = Vec::new();
        let mut skip = 0;
        for line in s.lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if skip > 0 {
                skip -= 1;
                continue;
            } else if fields.is_empty() {
                continue;
            } else if fields.len() == 1 {
                // one field, all letters => zmat
                if fields[0].chars().all(char::is_alphabetic) {
                    return Ok(Geom::Zmat(String::from(s)));
                } else {
                    // else, start of XYZ with comment line
                    skip = 1;
                    continue;
                }
            } else {
                atoms.push(line.parse().unwrap());
            }
        }
        Ok(Geom::Xyz(atoms))
    }
}

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

impl Geom {
    pub fn xyz(&self) -> Option<&Vec<Atom>> {
        match &self {
            Geom::Xyz(x) => Some(x),
            Geom::Zmat(_) => None,
        }
    }
    pub fn zmat(&self) -> Option<&String> {
        match &self {
            Geom::Zmat(x) => Some(x),
            Geom::Xyz(_) => None,
        }
    }

    pub fn is_xyz(&self) -> bool {
        match &self {
            Geom::Xyz(_) => true,
            _ => false,
        }
    }
    pub fn is_zmat(&self) -> bool {
        match &self {
            Geom::Zmat(_) => true,
            _ => false,
        }
    }
}

pub fn geom_string(geom: &Geom) -> String {
    use std::fmt::Write;
    let mut ret = String::new();
    match geom {
        Geom::Xyz(geom) => {
            for g in geom {
                write!(ret, "{}\n", g.to_string()).unwrap();
            }
        }
        Geom::Zmat(geom) => ret.push_str(&geom),
    }
    ret
}
