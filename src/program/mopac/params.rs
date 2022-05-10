use nalgebra as na;

#[derive(Debug, Clone)]
pub struct Params {
    pub names: Vec<String>,
    pub atoms: Vec<String>,
    pub values: na::DVector<f64>,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            names: Default::default(),
            atoms: Default::default(),
            values: na::DVector::from(vec![0.; 0]),
        }
    }
}

impl ToString for Params {
    fn to_string(&self) -> String {
        let mut ret = String::new();
        for (i, n) in self.names.iter().enumerate() {
            ret.push_str(
                &format!(
                    "{:<8}{:>8}{:20.12}\n",
                    n, self.atoms[i], self.values[i]
                )
                .to_string(),
            );
        }
        ret
    }
}

impl PartialEq for Params {
    fn eq(&self, other: &Self) -> bool {
        for (i, n) in self.names.iter().enumerate() {
            if *n != other.names[i] {
                #[cfg(test)]
                eprintln!("{}: {} != {}", i, *n, other.names[i]);
                return false;
            }
            if self.atoms[i] != other.atoms[i] {
                #[cfg(test)]
                eprintln!("{}: {} != {}", i, self.atoms[i], other.atoms[i]);
                return false;
            }
            let diff = (self.values[i] - other.values[i]).abs();
            if diff >= 1e-12 {
                #[cfg(test)]
                eprintln!(
                    "{}: {} != {}, diff = {}",
                    i, self.values[i], other.values[i], diff
                );
                return false;
            }
        }
        true
    }
}

impl Params {
    pub fn new(
        names: Vec<String>,
        atoms: Vec<String>,
        values: na::DVector<f64>,
    ) -> Self {
        Self {
            names,
            atoms,
            values,
        }
    }
    pub fn from(
        names: Vec<String>,
        atoms: Vec<String>,
        values: Vec<f64>,
    ) -> Self {
        Self {
            names,
            atoms,
            values: na::DVector::from(values),
        }
    }

    pub fn from_literal(
        names: Vec<&str>,
        atoms: Vec<&str>,
        values: Vec<f64>,
    ) -> Self {
        Self {
            names: names.iter().map(|s| s.to_string()).collect(),
            atoms: atoms.iter().map(|s| s.to_string()).collect(),
            values: na::DVector::from(values),
        }
    }

    pub fn len(&self) -> usize {
        assert_eq!(self.names.len(), self.atoms.len());
        assert_eq!(self.names.len(), self.values.len());
        self.names.len()
    }
}
