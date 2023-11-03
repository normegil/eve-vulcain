use rfesi::groups::{CostIndex, Position, Skill};

use crate::{
    api::sde::{Activities, Invention, Item, Manufacturing, ProbableMultipleItems},
    model::industry::IndustryType,
};

use super::*;

pub struct MockRequesterBuilder {
    //Characters
    character_base_info: Option<CharacterBaseInfo>,
    character_location_info: Option<LocationInfo>,
    character_wallet: HashMap<i32, f64>,
    character_public_infos: HashMap<i32, CharacterPublicInfo>,
    character_market_orders: HashMap<i32, Vec<CharacterOrder>>,
    character_industrial_jobs: HashMap<i32, Vec<IndustryJob>>,
    character_skills: HashMap<i32, Skills>,

    // Blueprints
    blueprints: HashMap<i32, SDEBlueprint>,

    // Alliance & Corps
    alliances: HashMap<i32, AllianceInfo>,
    corporations: HashMap<i32, CorporationPublicInfo>,

    // Markets
    price_items: Vec<PriceItem>,
    region_orders: HashMap<i32, Vec<MarketOrder>>,
    history_items: HashMap<i32, HashMap<i32, Vec<HistoryItem>>>,

    // Universe
    industrial_systems: Vec<IndustrialSystem>,
    region: HashMap<i32, Region>,
    constellations: HashMap<i32, Constellation>,
    systems: HashMap<i32, System>,
    structures: HashMap<i64, Structure>,
    stations: HashMap<i32, Station>,
    types: HashMap<i32, Type>,
}

impl MockRequesterBuilder {
    fn new() -> Self {
        Self {
            character_base_info: None,
            character_location_info: None,
            character_wallet: HashMap::default(),
            character_public_infos: HashMap::default(),
            character_market_orders: HashMap::default(),
            character_industrial_jobs: HashMap::default(),
            character_skills: HashMap::default(),
            blueprints: HashMap::default(),
            alliances: HashMap::default(),
            corporations: HashMap::default(),
            price_items: Vec::new(),
            region_orders: HashMap::default(),
            history_items: HashMap::default(),
            industrial_systems: Vec::new(),
            region: HashMap::default(),
            constellations: HashMap::default(),
            systems: HashMap::default(),
            structures: HashMap::default(),
            stations: HashMap::default(),
            types: HashMap::default(),
        }
    }

    fn with_character_base_info(mut self, character_base_info: CharacterBaseInfo) -> Self {
        self.character_base_info = Some(character_base_info);
        self
    }

    fn with_character_location(mut self, info: LocationInfo) -> Self {
        self.character_location_info = Some(info);
        self
    }

    fn insert_character_wallet(mut self, id: i32, value: f64) -> Self {
        self.character_wallet.insert(id, value);
        self
    }

    fn insert_character_public_info(mut self, id: i32, value: CharacterPublicInfo) -> Self {
        self.character_public_infos.insert(id, value);
        self
    }

    fn insert_character_market_orders(mut self, id: i32, value: CharacterOrder) -> Self {
        if let None = self.character_market_orders.get(&id) {
            self.character_market_orders.insert(id, Vec::new());
        }
        let orders = self.character_market_orders.get_mut(&id).unwrap();
        orders.push(value);
        self
    }

    fn insert_character_industrial_jobs(mut self, id: i32, value: IndustryJob) -> Self {
        if let None = self.character_industrial_jobs.get(&id) {
            self.character_industrial_jobs.insert(id, Vec::new());
        }
        let jobs = self.character_industrial_jobs.get_mut(&id).unwrap();
        jobs.push(value);
        self
    }

    fn insert_character_skills(mut self, id: i32, value: Skills) -> Self {
        self.character_skills.insert(id, value);
        self
    }

    fn insert_blueprints(mut self, id: i32, value: SDEBlueprint) -> Self {
        self.blueprints.insert(id, value);
        self
    }

    fn insert_alliance(mut self, id: i32, value: AllianceInfo) -> Self {
        self.alliances.insert(id, value);
        self
    }

    fn insert_corporation(mut self, id: i32, value: CorporationPublicInfo) -> Self {
        self.corporations.insert(id, value);
        self
    }

    fn insert_price_item(mut self, value: PriceItem) -> Self {
        self.price_items.push(value);
        self
    }

