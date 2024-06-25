mod opds;
mod plato;

use std::{
    collections::HashMap,
    env,
    fmt::Display,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{format_err, Context, Error};
use chrono::{Datelike, Local, Utc};
use reqwest::blocking::Client;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::json;
use url::Url;

use crate::opds::{Entry, Feed, Instance, Link};

const SETTINGS_PATH: &str = "Settings.toml";

/// Holds the settings for the application converted from a TOML file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Settings {
    /// Mapping of server names to their respective [Instance] settings.
    servers: HashMap<String, Instance>,
    /// List of preferred file types to download (i.e. application/x-cbz or application/pdf).
    preferred_file_types: Vec<String>,
    /// Whether files should be placed in a directory named after the server they have been pulled
    /// from.
    use_server_name_directories: bool,
    /// Whether files should be placed in directories based on their file type, giving more
    /// organization in Plato's UI. How these folders should be mapped to a name can be
    /// configured in the `organization` table.
    organize_by_file_type: bool,
    /// Mapping of file extensions to directory names. Used when `organize_by_file_type` is true.
    /// Key's are file extensions and values are the directory names they should be placed in.
    ///
    /// ## Example
    /// ```toml
    /// [organization]
    /// epub = "Books"
    /// cbz = "Manga"
    /// pdf = "Documents"
    /// ```
    organization: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            servers: HashMap::new(),
            preferred_file_types: vec!["application/epub+zip".to_string()],
            use_server_name_directories: true,
            organize_by_file_type: true,
            organization: {
                let mut map = HashMap::new();
                map.insert("epub".to_string(), "Books".to_string());
                map.insert("cbz".to_string(), "Comics".to_string());
                map.insert("pdf".to_string(), "Documents".to_string());
                map
            },
        }
    }
}

/// The type of file in a link. Used to easily convert between MIME types and file extensions.
#[derive(PartialEq, Debug, Clone)]
enum FileType {
    /// The file is an EPUB, likely a book.
    Epub,
    /// The file is a CBZ, likely a comic or manga.
    Cbz,
    /// The file is a PDF, likely a document.
    Pdf,
    /// The file is of an unknown type. Contains the MIME type.
    Other(String),
}

impl FromStr for FileType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "application/epub+zip" => Ok(FileType::Epub),
            "application/x-cbz" => Ok(FileType::Cbz),
            "application/pdf" => Ok(FileType::Pdf),
            _ => Ok(FileType::Other(s.to_string())),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Serialize)]
enum LinkType {
    Acquisition,
    Cover,
    Thumbnail,
    Sample,
    OpenAccess,
    Borrow,
    Buy,
    Subscribe,
    /// The next page of a paginated feed.
    Next,
    Other(String),
}

impl FromStr for LinkType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http://opds-spec.org/acquisition" => Ok(LinkType::Acquisition),
            "http://opds-spec.org/image" => Ok(LinkType::Cover),
            "http://opds-spec.org/image/thumbnail" => Ok(LinkType::Thumbnail),
            "http://opds-spec.org/acquisition/sample" => Ok(LinkType::Sample),
            "http://opds-spec.org/acquisition/preview" => Ok(LinkType::Sample),
            "http://opds-spec.org/acquisition/open-access" => Ok(LinkType::OpenAccess),
            "http://opds-spec.org/acquisition/borrow" => Ok(LinkType::Borrow),
            "http://opds-spec.org/acquisition/buy" => Ok(LinkType::Buy),
            "http://opds-spec.org/acquisition/subscribe" => Ok(LinkType::Subscribe),
            "next" => Ok(LinkType::Next),
            _ => Ok(LinkType::Other(s.to_string())),
        }
    }
}

impl<'de> Deserialize<'de> for LinkType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

struct EntryResult {
    pub link: Link,
    pub file_extension: FileExtension,
    pub entry: Entry,
    pub save_path: PathBuf,
}

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
enum FileExtension {
    Epub,
    Cbz,
    Pdf,
    Other(String),
}

impl FromStr for FileExtension {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "epub" => Ok(FileExtension::Epub),
            "cbz" => Ok(FileExtension::Cbz),
            "pdf" => Ok(FileExtension::Pdf),
            _ => Ok(FileExtension::Other(s.to_string())),
        }
    }
}

impl Display for FileExtension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match *self {
            FileExtension::Epub => "epub".to_string(),
            FileExtension::Cbz => "cbz".to_string(),
            FileExtension::Pdf => "pdf".to_string(),
            FileExtension::Other(ref s) => s.to_string(),
        };
        write!(f, "{}", str)
    }
}

