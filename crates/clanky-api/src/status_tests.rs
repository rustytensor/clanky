//! Real socket lifecycle and owner exclusion, with independently known counts.
use super::*;

#[test]
fn look_state_tracks_real_connections() {
    const CHILD_SOCKET: &str = "CLANKY_LOOK_STATE_TEST_SOCKET";
    const TEST_TOKEN: &str = "look-state-test-owner-capability";
    if let Some(socket) = std::env::var_os(CHILD_SOCKET) {
        // A separate server process is essential: same-process clients are not
        // owner-eligible. The parent client is outside this server's ancestry.
        serve_owned(Engine::new(), Path::new(&socket), Arc::new(AtomicBool::new(false)),
            TEST_TOKEN.into()).unwrap();
        return;
    }

    struct Server {
        child: std::process::Child,
        directory: std::path::PathBuf,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir(&self.directory);
        }
    }
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap().as_nanos();
    let directory = std::env::temp_dir().join(format!("clanky-look-{}-{nonce}", std::process::id()));
    let socket = directory.join("api.sock");
    // LocalListener creates the private parent directory with the platform's
    // security rules. No token file or inherited owner credential is used.
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "status_tests::look_state_tracks_real_connections"])
        .env(CHILD_SOCKET, &socket)
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn().unwrap();
    let mut server = Server { child, directory };
    let deadline = Instant::now() + Duration::from_secs(10);
    let owner = loop {
        if let Ok(client) = Client::connect_agent(&socket) { break client; }
        assert!(server.child.try_wait().unwrap().is_none(), "test server exited");
        assert!(Instant::now() < deadline, "test server did not accept connections");
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(owner.exchange(3, "auth", &json!({"token":TEST_TOKEN})).unwrap()["authenticated"], true);
    // Authenticated owners are absent from the connected-agent registry.
    let state = owner.exchange(3, "look_state", &json!({})).unwrap();
    assert_eq!(state["agents"].as_array().unwrap().len(), 0);

    let agent = Client::connect_agent(&socket).unwrap();
    agent.exchange(1, "hello", &json!({})).unwrap();
    let state = owner.exchange(3, "look_state", &json!({})).unwrap();
    assert_eq!(state["agents"].as_array().unwrap().len(), 1);
    assert_eq!(state["agents"][0]["peer_pid"], std::process::id());
    assert_eq!(state["agents"][0]["running"], false);
    let hello=agent.exchange(1,"hello",&json!({"client_name":"pi","model":"astra"})).unwrap();
    assert_eq!(hello,json!({"version":1,"capabilities":["tabs","commands","blocks"]}));
    let state=owner.exchange(3,"look_state",&json!({})).unwrap();
    assert_eq!(state["agents"][0]["client_name"],"pi");
    assert_eq!(state["agents"][0]["model"],"astra");
    assert_eq!(agent.exchange(1,"hello",&json!({"client_name":"bad\nname"})).unwrap_err().code,"invalid_request");
    let denied = agent.exchange(3, "look_state", &json!({})).unwrap_err();
    assert_eq!(denied.code, "permission_denied");

    drop(agent);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state = owner.exchange(3, "look_state", &json!({})).unwrap();
        if state["agents"].as_array().unwrap().is_empty() { break; }
        assert!(Instant::now() < deadline, "closed peer remained registered");
        thread::sleep(Duration::from_millis(10));
    }
    // Close the last terminal so the owned server exits through its normal path.
    let tab = owner.exchange(1, "open_tab", &json!({})).unwrap();
    owner.exchange(1, "close_tab", &json!({"tab_id":tab["tab_id"]})).unwrap();
    drop(owner);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = server.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "test server did not stop");
        thread::sleep(Duration::from_millis(10));
    }
}
