#[derive(Debug, Clone)]
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
