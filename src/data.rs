use std;
use std::fs::File;
use std::path::Path;
use std::hash::{Hash, Hasher};
use std::collections::HashMap;

use rss;
use serde_json;

use errors::*;

fn get_hash<T: Hash>(t: T) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::default();
    t.hash(&mut hasher);
    hasher.finish()
}

type FeedID = u64;
type SubscriberID = i64;

#[derive(Debug, Serialize, Deserialize)]
pub struct Feed {
    link: String,
    title: String,
    error_count: u32,
    hash_list: Vec<u64>,
    subscribers: Vec<SubscriberID>,
}

#[derive(Debug)]
pub struct Database {
    path: String,
    feeds: HashMap<FeedID, Feed>,
    subscribers: HashMap<SubscriberID, Vec<FeedID>>,
}

fn gen_hash_list(items: &[rss::Item]) -> Vec<u64> {
    items.iter()
        .map(|item| {
            item.guid
                .as_ref()
                .map(|guid| get_hash(&guid.value))
                .unwrap_or_else(|| {
                    let default = "".to_string();
                    let title = item.title.as_ref().unwrap_or(&default);
                    let link = item.link.as_ref().unwrap_or(&default);
                    let string = String::with_capacity(title.len() + link.len());
                    get_hash(string)
                })
        })
        .collect()
}

impl Database {
    pub fn create(path: &str) -> Result<Database> {
        let feeds: HashMap<FeedID, Feed> = HashMap::new();
        let subscribers: HashMap<SubscriberID, Vec<FeedID>> = HashMap::new();
        let mut result = Database {
            path: path.to_owned(),
            feeds: feeds,
            subscribers: subscribers,
        };

        result.save()?;

        Ok(result)
    }

    pub fn open(path: &str) -> Result<Database> {
        let p = Path::new(path);
        if p.exists() {
            let f = File::open(path).chain_err(|| ErrorKind::DatabaseOpen(path.to_owned()))?;
            let feeds_list: Vec<Feed> =
                serde_json::from_reader(&f).chain_err(|| ErrorKind::DatabaseFormat)?;

            let mut feeds: HashMap<FeedID, Feed> = HashMap::with_capacity(feeds_list.len());
            let mut subscribers: HashMap<SubscriberID, Vec<FeedID>> = HashMap::new();

            for feed in feeds_list {
                let feed_id = get_hash(&feed.link);
                for subscriber in &feed.subscribers {
                    if subscribers.contains_key(subscriber) {
                        subscribers.get_mut(subscriber).unwrap().push(feed_id);
                    } else {
                        subscribers.insert(subscriber.to_owned(), vec![feed_id]);
                    }
                }
                feeds.insert(feed_id, feed);
            }

            Ok(Database {
                path: path.to_owned(),
                feeds: feeds,
                subscribers: subscribers,
            })
        } else {
            Database::create(path)
        }
    }

    pub fn subscribe(&mut self,
                     subscriber: SubscriberID,
                     rss_link: &str,
                     rss: &rss::Channel)
                     -> Result<()> {
        let feed_id = get_hash(rss_link);
        {
            let subscribed_feed = self.subscribers.entry(subscriber).or_insert_with(|| {
                Vec::new()
            });
            if !subscribed_feed.contains(&feed_id) {
                subscribed_feed.push(feed_id);
            } else {
                return Err(ErrorKind::AlreadySubscribed.into());
            }
        }
        {
            let feed = self.feeds.entry(feed_id).or_insert_with(|| {
                Feed {
                    link: rss_link.to_owned(),
                    title: rss.title.to_owned(),
                    error_count: 0,
                    hash_list: gen_hash_list(&rss.items),
                    subscribers: Vec::new(),
                }
            });
            feed.subscribers.push(subscriber);
        }
        self.save()
    }

    fn save(&mut self) -> Result<()> {
        let feeds_list: Vec<&Feed> = self.feeds.iter().map(|(_id, feed)| feed).collect();
        let mut file =
            File::create(&self.path).chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))?;
        serde_json::to_writer(&mut file, &feeds_list)
            .chain_err(|| ErrorKind::DatabaseSave(self.path.to_owned()))
    }
}
