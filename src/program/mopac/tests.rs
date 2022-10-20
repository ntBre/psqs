use std::collections::HashSet;
use std::fs::{self, read_to_string};

use crate::program::Procedure;
use crate::string;

use crate::queue::{self, Queue, SubQueue, Submit};

use super::*;

fn test_mopac() -> Mopac {
    Mopac::new_full(
        String::from("/tmp/test"),
        Rc::new(Geom::Xyz(Vec::new())),
        0,
        Template::from("scfcrt=1.D-21 aux(precision=14) PM6 A0"),
    )
}

#[test]
fn test_write_input() {
    let mut tm = test_mopac();
    tm.param_dir = Some("/tmp".to_string());
    tm.write_input(Procedure::SinglePt);
    let got = fs::read_to_string("/tmp/test.mop").expect("file not found");
    let want = "scfcrt=1.D-21 aux(precision=14) PM6 A0 charge=0 1SCF XYZ
Comment line 1
Comment line 2

"
    .to_string();
    assert_eq!(got, want);
    fs::remove_file("/tmp/test.mop").unwrap();
}

#[test]
fn test_write_params() {
    let names = vec![
        "USS", "ZS", "BETAS", "GSS", "USS", "UPP", "ZS", "ZP", "BETAS",
        "BETAP", "GSS", "GPP", "GSP", "GP2", "HSP",
    ];
    let atoms = vec![
        "H", "H", "H", "H", "C", "C", "C", "C", "C", "C", "C", "C", "C", "C",
        "C",
    ];
    #[rustfmt::skip]
    let values = vec![
        -11.246958000000, 1.268641000000, -8.352984000000,
        14.448686000000, -51.089653000000, -39.937920000000,
        2.047558000000, 1.702841000000, -15.385236000000,
        -7.471929000000, 13.335519000000, 10.778326000000,
        11.528134000000, 9.486212000000, 0.717322000000,
    ];
    let params = Params::from(
        names.iter().map(|s| s.to_string()).collect(),
        atoms.iter().map(|s| s.to_string()).collect(),
        values,
    );
    Mopac::write_params(&params, &String::from("/tmp/params.dat"));
    let got = fs::read_to_string("/tmp/params.dat").expect("file not found");
    let want = "USS H -11.246958000000
ZS H 1.268641000000
BETAS H -8.352984000000
GSS H 14.448686000000
USS C -51.089653000000
UPP C -39.937920000000
ZS C 2.047558000000
ZP C 1.702841000000
BETAS C -15.385236000000
BETAP C -7.471929000000
GSS C 13.335519000000
GPP C 10.778326000000
GSP C 11.528134000000
GP2 C 9.486212000000
HSP C 0.717322000000
";
    assert_eq!(got, want);
    fs::remove_file("/tmp/params.dat").unwrap();
}

extern crate test;
use test::Bencher;
#[bench]
fn bench_read_output(b: &mut Bencher) {
    b.iter(|| Mopac::read_output("testfiles/job"));
}

#[bench]
fn bench_read_aux(b: &mut Bencher) {
    b.iter(|| Mopac::read_aux("testfiles/job"));
}

#[bench]
fn bench_write_input(b: &mut Bencher) {
    let mut tm = test_mopac();
    tm.param_dir = Some("/tmp".to_string());
    b.iter(|| tm.write_input(Procedure::SinglePt));
}

#[bench]
fn bench_geom_string(b: &mut Bencher) {
    let tm = test_mopac();
    b.iter(|| test::black_box(geom_string(&tm.geom)));
}

#[test]
fn test_read_output() {
    let res = Mopac::read_output("testfiles/job").unwrap();
    let got = res.energy;
    let want = 9.712_794_745_916_472e1 / KCALHT;
    assert!((got - want).abs() < 1e-20);

    assert_eq!(res.time, 0.015625);

    // opt success
    let got = Mopac::read_output("testfiles/opt").unwrap().cart_geom;
    let want = vec![
        Atom::new_from_label(
            "C",
            0.000000000000000000,
            0.000000000000000000,
            0.000000000000000000,
        ),
        Atom::new_from_label(
            "C",
            1.436_199_643_883_821_2,
            0.000000000000000000,
            0.000000000000000000,
        ),
        Atom::new_from_label(
            "C",
            0.799_331_622_330_450_3,
            1.193_205_084_901_411_7,
            0.000000000000000000,
        ),
        Atom::new_from_label(
            "H",
            2.360_710_453_618_393,
            -0.506_038_360_297_709_7,
            0.000000000000026804,
        ),
        Atom::new_from_label(
            "H",
            0.893_457_241_509_136_9,
            2.242_936_206_295_408_6,
            -0.000000000000026804,
        ),
    ];
    assert_eq!(got, Some(want));

    // failure (no termination message) in output - now catches noaux error
    // instead
    let f = String::from("testfiles/nojob");
    let got = Mopac::read_output(&f);
    assert_eq!(got.err().unwrap(), ProgramError::FileNotFound(f + ".aux"));

    // failure in aux
    let f = String::from("testfiles/noaux");
    let got = Mopac::read_output(&f);
    assert_eq!(got.err().unwrap(), ProgramError::FileNotFound(f + ".aux"));
}

/// minimal queue for testing general submission
struct TestQueue;

impl Submit<Mopac> for TestQueue {}

impl Queue<Mopac> for TestQueue {
    fn write_submit_script(&self, infiles: &[String], filename: &str) {
        let mut body = String::new();
        for f in infiles {
            body.push_str(&format!("echo {f}\n"));
        }
        let mut file =
            File::create(filename).expect("failed to create params file");
        write!(file, "{}", body).expect("failed to write params file");
    }
}

impl SubQueue<Mopac> for TestQueue {
    fn submit_command(&self) -> &str {
        "bash"
    }

    fn chunk_size(&self) -> usize {
        128
    }

    fn job_limit(&self) -> usize {
        1600
    }

    fn sleep_int(&self) -> usize {
        1
    }

    const SCRIPT_EXT: &'static str = "pbs";

    fn dir(&self) -> &str {
        "inp"
    }

    fn stat_cmd(&self) -> String {
        todo!()
    }

    fn status(&self) -> HashSet<String> {
        todo!()
    }
}

#[test]
fn test_submit() {
    let tq = TestQueue;
    tq.write_submit_script(
        &string!["input1.mop", "input2.mop", "input3.mop"],
        "/tmp/main.pbs",
    );
    let got = tq.submit("/tmp/main.pbs");
    let want = "input3.mop";
    assert_eq!(got, want);
}

#[test]
fn test_resubmit() {
    use std::path::Path;
    let tq = TestQueue;
    std::fs::copy("testfiles/job.mop", "/tmp/job.mop").unwrap();
    let got = tq.resubmit("/tmp/job.mop");
    assert!(Path::new("/tmp/job_redo.mop").exists());
    assert!(Path::new("/tmp/job_redo.pbs").exists());
    assert_eq!(
        read_to_string("/tmp/job.mop").unwrap(),
        read_to_string("/tmp/job_redo.mop").unwrap()
    );
    let want = queue::Resubmit {
        inp_file: String::from("/tmp/job_redo"),
        pbs_file: String::from("/tmp/job_redo.pbs"),
        job_id: String::from("/tmp/job_redo"),
    };
    assert_eq!(got, want);

    for f in ["/tmp/job.mop", "/tmp/job_redo.mop", "/tmp/job_redo.pbs"] {
        std::fs::remove_file(f).unwrap();
    }
}
