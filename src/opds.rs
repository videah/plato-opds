//! Contains the structures for parsing OPDS feeds.

use serde::{Deserialize, Serialize};

use crate::LinkType;

/// Holds the settings for a single instance of a server.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Instance {
    /// A URL string pointing to an OPDS feed.
    pub url: String,
    /// Optional username for basic authentication to the server.
    pub username: Option<String>,
    /// Optional password for basic authentication to the server.
    pub password: Option<String>,
}

/// The structure of an OPDS feed.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    /// List of book entries in the feed.
    #[serde(rename = "entry")]
    pub entries: Vec<Entry>,
    /// List of links in the feed.
    #[serde(rename = "link")]
    pub links: Vec<Link>,
}

/// The structure of an OPDS feed entry. Usually represents a book.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// The title of the book.
    pub title: String,
    /// The unique identifier of the book.
    pub id: String,
    /// The authors of the book.
    #[serde(rename = "author")]
    pub authors: Option<Vec<Author>>,
    /// The publisher of the book.
    #[serde(rename = "publisher")]
    pub publishers: Option<Vec<Publisher>>,
    /// The links to the book's resources. Usually contains a link to the book files.
    #[serde(rename = "link")]
    pub links: Option<Vec<Link>>,
}

/// The author listed in an OPDS feed entry.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    /// The name of the author.
    pub name: String,
}

/// The publisher listed in an OPDS feed entry.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Publisher {
    /// The name of the publisher.
    pub name: String,
}

/// A link to a resource in an OPDS feed entry.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    #[serde(rename = "@rel")]
    pub rel: Option<LinkType>,
    #[serde(rename = "@href")]
    pub href: Option<String>,
    #[serde(rename = "@type")]
    pub file_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test parsing an OPDS entry for Frank Herbert's Dune.
    #[test]
    fn parse_entry() {
        let xml = include_str!("../tests/entry.xml");
        let entry = quick_xml::de::from_str::<Entry>(&xml);
        assert!(entry.is_ok());

        let entry = entry.unwrap();
        assert_eq!(entry.title, "Dune");
        assert_eq!(entry.id, "urn:uuid:56e99d4d-bef9-445e-8162-35aaef306006");
        assert_eq!(entry.authors.unwrap()[0].name, "Frank Herbert");
        assert_eq!(
            entry.publishers.unwrap()[0].name,
            "Penguin Publishing Group"
        );
    }
}
