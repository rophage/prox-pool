use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use std::fs;
use chrono::prelude::*;
use regex::Regex;
use reqwest::{Client, Proxy, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    fs::File as AsyncFile,
    io::{AsyncBufReadExt, AsyncWriteExt},
    sync::Semaphore,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    ip: String,
}

pub async fn gen_list() -> Result<(), reqwest::Error>{
    // let socks5_sources = vec![];
    let http_sources = vec![
        "https://raw.githubusercontent.com/MuRongPIG/Proxy-Master/main/http.txt",
    ];
    let mut proxis = HashMap::new();
    proxis.insert("http", http_sources);
    // proxis.insert("socks5", socks5_sources);
    for (key, value) in &proxis {
        println!("{} {} sources", value.len(), key);

        for source in value{
            let url = Url::parse(source).unwrap();
            let mut domain = url.domain().unwrap().to_owned();
            if domain == "raw.githubusercontent.com" {
                let path = url.path().trim_matches('/');
                let parts: Vec<&str> = path.split('/').collect();
                // domain = format!("{}_{}", parts[0], parts[1]);
                domain = parts[0].to_string()
            } else {
                let parts: Vec<&str> = domain.split('.').collect();
                domain = format!("{}_{}", parts[1], parts[2]);        
            }

            let output_file       = format!("proxies/{}_{}.txt", domain, key);
            let output_path        = Path::new(&output_file);
            let mut out_file        = AsyncFile::create(output_path).await.unwrap();

            let response    = reqwest::get(url).await?;
            let body = response.text().await?;
            let re = Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}:\d{2,5}").unwrap();
            let matches: Vec<_> = re.find_iter(&body).map(|m| m.as_str()).collect();
            let matches: Vec<_> = matches.into_iter().collect::<HashSet<_>>().into_iter().collect();
            let content = matches.join("\n");
            let result = out_file.write_all(content.as_bytes()).await;
            
            match result {
                Ok(_) => println!("{}: {}", domain, body.lines().count()),
                Err(e) => println!("Error! {}", e),
            }
        }
    }
        Ok(())
}

fn open_text(filename: &str) -> Vec<String> {
    let file = File::open(filename).unwrap();
    let reader = BufReader::new(file);
    reader.lines().map(|line| line.unwrap()).collect()
}

pub async fn validate_proxy(proxy: String, local_ip: &str) -> Option<String> {
    let client = Client::builder()
        .proxy(reqwest::Proxy::all(&proxy).ok()?)
        .build()
        .ok()?;
    let result = client
        .get("https://ipinfo.io/json")
        .timeout(Duration::from_secs(25))
        .send()
        .await;

    match result {
        Ok(response) => match response.status() {
            StatusCode::OK => {
                let json_response: Value = response.json().await.ok()?;
                if local_ip == json_response.get("ip")?.as_str()? {
                    println!("Leaking");
                    None
                } else {
                    // println!("{:?}", json_response);
                    Some(proxy)
                }
            }
            StatusCode::TOO_MANY_REQUESTS => {
                // println!("429");
                None
            }
            _ => None,
        },
        Err(_) => None,
    }
}

pub async fn validate_source(source:String) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let local_ip = reqwest::get("https://ipinfo.io/json")
        .await?
        .json::<Value>()
        .await?
        .get("ip")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    let source_clone = source.clone();
    let source_dir = format!("proxies/{}.txt", source_clone);
    let unvalidated_proxies = open_text(&source_dir);
    let unvalidated_proxies: HashSet<String> = unvalidated_proxies.into_iter().collect();
    let valid_proxies = Arc::new(Mutex::new(Vec::new()));
    let mut tasks = Vec::new();
    let concurrent_limit = Arc::new(Semaphore::new(5000));

    for proxy in unvalidated_proxies {
        let local_ip = local_ip.clone();
        let valid_proxies = Arc::clone(&valid_proxies);
        let concurrent_limit = Arc::clone(&concurrent_limit);
        let task = tokio::spawn(async move {
            let _permit = concurrent_limit.acquire().await;
            if let Some(valid_proxy) = validate_proxy(proxy, &local_ip).await {
                valid_proxies.lock().unwrap().push(valid_proxy);
            }
        });
        tasks.push(task);
    }

    futures::future::join_all(tasks).await;

    let valid_proxies = valid_proxies.lock().unwrap();
    println!("{} {} proxies found in {:?}", valid_proxies.len(), source,start.elapsed());
    let now = Local::now();
    let datetime_string = now.format("%Y%m%d_%H%M%S ").to_string();
    
    for entry in fs::read_dir("validated")? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_string = file_name.to_str().unwrap();
        let i = file_string.split(" ").collect::<Vec<&str>>();
        let i = i[1];
        if i.contains(&source) {
            fs::remove_file(format!("validated/{}", file_string))?;
        }
    }
    let mut file = File::create(format!("validated/{}{}.txt", datetime_string, source))?;
    for proxy in &*valid_proxies {
        writeln!(file, "{}", proxy)?;
    }
    Ok(())
}
