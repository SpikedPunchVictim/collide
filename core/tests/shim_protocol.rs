//! Tests for the shim protocol, the journal, the frames and `exit.json` (ts-0016): serde round-trips for every
//! message and for `exit.json`, the frame and journal codecs cut at every byte offset, the version handshake, a
//! damaged `exit.json`, input that is not a message at all, and the generated schemas.

use collide_core::frame::{
    Decoded, FRAME_PREFIX_BYTES, FrameError, MAX_FRAME_PAYLOAD, decode_frame, decode_message,
    encode_frame, encode_message,
};
use collide_core::journal::{
    Direction, JournalEnd, JournalError, JournalRecord, MAX_RECORD_BYTES, RECORD_HEADER_BYTES,
    encode_record, read_journal, read_journal_from,
};
use collide_core::schema;
use collide_core::shim::{
    DaemonMessage, ExitEnd, ExitRecord, ExitRecordError, SHIM_PROTOCOL, ShimMessage, StopSignal,
    VersionMatch, accept_shim, answer_hello, check_shim_version, exit_record_bytes,
    parse_exit_record, parse_exit_record_for, previous_version,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: T,
    wire: Value,
) {
    assert_eq!(serde_json::to_value(&value).unwrap(), wire, "written form");
    assert_eq!(
        serde_json::from_value::<T>(wire).unwrap(),
        value,
        "read form"
    );
}

fn schema_file(name: &str) -> Value {
    serde_json::from_str(&schema::registry().render()[name]).unwrap()
}

fn exit_record() -> ExitRecord {
    ExitRecord {
        protocol: SHIM_PROTOCOL,
        end: ExitEnd::Exited { code: 3 },
        cpu_user_us: 379_000,
        cpu_system_us: 21_500,
        max_rss_bytes: 61_000_000,
    }
}

fn record(seq: u64, direction: Direction, bytes: &[u8]) -> JournalRecord {
    JournalRecord {
        seq,
        wall_ns: 1_700_000_000_000_000_000 + seq,
        direction,
        bytes: bytes.to_vec(),
    }
}

/// Three records of both directions, with one empty `bytes` and one that holds every byte value.
fn three_records() -> Vec<JournalRecord> {
    let every_byte: Vec<u8> = (0..=255).collect();
    vec![
        record(0, Direction::In, br#"{"jsonrpc":"2.0","id":1}"#),
        record(1, Direction::Out, &every_byte),
        record(2, Direction::In, b""),
    ]
}

fn journal_bytes(records: &[JournalRecord]) -> Vec<u8> {
    records
        .iter()
        .flat_map(|record| encode_record(record).unwrap())
        .collect()
}

/// Bytes that look like nothing: a deterministic generator, so a failure repeats.
fn noise(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as u8
        })
        .collect()
}

// Serde round-trips.

#[test]
fn every_shim_message_round_trips() {
    round_trip(
        ShimMessage::Hello { protocol: 1 },
        json!({ "type": "hello", "protocol": 1 }),
    );
    round_trip(
        ShimMessage::Wrote { seq: 41 },
        json!({ "type": "wrote", "seq": 41 }),
    );
    round_trip(
        ShimMessage::Exited {
            exit: exit_record(),
        },
        json!({
            "type": "exited",
            "exit": {
                "protocol": 1,
                "end": { "type": "exited", "code": 3 },
                "cpu_user_us": 379000,
                "cpu_system_us": 21500,
                "max_rss_bytes": 61000000
            }
        }),
    );
    round_trip(
        ShimMessage::Failed {
            message: "journal write failed: no space left on device".into(),
        },
        json!({ "type": "failed", "message": "journal write failed: no space left on device" }),
    );
}

