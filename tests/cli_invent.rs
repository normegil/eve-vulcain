use tempfile::tempdir;

use helpers::clitests;

mod helpers;

#[tokio::test]
async fn cli_invent_item() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "invent-item").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_invent_item_json() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "invent-item-json").run();

    server.reset().await;
}
