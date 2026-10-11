use std::io::Write;

use seula::error::FileError;
use seula::utils::decompress_gzip_file;

/// The first bytes of `4-DUNE.als`, a pre-8.2 Live set. It used to fail with "invalid
/// gzip header", which read like a corrupt file.
#[test]
fn test_pre_8_2_set_is_unsupported_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.als");
    std::fs::write(
        &path,
        [0xAB, 0x1E, 0x56, 0x78, 0x03, 0x7C, 0x00, 0x00, 0x00, 0x00],
    )
    .unwrap();

    match decompress_gzip_file(&path) {
        Err(FileError::UnsupportedVersion { path: p }) => assert_eq!(p, path),
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

/// The first bytes of a real `._FBM Session.als` found beside a project (4096 bytes, macOS
/// Finder metadata). It used to fail with "invalid gzip header".
#[test]
fn test_apple_double_sidecar_has_its_own_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("._session.als");
    std::fs::write(
        &path,
        [
            0x00, 0x05, 0x16, 0x07, 0x00, 0x02, 0x00, 0x00, b'M', b'a', b'c', b' ', b'O', b'S',
            b' ', b'X',
        ],
    )
    .unwrap();

    match decompress_gzip_file(&path) {
        Err(FileError::AppleDoubleSidecar { path: p }) => assert_eq!(p, path),
        other => panic!("expected AppleDoubleSidecar, got {other:?}"),
    }
}

/// The sidecar is recognised by its bytes, not its name: a real project that happens to be
/// called `._something.als` must still load.
#[test]
fn test_gzip_set_named_like_a_sidecar_still_decompresses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("._real project.als");
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(b"<Ableton/>").unwrap();
    std::fs::write(&path, enc.finish().unwrap()).unwrap();

    assert_eq!(decompress_gzip_file(&path).unwrap(), b"<Ableton/>");
}

/// Anything else that is not gzip keeps the ordinary error.
#[test]
fn test_other_non_gzip_still_a_decompression_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("junk.als");
    std::fs::write(&path, b"not gzip at all").unwrap();

    assert!(matches!(
        decompress_gzip_file(&path),
        Err(FileError::GzipDecompressionError { .. })
    ));
}

#[test]
fn test_gzip_still_decompresses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ok.als");
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(b"<Ableton/>").unwrap();
    std::fs::write(&path, enc.finish().unwrap()).unwrap();

    assert_eq!(decompress_gzip_file(&path).unwrap(), b"<Ableton/>");
}