#[test]
fn every_daemon_message_round_trips() {
    round_trip(
        DaemonMessage::Welcome { protocol: 1 },
        json!({ "type": "welcome", "protocol": 1 }),
    );
    round_trip(
        DaemonMessage::Refused {
            message: "too old".into(),
        },
        json!({ "type": "refused", "message": "too old" }),
    );
    round_trip(
        DaemonMessage::Write {
            data: "{\"a\":1}\n".into(),
        },
        json!({ "type": "write", "data": "{\"a\":1}\n" }),
    );
    round_trip(
        DaemonMessage::CloseStdin {},
        json!({ "type": "close_stdin" }),
    );
    round_trip(
        DaemonMessage::Signal {
            signal: StopSignal::Term,
        },
        json!({ "type": "signal", "signal": "term" }),
    );
    round_trip(DaemonMessage::AckExit {}, json!({ "type": "ack_exit" }));
}

#[test]
fn every_stop_signal_round_trips() {
    round_trip(StopSignal::Hup, json!("hup"));
    round_trip(StopSignal::Term, json!("term"));
    round_trip(StopSignal::Kill, json!("kill"));
}

#[test]
fn exit_json_round_trips_for_an_exit_and_for_a_signal() {
    round_trip(
        exit_record(),
        json!({
            "protocol": 1,
            "end": { "type": "exited", "code": 3 },
            "cpu_user_us": 379000,
            "cpu_system_us": 21500,
            "max_rss_bytes": 61000000
        }),
    );
    round_trip(
        ExitRecord {
            end: ExitEnd::Signaled { signal: 9 },
            ..exit_record()
        },
        json!({
            "protocol": 1,
            "end": { "type": "signaled", "signal": 9 },
            "cpu_user_us": 379000,
            "cpu_system_us": 21500,
            "max_rss_bytes": 61000000
        }),
    );
}

#[test]
fn exit_json_bytes_read_back_as_the_record_written() {
    for record in [
        exit_record(),
        ExitRecord {
            end: ExitEnd::Signaled { signal: 15 },
            ..exit_record()
        },
    ] {
        assert_eq!(
            parse_exit_record(&exit_record_bytes(&record)).unwrap(),
            record
        );
    }
}

#[test]
fn an_unknown_key_fails_in_every_message_and_in_exit_json() {
    let messages = [
        json!({ "type": "hello", "protocol": 1 }),
        json!({ "type": "wrote", "seq": 1 }),
        json!({ "type": "failed", "message": "m" }),
    ];
    for mut message in messages {
        assert!(serde_json::from_value::<ShimMessage>(message.clone()).is_ok());
        message["surprise"] = json!(1);
        let error = serde_json::from_value::<ShimMessage>(message)
            .unwrap_err()
            .to_string();
        assert!(error.contains("surprise"), "{error}");
    }
    let messages = [
        json!({ "type": "welcome", "protocol": 1 }),
        json!({ "type": "refused", "message": "m" }),
        json!({ "type": "write", "data": "d" }),
        json!({ "type": "close_stdin" }),
        json!({ "type": "signal", "signal": "hup" }),
        json!({ "type": "ack_exit" }),
    ];
    for mut message in messages {
        assert!(serde_json::from_value::<DaemonMessage>(message.clone()).is_ok());
        message["surprise"] = json!(1);
        let error = serde_json::from_value::<DaemonMessage>(message)
            .unwrap_err()
            .to_string();
        assert!(error.contains("surprise"), "{error}");
    }
    let mut exit = serde_json::to_value(exit_record()).unwrap();
    exit["end"]["surprise"] = json!(1);
    assert!(serde_json::from_value::<ExitRecord>(exit).is_err());
}

#[test]
fn a_signal_outside_the_vocabulary_fails() {
    let message = json!({ "type": "signal", "signal": "stop" });
    assert!(serde_json::from_value::<DaemonMessage>(message).is_err());
}

// Frames.

