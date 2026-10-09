//! T88 subprocess helper: runs a real av export under a kernel-enforced
//! file size limit (RLIMIT_FSIZE 64 KiB, SIGXFSZ ignored → writes fail
//! EFBIG). The parent test spawns this binary so the rlimit never
//! touches the shared test process.
//!
//! Exit 0 = the encode correctly failed and NOTHING published
//! (job Failed, no exports/av-* dir, staging cleaned). Any other
//! outcome is a failure of the honest-failure contract.

use std::path::PathBuf;
use void_av::AvRunner;
use void_av_tests::{fake_tools, spec};
use void_jobs::JobStatus;

fn main() {
    // Kernel-enforced "disk full": any write past 64 KiB fails EFBIG.
    unsafe {
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        let lim = libc::rlimit {
            rlim_cur: 64 * 1024,
            rlim_max: 64 * 1024,
        };
        if libc::setrlimit(libc::RLIMIT_FSIZE, &lim) != 0 {
            eprintln!("setrlimit failed");
            std::process::exit(2);
        }
    }
    let root = PathBuf::from(std::env::args().nth(1).expect("project root arg"));
    let project_id = "00000000-0000-4000-8000-0000000000aa".to_string();
    let jid = "00000000-0000-4000-8000-0000000000d1";
    let cid = "00000000-0000-4000-8000-0000000000c1";

    let runner = AvRunner::memory(&root).unwrap();
    // A 10 s program → ~300 VFMT KiB > the 64 KiB kernel cap, so the
    // artifact write must fail mid-render.
    let mut s = spec(jid, cid, &project_id);
    s.range_samples = "480000".into();
    runner.submit(&s).expect("submit");
    let st = runner.run(jid, &fake_tools(&[])).expect("run");
    assert_eq!(st, JobStatus::Failed, "full-disk encode must fail the job");

    // No artifact may exist anywhere.
    let mut leaked = false;
    for dir in [root.join("exports"), root.join("staging")] {
        if dir.is_dir() {
            leaked |= std::fs::read_dir(&dir).unwrap().next().is_some();
        }
    }
    if leaked {
        eprintln!("partial output leaked to disk after failed encode");
        std::process::exit(3);
    }
    println!("DISKFULL_OK");
}
