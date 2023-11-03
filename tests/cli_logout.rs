use tempfile::tempdir;

use helpers::clitests;

mod helpers;

#[tokio::test]
async fn cli_logout() {
    let dir = tempdir().unwrap().into_path();

    let server = helpers::mockserver::create().await;
    let base_uri = server.uri();

    clitests::attach_test_files(&dir);
    let test = clitests::init_cli_test(&dir, &base_uri, "logout");

    let refresh_token_file = dir.join("refresh_token_data.json");
    assert!(refresh_token_file.exists());

    test.run();

    assert!(!refresh_token_file.exists());

    server.reset().await;
}
