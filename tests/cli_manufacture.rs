use tempfile::tempdir;

use helpers::clitests;

mod helpers;

#[tokio::test]
async fn cli_manufacture_item_tech1() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-item-tech1").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_manufacture_item_tech1_json() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-item-tech1-json").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_manufacture_item_tech2() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-item-tech2").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_manufacture_item_tech2_json() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-item-tech2-json").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_manufacture_item_all() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-all").run();

    server.reset().await;
}

#[tokio::test]
async fn cli_manufacture_item_all_json() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "manufacture-all-json").run();

    server.reset().await;
}
