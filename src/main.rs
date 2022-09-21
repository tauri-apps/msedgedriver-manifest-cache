use std::{
    collections::HashMap,
    env,
    fs::{create_dir, remove_dir_all, write},
    io::Error as IoError,
    path::{Path, PathBuf},
    process::exit,
};

use anyhow::Result;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};

const MANIFEST_URL: &str = "https://msedgedriver.azureedge.net";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
const DIST: &str = "dist";

#[derive(Debug, Serialize, Hash, PartialEq, Eq)]
#[serde(transparent)]
struct Version(String);

#[derive(Debug, Serialize, Hash, PartialEq, Eq)]
#[serde(transparent)]
struct Platform(String);

#[derive(Debug, Default)]
struct Output(HashMap<Version, HashMap<Platform, Properties>>);

#[derive(Debug, Default, Deserialize)]
struct EnumerationResults {
    #[serde(rename = "Blobs", default)]
    blobs: Blobs,
}

#[derive(Debug, Default, Deserialize)]

struct Blobs {
    #[serde(rename = "Blob", default)]
    blobs: Vec<Blob>,
}

#[derive(Debug, Default, Deserialize)]

struct Blob {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Url", default)]
    url: String,
    #[serde(rename = "Properties", default)]
    properties: BlobProperties,
}

#[derive(Debug, Default, Deserialize)]

struct BlobProperties {
    #[serde(rename = "Last-Modified", default)]
    last_modified: String,
    #[serde(rename = "Etag", default)]
    etag: String,
    #[serde(rename = "Content-Length", default)]
    content_length: String,
    #[serde(rename = "Content-Type", default)]
    content_type: String,
    #[serde(rename = "Content-MD5", default)]
    content_md5: String,
}

#[derive(Debug, Default, Serialize)]
struct Properties {
    url: String,
    #[serde(rename = "lastModified")]
    last_modified: String,
    etag: String,
    md5: String,
    #[serde(rename = "contentLength")]
    content_length: String,
    #[serde(rename = "contentType", default)]
    content_type: String,
}

impl From<Blob> for Properties {
    fn from(blob: Blob) -> Self {
        Self {
            url: blob.url,
            last_modified: blob.properties.last_modified,
            etag: blob.properties.etag,
            md5: blob.properties.content_md5,
            content_length: blob.properties.content_length,
            content_type: blob.properties.content_type,
        }
    }
}

fn main() {
    if let Err(e) = run(env::current_dir()) {
        eprintln!("fatal error: {}", e);
        exit(1);
    }
}

fn run(cwd: Result<PathBuf, IoError>) -> Result<()> {
    let dist = cwd?.join(DIST);
    let versions = dist.join("versions");
    clean_dist_directory(&dist, &versions)?;

    let manifest = fetch_manifest_from_network()?;
    write(dist.join("manifest.xml"), manifest.as_bytes())?;

    let results: EnumerationResults = from_str(&manifest)?;

    // simple sanity check to make sure there *was* any results
    assert!(results.blobs.blobs.len() > 1);

    let output = results
        .blobs
        .blobs
        .into_iter()
        .fold(Output::default(), |mut acc, blob| {
            let (version, platform) = parse_version_and_platform(&blob.name).unwrap();
            let version = acc.0.entry(version).or_default();
            version.insert(platform, Properties::from(blob));

            acc
        });

    for (version, properties) in output.0 {
        let content = serde_json::to_string_pretty(&properties)?;
        write(
            versions.join(format!("{}.json", version.0)),
            content.as_bytes(),
        )?;
    }

    Ok(())
}

fn fetch_manifest_from_network() -> Result<String> {
    Ok(ureq::get(MANIFEST_URL)
        .set("User-Agent", USER_AGENT)
        .call()?
        .into_string()?)
}

fn clean_dist_directory(dist: &Path, versions: &Path) -> Result<()> {
    if dist.exists() {
        remove_dir_all(dist)?;
    }
    create_dir(dist)?;
    create_dir(versions)?;
    Ok(())
}

fn parse_version_and_platform(s: &str) -> Option<(Version, Platform)> {
    let mut sides = s.split('/');
    let version = Version(sides.next()?.to_string());
    let platform_raw = sides.next()?;

    if sides.next().is_some() {
        eprintln!("unknown version/platform format: {}", s);
        return None;
    }

    let platform = Platform(
        platform_raw
            .strip_prefix("edgedriver_")?
            .strip_suffix(".zip")?
            .to_string(),
    );

    Some((version, platform))
}

#[test]
fn version_and_platform() {
    let (version, platform) =
        parse_version_and_platform("100.0.1154.0/edgedriver_arm64.zip").unwrap();

    assert_eq!(version.0, "100.0.1154.0");
    assert_eq!(platform.0, "arm64");
}
