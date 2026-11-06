//! Human-facing activity labels. Raw transport IDs are never display fallbacks.
use crate::look::TimelineItem;
use serde_json::Value;
use std::collections::BTreeMap;

pub fn uptime_label(seconds:u64)->String {
    if seconds<3600{format!("server up · {} min",seconds/60)}else{format!("server up · {} h",seconds/3600)}
}
pub fn pane_label(tab:&str,index:usize)->String{format!("{tab} · pane {}",index+1)}
/// Each entry: age, the item, and for a typed command its (tab, command) so the window
/// can add the exit badge it knows from the pane's command cards.
pub fn human_recent(records:&Value,labels:&BTreeMap<String,String>,agents:&Value)->Vec<(u64,TimelineItem,Option<(String,String)>)>{
    let mut out=Vec::new();
    for record in records.as_array().into_iter().flatten(){
        let mut tab=record["tab_id"].as_str().unwrap_or("");
        let mut agent_name=record["client_name"].as_str().filter(|name|!name.is_empty()).map(|name|
            record["model"].as_str().filter(|model|!model.is_empty()).map(|model|format!("{name} · {model}")).unwrap_or_else(||name.into()));
        if let Some(agent)=agents.as_array().into_iter().flatten().find(|agent|agent["connection_id"]==record["connection_id"] && !record["connection_id"].is_null()) {
            if tab.is_empty(){tab=agent["tab_id"].as_str().unwrap_or("");}
            if let Some(name)=agent["client_name"].as_str().filter(|name|!name.is_empty()){
                agent_name=Some(agent["model"].as_str().filter(|model|!model.is_empty()).map(|model|format!("{name} · {model}")).unwrap_or_else(||name.into()));
            }
        }
        let pane=labels.get(tab).map(String::as_str).unwrap_or(if tab.is_empty(){"No pane assigned"}else{"Closed pane"});
        let (label,detail)=match record["kind"].as_str(){
            // The command itself is the title (calm.html: "Running gate-m5.sh"), the pane its detail.
            Some("command_run")=>{
                let command=record["program"].as_str().and_then(|text|text.lines().next()).map(|line|line.chars().map(|c|if c.is_control(){' '}else{c}).collect::<String>())
                    .map(|line|line.trim().to_owned()).filter(|line|!line.is_empty());
                match command {
                    Some(command)=>{out.push((record["age_ms"].as_u64().unwrap_or(0),TimelineItem{label:command.clone(),detail:pane.into()},Some((tab.to_owned(),command))));continue;}
                    None=>("Command run",pane.into()),
                }
            }
            Some("command_finished")=>(if record["exit_code"].as_i64().is_some_and(|code|code!=0){"Command failed"}else{"Command finished"},pane.into()),
            Some("agent_connected")=>("Agent connected",agent_name.map(|name|format!("{name} · {pane}")).unwrap_or_else(||pane.into())),
            Some("pane_closed")=>("Pane closed",pane.into()),
            // One line from the server, such as a saved-history file problem.
            Some("notice")=>{
                if let Some(message)=record["message"].as_str().filter(|m|!m.is_empty()){
                    out.push((record["age_ms"].as_u64().unwrap_or(0),TimelineItem{label:message.into(),detail:"Server notice".into()},None));
                }
                continue;
            }
            _=>continue,
        };
        out.push((record["age_ms"].as_u64().unwrap_or(0),TimelineItem{label:label.into(),detail},None));
    }
    out
}

#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::json;
    #[test]
    fn human_activity_labels(){
        assert_eq!(uptime_label(720),"server up · 12 min");
        assert_eq!(uptime_label(3600),"server up · 1 h");
        let labels=BTreeMap::from([("t1".into(),pane_label("marcm",0)),("t2".into(),pane_label("marcm",1))]);
        assert_eq!(labels["t2"],"marcm · pane 2");
        let events=json!([
            {"kind":"output_appended","tab_id":"t2","command_id":"c2"},
            {"kind":"command_run","tab_id":"t2","program":"sh","age_ms":0},
            {"kind":"agent_connected","connection_id":7,"tab_id":"t1","client_name":"worker","age_ms":1}
        ]);
        let shown=human_recent(&events,&labels,&json!([]));
        assert_eq!(shown.len(),2);
        assert_eq!(shown[0].1.label,"sh");
        assert_eq!(shown[0].1.detail,"marcm · pane 2");
        assert_eq!(shown[0].2,Some(("t2".to_owned(),"sh".to_owned())));
        assert_eq!(shown[1].1.detail,"worker · marcm · pane 1");
    }
}