#[test]
fn a_frame_is_a_four_byte_big_endian_length_then_the_payload() {
    let frame = encode_frame(b"hello").unwrap();
    assert_eq!(FRAME_PREFIX_BYTES, 4);
    assert_eq!(frame, [0, 0, 0, 5, b'h', b'e', b'l', b'l', b'o']);
    let long = vec![7u8; 0x0102];
    assert_eq!(&encode_frame(&long).unwrap()[..4], [0, 0, 1, 2]);
}

#[test]
fn a_frame_round_trips_with_its_consumed_length() {
    for payload in [&b""[..], b"x", b"{\"type\":\"ack_exit\"}"] {
        let frame = encode_frame(payload).unwrap();
        match decode_frame(&frame).unwrap() {
            Decoded::Whole {
                payload: read,
                consumed,
            } => {
                assert_eq!(read, payload);
                assert_eq!(consumed, frame.len());
            }
            other => panic!("expected a whole frame, got {other:?}"),
        }
    }
}

#[test]
fn a_frame_cut_at_every_offset_is_never_whole_and_is_whole_once_the_rest_arrives() {
    let payload = br#"{"type":"write","data":"hello"}"#;
    let frame = encode_frame(payload).unwrap();
    for cut in 0..frame.len() {
        match decode_frame(&frame[..cut]).unwrap() {
            Decoded::Incomplete { needed } => {
                assert!(needed >= 1, "cut at {cut}: needed {needed}");
                if cut >= FRAME_PREFIX_BYTES {
                    assert_eq!(needed, frame.len() - cut, "cut at {cut}");
                }
            }
            Decoded::Whole { .. } => panic!("a frame cut at {cut} was decoded as whole"),
        }
        let mut arrived = frame[..cut].to_vec();
        arrived.extend_from_slice(&frame[cut..]);
        match decode_frame(&arrived).unwrap() {
            Decoded::Whole {
                payload: read,
                consumed,
            } => {
                assert_eq!(read, payload, "cut at {cut}");
                assert_eq!(consumed, frame.len());
            }
            other => panic!("cut at {cut}: expected whole after the rest, got {other:?}"),
        }
    }
}

#[test]
fn a_message_cut_at_every_offset_is_never_a_message_and_decodes_once_the_rest_arrives() {
    let message = DaemonMessage::Write {
        data: "{\"method\":\"session/prompt\"}\n".into(),
    };
    let frame = encode_message(&message).unwrap();
    for cut in 0..frame.len() {
        assert_eq!(
            decode_message::<DaemonMessage>(&frame[..cut]).unwrap(),
            None,
            "cut at {cut}"
        );
    }
    assert_eq!(
        decode_message::<DaemonMessage>(&frame).unwrap(),
        Some((message, frame.len()))
    );
}

#[test]
fn two_frames_in_one_buffer_decode_one_at_a_time() {
    let first = encode_message(&DaemonMessage::CloseStdin {}).unwrap();
    let second = encode_message(&DaemonMessage::AckExit {}).unwrap();
    let both: Vec<u8> = [first.clone(), second.clone()].concat();
    let (message, consumed) = decode_message::<DaemonMessage>(&both).unwrap().unwrap();
    assert_eq!(message, DaemonMessage::CloseStdin {});
    assert_eq!(consumed, first.len());
    let (message, consumed) = decode_message::<DaemonMessage>(&both[consumed..])
        .unwrap()
        .unwrap();
    assert_eq!(message, DaemonMessage::AckExit {});
    assert_eq!(consumed, second.len());
}

#[test]
fn a_length_over_the_cap_is_refused_as_soon_as_the_prefix_is_read() {
    let too_big = (MAX_FRAME_PAYLOAD as u32 + 1).to_be_bytes();
    let error = decode_frame(&too_big).unwrap_err();
    assert_eq!(
        error,
        FrameError::TooLarge {
            len: MAX_FRAME_PAYLOAD + 1,
            max: MAX_FRAME_PAYLOAD
        }
    );
    let text = error.to_string();
    assert!(
        text.contains(&(MAX_FRAME_PAYLOAD + 1).to_string()),
        "{text}"
    );
    assert!(text.contains(&MAX_FRAME_PAYLOAD.to_string()), "{text}");
    assert!(encode_frame(&vec![0u8; MAX_FRAME_PAYLOAD + 1]).is_err());
}

