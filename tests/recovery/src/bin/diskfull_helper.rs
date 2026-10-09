//! T18 subprocess helper: runs a real save under a kernel-enforced file
//! size limit (RLIMIT_FSIZE, SIGXFSZ ignored → writes fail EFBIG). The
//! parent test spawns this binary so the rlimit never touches the shared
//! test process. Exit 0 = save correctly refused + last checkpoint intact.

use std::path::PathBuf;
use void_project::{recover, verify_checkpoint, PointerStatus, SaveRequest, SnapshotBundle};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: diskfull_helper <project_root> <revision>");
        std::process::exit(2);
    }
    let root = PathBuf::from(&args[1]);
    let revision: u64 = args[2].parse().unwrap();
    let project_id = args[3].clone();
    let parent = if args.len() > 4 {
        Some(args[4].clone())
    } else {
        None
    };

    // Kernel-enforced "disk full": any write past 64 KiB fails with EFBIG.
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

    // Payload larger than the 64 KiB file cap forces the write to fail
    // mid-stage — the real code path a full disk produces.
    let big = vec![0xABu8; 512 * 1024];
    let bundle = SnapshotBundle {
        engine_snapshot: big.clone(),
        app_state: big.clone(),
        command_receipts: big,
        asset_hashes: vec![],
    };
    let outcome = void_project::save(
        &root,
        SaveRequest {
            project_id,
            revision,
            engine_revision: "helper".into(),
            parent_checkpoint_id: parent.clone(),
        },
        &bundle,
    );

    match outcome {
        Ok(r) => {
            println!("UNEXPECTED_DURABLE {}", r.checkpoint_id);
            std::process::exit(1); // a "durable" save under a full disk is a bug
        }
        Err(e) => {
            println!("SAVE_REFUSED {e}");
        }
    }

    // The previously published checkpoint must still verify end to end.
    let report = recover(&root).expect("recover");
    match (&report.pointer, &report.current) {
        (Some(PointerStatus::Valid { .. }), Some(c)) => {
            match verify_checkpoint(&root, &c.checkpoint_id) {
                Ok(v) => {
                    println!("LAST_OK {} rev={}", v.checkpoint_id, v.revision);
                    std::process::exit(0);
                }
                Err(e) => {
                    println!("LAST_CORRUPT {e}");
                    std::process::exit(1);
                }
            }
        }
        _ if parent.is_none() => {
            // No prior checkpoint existed — "no durable save" is correct.
            println!("LAST_NONE");
            std::process::exit(0);
        }
        (p, c) => {
            println!("BAD_STATE pointer={p:?} current={c:?}");
            std::process::exit(1);
        }
    }
}
