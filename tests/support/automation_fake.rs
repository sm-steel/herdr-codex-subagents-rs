use std::{env, fs, io::Write};
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(env::var_os("HERDR_SOCKET_PATH").unwrap());
    let value = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
                (Some("agent"), Some("list")) => {
            let pane = fs::read_to_string(dir.join("source-pane")).unwrap_or_else(|_| "w1:p1".to_owned());
            format!(r#"{{"result":{{"agents":[{{"agent":"codex","pane_id":"{pane}","terminal_id":"source-1"}}]}}}}"#)
        },
                (Some("agent"), Some("get")) => {
            if dir.join("undetected").exists() {
                fs::remove_file(dir.join("undetected")).unwrap();
                println!(r#"{{"result":{{"agent":{{"agent":null}}}}}}"#);
                return;
            }
            if let Ok(id) = fs::read_to_string(dir.join("reported")) {
                format!(r#"{{"result":{{"agent":{{"agent":"codex","terminal_id":"source-1","agent_session":{{"value":"{id}"}}}}}}}}"#)
            } else { r#"{"result":{"agent":{"agent":"codex","terminal_id":"source-1"}}}"#.to_owned() }
        },
        (Some("pane"), Some("report-agent-session")) => {
            let id = args.windows(2).find(|a| a[0] == "--agent-session-id").unwrap()[1].clone();
            fs::write(dir.join("reported"), id).unwrap();
            String::new()
        },
        (Some("pane"), Some("process-info")) if dir.join("fresh").exists() => r#"{"result":{"process_info":{"foreground_processes":[{"argv":["/bin/codex"]}]}}}"#.to_owned(),
        (Some("pane"), Some("process-info")) => r#"{"result":{"process_info":{"foreground_processes":[{"argv":["/bin/codex","resume","11111111-1111-4111-8111-111111111111"]}]}}}"#.to_owned(),
        (Some("pane"), Some("get")) if dir.join("transient-get-error").exists() => {
            eprintln!(r#"{{"error":{{"code":"temporarily_unavailable"}}}}"#);
            std::process::exit(1);
        },
        (Some("pane"), Some("get")) => format!(r#"{{"result":{{"pane":{{"terminal_id":"{}"}}}}}}"#, fs::read_to_string(dir.join("panel-terminal")).unwrap()),
        (Some("plugin"), Some("pane")) => {
            fs::write(dir.join("panel-terminal"), "monitor-1").unwrap();
            writeln!(fs::OpenOptions::new().create(true).append(true).open(dir.join("opens")).unwrap(), "{}", args.join(" ")).unwrap();
            if dir.join("bad-open-reply").exists() { println!("bad response"); return; }
            r#"{"result":{"plugin_pane":{"plugin_id":"sm-steel.codex-subagents","entrypoint":"monitor","pane":{"pane_id":"w1:p2","terminal_id":"monitor-1"}}}}"#.to_owned()
        }
        _ => std::process::exit(1),
    };
    println!("{value}");
}
