use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::sync::RwLock;

use crate::authentication::RefreshToken;
use crate::display::Display;

use crate::logging;
use crate::model::facility::playerstructure::PlayerStructureStats;
use crate::model::facility::FacilityUsage;
use crate::model::industry::IndustryType;

#[derive(Error, Debug)]
#[error("Directory could not be created (Path: '{path})': {source}")]
pub struct DirectoryCreationError {
    path: String,
    source: std::io::Error,
}

pub async fn create_directory(dir_to_create: &PathBuf) -> Result<(), DirectoryCreationError> {
    if !dir_to_create.exists() {
        let dir_display = dir_to_create.to_display();
        crate::logging::debug!("Create directory: {}", &dir_display);
        tokio::fs::create_dir(dir_to_create)
            .await
            .map_err(|e| DirectoryCreationError {
                path: dir_display,
                source: e,
            })?;
    }
    Ok(())
}

#[derive(Error, Debug)]
pub enum FSError {
    #[error(transparent)]
    ReadError {
        #[from]
        source: FSReadError,
    },
    #[error(transparent)]
    WriteError {
        #[from]
        source: FSWriteError,
    },
}

#[derive(Error, Debug)]
pub enum FSReadError {
    #[error("check file existence of '{path}': {source}")]
    CheckFileExistence {
        path: String,
        source: std::io::Error,
    },
    #[error("read file '{path}': {source}")]
    ReadFile {
        path: String,
        source: std::io::Error,
    },
    #[error("deserialize json content of '{path}': {source}")]
    JSONDeserialization {
        path: String,
        source: serde_json::Error,
    },
}

#[derive(Error, Debug)]
pub enum FSWriteError {
    #[error("write file '{path}': {source}")]
    WriteFileError {
        path: String,
        source: std::io::Error,
    },
    #[error("serialize json content of '{path}': {source}")]
    JSONSeserializationError {
        path: String,
        source: serde_json::Error,
    },
}

#[derive(Error, Debug)]
pub enum FSDeleteError {
    #[error("delete file '{path}': {source}")]
    DeleteFileError {
        path: String,
        source: std::io::Error,
    },
}

pub struct FSData {
    data_directory: PathBuf,
    facilities_cache: RwLock<Option<Facilities>>,
}

impl FSData {
    pub fn new(data_directory: PathBuf) -> Self {
        logging::trace!("Data directory: {}", data_directory.to_display());
        FSData {
            data_directory,
            facilities_cache: RwLock::new(None),
        }
    }
}

// Facilities
impl FSData {
    pub async fn add_station(&self, to_insert: &NPCStation) -> Result<(), FSError> {
        let mut facilities = self.load_facilities().await?;
        for station in &facilities.stations {
            if station.id == to_insert.id {
                logging::info!("Duplicate found for {:?}", station);
                return Ok(());
            }
        }
        facilities.stations.push(to_insert.clone());
        self.save_facilities(&facilities).await?;
        Ok(())
    }

    pub async fn rm_station(&self, type_id: i32) -> Result<(), FSError> {
        let mut facilities = self.load_facilities().await?;

        let index = facilities.stations.iter().position(|t| t.id == type_id);
        if let Some(i) = index {
            facilities.stations.remove(i);
        }

        self.save_facilities(&facilities).await?;
        Ok(())
    }

    pub async fn add_structure(&self, to_insert: &PlayerStructure) -> Result<(), FSError> {
        logging::info!("Saving structure: {:?}", to_insert);
        let mut facilities = self.load_facilities().await?;
        for structure in &facilities.structures {
            if structure.id == to_insert.id {
                logging::info!("Duplicate found for {:?}", structure);
                return Ok(());
            }
        }
        facilities.structures.push(to_insert.clone());
        self.save_facilities(&facilities).await?;
        Ok(())
    }

