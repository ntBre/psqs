use nalgebra::vector;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, f64::consts::FRAC_PI_2, fmt::Display, str::FromStr,
};
use symm::atom::Atom;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Geom {
    Xyz(Vec<Atom>),
    Zmat(String),
}

impl Default for Geom {
    fn default() -> Self {
        Self::Xyz(Default::default())
    }
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
            Geom::Zmat(g) => write!(f, "{g}")?,
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

impl Geom {
    /// returns a reference to `self`'s atoms if it is already an XYZ geometry
    /// or None for a Z-matrix. see [Geom::into_xyz] for a version that returns
    /// the atoms or converts the Z-matrix if needed.
    pub fn xyz(&self) -> Option<&Vec<Atom>> {
        match &self {
            Geom::Xyz(x) => Some(x),
            Geom::Zmat(_) => None,
        }
    }

    /// returns the inner atoms if `self` is already an XYZ geometry or
    /// constructs them from the Z-matrix if not. panics if the Z-matrix cannot
    /// be converted to XYZ
    pub fn into_xyz(self) -> Vec<Atom> {
        match self {
            Geom::Xyz(atoms) => atoms,
            Geom::Zmat(s) => zmat_to_xyz(&s),
        }
    }

    pub fn zmat(&self) -> Option<&String> {
        match &self {
            Geom::Zmat(x) => Some(x),
            Geom::Xyz(_) => None,
        }
    }

    pub fn is_xyz(&self) -> bool {
        matches!(self, Geom::Xyz(_))
    }

    pub fn is_zmat(&self) -> bool {
        matches!(self, Geom::Zmat(_))
    }
}

/// call `eprintln` with the arguments and then exit(1)
macro_rules! die {
    ($($t:tt)+) => {
        eprintln!($($t)*);
        std::process::exit(1);
    };
}

pub(crate) fn zmat_to_xyz(s: &str) -> Vec<Atom> {
    let mut params = HashMap::new(); /* first pass for gathering parameters */
    let mut atom_lines = Vec::new();
    let mut sp = Vec::new();
    for line in s.lines().filter(|s| !s.trim().is_empty()) {
        if line.contains('=') {
            line.split_ascii_whitespace().collect_into(&mut sp);
            params.insert(sp[0], sp[2]);
            sp.clear();
        } else {
            atom_lines.push(line);
        }
    }

    let mut atoms = Vec::new();
    for atom in atom_lines {
        atom.split_ascii_whitespace().collect_into(&mut sp);
        match sp.len() {
            1 => {
                // put the first atom at 0, 0, 0
                atoms.push(Atom::new_from_label(sp[0], 0.0, 0.0, 0.0))
            }
            3 => {
                // second atom along z axis
                let z = get_parameter(&params, sp[2]);
                assert_eq!(sp[1], "1", "second atom must be bonded to first");
                atoms.push(Atom::new_from_label(sp[0], 0.0, 0.0, z));
            }
            5 => {
                let bond_index = parse_or_die::<usize>(sp[1]) - 1;
                let angl_index = parse_or_die::<usize>(sp[3]) - 1;
                let bond_atom = atoms[bond_index];
                match bond_index {
                    0 => assert_eq!(angl_index, 1),
                    1 => assert_eq!(angl_index, 0),
                    b => {
                        panic!("invalid bond index {b} for atom 3 in Z-matrix")
                    }
                }
                let origin = bond_atom;
                let r = get_parameter(&params, sp[2]);
                let t = get_parameter(&params, sp[4]).to_radians();
                // factor of -pi/2 to match molpro
                let p: f64 = -std::f64::consts::FRAC_PI_2;
                let x = r * t.sin() * p.cos();
                let y = r * t.sin() * p.sin();
                let z = r * t.cos();
                atoms.push(Atom::new_from_label(
                    sp[0],
                    origin.x - x,
                    origin.y - y,
                    origin.z - z,
                ));
            }
            7 => {
                let bond_index = parse_or_die::<usize>(sp[1]) - 1;
                let angl_index = parse_or_die::<usize>(sp[3]) - 1;
                let tors_index = parse_or_die::<usize>(sp[5]) - 1;
                assert!(
                    tors_index < atoms.len()
                        && tors_index != bond_index
                        && tors_index != angl_index
                );
                let b = atoms[bond_index];
                let a = atoms[angl_index];
                let origin = vector![b.x - a.x, b.y - a.y, b.z - a.z];
                let r = get_parameter(&params, sp[2]);
                let t = get_parameter(&params, sp[4]).to_radians();
                let p = get_parameter(&params, sp[6]).to_radians() - FRAC_PI_2;
                let x = r * t.sin() * p.cos();
                let y = r * t.sin() * p.sin();
                let z = r * t.cos();
                atoms.push(Atom::new_from_label(
                    sp[0],
                    origin.x - x,
                    origin.y - y,
                    origin.z - z,
                ));
            }
            _ => {
                eprintln!("malformed Z-matrix entry: {atom}");
                std::process::exit(1);
            }
        }
        sp.clear();
    }
    atoms.retain(|atom| atom.atomic_number != 0);
    atoms
}

#[inline]
fn parse_or_die<T: FromStr>(s: &str) -> T {
    s.parse::<T>().unwrap_or_else(|_| {
        die!("failed to parse `{s}` as {}", std::any::type_name::<T>());
    })
}

fn get_parameter(params: &HashMap<&str, &str>, s: &str) -> f64 {
    let x = match params.get(s) {
        Some(x) => x,
        None => s,
    };
    let Ok(x) = x.parse::<f64>() else {
        die!("failed to parse {x} as a float in Z-matrix");
    };
    x
}

pub fn geom_string(geom: &Geom) -> String {
    use std::fmt::Write;
    match geom {
        Geom::Xyz(geom) => {
            let mut ret = String::with_capacity(50 * geom.len());
            for g in geom {
                writeln!(
                    ret,
                    "{} {:.12} {:.12} {:.12}",
                    g.label(),
                    g.x,
                    g.y,
                    g.z
                )
                .unwrap();
            }
            ret
        }
        Geom::Zmat(geom) => geom.to_string(),
    }
}
