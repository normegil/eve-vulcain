#![feature(fn_traits)]
#![feature(error_generic_member_access)]
#![feature(fs_try_exists)]
#![feature(async_closure)]
#![feature(trait_alias)]

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use api::evecache::{CacheLevel, EveCache};
use api::sde::Sde;
use authentication::tokens::TokenHelper;
use cache::FSCache;
use clap::Parser;
use commands::invention::invention;
use configuration::ConfigurationDirectoryType;
use errors::{Advice, EnvironmentError, EveApiError, EveAuthenticationError, EveError};

use crate::authentication::Authenticator;
use crate::commands::facility::facility;
use crate::commands::init::init;
use crate::commands::item::item;
use crate::commands::login::login;
use crate::commands::logout::logout;
use crate::commands::manufacturing::manufacturing;
use crate::commands::state::state;
use crate::commands::update::update;
use crate::configuration::cli;
use crate::configuration::cli::{Args, Commands};
use crate::configuration::Configuration;
use crate::filesystem::FSData;
use crate::integration::DataIntegrator;
use crate::logging::Verbosity;

mod api;
mod authentication;
mod commands;
mod configuration;
mod errors;
mod http;
mod integration;
mod logging;

mod cache;
mod dates;
mod display;
mod filesystem;
mod interactive;
mod model;
mod retry;
mod round;
mod vector;

#[tokio::main]
async fn main() -> Result<(), EveError> {
    let args = cli::Args::parse();
    let verbosity = Verbosity::new(args.verbose, args.quiet);
    logging::init(false, verbosity);

    if let Err(e) = run(args, verbosity).await {
        let mut advice = None;
        if let Verbosity::Normal = verbosity {
            if let Some(adv) = e.advice() {
                advice = Some(adv)
            }
        }

        let any_err = anyhow::Error::from(e);
        logging::err(any_err);

        if let Verbosity::Normal = verbosity {
            if let Some(adv) = advice {
                logging::println_stderr("");
                logging::println_stderr(&adv)
            }
        }

        process::exit(1);
    }
    Ok(())
}

async fn run(args: Args, verbosity: Verbosity) -> Result<(), EveError> {
    if args.command == Commands::Init {
        init(args).await?;
        return Ok(());
    }

    let cache_dir = configuration::get_directory(ConfigurationDirectoryType::Cache, &args)
        .await?
        .ok_or(EnvironmentError::CacheDirectoryUnknown)?;
    filesystem::create_directory(&cache_dir).await?;
    let data_dir = configuration::get_directory(ConfigurationDirectoryType::Data, &args)
        .await?
        .ok_or(EnvironmentError::DataDirectoryUnknown)?;
    filesystem::create_directory(&data_dir).await?;
    let cfg_dir = configuration::get_directory(ConfigurationDirectoryType::Configuration, &args)
        .await?
        .ok_or(EnvironmentError::ConfigurationDirectoryUnknown)?;
    filesystem::create_directory(&cfg_dir).await?;

    let cfg = configuration::get(&args).await?;

    if args.force_color {
        colored::control::set_override(true);
    } else if let Some(no_color) =
        cfg.no_color()
            .map_err(|source| EnvironmentError::ConfigurationOptionLoading {
                option_name: "no_color".to_string(),
                source,
            })?
    {
        if no_color {
            colored::control::set_override(false);
        }
    }
    logging::init(false, verbosity);

    let fs_data = FSData::new(data_dir);

    let authenticator = Authenticator::new(&fs_data, &cfg);

    match &args.command {
        Commands::Init => {
            unreachable!()
        }
        Commands::Login => {
            login(
                &authenticator,
                cfg.api_client_id().map_err(|source| {
                    EnvironmentError::ConfigurationOptionLoading {
                        option_name: "api_client_id".to_string(),
                        source,
                    }
                })?,
                &fs_data,
            )
            .await?
        }
        Commands::Logout => {
            logout(&fs_data).await?;
        }
        Commands::State(opts) => {
            if opts.json {
                logging::init(true, verbosity);
            }
            let cache = Arc::new(get_eve_cache(&args, cache_dir, cfg, &fs_data).await?);
            let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
            state(&data_integrator).await?;
            cache.persist().await?;
        }
        Commands::Update(opts) => {
            update(cache_dir.clone(), opts).await?;
        }
        Commands::Manufacture(opts) => {
            if opts.json {
                logging::init(true, verbosity);
            }
            let cache = Arc::new(get_eve_cache(&args, cache_dir, cfg, &fs_data).await?);
            let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
            manufacturing(&data_integrator, opts).await?;
            cache.persist().await?;
        }
        Commands::Facility(opts) => {
            let cache = Arc::new(get_eve_cache(&args, cache_dir, cfg, &fs_data).await?);
            let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
            facility(&data_integrator, opts).await?;
            cache.persist().await?;
        }
        Commands::Item(opts) => {
            let cache = Arc::new(get_eve_cache(&args, cache_dir, cfg, &fs_data).await?);
            let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
            item(&data_integrator, opts).await?;
            cache.persist().await?;
        }
        Commands::Invent(opts) => {
            if opts.json {
                logging::init(true, verbosity);
            }
            let cache = Arc::new(get_eve_cache(&args, cache_dir, cfg, &fs_data).await?);
            let data_integrator = DataIntegrator::new(cache.clone(), fs_data);
            invention(&data_integrator, opts).await?;
            cache.persist().await?;
        }
    }

    Ok(())
}

async fn get_eve_cache(
    args: &Args,
    cache_dir: PathBuf,
    cfg: impl Configuration,
    fs_data: &FSData,
) -> Result<EveCache, EveError> {
    let mut esi = Authenticator::new(fs_data, &cfg)
        .authenticate()
        .await
        .map_err(|source| EveAuthenticationError::AuthenticationLoadingError { source })?;
    esi.update_spec()
        .await
        .map_err(|source| EveApiError::ESIInitFailed { source })?;

    let cache = FSCache::new(cache_dir);
    let cache_level = CacheLevel::from(&args.cache_level, cache.clone());

    let cache = EveCache::new(
        esi,
        Sde::new(cache),
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
    Ok(cache)
}
