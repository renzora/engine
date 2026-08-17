//! Framing, identity and path-safety tests.
//!
//! These are the parts of the feature that can be checked without a GPU or a
//! second editor, and they are also the parts where a bug is expensive: a
//! framing error desynchronises a link permanently, an id collision aliases two
//! people's entities, and a path escape lets one machine write anywhere on
//! another's disk.

use renzora_collab::identity::CollabIds;
use renzora_collab::protocol::{read_frame, write_frame, CamPose, CollabMsg, FileEntry};

/// Everything on the wire survives a round trip, including the large-payload
/// variants that the length prefix exists for.
#[test]
fn frames_round_trip() {
    let messages = vec![
        CollabMsg::Hello {
            protocol: 1,
            display_name: "ada".into(),
            project: "mygame".into(),
        },
        CollabMsg::Presence {
            peer: 3,
            camera: Some(CamPose {
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                fov: 1.2,
            }),
            selection: vec![7, 9],
        },
        CollabMsg::EntityUpsert {
            bsn: "(entities: {})".into(),
            ids: vec![(42, 1), (43, 2)],
            removed: vec![(1, vec!["bevy_pbr::light::PointLight".into()])],
        },
        CollabMsg::FileChunk {
            path: "models/tree.glb".into(),
            offset: 4096,
            bytes: vec![0xAB; 10_000],
            last: false,
        },
        CollabMsg::Control { allowed: true },
    ];

    let mut buffer: Vec<u8> = Vec::new();
    for msg in &messages {
        write_frame(&mut buffer, msg).expect("write");
    }

    // Read back from one contiguous stream — the point of the length prefix is
    // that several messages in one buffer are still separable.
    let mut cursor = std::io::Cursor::new(buffer);
    for expected in &messages {
        let got = read_frame(&mut cursor).expect("read");
        assert_eq!(got.label(), expected.label());
        match (got, expected) {
            (CollabMsg::EntityUpsert { ids, removed, .. }, CollabMsg::EntityUpsert { .. }) => {
                assert_eq!(ids, vec![(42, 1), (43, 2)]);
                assert_eq!(removed[0].1[0], "bevy_pbr::light::PointLight");
            }
            (CollabMsg::FileChunk { bytes, offset, last, .. }, CollabMsg::FileChunk { .. }) => {
                assert_eq!(bytes.len(), 10_000);
                assert_eq!(offset, 4096);
                assert!(!last);
            }
            (CollabMsg::Presence { selection, camera, .. }, CollabMsg::Presence { .. }) => {
                assert_eq!(selection, vec![7, 9]);
                assert_eq!(camera.unwrap().translation, [1.0, 2.0, 3.0]);
            }
            _ => {}
        }
    }
}

/// A truncated stream must fail rather than block or return a half-message.
#[test]
fn truncated_frame_is_an_error() {
    let mut buffer: Vec<u8> = Vec::new();
    write_frame(&mut buffer, &CollabMsg::Ping).expect("write");
    buffer.pop();
    let mut cursor = std::io::Cursor::new(buffer);
    assert!(read_frame(&mut cursor).is_err());
}

/// An absurd length prefix is refused *before* the buffer behind it is
/// allocated. Without this a peer could make the reader reserve gigabytes on
/// nothing more than its own say-so.
#[test]
fn oversized_frame_is_refused() {
    // Well-formed magic, absurd length — so this exercises the length guard
    // rather than stopping at the desync check in front of it.
    let mut buffer = renzora_collab::protocol::FRAME_MAGIC.to_vec();
    buffer.extend_from_slice(&u32::MAX.to_le_bytes());
    buffer.extend_from_slice(b"not really this long");
    let mut cursor = std::io::Cursor::new(buffer);
    let error = read_frame(&mut cursor).expect_err("should refuse");
    assert!(error.to_string().contains("announced"), "got: {error}");
}

