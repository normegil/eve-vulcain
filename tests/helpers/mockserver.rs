use std::{fs, path::Path};

use wiremock::{
    matchers::{any, body_string_contains, method, path, query_param},
    Mock, MockServer, Request, ResponseTemplate,
};

use super::paths::get_integration_test_root;

pub async fn create() -> MockServer {
    let server = MockServer::start().await;

    let responses_folder =
        get_integration_test_root().join("helpers/resources/mockserver_responses");

    register_api_character_handlers(&server, &responses_folder).await;
    register_api_corporation_handlers(&server, &responses_folder).await;
    register_api_industry_handlers(&server, &responses_folder).await;
    register_api_location_handlers(&server, &responses_folder).await;
    register_api_market_handlers(&server, &responses_folder).await;
    register_api_search_handlers(&server, &responses_folder).await;
    register_api_skills_handlers(&server, &responses_folder).await;
    register_api_universe_handlers(&server, &responses_folder).await;
    register_api_wallet_handlers(&server, &responses_folder).await;

    register_oauth_handlers(&server, &responses_folder).await;
    register_spec_handlers(&server, &responses_folder).await;
    register_logging_handler(&server).await;

    server
}

pub async fn register_api_character_handlers(server: &MockServer, responses_folder: &Path) {
    let character_api_responses = responses_folder.join("character");

    let character_public_info_response =
        fs::read_to_string(character_api_responses.join("public_info.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v5/characters/123456789/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(character_public_info_response))
        .mount(&server)
        .await;
}

pub async fn register_api_corporation_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("corporation");

    let corporation_info_response =
        fs::read_to_string(api_responses.join("corporation_info.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v5/corporations/1000044/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(corporation_info_response))
        .mount(&server)
        .await;
}

pub async fn register_api_industry_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("industry");

    let character_jobs_response =
        fs::read_to_string(api_responses.join("character_jobs.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/characters/123456789/industry/jobs/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(character_jobs_response))
        .mount(&server)
        .await;

    let industry_systems_response =
        fs::read_to_string(api_responses.join("industry_systems.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/industry/systems/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(industry_systems_response))
        .mount(&server)
        .await;
}

pub async fn register_api_location_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("location");

    let character_location_response =
        fs::read_to_string(api_responses.join("character_location.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v1/characters/123456789/location/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(character_location_response))
        .mount(&server)
        .await;
}

pub async fn register_api_market_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("market");

    let character_orders_response =
        fs::read_to_string(api_responses.join("character_orders.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v2/characters/123456789/orders/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(character_orders_response))
        .mount(&server)
        .await;

    let prices_response = fs::read_to_string(api_responses.join("prices.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v1/markets/prices/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(prices_response))
        .mount(&server)
        .await;

    let order_ids = vec![(2048, "sell"), (23527, "sell")];
    for (type_id, order_type) in order_ids {
        let response =
            fs::read_to_string(api_responses.join(format!("orders.{order_type}.{type_id}.json")))
                .unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/markets/10000002/orders/")))
            .and(query_param("order_type", order_type))
            .and(query_param("type_id", type_id.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .mount(&server)
            .await;

        let response =
            fs::read_to_string(api_responses.join(format!("history.{type_id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/markets/10000002/history/")))
            .and(query_param("type_id", type_id.to_string()))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .mount(&server)
            .await;
    }
}

pub async fn register_api_search_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("search");

    let drone_link_response =
        fs::read_to_string(api_responses.join("drone-link-augmentor-I.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v3/characters/123456789/search/"))
        .and(query_param("categories", "inventory_type"))
        .and(query_param("search", "Drone Link Augmentor I"))
        .and(query_param("strict", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(drone_link_response))
        .mount(&server)
        .await;

    let damage_control_response =
        fs::read_to_string(api_responses.join("damage-control-II.json")).unwrap();
    Mock::given(method("GET"))
        .and(path("/api/v3/characters/123456789/search/"))
        .and(query_param("categories", "inventory_type"))
        .and(query_param("search", "Damage Control II"))
        .and(query_param("strict", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_string(damage_control_response))
        .mount(&server)
        .await;
}

pub async fn register_api_skills_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("skills");

    let character_skills_response =
        fs::read_to_string(api_responses.join("character_skills.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/v4/characters/123456789/skills/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(character_skills_response))
        .mount(&server)
        .await;
}

pub async fn register_api_universe_handlers(server: &MockServer, responses_folder: &Path) {
    let api_responses = responses_folder.join("universe");

    let structure_ids: Vec<i64> = vec![1043367675664];
    for id in structure_ids {
        let structure_response =
            fs::read_to_string(api_responses.join(format!("structure_info.{id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/universe/structures/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(structure_response))
            .mount(&server)
            .await;
    }

    let station_ids: Vec<i64> = vec![60003760];
    for id in station_ids {
        let response =
            fs::read_to_string(api_responses.join(format!("station.{id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/universe/stations/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .mount(&server)
            .await;
    }

    let system_ids: Vec<i32> = vec![30000140, 30000142];
    for id in system_ids {
        let system_response =
            fs::read_to_string(api_responses.join(format!("system.{id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v4/universe/systems/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(system_response))
            .mount(&server)
            .await;
    }

    let constellation_ids: Vec<i64> = vec![20000020];
    for id in constellation_ids {
        let response =
            fs::read_to_string(api_responses.join(format!("constellation.{id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/universe/constellations/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .mount(&server)
            .await;
    }

    let region_ids: Vec<i64> = vec![10000002];
    for id in region_ids {
        let response = fs::read_to_string(api_responses.join(format!("region.{id}.json"))).unwrap();
        Mock::given(method("GET"))
            .and(path(format!("/api/v1/universe/regions/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(response))
            .mount(&server)
            .await;
    }

    let type_ids: Vec<i32> = vec![
        34, 35, 36, 37, 38, 39, 40, 2046, 2048, 2049, 3300, 3301, 3302, 3303, 3310, 3311, 3312,
        3315, 3316, 3317, 3318, 3319, 3320, 3321, 3327, 3328, 3330, 3334, 3340, 3342, 3355, 3356,
        3357, 3359, 3361, 3380, 3385, 3386, 3387, 3388, 3392, 3393, 3394, 3402, 3403, 3405, 3406,
        3410, 3411, 3412, 3413, 3416, 3417, 3418, 3419, 3420, 3423, 3424, 3425, 3426, 3427, 3428,
        3429, 3431, 3432, 3433, 3435, 3436, 3437, 3438, 3442, 3443, 3446, 3449, 3450, 3451, 3452,
        3453, 3454, 3455, 3551, 3559, 3689, 3828, 11395, 11399, 11442, 11446, 11453, 11475, 11529,
        11540, 11542, 11553, 11695, 12305, 12365, 13278, 16594, 16598, 17940, 20415, 20416, 20418,
        20419, 21059, 21718, 21791, 22578, 23121, 23527, 23606, 23618, 23719, 24241, 24242, 24268,
        24428, 25719, 25739, 25811, 25863, 26252, 32918, 33092, 33093, 33699, 36912, 60377,
    ];
    for id in type_ids {
        let type_response =
            match fs::read_to_string(api_responses.join(format!("types/type.{id}.json"))) {
                Ok(resp) => resp,
                Err(err) => panic!("Could not load type for id '{id}': {err}"),
            };
        Mock::given(method("GET"))
            .and(path(format!("/api/v3/universe/types/{id}/")))
            .respond_with(ResponseTemplate::new(200).set_body_string(type_response))
            .mount(&server)
            .await;
    }
}

pub async fn register_api_wallet_handlers(server: &MockServer, _responses_folder: &Path) {
    Mock::given(method("GET"))
        .and(path("/api/v1/characters/123456789/wallet/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("123456789.12"))
        .mount(&server)
        .await;
}

pub async fn register_oauth_handlers(server: &MockServer, responses_folder: &Path) {
    let refresh_access_token_response =
        fs::read_to_string(responses_folder.join("refresh_access_token.json")).unwrap();

    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=ABCDEFGHIJKLM"))
        .respond_with(ResponseTemplate::new(200).set_body_string(refresh_access_token_response))
        .mount(&server)
        .await;
}

pub async fn register_spec_handlers(server: &MockServer, responses_folder: &Path) {
    let swagger_response = fs::read_to_string(responses_folder.join("swagger.json")).unwrap();

    Mock::given(method("GET"))
        .and(path("/api/_/spec"))
        .respond_with(ResponseTemplate::new(200).set_body_string(swagger_response))
        .mount(&server)
        .await;
}

pub async fn register_logging_handler(server: &MockServer) {
    Mock::given(any())
        .respond_with(|req: &Request| {
            eprintln!("Failed request: \n{}\n", req);
            ResponseTemplate::new(404)
        })
        .mount(server)
        .await;
}