    pub async fn rm_structure(&self, type_id: i64) -> Result<(), FSError> {
        let mut facilities = self.load_facilities().await?;

        let index = facilities.structures.iter().position(|t| t.id == type_id);
        if let Some(i) = index {
            facilities.structures.remove(i);
        }

        self.save_facilities(&facilities).await?;
        Ok(())
    }

    pub async fn load_facility(&self, id: i64) -> Result<Option<FSFacilityType>, FSReadError> {
        let facilities = self.load_facilities().await?;
        let index = facilities.stations.iter().position(|t| t.id as i64 == id);
        if let Some(index) = index {
            return Ok(Some(FSFacilityType::NPCStation(
                facilities
                    .stations
                    .get(index)
                    .expect("station should exist because index was found just before searching")
                    .clone(),
            )));
        }
        let index = facilities.structures.iter().position(|t| t.id == id);
        if let Some(index) = index {
            return Ok(Some(FSFacilityType::PlayerStructure(
                facilities
                    .structures
                    .get(index)
                    .expect("structure should exist because index was found just before searching")
                    .clone(),
            )));
        }
        Ok(None)
    }

    pub async fn load_facilities(&self) -> Result<Facilities, FSReadError> {
        if self.facilities_cache.read().await.is_none() {
            let mut cache = self.facilities_cache.write().await;
            if cache.is_none() {
                let mut facilities_file = self.data_directory.clone();
                facilities_file.push("facilities_data.json");
                let facilities = if !facilities_file.exists() {
                    Facilities {
                        stations: vec![],
                        structures: vec![],
                    }
                } else {
                    let facilities_content = tokio::fs::read_to_string(&facilities_file)
                        .await
                        .map_err(|source| FSReadError::ReadFile {
                            path: facilities_file.to_display(),
                            source,
                        })?;
                    let facilities: Facilities = serde_json::from_str(&facilities_content)
                        .map_err(|source| FSReadError::JSONDeserialization {
                            path: facilities_file.to_display(),
                            source,
                        })?;
                    facilities
                };
                *cache = Some(facilities)
            }
        }
        Ok(self
            .facilities_cache
            .read()
            .await
            .clone()
            .expect("Cache should be already filled here"))
    }

    async fn save_facilities(&self, facilities: &Facilities) -> Result<(), FSWriteError> {
        let mut cache = self.facilities_cache.write().await;
        *cache = Some(facilities.clone());

        let mut facilities_file = self.data_directory.clone();
        facilities_file.push("facilities_data.json");
        let facilities_content = serde_json::to_string_pretty(&facilities).map_err(|source| {
            FSWriteError::JSONSeserializationError {
                path: facilities_file.to_display(),
                source,
            }
        })?;
        tokio::fs::write(&facilities_file, facilities_content)
            .await
            .map_err(|source| FSWriteError::WriteFileError {
                path: facilities_file.to_display(),
                source,
            })?;
        logging::info!("File written: {}", facilities_file.to_display());
        Ok(())
    }
}

// Items
impl FSData {
    pub async fn add_item(&self, item_id: i32) -> Result<(), FSError> {
        let mut items = self.load_items().await?;
        if !items.items.contains(&item_id) {
            items.items.push(item_id);
            self.save_items(&items).await?;
        }
        Ok(())
    }

    pub async fn rm_item(&self, item_id: i32) -> Result<(), FSError> {
        let mut items = self.load_items().await?;

        let index = items.items.iter().position(|id| id == &item_id);
        if let Some(i) = index {
            items.items.remove(i);
        }

        self.save_items(&items).await?;
        Ok(())
    }

    pub async fn load_items(&self) -> Result<Items, FSReadError> {
        let mut items_file = self.data_directory.clone();
        items_file.push("items_data.json");
        if !items_file.exists() {
            return Ok(Items { items: vec![] });
        }
        let items_content = tokio::fs::read_to_string(&items_file)
            .await
            .map_err(|source| FSReadError::ReadFile {
                path: items_file.to_display(),
                source,
            })?;
        let items: Items = serde_json::from_str(&items_content).map_err(|source| {
            FSReadError::JSONDeserialization {
                path: items_file.to_display(),
                source,
            }
        })?;
        Ok(items)
    }

