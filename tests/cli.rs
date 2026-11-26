use std::{fs, process::Command};
#[test]
fn json_contract_and_permissions() {
    let dir = std::env::temp_dir().join(format!("badgeify-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    fs::write(&path, r#"{"version":1,"items":[{"id":"com.apple.finder","name":"Finder","enabled":true,"type":"app","path":"/System/Library/CoreServices/Finder.app"}]}"#).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_pipmark"))
            .env("BADGEIFY_CONFIG", &path)
            .args(args)
            .output()
            .unwrap()
    };
    let list = run(&["list"]);
    assert!(list.status.success());
    assert!(list.stderr.is_empty());
    let data: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(data["data"][0]["id"], "com.apple.finder");
    let missing = run(&["status", "--id", "unknown"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&missing.stderr).unwrap();
    assert_eq!(error["error"]["code"], "NOT_FOUND");
    let status = run(&["status", "--id", "com.apple.finder"]);
    if status.status.success() {
        let data: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
        assert_eq!(data["data"]["id"], "com.apple.finder");
        assert!(data["data"]["unread"].is_u64());
    } else {
        assert!(status.stdout.is_empty());
        let data: serde_json::Value = serde_json::from_slice(&status.stderr).unwrap();
        assert_eq!(
            data["error"]["code"], "NO_PERMISSIONS",
            "Unexpected live Dock failure: {data}"
        );
    }
    fs::remove_dir_all(dir).unwrap();
}
