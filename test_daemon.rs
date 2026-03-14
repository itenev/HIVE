fn main() {
    let cmd = "while true; do date; sleep 2; done";
    let bg_cmd = format!("nohup {} > memory/daemons/test.log 2>&1 & echo \"---PID---$!\"", cmd);
    let child = std::process::Command::new("bash")
        .arg("-c")
        .arg(&bg_cmd)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&child.stdout).to_string();
    let stderr = String::from_utf8_lossy(&child.stderr).to_string();
    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);
}
