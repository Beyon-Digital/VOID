//! MTC byte-fixture encode/decode round-trips + MIDI Clock math
//! (T94 evidence: real bit packing, not approximations).

use void_sync::{
    decode_full_message, demux, encode_full_message, encode_quarter_frame_pieces,
    encode_quarter_frames, pulses_to_ticks, quarter_frame_message, ticks_to_pulses,
    ClockTransport, MtcFrameRate, MtcTime, MtcWireEvent, QuarterFrameAssembler, MIDI_CLOCK_PPQN,
};

#[test]
fn full_message_byte_fixture_is_exact() {
    // 01:23:45:12 @ 25fps → F0 7F 7F 01 01 21 17 2D 0C F7
    let t = MtcTime::new(1, 23, 45, 12, MtcFrameRate::F25).unwrap();
    let bytes = encode_full_message(&t);
    assert_eq!(
        bytes,
        [0xF0, 0x7F, 0x7F, 0x01, 0x01, 0x21, 0x17, 0x2D, 0x0C, 0xF7]
    );
    let back = decode_full_message(&bytes).unwrap();
    assert_eq!(back, t);

    // 30-drop rate code packs into the hour byte's bits 6–5.
    let t = MtcTime::new(23, 59, 59, 29, MtcFrameRate::F30Drop).unwrap();
    let bytes = encode_full_message(&t);
    assert_eq!(bytes[5], (0x02 << 5) | 23);
    assert_eq!(decode_full_message(&bytes).unwrap(), t);
}

#[test]
fn quarter_frame_pieces_are_bit_exact() {
    // 01:23:45:12 @ 25 → nibbles [C 0 D 2 7 1 1 5]:
    // frames=0x0C → piece0=0x0C piece1=0; seconds=0x2D → piece2=0x0D
    // piece3=0x02; minutes=0x17 → piece4=0x07 piece5=0x01;
    // hours=0x01 → piece6=0x01 piece7=(rate=1)<<1|0=0x02... wait:
    // rate 25 code = 1 → piece7 = (1<<1)|((1>>4)&1) = 0x02.
    let t = MtcTime::new(1, 23, 45, 12, MtcFrameRate::F25).unwrap();
    let pieces = encode_quarter_frame_pieces(&t);
    assert_eq!(pieces, [0x0C, 0x00, 0x0D, 0x02, 0x07, 0x01, 0x01, 0x02]);

    let wire = encode_quarter_frames(&t);
    for i in 0..8 {
        assert_eq!(wire[i * 2], 0xF1);
        assert_eq!(wire[i * 2 + 1], ((i as u8) << 4) | pieces[i]);
    }

    // Feed the wire back through the assembler → same time.
    let mut asm = QuarterFrameAssembler::new();
    let mut assembled = None;
    for chunk in wire.chunks(2) {
        assembled = asm.feed_message(chunk).unwrap().or(assembled);
    }
    assert_eq!(assembled, Some(t));
}

#[test]
fn quarter_frames_out_of_order_still_assemble() {
    let t = MtcTime::new(5, 30, 12, 8, MtcFrameRate::F24).unwrap();
    let mut asm = QuarterFrameAssembler::new();
    // Running order starts mid-cycle in real streams.
    let order = [4, 5, 6, 7, 0, 1, 2, 3];
    let pieces = encode_quarter_frame_pieces(&t);
    let mut got = None;
    for &i in &order {
        got = asm
            .feed_message(&quarter_frame_message(i, pieces[i]))
            .unwrap()
            .or(got);
    }
    assert_eq!(got, Some(t));
}

