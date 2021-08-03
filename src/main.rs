use structopt::StructOpt;
use anyhow::{Context, Result};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, BTreeMap};
use std::time::SystemTime;
use uuid::Uuid;
use crypto::sha2::Sha256;
use crypto::sha1::Sha1;
use crypto::hmac::Hmac;
use crypto::mac::Mac;
use urlencoding::encode;
use std::fs::File;
use std::io::Read;
use reqwest::Response;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(StructOpt)]
struct Cli {
    /// The pattern to look for
    pattern: String,
    /// The path to the file to read
    #[structopt(parse(from_os_str))]
    path: std::path::PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchResult {
    data: Vec<Data>,
    includes: Includes,
    meta: Meta,
}

#[derive(Debug, Serialize, Deserialize)]
struct Data {
    author_id: String,
    text: String,
    id: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Includes {
    users: Vec<User>
}

#[derive(Debug, Serialize, Deserialize)]
struct User {
    created_at: String,
    username: String,
    id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Meta {
    newest_id: String,
    oldest_id: String,
    result_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatusesResult {
    created_at: String,
    id: u64,
    id_str: String,
    text: String
}

#[derive(Debug)]
struct TweetClient {
    config: TwitterConfig
}

impl TweetClient {
    async fn search_recent(&self, query: &String) -> SearchResult {
        let response: SearchResult  = reqwest::Client::new()
            .get(&self.config.endpoints.search_recent)
            .bearer_auth(&self.config.credentials.bearer_token)
            .query(&[("query", query), ("tweet.fields", &String::from("created_at")), ("expansions", &String::from("author_id")), ("user.fields", &String::from("created_at"))])
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        return response
    }

    async fn update_statuses(&self, status: &String) -> StatusesResult {
        // Authentication ヘッダに含めるパラメータ
        let mut oauth_params = HashMap::new();
        oauth_params.insert("oauth_consumer_key", &self.config.credentials.consumer_key);
        oauth_params.insert("oauth_token", &self.config.credentials.access_token);
        let signature_method = String::from("HMAC-SHA1");
        oauth_params.insert("oauth_signature_method", &signature_method);
        let version = String::from("1.0");
        oauth_params.insert("oauth_version", &version);
        let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string();
        oauth_params.insert("oauth_timestamp", &timestamp);
        oauth_params.insert("oauth_nonce", &timestamp);

        // リクエストパラメータ
        let mut request_params = HashMap::new();
        request_params.insert("status", status);

        // signature_base 作成
        let mut signature_params = BTreeMap::new();
        for (key, value) in &oauth_params {
            signature_params.insert(key, value);
        }
        for (key, value) in &request_params {
            signature_params.insert(key, value);
        }

        let mut signature_array: Vec<String> = Vec::new();
        for (key, value) in &signature_params {
            let mut str = encode(key).into_owned();
            str.push_str("=");
            str.push_str(&encode(value).into_owned());
            signature_array.push(str);
        }
        let result: String = signature_array.join("&");

        let mut signature_base = String::from("POST");
        signature_base.push_str("&");
        signature_base.push_str(&encode(&&self.config.endpoints.update_statuses).into_owned());
        signature_base.push_str("&");
        signature_base.push_str(&encode(&result).into_owned());
        println!("result: {:?}", result);

        // signature_key 作成
        let mut signature_key = encode(&&self.config.credentials.consumer_secret).into_owned();
        signature_key.push_str("&");
        signature_key.push_str(&encode(&&self.config.credentials.access_secret).into_owned());

        // signature 作成
        let mut hmac = Hmac::new(Sha1::new(), signature_key.as_bytes());
        hmac.input(signature_base.as_bytes());
        let hmac_result = hmac.result();
        let hmac_code = hmac_result.code();
        let encoded = base64::encode(hmac_code);

        // OAuth パラメータ作成
        oauth_params.insert("oauth_signature", &encoded);
        let mut oauth_header: String = format!("OAuth oauth_consumer_key={}, oauth_nonce={}, oauth_signature={}, oauth_signature_method={}, oauth_timestamp={}, oauth_token={}, oauth_version={}",
                                               encode(oauth_params.get("oauth_consumer_key").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_nonce").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_signature").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_signature_method").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_timestamp").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_token").unwrap()).into_owned(),
                                               encode(oauth_params.get("oauth_version").unwrap()).into_owned()
        );
        println!("oauth_header: {:#?}", oauth_header);

        let mut url = &self.config.endpoints.update_statuses;

        let response: StatusesResult = reqwest::Client::new()
            .post(url)
            .query(&request_params)
            .header("Authorization", oauth_header)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        // let result: String = response.text().await.unwrap();
        return response
    }
}

#[derive(Debug, Deserialize)]
struct TwitterConfig {
    credentials: Credentials,
    endpoints: Endpoints
}

#[derive(Debug, Deserialize)]
struct Credentials {
    bearer_token: String,
    consumer_key: String,
    consumer_secret: String,
    access_token: String,
    access_secret: String
}

#[derive(Debug, Deserialize)]
struct Endpoints {
    search_recent: String,
    update_statuses: String
}

#[tokio::main]
async fn main() -> Result<()> {

    // Twitter アカウント設定ファイル読み込み
    let mut file = File::open("./config/twitter_config.toml").expect("twitter_config.toml not found");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("something went wrong reading the twitter_config.toml");
    let config: TwitterConfig = match toml::from_str(&contents) {
        Ok(c) => c,
        Err(e) => panic!("fail to parse toml: {}", e)
    };
    let client = TweetClient {
        config,
    };

    let status = String::from("hello world from twitter client5");
    let response = client.update_statuses(&status);
    println!("response: {:#?}", response.await);

    Ok(())
}