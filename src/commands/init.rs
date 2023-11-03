use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use colored::Colorize;
use inquire::parser::CustomTypeParser;
use inquire::{parse_type, required, Confirm, Text};

use crate::api::evecache::{CacheLevel, EveCache};
use crate::api::sde::Sde;
use crate::authentication::tokens::TokenHelper;
use crate::authentication::Authenticator;
use crate::cache::FSCache;
use crate::commands::update::update;
use crate::configuration::cli::{Args, ItemAddOptions, UpdateOptions};
use crate::configuration::default::DefaultConfiguration;
use crate::configuration::files::{
    APIConfiguration, AuthenticationServerConfiguration, MainConfiguration,
};
use crate::configuration::{self, get_config_file, Configuration, ConfigurationDirectoryType};
use crate::display::Display;
use crate::errors::{EnvironmentError, EveApiError, EveAuthenticationError, EveError};
use crate::filesystem::{self, FSData};
use crate::integration::DataIntegrator;
use crate::interactive::HandleInquireExitSignals;
use crate::logging::Msg;
use crate::logging::{self, Empty};

use super::facility;
use super::item;
use super::login;

pub(crate) async fn init(args: Args) -> Result<(), EveError> {
    logging::println(Msg("Welcome to Eve Vulcain.".to_string()));
    logging::println(Msg(
        "We will guide you through the setup required to run the application.\n".to_string(),
    ));
    logging::println(Msg(format!("{}", "STEP 1/6: Download SDE".underline())));
    let cache_dir = configuration::get_directory(ConfigurationDirectoryType::Cache, &args)
        .await?
        .ok_or(EnvironmentError::CacheDirectoryUnknown)?;
    update(cache_dir, &UpdateOptions { no_download: false }).await?;
    let cfg = get_config_file(&args)
        .await?
        .ok_or(EnvironmentError::ConfigurationFilePathNotFound)?;

    logging::println(Empty);

    user_questions_init(&cfg, &args).await?;

    Ok(())
}

async fn user_questions_init(cfg_file: &PathBuf, args: &Args) -> Result<(), EveError> {
    let overwrite_application_configuration = match define_application(cfg_file)? {
        Some(overwrite_application_configuration) => overwrite_application_configuration,
        None => return Ok(()),
    };

    logging::println(Empty);

    if save_configuration(cfg_file, overwrite_application_configuration)?.is_none() {
        return Ok(());
    }

    logging::println(Empty);

    logging::println(Msg(format!("{}", "STEP 4/6: Login to eve API".underline())));
    let data_dir = configuration::get_directory(ConfigurationDirectoryType::Data, args)
        .await?
        .ok_or(EnvironmentError::DataDirectoryUnknown)?;
    filesystem::create_directory(&data_dir).await?;
    let fs_data = FSData::new(data_dir);
    let cfg = configuration::get(args).await?;
    let authenticator = Authenticator::new(&fs_data, &cfg);
    login::login(
        &authenticator,
        cfg.api_client_id()
            .map_err(|source| EnvironmentError::ConfigurationOptionLoading {
                option_name: "api_client_id".to_string(),
                source,
            })?,
        &fs_data,
    )
    .await?;

    logging::println(Empty);

    let cache_dir = configuration::get_directory(ConfigurationDirectoryType::Cache, args)
        .await?
        .ok_or(EnvironmentError::CacheDirectoryUnknown)?;
    let data_dir = configuration::get_directory(ConfigurationDirectoryType::Data, args)
        .await?
        .ok_or(EnvironmentError::DataDirectoryUnknown)?;
    let fs_data = FSData::new(data_dir);

    let mut esi = Authenticator::new(&fs_data, &cfg)
        .authenticate()
        .await
        .map_err(|source| EveAuthenticationError::AuthenticationLoadingError { source })?;
    esi.update_spec()
        .await
        .map_err(|source| EveApiError::ESIInitFailed { source })?;

    let fs_cache = FSCache::new(cache_dir);
    let cache_level = CacheLevel::from(&args.cache_level, fs_cache.clone());

    let cache = EveCache::new(
        esi,
        Sde::new(fs_cache),
        TokenHelper {
            api_client_id: cfg.api_client_id().map_err(|source| {
                EnvironmentError::ConfigurationOptionLoading {
                    option_name: "api_client_id".to_string(),
                    source,
                }
            })?,
        },
        cache_level,
    )
    .await
    .map_err(|source| EveApiError::ESICacheInitFailed { source })?;
    let cache = Arc::new(cache);
    let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
    logging::println(Msg(format!(
        "{}",
        "STEP 5/6: Add facilities as processing industries and/or markets.".underline()
    )));
    let res = Confirm::new("Do you want to add a facility ?")
        .with_default(true)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "facility add".to_string(),
            source,
        })?;
    if let Some(true) = res {
        logging::println(Empty);
        facility::add::add(&data_integrator).await?;
    } else if res.is_none() {
        return Ok(());
    }

    logging::println(Empty);
    logging::println(Msg(format!(
        "{}",
        "STEP 6/6: Add items to process and surveil.".underline()
    )));
    let res = Confirm::new("Do you want to add an item ?")
        .with_default(true)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "item add".to_string(),
            source,
        })?;
    if let Some(true) = res {
        logging::println(Empty);
        item::add::add(&data_integrator, &ItemAddOptions { item: None }).await?;
    } else if res.is_none() {
        return Ok(());
    }

    logging::println(Empty);
    logging::println(Msg("Initialization finished.".to_string()));

    Ok(())
}

