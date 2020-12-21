use anyhow::{anyhow, Context, Result};
use quick_xml::de::from_reader;
use rayon::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

impl Manifest {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).context("Could not open update.xml in provided path")?;
        Ok(from_reader(BufReader::new(file)).context("Could not parse update.xml")?)
    }

    pub fn countries(&self) -> Result<Vec<&Country>> {
        let mut country_map: HashMap<_, _> = self
            .drm_entry
            .map_catalog
            .regions
            .iter()
            .flat_map(|r| &r.regions)
            .map(|c| (c.id, c))
            .collect();
        self.drm_entry
            .sales_region
            .regions
            .iter()
            .map(|r| {
                country_map.remove(&r.id).ok_or_else(|| {
                    anyhow!("Could not link country with id: {} in update.xml", r.id)
                })
            })
            .collect()
    }

    pub fn region_name(&self) -> &str {
        &self.drm_entry.sales_region.name
    }
}

impl Country {
    pub fn file_count(&self) -> u64 {
        let opt_sr = if self.speech_recognition.is_some() {
            1
        } else {
            0
        };
        self.data_groups.len() as u64 + opt_sr
    }
    pub fn files(&self) -> impl ParallelIterator<Item = ZipFile> {
        self.data_groups
            .par_iter()
            .map(move |dg| ZipFile {
                filename: format!("{}_{:02}.zip", self.id, dg.id),
                info: &dg.info,
            })
            .chain(self.speech_recognition.as_ref().map(|info| ZipFile {
                filename: format!("{}_speech_recognition.zip", self.id),
                info,
            }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    drm_entry: DrmEntry,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DrmEntry {
    map_catalog: MapCatalog,
    sales_region: SalesRegion,
}

#[derive(Debug, Deserialize)]
struct MapCatalog {
    #[serde(rename = "region")]
    regions: Vec<Continent>,
}

#[derive(Debug, Deserialize)]
struct SalesRegion {
    name: String,

    #[serde(rename = "region")]
    regions: Vec<Region>,
}

#[derive(Debug, Deserialize)]
struct Region {
    id: u32,
}

#[derive(Debug, Deserialize)]
struct Continent {
    #[serde(rename = "region")]
    regions: Vec<Country>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    id: u32,
    pub name: String,
    #[serde(rename = "dataGroup")]
    data_groups: Vec<DataGroup>,
    speech_recognition: Option<FileInfo>,
}

#[derive(Debug, Deserialize)]
pub struct FileInfo {
    pub unpackedsize: String,
    pub packedsize: String,
    pub md5: String,
}

#[derive(Debug, Deserialize)]
struct DataGroup {
    #[serde(flatten)]
    info: FileInfo,
    id: u32,
}

pub struct ZipFile<'a> {
    info: &'a FileInfo,
    pub filename: String,
}

impl<'a> ZipFile<'a> {
    pub fn md5(&self) -> &str {
        &self.info.md5
    }

    pub fn packedsize(&self) -> u64 {
        self.info
            .packedsize
            .parse()
            .expect("Could not parse packedsize")
    }
}
