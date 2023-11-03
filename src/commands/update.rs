use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::configuration::cli::UpdateOptions;
use crate::display::Display;
use crate::errors::{EnvironmentError, EveApiError, EveError};
use crate::interactive::HandleInquireExitSignals;
use futures_util::future::TryJoinAll;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::Confirm;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::try_join;
use zip::ZipArchive;

use crate::{filesystem, logging};

const SDE_FILES_PATHS: [&str; 2] = ["sde/fsd/blueprints.yaml", "sde/fsd/typeMaterials.yaml"];

pub async fn update(cache_dir: PathBuf, opts: &UpdateOptions) -> Result<(), EveError> {
    let installed = check_installed(&cache_dir).await?;
    if installed {
        let redownload = Confirm::new("SDE is already installed. Do you want to update it ?")
            .with_default(false)
            .prompt()
            .handle_exit_signals()
            .map_err(|source| EnvironmentError::SpecificInputError {
                description: "SDE redownload confirmation".to_string(),
                source,
            })?;
        if let None | Some(false) = redownload {
            return Ok(());
        }
    }

    logging::info!("Preparing SDE");
    let mut download_destination = cache_dir.clone();
    download_destination.push("sde/sde.zip");

    if !opts.no_download {
        if download_destination.exists() {
            tokio::fs::remove_file(&download_destination)
                .await
                .map_err(|source| EveApiError::SDEDeletePreviousZip { source })?
        }
        logging::debug!("Downloading SDE");
        download_sde(&download_destination).await?;
    }

    logging::debug!("Extracting from SDE");
    let mut futures = vec![];
    for to_extract_path in SDE_FILES_PATHS {
        futures.push(extract_sde_files(
            &cache_dir,
            &download_destination,
            to_extract_path,
        ));
    }
    let all_extract_operations = futures.into_iter().collect::<TryJoinAll<_>>();

    try_join!(all_extract_operations)?;

    let to_split_paths = vec![];

    logging::debug!("Splitting large SDE files. This operation might take several minutes ...");
    let mut futures = vec![];
    for to_split_path in to_split_paths {
        futures.push(filesystem::split_into_subfiles(&cache_dir, to_split_path));
    }
    let all_operations = futures.into_iter().collect::<TryJoinAll<_>>();

    try_join!(all_operations).map_err(|source| EveApiError::SDESplitFile { source })?;

    logging::info!("SDE is ready to use");
    Ok(())
}

async fn download_sde(destination_path: &PathBuf) -> Result<(), EveError> {
    let url = "https://eve-static-data-export.s3-eu-west-1.amazonaws.com/tranquility/sde.zip";
    logging::debug!("Downloading from {}", url);

    create_parent(destination_path).await?;

    let resp = reqwest::get(url)
        .await
        .map_err(|source| EveApiError::SDEDownloadFailed { source })?;
    let download_size = resp.content_length().unwrap_or(0);

    let progress_bar = ProgressBar::new(download_size);
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:40.cyan/blue} ({percent}%)")
            .unwrap()
            .progress_chars("=>-"),
    );

    let destination = tokio::fs::File::create(&destination_path)
        .await
        .map_err(|source| EveApiError::SDEFileCreationFailed { source })?;

    let mut writer = BufWriter::new(destination);

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| EveApiError::SDEBodyReadFailed { source })?;
        progress_bar.set_position(progress_bar.position() + chunk.len() as u64);
        writer
            .write_all(&chunk)
            .await
            .map_err(|source| EveApiError::SDEFileWriteFailed {
                path: destination_path.to_display(),
                source,
            })?;
    }
    writer
        .flush()
        .await
        .map_err(|source| EveApiError::SDEFileWriteFailed {
            path: destination_path.to_display(),
            source,
        })?;

    logging::debug!(
        "SDE Written to {}",
        destination_path.to_str().unwrap_or("UNKNOWN")
    );
    Ok(())
}

async fn extract_sde_files(
    cache_dir: &Path,
    zip_file: &PathBuf,
    to_extract_path: &str,
) -> Result<(), EveError> {
    let zip_file_reader =
        fs::File::open(zip_file).map_err(|source| EveApiError::SDEZipOpenError { source })?;
    let mut zip_archive = ZipArchive::new(zip_file_reader)
        .map_err(|source| EveApiError::SDEZipOpenArchival { source })?;
    let mut file = zip_archive
        .by_name(to_extract_path)
        .map_err(|source| EveApiError::SDEZipInternalFileOpenError { source })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|source| EveApiError::SDEZipInternalFileReadError { source })?;

    let mut path = cache_dir.to_path_buf();
    path.push(to_extract_path);

    create_parent(&path).await?;

    fs::write(path, contents)
        .map_err(|source| EveApiError::SDEZipWriteReadContentToFile { source })?;

    Ok(())
}

async fn create_parent(destination_path: &Path) -> Result<(), EveError> {
    if let Some(parent) = destination_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir(parent).await.map_err(|source| {
                EveApiError::SDEDirectoryCreationFailed {
                    path: parent.to_str().unwrap_or("UNKNOWN").to_string(),
                    source,
                }
            })?;
        }
    }
    Ok(())
}

async fn check_installed(cache_dir: &Path) -> Result<bool, EveError> {
    for p in SDE_FILES_PATHS {
        let mut path = cache_dir.to_path_buf();
        path.push(p);
        let exist = tokio::fs::try_exists(&path).await.map_err(|source| {
            EveApiError::SDEEFileExistenceCheckFailed {
                path: path.to_display(),
                source,
            }
        })?;
        if !exist {
            return Ok(false);
        }
    }

    Ok(true)
}
