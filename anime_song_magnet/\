mod models;

use anyhow::*;
use clap::*;
use directories::ProjectDirs;
use mal_api::prelude::*;
use nyaa_si::Client as NyaaClient;
use nyaa_si::*;
use std::collections::{HashMap, HashSet};
use chrono::prelude::*;

const DATA_FILE: &str = "data.json";

enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

#[derive(Serialize, Deserialize)]
struct Data {
    #[serde(default)]
    anime: HashSet<Anime>,
    #[serde(default)]
    nyaa: HashSet<Torrent>,
    #[serde(default)]
    season: HashMap<Season, Vec<String>>,
    #[serde(default)]
    update_on: DateTime<Utc>,
}

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long, short)]
    /// Specific user to get the list from
    user: String,

    #[arg(long, short)]
    /// Specific anime season to get from
    season: Option<Season>,

    #[arg(long, short)]
    /// Specific year anime to get from
    year: Option<u16>,

    #[arg(long, short default_value_t = 7 * 24)]
    /// Refresh anime list after x hours
    /// Default to a week
    refesh_list_after: usize,
}

impl Args {
    #[tokio::main]
    pub async fn exec(&self) -> Result<()> {
        Client::new(self).exec()
    }
}

struct Client<'a> {
    data: Data,
    nyaa: NyaaClient,
    mal: AnimeApiClient,
    args: &'a Args,
}

impl Client<'a> {
    async fn new(args: &Args) -> Result<Self> {
        let Some(project_dirs) = ProjectDirs::from("", "tmokenc", "anime_song_magnet") else {
            bail!("Cannot get the project directory")
        };

        let data_dir = project_dirs.data_dir();
        let data_path = data.
        let nyaa = NyaaClient::new();
        let mal = AnimeApiClient::new();

        Ok(Self {
            nyaa,
            mal,
            args,
        })
    }

    async fn exec(&self) -> Result<()> {
        let anime_list = self.get_anime_list().await?;

        for anime in anime_list {
            let torrents = search_nyaa(&anime.title_jpn, NyaaCategoy::AudioLossless).await?;

            todo!();
        }

        Ok(())
    }

    async fn get_anime_list(&self) -> Result<Vec<Anime>> {

        let query = GetUserAnimeList::builder()
            .username(self.args.user)
            .enable_nsfw()
            .fields(mal_api::anime::all_common_fields())
            .build()?;


        let data = self.mal.get_user_anime_list(&query).await?;

        while let Some(result) = self.mal.next(&data).await? {
             todo!()
        }

        Ok(result)
    }

    async fn search_nyaa(
        &self,
        query: &str,
        category: impl Into<Option<NyaaCategory>>,
    ) -> Result<Vec<Torrent>> {
        let query = QueryBuilder::new().search(query).category(category).build();
        let res = CLIENT.get(&query).await.unwrap();

        todo!();

        Ok(())
    }

    async fn save_file()
}
