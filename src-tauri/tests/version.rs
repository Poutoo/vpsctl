//! La version de l'application est écrite dans trois fichiers : ils doivent
//! rester identiques (le workflow de publication compare aussi le tag).

use std::path::Path;

fn json_version(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    value["version"].as_str().unwrap().to_string()
}

#[test]
fn version_is_the_same_in_cargo_tauri_and_npm() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = env!("CARGO_PKG_VERSION");
    assert_eq!(json_version(&dir.join("tauri.conf.json")), cargo);
    assert_eq!(json_version(&dir.join("../package.json")), cargo);
}
