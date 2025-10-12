use crate::Season;
use chrono::*;
use nyaa_si::model::Torrent as NyaaTorrent;
use serde::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Anime {
    pub mal_id: String,
    pub title: String,
    pub title_jpn: Option<String>,
    pub altenative_titles: Vec<String>,
    pub season: Season,
    pub year: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Torrent {
    pub nyaa_id: String,
    pub title: String,
    pub magnet: Option<String>,
    pub date: DateTime<Utc>,
}

impl From<NyaaTorrent> for Torrent {
    fn from(nyaa: NyaaTorrent) -> Self {
        let id = nyaa.link.split("/").rev().next().expect("Some nyaa link");
        Self {
            nyaa_id: id,
            title: nyaa.title,
            magnet: nyaa.magnet_url,
            date: nyaa.date,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
impl Link {
    mal_id: String,
    nyaa_id: String,
}