#[test]
fn a_whole_frame_that_is_not_a_message_is_a_typed_error_naming_the_cause() {
    let frame = encode_frame(br#"{"type":"nonsense"}"#).unwrap();
    let error = decode_message::<DaemonMessage>(&frame).unwrap_err();
    assert!(matches!(error, FrameError::Payload { .. }), "{error:?}");
    assert!(error.to_string().contains("nonsense"), "{error}");
    let frame = encode_frame(&[0xff, 0xfe]).unwrap();
    assert!(matches!(
        decode_message::<DaemonMessage>(&frame),
        Err(FrameError::Payload { .. })
    ));
}

#[test]
fn the_frame_decoder_never_panics_on_garbage() {
    for seed in 0..200 {
        for len in [0, 1, 3, 4, 5, 17, 64] {
            let bytes = noise(seed, len);
            let _ = decode_frame(&bytes);
            let _ = decode_message::<DaemonMessage>(&bytes);
            let _ = decode_message::<ShimMessage>(&bytes);
        }
    }
}

// The journal.

#[test]
fn a_journal_record_is_the_documented_bytes() {
    let bytes = encode_record(&JournalRecord {
        seq: 1,
        wall_ns: 2,
        direction: Direction::Out,
        bytes: b"ab".to_vec(),
    })
    .unwrap();
    assert_eq!(RECORD_HEADER_BYTES, 21);
    let mut expected = Vec::new();
    expected.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 1]); // seq, u64 big-endian
    expected.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 2]); // wall_ns, u64 big-endian
    expected.push(1); // dir: 0 in, 1 out
    expected.extend_from_slice(&[0, 0, 0, 2]); // len, u32 big-endian
    expected.extend_from_slice(b"ab");
    assert_eq!(bytes, expected);
    let in_record = encode_record(&record(0, Direction::In, b"")).unwrap();
    assert_eq!(in_record[16], 0);
}

#[test]
fn a_journal_round_trips_both_directions_in_seq_order() {
    let records = three_records();
    let read = read_journal(&journal_bytes(&records));
    assert_eq!(read.records, records);
    assert_eq!(read.end, JournalEnd::Clean);
    assert_eq!(read.valid_len, journal_bytes(&records).len());
}

#[test]
fn an_empty_journal_reads_as_no_records() {
    let read = read_journal(&[]);
    assert!(read.records.is_empty());
    assert_eq!(read.end, JournalEnd::Clean);
    assert_eq!(read.valid_len, 0);
}

#[test]
fn a_journal_cut_at_every_offset_reads_up_to_the_last_whole_record() {
    let records = three_records();
    let bytes = journal_bytes(&records);
    let mut boundaries = vec![0];
    for record in &records {
        boundaries.push(boundaries.last().unwrap() + encode_record(record).unwrap().len());
    }
    for cut in 0..=bytes.len() {
        let read = read_journal(&bytes[..cut]);
        let whole = boundaries.iter().filter(|b| **b <= cut && **b > 0).count();
        let last_boundary = *boundaries.iter().rfind(|b| **b <= cut).unwrap();
        assert_eq!(read.records, records[..whole], "cut at {cut}");
        assert_eq!(read.valid_len, last_boundary, "cut at {cut}");
        if cut == last_boundary {
            assert_eq!(read.end, JournalEnd::Clean, "cut at {cut}");
        } else {
            assert_eq!(
                read.end,
                JournalEnd::Cut {
                    offset: last_boundary
                },
                "cut at {cut}"
            );
        }
    }
}