    pub fn insert_region_order(mut self, id: i32, value: MarketOrder) -> Self {
        if let None = self.region_orders.get(&id) {
            self.region_orders.insert(id, Vec::new());
        }
        let orders = self.region_orders.get_mut(&id).unwrap();
        orders.push(value);
        self
    }

    pub fn insert_history_item(mut self, region_id: i32, item_id: i32, value: HistoryItem) -> Self {
        if let None = self.history_items.get(&region_id) {
            self.history_items.insert(region_id, HashMap::default());
        }
        let items_history = self.history_items.get_mut(&region_id).unwrap();
        if let None = items_history.get(&item_id) {
            items_history.insert(item_id, Vec::new());
        }
        let history = items_history.get_mut(&item_id).unwrap();
        history.push(value);
        self
    }

    fn insert_industrial_system(mut self, value: IndustrialSystem) -> Self {
        self.industrial_systems.push(value);
        self
    }

    fn insert_region(mut self, id: i32, value: Region) -> Self {
        self.region.insert(id, value);
        self
    }

    fn insert_constellation(mut self, id: i32, value: Constellation) -> Self {
        self.constellations.insert(id, value);
        self
    }

    fn insert_system(mut self, id: i32, value: System) -> Self {
        self.systems.insert(id, value);
        self
    }

    fn insert_structures(mut self, id: i64, value: Structure) -> Self {
        self.structures.insert(id, value);
        self
    }

    fn insert_stations(mut self, id: i32, value: Station) -> Self {
        self.stations.insert(id, value);
        self
    }

    fn insert_type(mut self, id: i32, value: Type) -> Self {
        self.types.insert(id, value);
        self
    }

    pub fn build(self) -> MockRequester {
        MockRequester {
            character_base_info: self.character_base_info,
            character_location_info: self.character_location_info,
            character_wallet: self.character_wallet,
            character_public_infos: self.character_public_infos,
            character_market_orders: self.character_market_orders,
            character_industrial_jobs: self.character_industrial_jobs,
            character_skills: self.character_skills,
            blueprints: self.blueprints,
            alliances: self.alliances,
            corporations: self.corporations,
            price_items: self.price_items,
            region_orders: self.region_orders,
            history_items: self.history_items,
            industrial_systems: self.industrial_systems,
            region: self.region,
            constellations: self.constellations,
            systems: self.systems,
            structures: self.structures,
            stations: self.stations,
            types: self.types,
        }
    }
}