    async fn save_items(&self, items: &Items) -> Result<(), FSWriteError> {
        let mut items_file = self.data_directory.clone();
        items_file.push("items_data.json");
        let items_content = serde_json::to_string_pretty(&items).map_err(|source| {
            FSWriteError::JSONSeserializationError {
                path: items_file.to_display(),
                source,
            }
        })?;
        tokio::fs::write(&items_file, items_content)
            .await
            .map_err(|source| FSWriteError::WriteFileError {
                path: items_file.to_display(),
                source,
            })?;
        logging::info!("File written: {}", &items_file.to_display());
        Ok(())
    }
}

// Refresh Token
impl FSData {
    pub fn load_refresh_token(&self) -> Result<Option<String>, FSReadError> {
        let mut refresh_token_store_path = self.data_directory.clone();
        refresh_token_store_path.push("refresh_token_data.json");
        if !std::fs::try_exists(&refresh_token_store_path).map_err(|e| {
            FSReadError::CheckFileExistence {
                path: refresh_token_store_path.to_display(),
                source: e,
            }
        })? {
            return Ok(None);
        }
        let refresh_token_store_str =
            std::fs::read_to_string(&refresh_token_store_path).map_err(|source| {
                FSReadError::ReadFile {
                    path: refresh_token_store_path.to_display(),
                    source,
                }
            })?;
        let refresh_token_store: RefreshTokenStore = serde_json::from_str(&refresh_token_store_str)
            .map_err(|e| FSReadError::JSONDeserialization {
                path: refresh_token_store_path.to_display(),
                source: e,
            })?;
        Ok(Some(refresh_token_store.refresh_token))
    }

    pub async fn save_refresh_token(&self, token: &RefreshToken) -> Result<(), FSWriteError> {
        let mut refresh_token_store_path = self.data_directory.clone();
        refresh_token_store_path.push("refresh_token_data.json");

        let refresh_token_store_str = serde_json::to_string(&RefreshTokenStore {
            refresh_token: token.clone(),
        })
        .map_err(|e| FSWriteError::JSONSeserializationError {
            path: refresh_token_store_path.to_display(),
            source: e,
        })?;

        logging::debug!(
            "Save refresh token to {}",
            refresh_token_store_path.to_display()
        );
        tokio::fs::write(&refresh_token_store_path, refresh_token_store_str)
            .await
            .map_err(|e| FSWriteError::WriteFileError {
                path: refresh_token_store_path.to_display(),
                source: e,
            })?;
        Ok(())
    }