#[test]
fn a_cut_journal_reads_everything_once_the_rest_arrives() {
    let records = three_records();
    let bytes = journal_bytes(&records);
    for cut in 0..bytes.len() {
        let mut arrived = bytes[..cut].to_vec();
        assert!(read_journal(&arrived).records.len() < records.len());
        arrived.extend_from_slice(&bytes[cut..]);
        assert_eq!(read_journal(&arrived).records, records, "cut at {cut}");
    }
}

#[test]
fn a_direction_byte_that_is_neither_in_nor_out_stops_the_read_with_the_offset() {
    let records = three_records();
    let mut bytes = journal_bytes(&records);
    let second = encode_record(&records[0]).unwrap().len();
    bytes[second + 16] = 7;
    let read = read_journal(&bytes);
    assert_eq!(read.records, records[..1]);
    assert_eq!(read.valid_len, second);
    assert_eq!(
        read.end,
        JournalEnd::Damaged {
            error: JournalError::BadDirection {
                offset: second,
                found: 7
            }
        }
    );
    let text = JournalError::BadDirection {
        offset: second,
        found: 7,
    }
    .to_string();
    assert!(
        text.contains(&second.to_string()) && text.contains('7'),
        "{text}"
    );
}

#[test]
fn a_record_length_over_the_cap_stops_the_read_instead_of_waiting_for_more() {
    let mut bytes = encode_record(&record(0, Direction::In, b"x")).unwrap();
    let len = (MAX_RECORD_BYTES as u32 + 1).to_be_bytes();
    bytes[17..21].copy_from_slice(&len);
    let read = read_journal(&bytes);
    assert!(read.records.is_empty());
    assert_eq!(
        read.end,
        JournalEnd::Damaged {
            error: JournalError::RecordTooLarge {
                offset: 0,
                len: MAX_RECORD_BYTES + 1,
                max: MAX_RECORD_BYTES
            }
        }
    );
}

#[test]
fn a_seq_that_does_not_follow_the_last_stops_the_read_with_expected_and_found() {
    let first = record(0, Direction::In, b"a");
    let skipped = record(2, Direction::Out, b"b");
    let bytes = journal_bytes(&[first.clone(), skipped]);
    let offset = encode_record(&first).unwrap().len();
    let read = read_journal(&bytes);
    assert_eq!(read.records, vec![first]);
    let error = JournalError::OutOfOrder {
        offset,
        expected: 1,
        found: 2,
    };
    assert_eq!(
        read.end,
        JournalEnd::Damaged {
            error: error.clone()
        }
    );
    let text = error.to_string();
    assert!(
        text.contains("expected seq 1") && text.contains("found 2"),
        "{text}"
    );

    let starts_late = journal_bytes(&[record(5, Direction::In, b"a")]);
    assert!(read_journal(&starts_late).records.is_empty());
}

#[test]
fn a_record_over_the_cap_is_not_encoded() {
    let big = record(0, Direction::In, &vec![0u8; MAX_RECORD_BYTES + 1]);
    let error = encode_record(&big).unwrap_err();
    assert!(error.to_string().contains(&MAX_RECORD_BYTES.to_string()));
}

#[test]
fn the_journal_reader_never_panics_on_garbage() {
    for seed in 0..300 {
        for len in [0, 1, 20, 21, 22, 40, 200] {
            let _ = read_journal(&noise(seed, len));
        }
    }
}

// The handshake.

#[test]
fn the_shim_protocol_starts_at_version_one() {
    assert_eq!(SHIM_PROTOCOL, 1);
}

#[test]
fn the_previous_version_is_one_below_and_never_zero() {
    assert_eq!(previous_version(3), Some(2));
    assert_eq!(previous_version(2), Some(1));
    assert_eq!(previous_version(1), None);
}

#[test]
fn a_daemon_at_version_three_accepts_three_and_two() {
    assert_eq!(check_shim_version(3, 3), Ok(VersionMatch::Current));
    assert_eq!(check_shim_version(3, 2), Ok(VersionMatch::Previous));
}

