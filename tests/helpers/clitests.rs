use std::{fs, path::Path};

use eve_vulcain::display::Display;
use trycmd::TestCases;

use super::paths::get_project_root;

pub fn init_cli_test(dir: &Path, base_uri: &str, test_name: &str) -> TestCases {
    let cfg = dir.join("eve-vulcain.toml");

    let case = trycmd::TestCases::new();

    case.case(format!("tests/cli/{test_name}/{test_name}.toml"))
        .env("EVEVULCAIN_DATA_DIR", dir.to_display())
        .env("EVEVULCAIN_CACHE_DIR", dir.to_display())
        .env("EVEVULCAIN_CONFIG", cfg.to_display())
        .env("EVEVULCAIN_BASE_API_URL", format!("{}/api/", base_uri))
        .env(
            "EVEVULCAIN_AUTHORIZE_URL",
            format!("{}/oauth/authorize", base_uri),
        )
        .env("EVEVULCAIN_TOKEN_URL", format!("{}/oauth/token", base_uri))
        .env("EVEVULCAIN_SPEC_URL", format!("{}/api/_/spec", base_uri));

    case
}

pub fn attach_test_files(dir: &Path) {
    let init_files = get_project_root().join("tests/helpers/resources/init_files");

    let refresh_token_source = init_files.join("refresh_token_data.json");
    let refresh_token_target = dir.join("refresh_token_data.json");
    fs::copy(refresh_token_source, refresh_token_target).unwrap();

    let refresh_token_source = init_files.join("sde/blueprints.yaml");
    let fsd_dir = dir.join("sde/fsd");
    fs::create_dir_all(&fsd_dir).unwrap();
    let refresh_token_target = fsd_dir.join("blueprints.yaml");
    fs::copy(refresh_token_source, refresh_token_target).unwrap();

    let facilities_source = init_files.join("facilities_data.json");
    let facilities_target = dir.join("facilities_data.json");
    fs::copy(facilities_source, facilities_target).unwrap();

    let items_source = init_files.join("items_data.json");
    let items_target = dir.join("items_data.json");
    fs::copy(items_source, items_target).unwrap();
}
