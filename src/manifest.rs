use anyhow::{Context, Result};
use console::Style;
use quick_xml::de::from_reader;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

impl Manifest {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).context("Could not open update.xml in provided path")?;
        from_reader(BufReader::new(file)).context("Could not parse update.xml")
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
            .filter_map(|r| {
                let country = country_map.remove(&r.id).map(Ok);
                if country.is_none() {
                    eprintln!(
                        "{}: No info found for country with id: {}\n(country will be skipped in integrity checks)",
                        Style::new().red().bold().apply_to("WARNING"),
                        Style::new().bold().apply_to(r.id)
                    )
                }
                country
            })
            .collect()
    }

    pub fn region_name(&self) -> &str {
        &self.drm_entry.sales_region.name
    }
}

impl Country {
    pub fn files(&self) -> impl Iterator<Item = ZipFile> {
        self.data_groups
            .iter()
            .map(|dg| ZipFile::new(format!("{}_{:02}.zip", self.id, dg.id), &dg.info))
            .chain(
                self.speech_recognition
                    .as_ref()
                    .map(|info| ZipFile::new(format!("{}_speech_recognition.zip", self.id), info)),
            )
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
    pub filename: String,
    pub packedsize: u64,
    pub md5: &'a str,
}

impl<'a> ZipFile<'a> {
    fn new(filename: String, info: &'a FileInfo) -> Self {
        ZipFile {
            filename,
            packedsize: info.packedsize.parse().expect("Could not parse packedsize"),
            md5: &info.md5,
        }
    }
}
