//! MMC command-model tests — byte-exact encode, bounds-checked
//! decode, unsupported commands rejected (T94 model side).

use void_sync::{
    mmc_decode, mmc_encode, MmcCommand, MtcFrameRate, MtcTime,
};

#[test]
fn transport_commands_are_byte_exact() {
    // STOP for device 0x7F: F0 7F 7F 06 01 F7
    let b = mmc_encode(0x7F, &MmcCommand::Stop).unwrap();
    assert_eq!(b, vec![0xF0, 0x7F, 0x7F, 0x06, 0x01, 0xF7]);
    let (dev, cmd) = mmc_decode(&b).unwrap();
    assert_eq!(dev, 0x7F);
    assert_eq!(cmd, MmcCommand::Stop);

    for c in [
        MmcCommand::Play,
        MmcCommand::DeferredPlay,
        MmcCommand::FastForward,
        MmcCommand::Rewind,
        MmcCommand::RecordStrobe,
        MmcCommand::RecordExit,
        MmcCommand::RecordPause,
        MmcCommand::Pause,
        MmcCommand::Eject,
        MmcCommand::Chase,
        MmcCommand::CommandErrorReset,
        MmcCommand::MmcReset,
    ] {
        let b = mmc_encode(0x01, &c).unwrap();
        let (dev, back) = mmc_decode(&b).unwrap();
        assert_eq!(dev, 0x01);
        assert_eq!(back, c);
    }
}

#[test]
fn locate_carries_a_five_byte_time() {
    let t = MtcTime::new(1, 23, 45, 12, MtcFrameRate::F25).unwrap();
    let b = mmc_encode(
        0x02,
        &MmcCommand::Locate {
            time: t,
            subframe: 0,
        },
    )
    .unwrap();
    // F0 7F 02 06 44 06 01 <5-byte time> F7 = 13 bytes
    assert_eq!(b.len(), 13);
    assert_eq!(&b[..7], &[0xF0, 0x7F, 0x02, 0x06, 0x44, 0x06, 0x01]);
    // Time field: hr=(rate 1<<5)|1=0x21, mn=0x17, sc=0x2D, fr=0x0C, ff=0.
    assert_eq!(&b[7..12], &[0x21, 0x17, 0x2D, 0x0C, 0x00]);
    let (dev, cmd) = mmc_decode(&b).unwrap();
    assert_eq!(dev, 0x02);
    match cmd {
        MmcCommand::Locate { time, subframe } => {
            assert_eq!(time, t);
            assert_eq!(subframe, 0);
        }
        other => panic!("expected Locate, got {other:?}"),
    }
}

#[test]
fn shuttle_roundtrips_integral_packing() {
    for (fwd, sp) in [(true, 0u16), (true, 1000), (false, 500), (true, 0x3FFF)] {
        let b = mmc_encode(
            0x03,
            &MmcCommand::Shuttle {
                forward: fwd,
                speed_permille: sp,
            },
        )
        .unwrap();
        let (dev, cmd) = mmc_decode(&b).unwrap();
        assert_eq!(dev, 0x03);
        assert_eq!(
            cmd,
            MmcCommand::Shuttle {
                forward: fwd,
                speed_permille: sp
            }
        );
    }
    // Out of range.
    assert!(mmc_encode(
        0x03,
        &MmcCommand::Shuttle {
            forward: true,
            speed_permille: 0x4000
        }
    )
    .is_err());
}

#[test]
fn malformed_and_unsupported_frames_are_rejected() {
    assert!(mmc_decode(&[]).is_err());
    assert!(mmc_decode(&[0xF0, 0x7F, 0x00, 0x06, 0x01]).is_err()); // no F7
    assert!(mmc_decode(&[0xF0, 0x7F, 0x00, 0x07, 0x01, 0xF7]).is_err()); // response frame
    assert!(mmc_decode(&[0xF0, 0x7F, 0x00, 0x06, 0x7F, 0xF7]).is_err()); // unsupported cmd
    assert!(mmc_encode(0x80, &MmcCommand::Stop).is_err()); // device id bound
}
