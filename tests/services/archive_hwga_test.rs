#[test]
fn archive_uri_parse_count_stays_bounded() {
    let uri = "/archive://%2Ftmp%2Fbig.zip/src/main.rs";
    flux::hwga::reset_all();
    for _ in 0..100 {
        let _ = flux::services::archive::parse_archive_uri(uri);
    }
    assert!(flux::hwga::count("flux::services::archive::parse_archive_uri") == 100);
}