impl From<&FileType> for FileExtension {
    fn from(file_type: &FileType) -> Self {
        match file_type {
            FileType::Epub => FileExtension::Epub,
            FileType::Cbz => FileExtension::Cbz,
            FileType::Pdf => FileExtension::Pdf,
            FileType::Other(s) => FileExtension::Other(s.clone()),
        }
    }
}

fn print_sync_notification(server_name: &String, results: &[EntryResult]) {
    if results.is_empty() {
        return;
    }

    plato::show_notification(&format!(
        "Downloading {} new documents found on '{}'",
        results.len(),
        server_name
    ));

    // Iterate over each result's file type and count up each instance so we can
    // display the number of each type of file that's being downloaded.
    results
        .iter()
        .fold(HashMap::new(), |mut map, result| {
            *map.entry(result.file_extension.clone()).or_insert(0) += 1;
            map
        })
        .iter()
        .for_each(|(file_extension, count)| {
            plato::show_notification(&format!(
                "Downloading {} new {}'s",
                count,
                file_extension.to_string()
            ));
        });
}

fn load_and_process_opds() -> Result<(), Error> {
    let mut args = env::args().skip(1);
    let library_path = PathBuf::from(
        args.next()
            .ok_or_else(|| format_err!("missing argument: library path"))?,
    );
    let save_path = PathBuf::from(
        args.next()
            .ok_or_else(|| format_err!("missing argument: save path"))?,
    );
    let wifi = args
        .next()
        .ok_or_else(|| format_err!("missing argument: wifi status"))
        .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let online = args
        .next()
        .ok_or_else(|| format_err!("missing argument: online status"))
        .and_then(|v| v.parse::<bool>().map_err(Into::into))?;
    let settings: Settings = load_toml::<Settings, _>(SETTINGS_PATH)
        .with_context(|| format!("can't load settings from {}", SETTINGS_PATH))?;

    if !online {
        if !wifi {
            plato::show_notification("Establishing a network connection.");
            plato::set_wifi(true);
        } else {
            plato::show_notification("Waiting for the network to come up.");
        }
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;
    }

    if !save_path.exists() {
        fs::create_dir(&save_path)?;
    }

    let client = Client::builder().user_agent("Plato-OPDS/0.1.0").build()?;
    let sigterm = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&sigterm))?;

    // Create directory for each instance name in the save path.
    if settings.use_server_name_directories {
        for name in settings.servers.keys() {
            let instance_path = save_path.join(name);
            if !instance_path.exists() {
                fs::create_dir(&instance_path)?;
            }
        }
    }

    for (name, instance) in &settings.servers {
        if sigterm.load(Ordering::Relaxed) {
            break;
        }

        let instance_path = save_path.join(name);
        let username = &instance.username.clone().unwrap_or("admin".to_string());
        let password = instance.password.as_ref();

        let response = client
            .get(&instance.url)
            .basic_auth(username, password)
            .send()?;

        let xml = response.text()?;
        let mut feed = quick_xml::de::from_str::<Feed>(&xml)?;

        // Check if a `next` link exists, if so the catalog is paginated, and we need to crawl until
        // it doesn't exist.
        while let Some(next_link) = feed
            .links
            .iter()
            .find(|link| link.rel == Some(LinkType::Next))
        {
            // If the next link is relative, we need to attach it to the instance url.
            let url_string = next_link.href.clone().expect("Paginated link is empty");
            let url = match url_string.starts_with('/') {
                true => {
                    let url = Url::parse(&instance.url)?;
                    let host = url.host_str().expect("No host in instance url");
                    let new_url = format!("{}://{}{}", url.scheme(), host, url_string);

                    Url::parse(&new_url).expect("Can't parse paginated url")
                }
                false => Url::parse(&url_string).expect("Can't parse paginated url"),
            };

            let response = client.get(url).basic_auth(username, password).send()?;

            let xml = response.text()?;
            let next_feed = quick_xml::de::from_str::<Feed>(&xml)?;
            feed.entries.extend(next_feed.entries);
            feed.links = next_feed.links;
        }

        let results: Vec<EntryResult> = feed
            .entries
            .into_iter()
            .filter_map(|entry| {
                let file_types = settings.preferred_file_types.clone();

                let link = file_types
                    .into_iter()
                    .find_map(|file_type| {
                        entry.links.clone().into_iter().flatten().find(|link| {
                            link.rel == Some(LinkType::Acquisition)
                                && link.file_type == Some(file_type.clone())
                        })
                    })
                    .ok_or_else(|| format_err!("no acquisition link found"));

                // Strip 'urn:uuid:' prefix.
                let uuid = entry.id.strip_prefix("urn:uuid:")?;

                if let Err(err) = link {
                    plato::show_notification(&format!(
                        "Error downloading '{}': {:#}.",
                        entry.title, err
                    ));
                    return None;
                }

                // Get the file type of the link.
                let file_type_string = link.as_ref().ok()?.file_type.clone()?;
                let file_type = FileType::from_str(&file_type_string).ok()?;
                let file_extension = FileExtension::from(&file_type);
                let file_name = format!("{}.{}", uuid, file_extension.to_string());

                // If the 'user_server_name_directories' setting is true, we set the file
                // path to a directory named after the server name. Otherwise, we stick it in
                // the root of the save path.
                println!(
                    "use_server_name_directories: {:?}",
                    settings.use_server_name_directories
                );
                let mut doc_path = if settings.use_server_name_directories {
                    save_path.clone()
                } else {
                    instance_path.clone()
                };

                // If the 'organize-by-file-type' setting is true, we set the file path
                // to include a folder mapped from the file extension to a value set in
                // 'organization'. If there's no value for the extension, we just
                // use the root of the save path.
                doc_path = if settings.organize_by_file_type {
                    let extension = file_extension.to_string();

                    match settings.organization.get(&extension) {
                        Some(directory) => {
                            let organized_path = doc_path.join(directory);
                            if !organized_path.exists() {
                                fs::create_dir_all(&organized_path).ok()?
                            }
                            organized_path
                        }
                        None => doc_path,
                    }
                } else {
                    doc_path
                };

                doc_path = doc_path.join(file_name);

                if doc_path.exists() {
                    return None;
                }

                Some(EntryResult {
                    link: link.ok()?,
                    file_extension,
                    entry,
                    save_path: doc_path,
                })
            })
            .collect();

        print_sync_notification(name, &results);
        let is_empty = results.is_empty();

        for result in results {
            if sigterm.load(Ordering::Relaxed) {
                break;
            }

            let doc_path = result.save_path;
            if doc_path.exists() {
                continue;
            }

            let mut file = File::create(&doc_path)?;
            let mut url = Url::parse(&instance.url)?;
            url.set_path(&result.link.href.ok_or(format_err!(
                "no href found for link in '{}'",
                result.entry.title
            ))?);

            let response = client
                .get(url)
                .basic_auth(username, password)
                .send()
                .and_then(|mut response| response.copy_to(&mut file));

            if let Err(err) = response {
                plato::show_notification(&format!(
                    "Error downloading '{}': {:#}.",
                    result.entry.title, err
                ));
                fs::remove_file(doc_path).ok();
                continue;
            }

            if let Ok(path) = doc_path.strip_prefix(&library_path) {
                let file_info = json!({
                    "path": path,
                    "kind": result.file_extension.to_string(),
                    "size": file.metadata().ok().map_or(0, |m| m.len()),
                });

                // If there's an author, get the first one. Otherwise, use 'Unknown Author'.
                let author = result
                    .entry
                    .authors
                    .into_iter()
                    .flat_map(|authors| authors.into_iter())
                    .next()
                    .map_or("Unknown Author".to_string(), |author| author.name);

                let year = match result.entry.published {
                    Some(date) => date.year().to_string(),
                    None => "".to_string(),
                };

                // Get the current time.
                let updated_at = Utc::now();

                let mut read_state = json!({
                    "opened": updated_at.with_timezone(&Local)
                                       .format("%Y-%m-%d %H:%M:%S")
                                       .to_string(),
                    "currentPage": 0,
                    "PagesCount": 1,
                    "finished": false,
                    "dithered": "false"
                });

                if instance.url.contains("/readbooks") {
                    *read_state.pointer_mut("/finished").unwrap() = true.into();
                }

                let info = json!({
                    "title": result.entry.title,
                    "author": author,
                    "year": year,
                    "identifier": result.entry.id,
                    "added": updated_at.with_timezone(&Local)
                                       .format("%Y-%m-%d %H:%M:%S")
                                       .to_string(),
                    "file": file_info,
                    "reader": read_state
                });

                plato::add_document(info);
            }
        }

        if !is_empty {
            plato::show_notification(&format!("Finished syncing with '{}'", name));
        }
    }

    Ok(())
}

fn main() -> Result<(), Error> {
    log_panics::init();
    if let Err(err) = load_and_process_opds() {
        eprintln!("Error: {:#}", err);
        plato::show_notification(&format!("Error: {err}"));
        fs::write("opds_error.txt", format!("{:#}", err)).ok();
        return Err(err);
    }

    Ok(())
}

pub fn load_toml<T, P: AsRef<Path>>(path: P) -> Result<T, Error>
where
    for<'a> T: Deserialize<'a>,
{
    let s = fs::read_to_string(path.as_ref())
        .with_context(|| format!("can't read file {}", path.as_ref().display()))?;
    toml::from_str(&s)
        .with_context(|| format!("can't parse TOML content from {}", path.as_ref().display()))
        .map_err(Into::into)
}
