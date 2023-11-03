use std::{path::PathBuf, str::FromStr};

pub fn get_project_root() -> PathBuf {
    let root_path = env!("CARGO_MANIFEST_DIR");
    PathBuf::from_str(root_path).unwrap()
}

pub fn get_integration_test_root() -> PathBuf {
    get_project_root().join("tests/")
}