/// Two peers minting freely must never produce the same id — this is what makes
/// a guest able to spawn an entity without asking the host for a name first.
#[test]
fn peers_cannot_mint_colliding_ids() {
    let mut host = CollabIds::default();
    let mut guest = CollabIds::default();
    host.begin(0);
    guest.begin(1);

    let host_ids: Vec<u64> = (0..1000).map(|_| host.mint().0).collect();
    let guest_ids: Vec<u64> = (0..1000).map(|_| guest.mint().0).collect();

    for id in &host_ids {
        assert!(!guest_ids.contains(id), "id {id} was minted by both peers");
    }
    // And each id says who minted it, which is what the panel uses to attribute
    // an entity to a collaborator.
    assert!(host_ids.iter().all(|&id| renzora_collab::identity::CollabId(id).slot() == 0));
    assert!(guest_ids.iter().all(|&id| renzora_collab::identity::CollabId(id).slot() == 1));
}

#[test]
fn ids_are_unique_within_a_peer() {
    let mut ids = CollabIds::default();
    ids.begin(4);
    let minted: std::collections::HashSet<u64> = (0..10_000).map(|_| ids.mint().0).collect();
    assert_eq!(minted.len(), 10_000);
}

/// The manifest carries paths chosen by the far side, so the join must reject
/// anything that leaves the project. Each of these has been a real-world path
/// traversal.
#[test]
fn unsafe_paths_are_refused() {
    let root = std::path::Path::new("/projects/mygame");
    for hostile in [
        "../secrets.txt",
        "../../.ssh/authorized_keys",
        "models/../../etc/passwd",
        "/etc/passwd",
        "",
    ] {
        assert!(
            renzora_collab::files::safe_join(root, hostile).is_none(),
            "{hostile} should have been refused"
        );
    }
}

#[test]
fn ordinary_paths_are_accepted() {
    let root = std::path::Path::new("/projects/mygame");
    let joined = renzora_collab::files::safe_join(root, "models/tree.glb")
        .expect("a normal relative path is fine");
    assert!(joined.ends_with("models/tree.glb"));
}

/// A file entry is compared by content, so an identical file on both sides is
/// never transferred twice.
#[test]
fn file_entries_compare_by_content() {
    let a = FileEntry { path: "a.png".into(), size: 10, hash: 7 };
    let b = FileEntry { path: "a.png".into(), size: 10, hash: 7 };
    let c = FileEntry { path: "a.png".into(), size: 10, hash: 8 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

/// Regression: a desynchronised stream must say so at the first bad byte.
///
/// The original transport set a read timeout so its reader could poll a
/// shutdown flag between frames. A timeout landing mid-frame had already
/// consumed part of it — `read_exact` does not report how much — so the retry
/// read a length prefix out of the middle of a payload. In a live session that
/// surfaced as `peer announced a 1560347651-byte frame`: a fault on the reading
/// side, reported as the sending side's fault, with a number that meant nothing.
///
/// The timeout is gone (the reader blocks and is interrupted by closing the
/// socket), and the frame magic makes the same corruption legible if anything
/// ever reintroduces it.
#[test]
fn desynchronised_stream_names_itself() {
    let mut buffer: Vec<u8> = Vec::new();
    write_frame(&mut buffer, &CollabMsg::Ping).expect("write");

    // Resume from one byte in — exactly what dropping part of a frame does.
    let mut cursor = std::io::Cursor::new(buffer[1..].to_vec());
    let error = read_frame(&mut cursor).expect_err("misaligned read should fail");
    assert!(
        error.to_string().contains("desynchronised"),
        "expected a desync diagnosis, got: {error}"
    );
}

/// The magic is checked before the length, so corruption cannot reach the
/// allocation at all.
#[test]
fn bad_magic_is_refused_before_allocating() {
    let mut buffer = b"XXXX".to_vec();
    buffer.extend_from_slice(&u32::MAX.to_le_bytes());
    let mut cursor = std::io::Cursor::new(buffer);
    let error = read_frame(&mut cursor).expect_err("should refuse");
    assert!(error.to_string().contains("desynchronised"));
}
