#![no_main]
use flux::services::network::ConnectToServerParams;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // 4 lines: protocol, host, username, path
        let mut lines = s.splitn(4, '\n');
        let protocol = lines.next().unwrap_or("").trim().to_lowercase();
        let host = lines.next().unwrap_or("");
        let username = lines.next().unwrap_or("");
        let path = lines.next().unwrap_or("");

        let params = ConnectToServerParams {
            protocol,
            host: host.to_string(),
            port: None,
            path: if path.is_empty() {
                None
            } else {
                Some(path.to_string())
            },
            username: if username.is_empty() {
                None
            } else {
                Some(username.to_string())
            },
        };

        if let Some(uri) = params.build_uri() {
            // If a URI was produced, it must at minimum have a scheme.
            assert!(
                uri.contains("://"),
                "build_uri produced a URI without a scheme: {uri:?}"
            );
        }
    }
});
