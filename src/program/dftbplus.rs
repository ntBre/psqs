use std::{fs::File, sync::OnceLock};

use regex::Regex;

use crate::{
    geom::{geom_string, Geom},
    program::Procedure,
};

use super::{Program, Template};

static INPUT_CELL: OnceLock<[Regex; 3]> = OnceLock::new();

struct DFTBPlus {
    filename: String,
    template: Template,
    charge: isize,
    geom: Geom,
}

impl Program for DFTBPlus {
    fn filename(&self) -> String {
        self.filename.clone()
    }

    fn infile(&self) -> String {
        todo!()
    }

    fn set_filename(&mut self, filename: &str) {
        self.filename = filename.into();
    }

    fn template(&self) -> &super::Template {
        &self.template
    }

    /// every file has to have the same name, so I don't actually need to match
    /// up extensions
    fn extension(&self) -> String {
        String::new()
    }

    fn charge(&self) -> isize {
        self.charge
    }

    /// Example [Template]:
    /// ```text
    /// Geometry = xyzFormat {
    /// {{.geom}}
    /// }
    ///
    /// Hamiltonian = DFTB {
    ///   Scc = Yes
    ///   SlaterKosterFiles = Type2FileNames {
    ///     Prefix = "/opt/dftb+/slako/mio/mio-1-1/"
    ///     Separator = "-"
    ///     Suffix = ".skf"
    ///   }
    ///   MaxAngularMomentum {
    ///     O = "p"
    ///     H = "s"
    ///   }
    ///   Charge = {{.charge}}
    /// }
    ///
    /// Options {}
    ///
    /// Analysis {
    ///   CalculateForces = Yes
    /// }
    ///
    /// ParserOptions {
    ///   ParserVersion = 12
    /// }

    /// ```
    fn write_input(&mut self, proc: Procedure) {
        use std::io::Write;
        let mut body = self.template().clone().header;
        // skip optgrad but accept optg at the end of a line
        let [opt, charge, geom_re] = INPUT_CELL.get_or_init(|| {
            [
                Regex::new(r"(?i)Driver = GeometryOptimization").unwrap(),
                Regex::new(r"\{\{.charge\}\}").unwrap(),
                Regex::new(r"\{\{.geom\}\}").unwrap(),
            ]
        });
        let mut found_opt = false;
        if opt.is_match(&body) {
            found_opt = true;
        }
        {
            use std::fmt::Write;
            match proc {
                Procedure::Opt => {
                    if !found_opt {
                        writeln!(
                            body,
                            r#"Driver = GeometryOptimization {{
  Optimizer = Rational {{}}
  MovedAtoms = 1:-1
  MaxSteps = 100
  OutputPrefix = "geom.out"
  Convergence {{GradAMax = 1E-8}}
}}"#,
                        )
                        .unwrap();
                    }
                }
                Procedure::Freq => todo!(),
                Procedure::SinglePt => {
                    if found_opt {
                        todo!("rewrite single point file without opt");
                    }
                }
            }
        }
        let geom = geom_string(&self.geom);
        let geom = if let Geom::Zmat(_) = &self.geom {
            panic!("don't know how to handle a Z-matrix in dftb+");
        } else {
            format!("{geom}\n")
        };
        body = geom_re.replace(&body, geom).to_string();
        body = charge
            .replace(&body, &format!("{}", self.charge))
            .to_string();

        let mut file = File::create(&self.filename).unwrap_or_else(|e| {
            panic!("failed to create {} with {e}", self.filename)
        });
        write!(file, "{body}").expect("failed to write input file");
    }

    fn read_output(
        filename: &str,
    ) -> Result<super::ProgramResult, super::ProgramError> {
        todo!("read {filename}")
    }

    fn associated_files(&self) -> Vec<String> {
        todo!()
    }

    fn new(
        filename: String,
        template: super::Template,
        charge: isize,
        geom: crate::geom::Geom,
    ) -> Self {
        Self {
            filename,
            template,
            charge,
            geom,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;
    use std::str::FromStr;

    use super::*;

    use crate::check;

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
}