impl Default for MockRequesterBuilder {
    fn default() -> Self {
        let ignored_number = 0;
        let ignored_string = "Ignored".to_string();
        MockRequester::builder()
            .with_character_base_info(CharacterBaseInfo {
                id: 1,
                name: "Test Name".to_string(),
            })
            .with_character_location(LocationInfo {
                solar_system_id: 9,
                station_id: Some(8),
                structure_id: None,
            })
            .insert_character_public_info(
                1,
                CharacterPublicInfo {
                    alliance_id: Some(3),
                    birthday: ignored_string.clone(),
                    bloodline_id: ignored_number,
                    corporation_id: 2,
                    description: None,
                    gender: ignored_string.clone(),
                    name: "Test Name".to_string(),
                    race_id: ignored_number as u16,
                    security_status: None,
                    title: None,
                },
            )
            .insert_character_wallet(1, 12345.67)
            .insert_stations(
                8,
                Station {
                    max_dockable_ship_volume: ignored_number as f64,
                    name: "Test Station Name".to_string(),
                    office_rental_cost: ignored_number as f64,
                    owner: None,
                    position: Position {
                        x: ignored_number as f64,
                        y: ignored_number as f64,
                        z: ignored_number as f64,
                    },
                    race_id: None,
                    reprocessing_efficiency: ignored_number as f64,
                    reprocessing_stations_take: ignored_number as f64,
                    services: vec![],
                    station_id: 8,
                    system_id: 9,
                    type_id: ignored_number as i32,
                },
            )
            .insert_industrial_system(IndustrialSystem {
                cost_indices: vec![
                    CostIndex {
                        activity: "manufacturing".to_string(),
                        cost_index: 0.456,
                    },
                    CostIndex {
                        activity: "invention".to_string(),
                        cost_index: 0.789,
                    },
                ],
                solar_system_id: 9,
            })
            .insert_system(
                9,
                System {
                    constellation_id: 10,
                    name: "Test Solar System".to_string(),
                    planets: None,
                    position: Position {
                        x: ignored_number as f64,
                        y: ignored_number as f64,
                        z: ignored_number as f64,
                    },
                    security_class: None,
                    security_status: 0.1234,
                    star_id: None,
                    stargates: None,
                    stations: Some(vec![8]),
                    system_id: 9,
                },
            )
            .insert_constellation(
                10,
                Constellation {
                    constellation_id: 10,
                    name: "Test Constellation".to_string(),
                    position: Position {
                        x: ignored_number as f64,
                        y: ignored_number as f64,
                        z: ignored_number as f64,
                    },
                    region_id: 11,
                    systems: vec![9],
                },
            )
            .insert_region(
                11,
                Region {
                    constellations: vec![10],
                    description: None,
                    name: "Test Region".to_string(),
                    region_id: 11,
                },
            )
            .insert_corporation(
                2,
                CorporationPublicInfo {
                    alliance_id: Some(3),
                    ceo_id: ignored_number,
                    creator_id: ignored_number,
                    date_founded: None,
                    description: None,
                    faction_id: None,
                    home_station_id: None,
                    member_count: 1,
                    name: "Test Corp".to_string(),
                    shares: None,
                    tax_rate: ignored_number as f64,
                    ticker: None,
                    url: None,
                    war_eligible: None,
                },
            )
            .insert_alliance(
                3,
                AllianceInfo {
                    creator_corporation_id: ignored_number,
                    creator_id: ignored_number,
                    date_founded: ignored_string.clone(),
                    executor_corporation_id: None,
                    faction_id: None,
                    name: "Test Alliance".to_string(),
                    ticker: ignored_string.clone(),
                },
            )
            .insert_character_skills(
                1,
                Skills {
                    skills: vec![
                        Skill {
                            active_skill_level: ignored_number as i32,
                            skill_id: 4,
                            skillpoints_in_skill: ignored_number as i64,
                            trained_skill_level: 2,
                        },
                        Skill {
                            active_skill_level: ignored_number as i32,
                            skill_id: 5,
                            skillpoints_in_skill: ignored_number as i64,
                            trained_skill_level: 5,
                        },
                        Skill {
                            active_skill_level: ignored_number as i32,
                            skill_id: 6,
                            skillpoints_in_skill: ignored_number as i64,
                            trained_skill_level: 1,
                        },
                        Skill {
                            active_skill_level: ignored_number as i32,
                            skill_id: 7,
                            skillpoints_in_skill: ignored_number as i64,
                            trained_skill_level: 0,
                        },
                    ],
                    total_sp: ignored_number as i64,
                    unallocated_sp: ignored_number as i32,
                },
            )
            .insert_type(4, create_skill_type(4, "Test Skill n4"))
            .insert_type(5, create_skill_type(5, "Test Skill n5"))
            .insert_type(6, create_skill_type(6, "Test Skill n6"))
            .insert_type(7, create_skill_type(7, "Test Skill n7"))
            .insert_structures(
                15,
                Structure {
                    name: "Test Structure".to_string(),
                    owner_id: ignored_number as i32,
                    position: Position {
                        x: ignored_number as f64,
                        y: ignored_number as f64,
                        z: ignored_number as f64,
                    },
                    solar_system_id: 9,
                    type_id: None,
                },
            )
            .insert_structures(
                16,
                Structure {
                    name: "Another Structure".to_string(),
                    owner_id: ignored_number as i32,
                    position: Position {
                        x: ignored_number as f64,
                        y: ignored_number as f64,
                        z: ignored_number as f64,
                    },
                    solar_system_id: 9,
                    type_id: None,
                },
            )
            .insert_type(18, create_item_type(18, "Item 18", Some(2.5)))
            .insert_price_item(PriceItem {
                adjusted_price: Some(18.1),
                average_price: Some(18.2),
                type_id: 18,
            })
            .insert_type(19, create_item_type(19, "Item 19", None))
            .insert_price_item(PriceItem {
                adjusted_price: Some(19.1),
                average_price: Some(19.2),
                type_id: 19,
            })
            .insert_blueprints(
                24,
                SDEBlueprint {
                    activities: Activities {
                        copying: None,
                        invention: None,
                        manufacturing: Some(Manufacturing {
                            materials: None,
                            products: Some(vec![Item {
                                quantity: 10,
                                type_id: 19,
                            }]),
                            time: 1054,
                        }),
                        research_material: None,
                        research_time: None,
                    },
                    blueprint_type_id: 24,
                    max_production_limit: 1,
                },
            )
            .insert_type(
                24,
                create_item_type(24, "Item 19 Manufacturing Blueprint", None),
            )
            .insert_type(20, create_item_type(20, "Item 20", Some(123.0)))
            .insert_price_item(PriceItem {
                adjusted_price: Some(20.1),
                average_price: Some(20.2),
                type_id: 20,
            })
            .insert_blueprints(
                21,
                SDEBlueprint {
                    activities: Activities {
                        copying: None,
                        invention: None,
                        manufacturing: Some(Manufacturing {
                            materials: None,
                            products: Some(vec![Item {
                                quantity: 1,
                                type_id: 20,
                            }]),
                            time: 100,
                        }),
                        research_material: None,
                        research_time: None,
                    },
                    blueprint_type_id: 21,
                    max_production_limit: 1,
                },
            )
            .insert_type(
                21,
                create_item_type(21, "Item 20 Manufacturing Blueprint", None),
            )
            .insert_blueprints(
                22,
                SDEBlueprint {
                    activities: Activities {
                        copying: None,
                        invention: Some(Invention {
                            materials: None,
                            products: Some(vec![ProbableMultipleItems {
                                probability: Some(0.3),
                                quantity: 1,
                                type_id: 21,
                            }]),
                            skills: None,
                            time: 100,
                        }),
                        manufacturing: None,
                        research_material: None,
                        research_time: None,
                    },
                    blueprint_type_id: 22,
                    max_production_limit: 1,
                },
            )
            .insert_type(
                22,
                create_item_type(22, "Item 21 Invention Blueprint", None),
            )
            .insert_character_industrial_jobs(
                1,
                create_industry_job(
                    IndustryType::Manufacturing,
                    24,
                    "2014-07-08T09:10:11+00:00".to_string(),
                    19,
                    6,
                ),
            )
            .insert_character_industrial_jobs(
                1,
                create_industry_job(
                    IndustryType::Invention,
                    22,
                    "2014-10-02T10:11:12+00:00".to_string(),
                    21,
                    2,
                ),
            )
            .insert_character_market_orders(
                1,
                create_character_order(OrderType::Sell, 123456.78, 19, 456, 789),
            )
            .insert_character_market_orders(
                1,
                create_character_order(OrderType::Buy, 456789.12, 20, 789, 1230),
            )
    }
}

