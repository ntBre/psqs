use symm::atom::Atom;

#[derive(Debug)]
pub enum Geom {
    Xyz(Vec<Atom>),
    Zmat(String),
}

impl std::fmt::Display for Geom {
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