    pub async fn delete_refresh_token(&self) -> Result<(), FSDeleteError> {
        let mut refresh_token_store_path = self.data_directory.clone();
        refresh_token_store_path.push("refresh_token_data.json");

        logging::debug!(
            "Delete refresh token from {}",
            refresh_token_store_path.to_display()
        );
        let result = tokio::fs::remove_file(&refresh_token_store_path).await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(()),
                _ => Err(FSDeleteError::DeleteFileError {
                    path: refresh_token_store_path.to_display(),
                    source: e,
                }),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum SplitError {
    #[error("Path without extention is not valid ({path})")]
    InvalidPathToSplit { path: String },
    #[error("Line to add to struct but no current key detected ({path}:{line})")]
    SplittedFileInvalidReadNoCurrentKey { path: String, line: u64 },
    #[error("Directory could not be created ({path}): {source}")]
    DirectoryCreationFailed { path: String, source: io::Error },
    #[error("Could not read file ({path}:{line}): {source}")]
    FileReadFailed {
        path: String,
        line: u64,
        source: io::Error,
    },
    #[error("Could not convert string to Yaml: {source}")]
    StringToYamlConversionFailed { source: serde_yaml::Error },
    #[error("Could not convert Yaml to String: {source}")]
    YAMLToStringConversionFailed { source: serde_yaml::Error },
    #[error("File could not be written to destination ({path}): {source}")]
    FileWriteFailed { path: String, source: io::Error },
}

pub async fn split_into_subfiles(cache_dir: &Path, path: &str) -> Result<(), SplitError> {
    let full_path = cache_dir.join(path);
    let sde_directory_name = full_path
        .file_stem()
        .ok_or(SplitError::InvalidPathToSplit {
            path: full_path.to_display(),
        })?;
    let parent = full_path
        .parent()
        .expect("Full path should not be relative (or root)");
    let directory = parent.join(sde_directory_name);
    let directory_str = directory.to_str().unwrap_or("UNKNOWN").to_string();

    logging::debug!("Split {} into {}", full_path.to_display(), directory_str);

    tokio::fs::create_dir(&directory).await.map_err(|source| {
        SplitError::DirectoryCreationFailed {
            path: directory.to_str().unwrap_or("UNKNOWN").to_string(),
            source,
        }
    })?;

    let to_split =
        tokio::fs::File::open(&full_path)
            .await
            .map_err(|source| SplitError::FileReadFailed {
                path: full_path.to_display(),
                line: 0,
                source,
            })?;
    let reader = tokio::io::BufReader::new(to_split);

    let mut current_key = String::new();
    let mut current_content = Vec::new();

    let mut lines = reader.lines();

    let mut line_nb = 0;
    let new_line_regex = Regex::new(r"^[a-zA-Z0-9_]*:$").expect("Hardcoded regex should compile.");
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|source| SplitError::FileReadFailed {
            path: full_path.to_display(),
            line: line_nb,
            source,
        })?
    {
        line_nb += 1;

        let trimmed_line = line.trim();
        if trimmed_line.is_empty() {
            println!("Line {}: '{}' - Ignored", line_nb, trimmed_line);
            continue;
        }
        if new_line_regex.is_match(&line) {
            println!("Line {}: '{}' - New Key", line_nb, line);
            write_split_result(directory.clone(), &mut current_key, &mut current_content).await?;
            current_key = trimmed_line.trim_end_matches(':').to_string();
            continue;
        }
        if current_key.is_empty() {
            println!("Line {}: '{}' - Error", line_nb, line);
            return Err(SplitError::SplittedFileInvalidReadNoCurrentKey {
                path: full_path.to_display(),
                line: line_nb - 1,
            });
        }
        println!("Line {}: '{}' - Added", line_nb, line);
        current_content.push(line);
    }

    write_split_result(directory.clone(), &mut current_key, &mut current_content).await?;

    Ok(())
}

async fn write_split_result(
    directory: PathBuf,
    current_key: &mut String,
    current_content: &mut Vec<String>,
) -> Result<(), SplitError> {
    println!("Write {:?} into {}", current_content, current_key);
    if !current_key.is_empty() {
        let mut file = directory;
        file.push(format!("{current_key}.yaml").as_str());
        logging::trace!("Write {}", file.to_str().unwrap_or("UNKNOWN").to_string());

        let content = current_content.join("\n");

        let yaml_content: Value = serde_yaml::from_str(&content)
            .map_err(|source| SplitError::StringToYamlConversionFailed { source })?;
        let content = serde_yaml::to_string(&yaml_content)
            .map_err(|source| SplitError::YAMLToStringConversionFailed { source })?;

        tokio::fs::write(&file, &content)
            .await
            .map_err(|source| SplitError::FileWriteFailed {
                path: file.to_str().unwrap_or("UNKNOWN").to_string(),
                source,
            })?;

        current_key.clear();
        current_content.clear();
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Facilities {
    pub stations: Vec<NPCStation>,
    pub structures: Vec<PlayerStructure>,
}

pub enum FSFacilityType {
    NPCStation(NPCStation),
    PlayerStructure(PlayerStructure),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NPCStation {
    pub(crate) id: i32,
    pub usages: Vec<FacilityUsage>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PlayerStructure {
    pub id: i64,
    pub usages: Vec<FacilityUsage>,
    pub activities: HashMap<IndustryType, PlayerStructureStats>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Items {
    pub items: Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug)]
struct RefreshTokenStore {
    refresh_token: String,
}

#[cfg(test)]
pub mod testutils {
    use std::{fs, path::PathBuf};

    use super::FSData;

    pub fn create_test_fs_data() -> (FSData, PathBuf) {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        (FSData::new(data_directory.clone()), data_directory)
    }

    pub fn prewrite_facilities(data_directory: &PathBuf, content: &str) {
        let mut facilities_file = data_directory.clone();
        facilities_file.push("facilities_data.json");
        prewrite(&facilities_file, content);
    }

    pub fn prewrite_items(data_directory: &PathBuf, content: &str) {
        let mut items_file = data_directory.clone();
        items_file.push("items_data.json");
        prewrite(&items_file, content);
    }

    pub fn prewrite_refresh_token(data_directory: &PathBuf, content: &str) {
        let mut items_file = data_directory.clone();
        items_file.push("refresh_token_data.json");
        prewrite(&items_file, content);
    }

    pub fn prewrite(path: &PathBuf, content: &str) {
        fs::write(&path, content).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use tests::testutils::*;

    use super::*;

    #[tokio::test]
    async fn add_station_no_station_exist() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data
            .add_station(&NPCStation {
                id: 1,
                usages: vec![FacilityUsage::Market, FacilityUsage::Industry],
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 1);
        let station = &facilities.stations[0];
        assert_eq!(station.id, 1);
        assert_eq!(
            station.usages,
            vec![FacilityUsage::Market, FacilityUsage::Industry]
        );
    }

    #[tokio::test]
    async fn add_station_preexisting_stations() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [
              {
                "id": 1,
                "usages": [
                  "Market",
                  "Industry"
                ]
              }
            ],
            "structures": []
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data
            .add_station(&NPCStation {
                id: 2,
                usages: vec![FacilityUsage::Industry],
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 2);
        let station = &facilities.stations[1];
        assert_eq!(station.id, 2);
        assert_eq!(station.usages, vec![FacilityUsage::Industry]);
    }

    #[tokio::test]
    async fn add_station_handle_duplicate() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [
              {
                "id": 1,
                "usages": [
                  "Market",
                  "Industry"
                ]
              }
            ],
            "structures": []
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data
            .add_station(&NPCStation {
                id: 1,
                usages: vec![FacilityUsage::Industry],
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 1);
        let station = &facilities.stations[0];
        assert_eq!(station.id, 1);
        assert_eq!(
            station.usages,
            vec![FacilityUsage::Market, FacilityUsage::Industry]
        );
    }

    #[tokio::test]
    async fn rm_station() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [
              {
                "id": 1,
                "usages": [
                  "Market",
                  "Industry"
                ]
              }
            ],
            "structures": []
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_station(1).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 0);
    }

    #[tokio::test]
    async fn rm_station_empty() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [],
            "structures": []
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_station(0).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 0);
    }

    #[tokio::test]
    async fn rm_station_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data.rm_station(0).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.stations.len(), 0);
    }

