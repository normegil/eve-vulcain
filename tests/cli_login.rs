use tempfile::tempdir;

use helpers::clitests;

mod helpers;

#[tokio::test]
async fn cli_login() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    clitests::init_cli_test(&dir, &base_uri, "login").run();

    server.reset().await;
}