#[test]
fn malformed_mtc_is_rejected_not_coerced() {
    // Wrong length.
    assert!(decode_full_message(&[0xF0, 0x7F]).is_err());
    // Missing F7.
    let mut bad = encode_full_message(&MtcTime::new(0, 0, 0, 0, MtcFrameRate::F30).unwrap());
    bad[9] = 0x00;
    assert!(decode_full_message(&bad).is_err());
    // Wrong sub-id (not an MTC full frame).
    let mut bad = encode_full_message(&MtcTime::new(0, 0, 0, 0, MtcFrameRate::F30).unwrap());
    bad[3] = 0x02;
    assert!(decode_full_message(&bad).is_err());
    // High bit set in a data byte.
    let mut bad = encode_full_message(&MtcTime::new(0, 0, 0, 0, MtcFrameRate::F30).unwrap());
    bad[6] = 0x80;
    assert!(decode_full_message(&bad).is_err());
    // Out-of-range fields on construction.
    assert!(MtcTime::new(24, 0, 0, 0, MtcFrameRate::F24).is_err());
    assert!(MtcTime::new(0, 0, 0, 24, MtcFrameRate::F24).is_err());
    // Assembler rejects high-bit data bytes.
    let mut asm = QuarterFrameAssembler::new();
    assert!(asm.feed_data_byte(0x8F).is_err());
}

#[test]
fn demux_separates_frames_and_transport() {
    let t = MtcTime::new(0, 0, 0, 0, MtcFrameRate::F30).unwrap();
    let mut stream = Vec::new();
    stream.extend_from_slice(&encode_full_message(&t));
    stream.extend_from_slice(&[0xFA, 0xF8, 0xF8, 0xFC]);
    let (events, leftover) = demux(&stream);
    assert!(leftover.is_empty());
    assert_eq!(events[0], MtcWireEvent::Full(t));
    assert_eq!(events[1], MtcWireEvent::ClockStart);
    assert_eq!(events[2], MtcWireEvent::ClockTick);
    assert_eq!(events[4], MtcWireEvent::ClockStop);
}

#[test]
fn clock_tick_math_is_exact_rational() {
    // VOID contract: 960_000 ticks per quarter; 24 pulses per quarter.
    let ppq = 960_000u32;
    let tp = ticks_to_pulses(960_000, ppq).unwrap();
    assert_eq!(tp.pulses, 24);
    assert_eq!(tp.remainder_ticks, 0);
    // 2.5 quarters = 2_400_000 ticks = 60 pulses.
    let tp = ticks_to_pulses(2_400_000, ppq).unwrap();
    assert_eq!(tp.pulses, 60);
    // Fractional ticks: 40_000 ticks = 1 pulse (40_000*24/960_000 = 1).
    let tp = ticks_to_pulses(40_000, ppq).unwrap();
    assert_eq!(tp.pulses, 1);
    // 20_000 ticks = 0.5 pulse — remainder is reported, not rounded.
    let tp = ticks_to_pulses(20_000, ppq).unwrap();
    assert_eq!(tp.pulses, 0);
    assert!(tp.remainder_ticks > 0);
    // Inverse: 24 pulses → 960_000 ticks exactly.
    let (ticks, rem) = pulses_to_ticks(24, ppq).unwrap();
    assert_eq!(ticks, 960_000);
    assert_eq!(rem, 0);
    assert_eq!(MIDI_CLOCK_PPQN, 24);
}

#[test]
fn clock_transport_start_continue_stop() {
    let mut t = ClockTransport::new(960_000).unwrap();
    assert!(!t.is_running());
    assert!(t.pulse_at(960_000).is_err()); // not running → typed error

    t.start();
    assert!(t.is_running());
    assert_eq!(t.pulse_at(960_000).unwrap(), 24);
    assert_eq!(t.pulse_at(480_000).unwrap(), 12);
    assert_eq!(t.song_ticks_at(24).unwrap(), 960_000);

    // Stop holds; continue resumes the mapping from held position.
    t.stop();
    assert!(t.pulse_at(0).is_err());
    t.continue_at(960_000, 24);
    assert_eq!(t.pulse_at(1_920_000).unwrap(), 48);
}