fn define_application(cfg_file: &PathBuf) -> Result<Option<bool>, EveError> {
    logging::println(Msg(format!(
        "{}",
        "STEP 2/6: Setup an eve online api access".underline()
    )));

    if cfg_file.exists() {
        logging::println(Msg(format!(
            "Configuration file detected: {}",
            cfg_file.to_display().bold()
        )));
        let overwrite = Confirm::new(
            "Initialization process will overwrite this file. Are you sure you want to continue ?",
        )
        .with_default(false)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "overwrite config".to_string(),
            source,
        })?;
        let overwrite = match overwrite {
            Some(overwrite) => overwrite,
            None => return Ok(None),
        };
        if !overwrite {
            return Ok(Some(false));
        } else {
            logging::println(Msg("Configuration overwrite confirmed.\n".to_string()));
        }
    }

    logging::println(Msg("You can use the default registered application, or define your own application in EVE Developers portal.".to_string()));
    let define_application = Confirm::new("Do you want to define your own application ?")
        .with_default(false)
        .prompt()
        .handle_exit_signals()
        .map_err(|source| EnvironmentError::SpecificInputError {
            description: "custom application".to_string(),
            source,
        })?;
    let define_application = match define_application {
        Some(define_application) => define_application,
        None => return Ok(None),
    };
    logging::println(Empty);
    if define_application {
        logging::println(Msg(
            "To access eve api, we first need to setup a new applications in the CCP database."
                .to_string(),
        ));
        logging::println(Msg("You will need to create a new application in EVE developer portal (https://developers.eveonline.com/).\n".to_string()));
        logging::println(Msg("Here is the step by step guide:".to_string()));
        logging::println(Msg(format!(
            "\t1. Login to the eve application manager using the '{}' button (Upper Right corner).",
            "Log in".bold()
        )));
        logging::println(Msg(format!(
            "\t2. Click the '{}' button.",
            "Manage Applications".bold()
        )));
        logging::println(Msg(format!(
            "\t3. Click the '{}' button.",
            "Create new Application".bold()
        )));
        logging::println(Msg("\t4. Fill the new application form:".to_string()));
        logging::println(Msg(
            "\t\ta. Enter any name you wish in the 'Name' field.".to_string()
        ));
        logging::println(Msg(
            "\t\tb. Enter any description you wish in the 'Description' field.".to_string(),
        ));
        logging::println(Msg(
            "\t\tc. 'Connection type' should be set to 'Authentication & API Access'.".to_string(),
        ));
        logging::println(Msg(
            "\t\td. Under 'Permissions' requested scopes should at least contains:".to_string(),
        ));

        logging::println(Msg("\t\t\tpublicData".to_string()));
        logging::println(Msg("\t\t\tesi-location.read_location.v1".to_string()));
        logging::println(Msg("\t\t\tesi-search.search_structures.v1".to_string()));
        logging::println(Msg("\t\t\tesi-universe.read_structures.v1".to_string()));
        logging::println(Msg("\t\t\tesi-skills.read_skills.v1".to_string()));
        logging::println(Msg("\t\t\tesi-wallet.read_character_wallet.v1".to_string()));
        logging::println(Msg("\t\t\tesi-industry.read_character_jobs.v1".to_string()));
        logging::println(Msg("\t\t\tesi-markets.read_character_orders.v1".to_string()));

        logging::println(Msg("\t\te. Set 'Callback URL' to 'http://localhost:54631/' (by default, another port can be selected).".to_string()));
        logging::println(Msg(
            "\t\tf. Click on 'Create Application' (by default, another port can be selected)."
                .to_string(),
        ));
        logging::println(Msg(format!(
            "\t5. Find your newly created application and click on '{}'.",
            "View Application".bold()
        )));
        logging::println(Msg(format!(
            "\t6. Note the following informations (available under '{}'):",
            "Application Settings".bold()
        )));
        logging::println(Msg("\t\tClient ID".to_string()));
        logging::println(Msg("\t\tCallback URL".to_string()));

        logging::println(Msg(format!(
            "\nPush {} to open your browser to the eve application manager.",
            "Enter".bold()
        )));
        let mut ignored_buffer = String::new();
        io::stdin()
            .read_line(&mut ignored_buffer)
            .map_err(|source| EnvironmentError::STDInReadFailed { source })?;

        let url = "https://developers.eveonline.com/";
        open::that(url).map_err(|source| EnvironmentError::BrowserOpening {
            url: url.to_string(),
            source,
        })?;

        logging::println(Msg(format!(
            "Push {} when you've registered a new application and you're ready to continue.",
            "Enter".bold()
        )));
        io::stdin()
            .read_line(&mut ignored_buffer)
            .map_err(|source| EnvironmentError::STDInReadFailed { source })?;
    } else {
        logging::println(Msg("Using default application.".to_string()));
    }
    Ok(Some(define_application))
}

