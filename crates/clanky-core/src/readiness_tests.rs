use super::*;

#[test]
fn readiness_event_wake() {
    struct Session(Engine);
    impl Drop for Session{fn drop(&mut self){self.0.shutdown();}}
    let session=Session(Engine::new());let engine=&session.0;
    let tab=engine.request("open_tab",&json!({"rows":8,"cols":80,"allow_agent":true}),true).unwrap()["tab_id"].as_str().unwrap().to_owned();
    // Worked out before execution: echo is disabled; the only bytes printed
    // are "ready" and "got:hello". The shell blocks on a real slave read.
    let command=engine.request("run",&json!({"tab_id":tab,"argv":["sh","-c","stty -echo; printf ready; IFS= read -r line; printf 'got:%s' \"$line\""],"cwd":"/"}),true).unwrap()["command_id"].as_str().unwrap().to_owned();
    let waiting=engine.agent_request("wait_input",&json!({"tab_id":tab,"timeout_ms":2000}),true).unwrap();
    assert_eq!(waiting["waiting"],true);
    assert_eq!(waiting["evidence"],"pty_read");
    let events=engine.agent_request("events",&json!({"tab_id":tab,"after":0,"limit":128,"timeout_ms":0}),true).unwrap();
    assert!(events["events"].as_array().unwrap().iter().any(|event|event["kind"]=="waiting_for_input" && event["waiting"]==true));
    engine.input(&tab,b"hello\r").unwrap();
    let done=engine.request("wait",&json!({"tab_id":tab,"command_id":command,"timeout_ms":2000}),true).unwrap();
    assert_eq!(done["done"],true);
    assert_eq!(done["exit_code"],0);
    let output=engine.request("get_output",&json!({"tab_id":tab,"command_id":command,"offset":0,"limit":1024,"encoding":"utf8"}),true).unwrap();
    assert_eq!(output["data"],"readygot:hello");
}
