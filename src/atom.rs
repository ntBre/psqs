use std::{
    io::{self, ErrorKind},
    str::FromStr,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub label: String,
    pub coord: Vec<f64>,
}

impl Atom {
    pub fn new(label: &str, coord: Vec<f64>) -> Self {
        Self {
            label: label.to_string(),
            coord,
        }
    }
}

impl FromStr for Atom {
    type Err = io::Error;

    /// parse an Atom from a line like
    ///  C 1.0 1.0 1.0
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = s.split_whitespace().collect();
        if fields.len() != 4 {
            return Err(io::Error::new(
                ErrorKind::Other,
                "wrong number of fields in Atom",
            ));
        }
        let coord = fields[1..].iter().map(|s| s.parse());
        if coord.clone().any(|s| s.is_err()) {
            return Err(io::Error::new(
                ErrorKind::Other,
                "failed to parse coordinate field as f64",
            ));
        }
        let coord: Vec<_> = coord.flatten().collect();
        Ok(Self {
            label: fields[0].to_string(),
            coord: vec![coord[0], coord[1], coord[2]],
        })
    }
}

impl ToString for Atom {
    fn to_string(&self) -> String {
        format!(
            "{:2} {:15.10} {:15.10} {:15.10}",
            self.label, self.coord[0], self.coord[1], self.coord[2]
        )
    }
}

#[derive(Debug)]
pub enum Geom {
    Xyz(Vec<Atom>),
    Zmat(String),
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