#[test]
fn an_older_version_is_refused_with_a_message_naming_the_shim_the_current_and_the_previous() {
    let error = check_shim_version(3, 1).unwrap_err();
    let text = error.to_string();
    for number in ["1", "3", "2"] {
        assert!(text.contains(number), "{text}");
    }
    assert!(text.contains("older"), "{text}");
    assert!(
        text.contains("current") && text.contains("previous"),
        "{text}"
    );
}

#[test]
fn a_newer_version_is_refused_too_and_the_message_says_newer() {
    let error = check_shim_version(3, 4).unwrap_err();
    let text = error.to_string();
    assert!(text.contains("newer"), "{text}");
    assert!(
        text.contains('4') && text.contains('3') && text.contains('2'),
        "{text}"
    );
}

#[test]
fn at_version_one_there_is_no_previous_version_and_zero_is_refused() {
    assert_eq!(check_shim_version(1, 1), Ok(VersionMatch::Current));
    let text = check_shim_version(1, 0).unwrap_err().to_string();
    assert!(text.contains('0') && text.contains('1'), "{text}");
    assert!(text.contains("no previous"), "{text}");
}

#[test]
fn this_build_accepts_its_own_shim_and_refuses_the_next_one() {
    assert_eq!(accept_shim(SHIM_PROTOCOL), Ok(VersionMatch::Current));
    assert!(accept_shim(SHIM_PROTOCOL + 1).is_err());
}