fn save_configuration(
    cfg_file: &PathBuf,
    overwrite_application_configuration: bool,
) -> Result<Option<()>, EveError> {
    let cfg_path_str = cfg_file.to_display();

    let mut cfg = MainConfiguration {
        authentication_server: Some(AuthenticationServerConfiguration { port: Some(54621) }),
        api: Some(APIConfiguration {
            client_id: Some(DefaultConfiguration.api_client_id().map_err(|source| {
                EnvironmentError::ConfigurationOptionLoading {
                    option_name: "api_client_id".to_string(),
                    source,
                }
            })?),
            callback_url: Some(DefaultConfiguration.api_callback_url().map_err(|source| {
                EnvironmentError::ConfigurationOptionLoading {
                    option_name: "api_callback_url".to_string(),
                    source,
                }
            })?),
        }),
        facilities: None,
    };
    logging::println(Msg(format!(
        "{}",
        "STEP 3/6: Generate the local configuration files".underline()
    )));
    if !overwrite_application_configuration {
        if cfg_file.exists() {
            logging::println(Msg(
                "Not changing configuration file. This step is only required if a custom application is defined"
                    .to_string(),
            ));
            return Ok(Some(()));
        } else {
            logging::println(Msg(
                "Local configuration file generated. This step is only required if a custom application is defined"
                    .to_string(),
            ));
        }
    } else {
        logging::println(Msg(format!(
            "The configuration will be generated at this path: {}",
            cfg_path_str.bold()
        )));
        logging::println(Msg(
            "To generate the configuration, please answer the few following questions:\n"
                .to_string(),
        ));

        let client_id = Text::new("Client ID:")
            .with_help_message(
                "You can get it from your custom app page, in the Eve Developper portal.",
            )
            .with_validator(required!())
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "client id".to_string(),
                source,
            })?;
        let client_id = match client_id {
            Some(client_id) => client_id,
            None => return Ok(None),
        };

        let callback_url = Text::new("Callback URL (http://localhost:54621/):")
            .with_help_message(
                "You can get it from your custom app page, in the Eve Developper portal.",
            )
            .with_default("http://localhost:54621/")
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "callback url".to_string(),
                source,
            })?;
        let callback_url = match callback_url {
            Some(callback_url) => callback_url,
            None => return Ok(None),
        };
        let callback_url = callback_url.trim();

        let port_parser: CustomTypeParser<u16> = parse_type!(u16);
        let port_parser_error_msg = inquire::validator::ErrorMessage::Custom(
            "Should be a number between 1 and ".to_string() + u16::MAX.to_string().as_str() + ".",
        );
        let auth_server_port = Text::new("Authentication server listening port (54621):")
            .with_help_message(
                "You can get it from your custom app page, in the Eve Developper portal.",
            )
            .with_default("54621")
            .with_validator(required!())
            .with_validator(move |input: &str| match port_parser(input) {
                Ok(val) => {
                    if val == 0 {
                        Ok(inquire::validator::Validation::Invalid(
                            port_parser_error_msg.clone(),
                        ))
                    } else {
                        Ok(inquire::validator::Validation::Valid)
                    }
                }
                Err(_) => Ok(inquire::validator::Validation::Invalid(
                    port_parser_error_msg.clone(),
                )),
            })
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "auth_server_port".to_string(),
                source,
            })?;
        let auth_server_port = match auth_server_port {
            Some(auth_server_port) => auth_server_port,
            None => return Ok(None),
        };

        cfg = MainConfiguration {
            authentication_server: Some(AuthenticationServerConfiguration {
                port: Some(auth_server_port.trim().parse::<u16>().map_err(|source| {
                    EnvironmentError::ParsePortError {
                        port: auth_server_port.clone(),
                        source,
                    }
                })?),
            }),
            api: Some(APIConfiguration {
                client_id: Some(client_id.trim().to_string()),
                callback_url: Some(callback_url.trim().to_string()),
            }),
            facilities: None,
        };
    }

    let cfg_content = toml::to_string(&cfg)
        .map_err(|source| EnvironmentError::TOMLConfigurationSerilizationError { source })?;
    std::fs::write(cfg_file, cfg_content).map_err(|source| {
        EnvironmentError::TOMLConfigurationWriteError {
            path: cfg_file.to_display(),
            source,
        }
    })?;

    logging::println(Msg(format!(
        "\nConfiguration written to {}.",
        cfg_path_str.bold()
    )));

    Ok(Some(()))
}