    #[tokio::test]
    async fn add_structure_no_structure_exist() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        let mut activities = HashMap::new();
        activities.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.34,
                job_duration_modifier: Some(3.0),
                job_cost_modifier: Some(5.0),
                material_consumption_modifier: Some(2.5),
            },
        );
        activities.insert(
            IndustryType::Invention,
            PlayerStructureStats {
                tax_rate: 0.32,
                job_duration_modifier: None,
                job_cost_modifier: None,
                material_consumption_modifier: None,
            },
        );
        activities.insert(
            IndustryType::Copying,
            PlayerStructureStats {
                tax_rate: 0.92,
                job_duration_modifier: None,
                job_cost_modifier: None,
                material_consumption_modifier: None,
            },
        );
        fs_data
            .add_structure(&PlayerStructure {
                id: 1,
                usages: vec![FacilityUsage::Market, FacilityUsage::Industry],
                activities,
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 1);
        let structure = &facilities.structures[0];
        assert_eq!(structure.id, 1);
        assert_eq!(
            structure.usages,
            vec![FacilityUsage::Market, FacilityUsage::Industry]
        );

        let manufacturing = &structure.activities[&IndustryType::Manufacturing];
        assert_eq!(manufacturing.tax_rate, 0.34);
        assert_eq!(manufacturing.job_duration_modifier, Some(3.0));
        assert_eq!(manufacturing.job_cost_modifier, Some(5.0));
        assert_eq!(manufacturing.material_consumption_modifier, Some(2.5));

        let invention = &structure.activities[&IndustryType::Invention];
        assert_eq!(invention.tax_rate, 0.32);
        assert_eq!(invention.job_duration_modifier, None);
        assert_eq!(invention.job_cost_modifier, None);
        assert_eq!(invention.material_consumption_modifier, None);

        let copying = &structure.activities[&IndustryType::Copying];
        assert_eq!(copying.tax_rate, 0.92);
        assert_eq!(copying.job_duration_modifier, None);
        assert_eq!(copying.job_cost_modifier, None);
        assert_eq!(copying.material_consumption_modifier, None);
    }

    #[tokio::test]
    async fn add_structure_preexisting_exist() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [],
            "structures": [
              {
                "id": 1,
                "usages": [
                  "Market",
                  "Industry"
                ],
                "activities": {
                  "Manufacturing": {
                    "tax_rate": 0.34,
                    "job_duration_modifier": 3.0,
                    "job_cost_modifier": 5.0,
                    "material_consumption_modifier": 2.5
                  },
                  "Invention": {
                    "tax_rate": 0.32,
                    "job_duration_modifier": null,
                    "job_cost_modifier": null,
                    "material_consumption_modifier": null
                  },
                  "Copying": {
                    "tax_rate": 0.92,
                    "job_duration_modifier": null,
                    "job_cost_modifier": null,
                    "material_consumption_modifier": null
                  }
                }
              }
            ]
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        let mut activities = HashMap::new();
        activities.insert(
            IndustryType::Manufacturing,
            PlayerStructureStats {
                tax_rate: 0.56,
                job_duration_modifier: Some(3.0),
                job_cost_modifier: Some(5.0),
                material_consumption_modifier: Some(2.5),
            },
        );
        activities.insert(
            IndustryType::Invention,
            PlayerStructureStats {
                tax_rate: 0.57,
                job_duration_modifier: None,
                job_cost_modifier: None,
                material_consumption_modifier: None,
            },
        );
        activities.insert(
            IndustryType::Copying,
            PlayerStructureStats {
                tax_rate: 0.58,
                job_duration_modifier: None,
                job_cost_modifier: None,
                material_consumption_modifier: None,
            },
        );
        fs_data
            .add_structure(&PlayerStructure {
                id: 2,
                usages: vec![FacilityUsage::Industry],
                activities,
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 2);
        let structure = &facilities.structures[1];
        assert_eq!(structure.id, 2);
        assert_eq!(structure.usages, vec![FacilityUsage::Industry]);

        let manufacturing = &structure.activities[&IndustryType::Manufacturing];
        assert_eq!(manufacturing.tax_rate, 0.56);
        assert_eq!(manufacturing.job_duration_modifier, Some(3.0));
        assert_eq!(manufacturing.job_cost_modifier, Some(5.0));
        assert_eq!(manufacturing.material_consumption_modifier, Some(2.5));

        let invention = &structure.activities[&IndustryType::Invention];
        assert_eq!(invention.tax_rate, 0.57);
        assert_eq!(invention.job_duration_modifier, None);
        assert_eq!(invention.job_cost_modifier, None);
        assert_eq!(invention.material_consumption_modifier, None);

        let copying = &structure.activities[&IndustryType::Copying];
        assert_eq!(copying.tax_rate, 0.58);
        assert_eq!(copying.job_duration_modifier, None);
        assert_eq!(copying.job_cost_modifier, None);
        assert_eq!(copying.material_consumption_modifier, None);
    }

    #[tokio::test]
    async fn add_structure_handle_duplicate() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [],
            "structures": [
              {
                "id": 1,
                "usages": [
                  "Market",
                  "Industry"
                ],
                "activities": {
                  "Manufacturing": {
                    "tax_rate": 0.34,
                    "job_duration_modifier": 3.0,
                    "job_cost_modifier": 5.0,
                    "material_consumption_modifier": 2.5
                  },
                  "Invention": {
                    "tax_rate": 0.32,
                    "job_duration_modifier": null,
                    "job_cost_modifier": null,
                    "material_consumption_modifier": null
                  },
                  "Copying": {
                    "tax_rate": 0.92,
                    "job_duration_modifier": null,
                    "job_cost_modifier": null,
                    "material_consumption_modifier": null
                  }
                }
              }
            ]
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data
            .add_structure(&PlayerStructure {
                id: 1,
                usages: vec![FacilityUsage::Industry],
                activities: HashMap::new(),
            })
            .await
            .unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 1);
    }

    #[tokio::test]
    async fn rm_structure() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
                "stations": [],
                "structures": [
                  {
                    "id": 1,
                    "usages": [
                      "Market",
                      "Industry"
                    ],
                    "activities": {
                      "Manufacturing": {
                        "tax_rate": 0.34,
                        "job_duration_modifier": 3.0,
                        "job_cost_modifier": 5.0,
                        "material_consumption_modifier": 2.5
                      },
                      "Invention": {
                        "tax_rate": 0.32,
                        "job_duration_modifier": null,
                        "job_cost_modifier": null,
                        "material_consumption_modifier": null
                      },
                      "Copying": {
                        "tax_rate": 0.92,
                        "job_duration_modifier": null,
                        "job_cost_modifier": null,
                        "material_consumption_modifier": null
                      }
                    }
                  }
                ]
              }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_structure(1).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 0);
    }

    #[tokio::test]
    async fn rm_structure_empty() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_facilities(
            &data_directory,
            r#"{
            "stations": [],
            "structures": []
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_structure(0).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 0);
    }

    #[tokio::test]
    async fn rm_structure_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data.rm_structure(0).await.unwrap();

        let facilities = fs_data.load_facilities().await.unwrap();

        assert_eq!(facilities.structures.len(), 0);
    }

    #[tokio::test]
    async fn add_items_no_item_exist() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data.add_item(12345).await.unwrap();

        let items = fs_data.load_items().await.unwrap();

        assert_eq!(items.items.len(), 1);
        assert_eq!(items.items[0], 12345);
    }

    #[tokio::test]
    async fn add_item_preexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_items(
            &data_directory,
            r#"{
                "items": [
                  1236
                ]
              }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.add_item(12345).await.unwrap();

        let items = fs_data.load_items().await.unwrap();

        println!("{:?}", items);
        assert_eq!(items.items.len(), 2);
        assert_eq!(items.items[1], 12345);
    }

    #[tokio::test]
    async fn add_item_handle_duplicate() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_items(
            &data_directory,
            r#"{
            "items": [
              1236
            ]
          }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.add_item(1236).await.unwrap();

        let items = fs_data.load_items().await.unwrap();
        assert_eq!(items.items.len(), 1);
    }

    #[tokio::test]
    async fn rm_items() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_items(
            &data_directory,
            r#"{
                "items": [
                  1236
                ]
              }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_item(1236).await.unwrap();

        let items = fs_data.load_items().await.unwrap();

        assert_eq!(items.items.len(), 0);
    }

    #[tokio::test]
    async fn rm_item_empty() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_items(
            &data_directory,
            r#"{
                "items": [
                ]
              }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data.rm_item(1236).await.unwrap();

        let items = fs_data.load_items().await.unwrap();

        assert_eq!(items.items.len(), 0);
    }

    #[tokio::test]
    async fn rm_item_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data.rm_item(1).await.unwrap();

        let items = fs_data.load_items().await.unwrap();

        assert_eq!(items.items.len(), 0);
    }

    #[tokio::test]
    async fn load_refresh_token_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        let fs_data = FSData::new(data_directory);
        let token = fs_data.load_refresh_token().unwrap();

        assert_eq!(token, None);
    }

    #[tokio::test]
    async fn load_refresh_token() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_refresh_token(
            &data_directory,
            r#"{
            "refresh_token": "abcd1234"
        }"#,
        );

        let fs_data = FSData::new(data_directory);
        let token = fs_data.load_refresh_token().unwrap();

        assert_eq!(token, Some("abcd1234".to_string()));
    }

    #[tokio::test]
    async fn save_refresh_token_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let fs_data = FSData::new(data_directory);
        fs_data
            .save_refresh_token(&"abcde12345".to_string())
            .await
            .unwrap();

        let token = fs_data.load_refresh_token().unwrap();
        assert_eq!(token, Some("abcde12345".to_string()));
    }

    #[tokio::test]
    async fn save_refresh_token_overwritite() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_refresh_token(
            &data_directory,
            r#"{
            "refresh_token": "abcd1234"
        }"#,
        );

        let fs_data = FSData::new(data_directory);
        fs_data
            .save_refresh_token(&"abcde12345".to_string())
            .await
            .unwrap();

        let token = fs_data.load_refresh_token().unwrap();
        assert_eq!(token, Some("abcde12345".to_string()));
    }

    #[tokio::test]
    async fn delete_refresh_token_inexisting() {
        let data_directory = tempfile::tempdir().unwrap().into_path();

        let mut token_path = data_directory.clone();
        token_path.push("refresh_token_data.json");

        let fs_data = FSData::new(data_directory);
        fs_data.delete_refresh_token().await.unwrap();

        assert!(!token_path.exists())
    }

    #[tokio::test]
    async fn delete_refresh_token() {
        let data_directory = tempfile::tempdir().unwrap().into_path();
        prewrite_refresh_token(
            &data_directory,
            r#"{
            "refresh_token": "abcd1234"
        }"#,
        );

        let mut token_path = data_directory.clone();
        token_path.push("refresh_token_data.json");

        let fs_data = FSData::new(data_directory);
        fs_data.delete_refresh_token().await.unwrap();

        assert!(!token_path.exists())
    }

    #[tokio::test]
    async fn test_split_into_subfiles() {
        let directory = tempfile::tempdir().unwrap().into_path();

        // Don't change formatting ! Space & Tabs are important !
        prewrite(
            &directory.join("to_split.yml"),
            r#"
a:
  b:
    c:
      d: abcd
e:
  f: ef
  g: eh
  h:
    o: eho
  t: et
1:
  2:
    3: 123
    4: 124
        "#,
        );

        split_into_subfiles(&directory, "to_split.yml")
            .await
            .unwrap();

        let file_a = directory.join("to_split/a.yaml");
        let file_e = directory.join("to_split/e.yaml");
        let file_1 = directory.join("to_split/1.yaml");

        assert!(file_a.exists());
        assert!(file_e.exists());
        assert!(file_1.exists());

        let file_a_content = tokio::fs::read_to_string(file_a).await.unwrap();
        let file_e_content = tokio::fs::read_to_string(file_e).await.unwrap();
        let file_1_content = tokio::fs::read_to_string(file_1).await.unwrap();
        // Don't change formatting ! Space & Tabs are important !
        assert_eq!(
            file_a_content,
            r#"b:
  c:
    d: abcd
"#
        );
        assert_eq!(
            file_e_content,
            r#"f: ef
g: eh
h:
  o: eho
t: et
"#
        );
        assert_eq!(
            file_1_content,
            r#"2:
  3: 123
  4: 124
"#
        );
    }
}