pub fn create_skill_type(id: i32, name: &str) -> Type {
    let ignored_number = 0;
    let ignored_string = "Ignored".to_string();
    Type {
        capacity: None,
        description: ignored_string.clone(),
        dogma_attributes: None,
        dogma_effects: None,
        graphic_id: None,
        group_id: ignored_number,
        icon_id: None,
        market_group_id: None,
        mass: None,
        name: name.to_string(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        type_id: id,
        volume: None,
    }
}

pub fn create_item_type(id: i32, name: &str, volume: Option<f64>) -> Type {
    let ignored_number = 0;
    let ignored_string = "Ignored".to_string();
    Type {
        capacity: None,
        description: ignored_string.clone(),
        dogma_attributes: None,
        dogma_effects: None,
        graphic_id: None,
        group_id: ignored_number,
        icon_id: None,
        market_group_id: None,
        mass: None,
        name: name.to_string(),
        packaged_volume: None,
        portion_size: None,
        published: true,
        radius: None,
        type_id: id,
        volume: volume,
    }
}

pub fn create_industry_job(
    industry_type: IndustryType,
    blueprint_id: i64,
    end_date: String,
    item_produced_id: i32,
    nb_runs: i32,
) -> IndustryJob {
    let ignored_number = 0;
    let ignored_string = "Ignored".to_string();

    IndustryJob {
        activity_id: industry_type.to_activity_id(),
        blueprint_id: blueprint_id,
        blueprint_location_id: ignored_number,
        blueprint_type_id: ignored_number as i32,
        completed_character_id: None,
        completed_date: None,
        cost: None,
        duration: ignored_number as i32,
        end_date: end_date,
        facility_id: ignored_number,
        installer_id: ignored_number as i32,
        job_id: ignored_number as i32,
        licensed_runs: None,
        output_location_id: ignored_number,
        pause_date: None,
        probability: None,
        product_type_id: Some(item_produced_id),
        runs: nb_runs,
        start_date: ignored_string.clone(),
        station_id: ignored_number,
        status: ignored_string.clone(),
        successful_runs: None,
    }
}

pub fn create_character_order(
    order_type: OrderType,
    price: f64,
    item_id: i32,
    volume_remain: i32,
    volume_total: i32,
) -> CharacterOrder {
    let ignored_number = 0;
    let ignored_string = "Ignored".to_string();

    CharacterOrder {
        duration: ignored_number,
        escrow: None,
        is_buy_order: Some(order_type == OrderType::Buy),
        is_corporation: false,
        issued: ignored_string.clone(),
        location_id: ignored_number as i64,
        min_volume: None,
        order_id: ignored_number as i64,
        price: price,
        range: ignored_string,
        region_id: ignored_number,
        type_id: item_id,
        volume_remain: volume_remain,
        volume_total: volume_total,
    }
}

pub struct MockRequester {
    //Characters
    character_base_info: Option<CharacterBaseInfo>,
    character_location_info: Option<LocationInfo>,
    character_wallet: HashMap<i32, f64>,
    character_public_infos: HashMap<i32, CharacterPublicInfo>,
    character_market_orders: HashMap<i32, Vec<CharacterOrder>>,
    character_industrial_jobs: HashMap<i32, Vec<IndustryJob>>,
    character_skills: HashMap<i32, Skills>,

    // Blueprints
    blueprints: HashMap<i32, SDEBlueprint>,

    // Alliance & Corps
    alliances: HashMap<i32, AllianceInfo>,
    corporations: HashMap<i32, CorporationPublicInfo>,

    // Markets
    price_items: Vec<PriceItem>,
    region_orders: HashMap<i32, Vec<MarketOrder>>,
    history_items: HashMap<i32, HashMap<i32, Vec<HistoryItem>>>,

    // Universe
    industrial_systems: Vec<IndustrialSystem>,
    region: HashMap<i32, Region>,
    constellations: HashMap<i32, Constellation>,
    systems: HashMap<i32, System>,
    structures: HashMap<i64, Structure>,
    stations: HashMap<i32, Station>,
    types: HashMap<i32, Type>,
}

impl MockRequester {
    pub fn builder() -> MockRequesterBuilder {
        MockRequesterBuilder::new()
    }
}

impl Default for MockRequester {
    fn default() -> Self {
        MockRequesterBuilder::default().build()
    }
}

impl EveRequester for MockRequester {}

#[async_trait]
impl StationLoader for MockRequester {
    async fn get_station(&self, id: i32) -> Result<Station, CacheError> {
        match self.stations.get(&id) {
            Some(station) => return Ok(station.clone()),
            None => panic!("Could not load station: {}", id),
        }
    }
}

#[async_trait]
impl StructureLoader for MockRequester {
    async fn get_structure(&self, id: i64) -> Result<Structure, CacheError> {
        match self.structures.get(&id) {
            Some(structure) => return Ok(structure.clone()),
            None => panic!("Could not load structure: {}", id),
        }
    }
}

#[async_trait]
impl SystemLoader for MockRequester {
    async fn get_system(&self, id: i32) -> Result<System, CacheError> {
        match self.systems.get(&id) {
            Some(system) => return Ok(system.clone()),
            None => panic!("Could not load system: {}", id),
        }
    }
}

#[async_trait]
impl ConstellationLoader for MockRequester {
    async fn get_constellation(&self, id: i32) -> Result<Constellation, CacheError> {
        match self.constellations.get(&id) {
            Some(constellation) => return Ok(constellation.clone()),
            None => panic!("Could not load constellation: {}", id),
        }
    }
}

#[async_trait]
impl RegionLoader for MockRequester {
    async fn get_region(&self, id: i32) -> Result<Region, CacheError> {
        match self.region.get(&id) {
            Some(region) => return Ok(region.clone()),
            None => panic!("Could not load region: {}", id),
        }
    }
}

#[async_trait]
impl TypeLoader for MockRequester {
    async fn get_type(&self, id: i32) -> Result<Type, CacheError> {
        match self.types.get(&id) {
            Some(t) => Ok(t.clone()),
            None => panic!("Could not load type: {}", id),
        }
    }
}

#[async_trait]
impl CorporationLoader for MockRequester {
    async fn get_corporation(&self, id: i32) -> Result<CorporationPublicInfo, CacheError> {
        match self.corporations.get(&id) {
            Some(corp) => return Ok(corp.clone()),
            None => panic!("Could not load corp: {}", id),
        }
    }
}

#[async_trait]
impl AllianceLoader for MockRequester {
    async fn get_alliance(&self, id: i32) -> Result<AllianceInfo, CacheError> {
        match self.alliances.get(&id) {
            Some(alliance) => return Ok(alliance.clone()),
            None => panic!("Could not load alliance: {}", id),
        }
    }
}

#[async_trait]
impl MarketOrderLoader for MockRequester {
    async fn get_market_orders(
        &self,
        region_id: i32,
        order_type: OrderType,
    ) -> Result<Vec<MarketOrder>, CacheError> {
        self.get_region_orders(region_id, Some(order_type.to_string()), None, None)
            .await
    }
}

#[async_trait]
impl Searcher for MockRequester {
    async fn search(
        &self,
        _: i32,
        categories: &str,
        search_str: &str,
        _: Option<bool>,
    ) -> Result<SearchResult, CacheError> {
        let mut found_inventory_types = None;
        if categories.contains("inventory_type") {
            let mut inventory_types = vec![];
            for (id, eve_type) in &self.types {
                if eve_type.name.contains(search_str) {
                    inventory_types.push(*id)
                }
            }
            found_inventory_types = Some(inventory_types);
        }

        let mut found_structures = None;
        if categories.contains("structure") {
            let mut structures = vec![];
            for (id, structure) in &self.structures {
                if structure.name.contains(search_str) {
                    structures.push(*id as u64)
                }
            }
            found_structures = Some(structures);
        }

        Ok(SearchResult {
            agent: None,
            alliance: None,
            character: None,
            constellation: None,
            corporation: None,
            faction: None,
            inventory_type: found_inventory_types,
            region: None,
            solar_system: None,
            station: None,
            structure: found_structures,
        })
    }
}

#[async_trait]
impl MarketPricesLoader for MockRequester {
    async fn get_market_prices(&self) -> Result<Vec<PriceItem>, CacheError> {
        Ok(self.price_items.clone())
    }
}

#[async_trait]
impl IndustrialSystemsLoader for MockRequester {
    async fn get_industry_systems(&self) -> Result<Vec<IndustrialSystem>, CacheError> {
        Ok(self.industrial_systems.clone())
    }
}

#[async_trait]
impl RegionIDsLoader for MockRequester {
    async fn get_region_ids(&self) -> Result<Vec<i32>, CacheError> {
        let to_return: Vec<i32> = self.region.keys().map(|id| id.clone()).collect();
        Ok(to_return)
    }
}

#[async_trait]
impl CharacterSkillsLoader for MockRequester {
    async fn get_character_skill(&self, id: i32) -> Result<Skills, CacheError> {
        match self.character_skills.get(&id) {
            Some(skills) => return Ok(skills.clone()),
            None => panic!("Could not load skill: {}", id),
        }
    }
}

#[async_trait]
impl CharacterIndustryJobsLoader for MockRequester {
    async fn get_character_industry_jobs(&self, id: i32) -> Result<Vec<IndustryJob>, CacheError> {
        match self.character_industrial_jobs.get(&id) {
            Some(jobs) => return Ok(jobs.clone()),
            None => panic!("Could not load jobs: {}", id),
        }
    }
}

#[async_trait]
impl CharacterMarketOrdersLoader for MockRequester {
    async fn get_character_orders(&self, id: i32) -> Result<Vec<CharacterOrder>, CacheError> {
        match self.character_market_orders.get(&id) {
            Some(orders) => return Ok(orders.clone()),
            None => panic!("Could not load character market orders: {}", id),
        }
    }
}

#[async_trait]
impl BlueprintsLoader for MockRequester {
    async fn get_blueprint(
        &self,
        product_id: i32,
        activity: &BlueprintActivityType,
    ) -> Result<Option<(i32, SDEBlueprint)>, CacheError> {
        let founds: Vec<(i32, SDEBlueprint)> = self
            .blueprints
            .iter()
            .filter(|(_, bp)| bp.can_produce(product_id, activity))
            .map(|(id, bp)| (id.clone(), bp.clone()))
            .collect();
        if founds.is_empty() {
            return Ok(None);
        }
        return Ok(Some(founds[0].clone()));
    }
    async fn get_blueprints(&self) -> Result<HashMap<i32, SDEBlueprint>, CacheError> {
        Ok(self.blueprints.clone())
    }
}

#[async_trait]
impl CharacterPublicInfoLoader for MockRequester {
    async fn get_character_public_info(&self, id: i32) -> Result<CharacterPublicInfo, CacheError> {
        match self.character_public_infos.get(&id) {
            Some(infos) => {
                return Ok(CharacterPublicInfo {
                    alliance_id: infos.alliance_id,
                    birthday: infos.birthday.clone(),
                    bloodline_id: infos.bloodline_id,
                    corporation_id: infos.corporation_id,
                    description: infos.description.clone(),
                    gender: infos.gender.clone(),
                    name: infos.name.clone(),
                    race_id: infos.race_id,
                    security_status: infos.security_status,
                    title: infos.title.clone(),
                })
            }
            None => panic!("Unsupported character id: {}", id),
        }
    }
}

#[async_trait]
impl CharacterWalletLoader for MockRequester {
    async fn get_character_wallet(&self, id: i32) -> Result<f64, CacheError> {
        match self.character_wallet.get(&id) {
            Some(wallet) => return Ok(*wallet),
            None => panic!("Unsupported wallet id: {}", id),
        }
    }
}

#[async_trait]
impl CharacterLocationLoader for MockRequester {
    async fn get_character_location(&self, _: i32) -> Result<LocationInfo, CacheError> {
        let character_location_info = self
            .character_location_info
            .as_ref()
            .expect("Character Location requested but none was available");
        return Ok(LocationInfo {
            solar_system_id: character_location_info.solar_system_id,
            station_id: character_location_info.station_id,
            structure_id: character_location_info.structure_id,
        });
    }
}

#[async_trait]
impl CharacterBasicInfoLoader for MockRequester {
    async fn get_character_basic_info(&self) -> Result<CharacterBaseInfo, CacheError> {
        return Ok(self
            .character_base_info
            .as_ref()
            .expect("Character Base Info requested but none was available")
            .clone());
    }
}

impl MarketTraits for MockRequester {}

#[async_trait]
impl MarketRegionOrdersLoader for MockRequester {
    async fn get_region_orders(
        &self,
        region_id: i32,
        order_type: Option<String>,
        _page: Option<i32>,
        item_id: Option<i32>,
    ) -> Result<Vec<MarketOrder>, CacheError> {
        let region_orders = self.region_orders.get(&region_id);
        match region_orders {
            None => return Ok(vec![]),
            Some(orders) => {
                let mut to_return = vec![];
                for order in orders {
                    let mut add_order = true;
                    if let Some(order_type) = &order_type {
                        add_order = add_order
                            && match order_type.as_str() {
                                "sell" => !order.is_buy_order,
                                "buy" => order.is_buy_order,
                                _ => unimplemented!(),
                            }
                    }
                    if let Some(item_id) = item_id {
                        add_order = add_order && item_id == order.type_id;
                    }
                    if add_order {
                        to_return.push(order.clone());
                    }
                }
                Ok(to_return)
            }
        }
    }
}

#[async_trait]
impl MarketRegionHistoryLoader for MockRequester {
    async fn get_region_market_history(
        &self,
        region_id: i32,
        item_id: i32,
    ) -> Result<Vec<HistoryItem>, CacheError> {
        let items = self.history_items.get(&region_id);
        match items {
            None => return Ok(vec![]),
            Some(items) => {
                let items = items.get(&item_id);
                match items {
                    Some(items) => return Ok(items.clone()),
                    None => return Ok(vec![]),
                }
            }
        }
    }
}
