use std::process::Command;
use std::time::Duration;

fn main() {
    if std::env::args().nth(1).as_deref()==Some("hold") {
        let marker=std::path::PathBuf::from(std::env::args_os().nth(2).expect("test marker"));
        let deadline=std::time::Instant::now()+Duration::from_secs(12);
        while marker.exists()&&std::time::Instant::now()<deadline {std::thread::sleep(Duration::from_millis(20));}
        std::fs::write(marker.with_extension("done"),"done").expect("test completion marker");return;
    }
    let executable=std::env::current_exe().expect("test executable");
    let marker=std::env::args_os().nth(1).expect("test marker path");
    let _descendant=Command::new(executable).arg("hold").arg(marker).spawn().expect("test descendant");
    println!("{{\"result\":{{\"ok\":true}}}}");
}
