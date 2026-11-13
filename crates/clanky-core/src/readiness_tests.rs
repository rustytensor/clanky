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
    assert_eq!(engine.owner_request("terminal_info",&json!({"tab_id":tab})).unwrap()["program"],"sh");
    let events=engine.agent_request("events",&json!({"tab_id":tab,"after":0,"limit":128,"timeout_ms":0}),true).unwrap();
    assert!(events["events"].as_array().unwrap().iter().any(|event|event["kind"]=="waiting_for_input" && event["waiting"]==true));
    engine.input(&tab,b"hello\r").unwrap();
    let done=engine.request("wait",&json!({"tab_id":tab,"command_id":command,"timeout_ms":2000}),true).unwrap();
    assert_eq!(done["done"],true);
    assert_eq!(done["exit_code"],0);
    let output=engine.request("get_output",&json!({"tab_id":tab,"command_id":command,"offset":0,"limit":1024,"encoding":"utf8"}),true).unwrap();
    assert_eq!(output["data"],"readygot:hello");
    let state=engine.owner_request("look_state",&json!({})).unwrap();
    let recent=state["recent"].as_array().unwrap();
    assert!(recent.iter().any(|event|event["kind"]=="command_run" && event["program"]=="sh"));
    assert!(recent.iter().all(|event|event["kind"]!="output_appended" && event["kind"]!="waiting_for_input"));
}

#[test]
fn a_window_shell_that_exits_is_listed_for_closing() {
    struct Session(Engine);
    impl Drop for Session{fn drop(&mut self){self.0.shutdown();}}
    let session=Session(Engine::new());let engine=&session.0;
    let open=|| engine.request("open_tab",&json!({"rows":8,"cols":80,"allow_agent":true}),true).unwrap()["tab_id"].as_str().unwrap().to_owned();
    let (window,agent)=(open(),open());
    // Worked out before the run: the window's shell exits 0 and is listed; the agent's command
    // exits 3 and is not, so the agent can still read its result.
    let run=|tab:&str,code:&str,owner:bool| engine.request("run",&json!({"tab_id":tab,"argv":["sh","-c",format!("exit {code}")],"cwd":"/"}),owner).unwrap()["command_id"].as_str().unwrap().to_owned();
    for (tab,command) in [(&window,run(&window,"0",true)),(&agent,run(&agent,"3",false))] {
        assert_eq!(engine.request("wait",&json!({"tab_id":tab,"command_id":command,"timeout_ms":5000}),true).unwrap()["done"],true);
    }
    assert_eq!(engine.owner_request("finished_shells",&json!({})).unwrap()["tab_ids"],json!([window]));
    // The status Marc saw after exit: the shell's folder is read from /proc after it is gone.
    assert_eq!(engine.owner_request("terminal_info",&json!({"tab_id":window})).unwrap_err().message,"No such file or directory (os error 2)");
}
