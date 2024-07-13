use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, str::FromStr};
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
                atoms.push(Atom::new_from_label(sp[0], 0.0, 0.0, z));
            }
            5 => {
                // third atom - use a 2D rotation matrix
                // https://stackoverflow.com/a/11774765:
                // (x cos θ + y sin θ, -x sin θ + y cos θ), but we know y is
                // zero, giving (x cos θ, -x sin θ). the answer also states that
                // this will give the magnitude in terms of the known vector, so
                // we need to divide by its magnitude and multiply by the new
                // magnitude. again, the magnitude is x, so the answer becomes
                // (r cos θ, -r sin θ), where r is the new parameter's magnitude
                let r = get_parameter(&params, sp[2]);
                let t = get_parameter(&params, sp[4]).to_radians();
                atoms.push(Atom::new_from_label(
                    sp[0],
                    r * t.cos(),
                    -r * t.sin(),
                    0.0,
                ));
            }
            7 => {
                // let r = get_parameter(&params, sp[2]);
                // let t = get_parameter(&params, sp[4]).to_radians();
                // let p = get_parameter(&params, sp[6]).to_radians();

                // this is the atom we're bound to (central atom in angle)
                let b = atoms[parse_or_die::<usize>(sp[1]) - 1];
                // and the atom to make an angle with
                let a = atoms[parse_or_die::<usize>(sp[3]) - 1];
                // this time we actually have to look up the atom it makes an
                // angle with and handle a rotation in 3D. this one is also not
                // necessarily about the origin. we will also need the central
                // atom because the vectors are relative to that, not the
                // origin. otherwise an angle with atom 1 (0,0,0) wouldn't make
                // much sense
                use nalgebra as na;
                let b = na::vector![b.x, b.y, b.z];
                let a = na::vector![a.x, a.y, a.z];

                // see
                // en.wikipedia.org/wiki/Rotation_matrix#Rotation_matrix_from_axis_and_angle
                let ba = a - b;
                let e_ba = ba / ba.magnitude();
                todo!()
            }
            _ => {
                eprintln!("malformed Z-matrix entry: {atom}");
                std::process::exit(1);
            }
        }
        sp.clear();
    }
    atoms
}

#[inline]
fn parse_or_die<T: FromStr>(s: &str) -> T {
    s.parse::<T>().unwrap_or_else(|_| {
        die!("failed to parse `{s}` as {}", std::any::type_name::<T>());
    })
}

fn get_parameter(params: &HashMap<&str, &str>, s: &str) -> f64 {
    let Some(x) = params.get(s) else {
        die!("unrecognized parameter `{s}` in Z-matrix");
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
