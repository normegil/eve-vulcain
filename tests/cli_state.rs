use tempfile::tempdir;

use helpers::clitests;

mod helpers;

#[tokio::test]
async fn cli_state() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "state").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_state_json() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "state-json").run();

    server.reset().await;
}