#[test]
fn the_daemon_answers_a_hello_with_welcome_or_a_refusal_it_can_send() {
    assert_eq!(
        answer_hello(3, 2),
        DaemonMessage::Welcome { protocol: 2 },
        "it speaks the shim's version to a shim of the previous version"
    );
    assert_eq!(answer_hello(3, 3), DaemonMessage::Welcome { protocol: 3 });
    match answer_hello(3, 1) {
        DaemonMessage::Refused { message } => {
            assert!(message.contains('1') && message.contains('3') && message.contains('2'));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_hello_and_its_answer_travel_as_frames() {
    let hello = encode_message(&ShimMessage::Hello { protocol: 2 }).unwrap();
    let (ShimMessage::Hello { protocol }, _) = decode_message(&hello).unwrap().unwrap() else {
        panic!("not a hello");
    };
    let answer = encode_message(&answer_hello(3, protocol)).unwrap();
    assert_eq!(
        decode_message::<DaemonMessage>(&answer).unwrap().unwrap().0,
        DaemonMessage::Welcome { protocol: 2 }
    );
}

// A damaged exit.json.

#[test]
fn an_empty_exit_json_is_a_typed_error() {
    assert_eq!(parse_exit_record(b""), Err(ExitRecordError::Empty));
    assert_eq!(parse_exit_record(b" \n\t"), Err(ExitRecordError::Empty));
}

#[test]
fn an_exit_json_cut_at_every_offset_is_a_typed_error_never_a_record() {
    let bytes = exit_record_bytes(&exit_record());
    let whole = bytes.len() - 1; // the final newline is not part of the JSON value
    for cut in 0..whole {
        let error = parse_exit_record(&bytes[..cut])
            .expect_err(&format!("a record cut at {cut} was read as whole"));
        assert!(
            matches!(error, ExitRecordError::Empty | ExitRecordError::Cut { .. }),
            "cut at {cut}: {error:?}"
        );
    }
    assert!(parse_exit_record(&bytes[..whole]).is_ok());
}

#[test]
fn an_exit_json_of_the_wrong_shape_is_invalid_with_the_position_and_reason() {
    let error = parse_exit_record(b"{\"protocol\":1}").unwrap_err();
    let ExitRecordError::Invalid { reason, .. } = &error else {
        panic!("expected invalid, got {error:?}");
    };
    assert!(reason.contains("missing field"), "{reason}");
    assert!(error.to_string().contains("exit.json"), "{error}");

    let error = parse_exit_record(b"not json at all").unwrap_err();
    assert!(
        matches!(
            error,
            ExitRecordError::Invalid {
                line: 1,
                column: 2,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(parse_exit_record(&[0xff, 0xfe, 0x00]).is_err());
    assert!(parse_exit_record(b"[1,2,3]").is_err());
    let negative = br#"{"protocol":1,"end":{"type":"exited","code":0},"cpu_user_us":-1,"cpu_system_us":0,"max_rss_bytes":0}"#;
    assert!(matches!(
        parse_exit_record(negative),
        Err(ExitRecordError::Invalid { .. })
    ));
}

#[test]
fn an_exit_json_with_an_unaccepted_protocol_names_it_and_the_accepted_ones() {
    let record = ExitRecord {
        protocol: 9,
        ..exit_record()
    };
    let error = parse_exit_record(&exit_record_bytes(&record)).unwrap_err();
    assert!(
        matches!(error, ExitRecordError::Protocol { .. }),
        "{error:?}"
    );
    let text = error.to_string();
    assert!(text.contains('9') && text.contains('1'), "{text}");
}

#[test]
fn an_exit_json_of_the_previous_protocol_is_read() {
    let previous = ExitRecord {
        protocol: 2,
        ..exit_record()
    };
    assert_eq!(
        parse_exit_record_for(3, &exit_record_bytes(&previous)).unwrap(),
        previous
    );
    let older = ExitRecord {
        protocol: 1,
        ..exit_record()
    };
    assert!(parse_exit_record_for(3, &exit_record_bytes(&older)).is_err());
}

#[test]
fn the_exit_json_reader_never_panics_on_garbage() {
    for seed in 0..200 {
        for len in [0, 1, 2, 10, 80, 200] {
            let _ = parse_exit_record(&noise(seed, len));
        }
    }
}

// The generated schemas.

#[test]
fn the_registry_has_the_shim_types_and_each_object_is_closed() {
    let files = schema::registry().render();
    for name in ["DaemonMessage", "ExitRecord", "ShimMessage"] {
        for side in ["input", "output"] {
            let file = format!("{name}.{side}.json");
            assert!(files.contains_key(&file), "{file}");
            let schema: Value = serde_json::from_str(&files[&file]).unwrap();
            assert!(!schema.to_string().contains("\"additionalProperties\":true"));
        }
    }
}

#[test]
fn an_empty_shim_variant_is_an_empty_object_in_the_schema() {
    let schema = schema_file("DaemonMessage.input.json");
    let ack = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["properties"]["type"]["const"] == "ack_exit")
        .unwrap();
    assert_eq!(ack["additionalProperties"], json!(false));
    assert_eq!(ack["properties"].as_object().unwrap().len(), 1);
}

#[test]
fn the_exit_json_schema_requires_every_field() {
    let schema = schema_file("ExitRecord.input.json");
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    for name in [
        "protocol",
        "end",
        "cpu_user_us",
        "cpu_system_us",
        "max_rss_bytes",
    ] {
        assert!(required.contains(&name), "{name} is optional in the schema");
    }
}

#[test]
fn a_journal_read_from_a_boundary_numbers_its_first_record_from_the_seq_it_is_given() {
    let records = three_records();
    let bytes = journal_bytes(&records);
    let first = journal_bytes(&records[..1]).len();
    let later = read_journal_from(&bytes[first..], 1);
    assert_eq!(later.records, records[1..]);
    assert_eq!(later.valid_len, bytes.len() - first);
    assert_eq!(later.end, JournalEnd::Clean);
}

#[test]
fn a_journal_read_from_a_boundary_refuses_a_first_record_with_another_number() {
    let records = three_records();
    let bytes = journal_bytes(&records);
    let first = journal_bytes(&records[..1]).len();
    let read = read_journal_from(&bytes[first..], 0);
    assert!(read.records.is_empty());
    assert!(matches!(read.end, JournalEnd::Damaged { .. }));
}
