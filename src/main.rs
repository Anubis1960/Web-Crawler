use reqwest::Client;
use std::collections::{HashSet, VecDeque};
use scraper::{Html, Selector};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use std::{fs,io};

/**
Include the following in Cargo.toml
reqwest = "0.11.23"
serde = { version = "1.0.194", features = ["derive"] }
tokio = { version = "1.35.1", features = ["full"] }
serde_json = { version = "1.0.111", features = ["default"] }
scraper = "0.18.1"
async-recursion = "1.0.5"
async-std = { version = "1.12.0", features = ["attributes"] }
ctrlc = "3.1.5"
**/

#[derive(Serialize, Deserialize)]
struct UrlList {
    urls_to_visit: VecDeque<String>,
    visited_urls: HashSet<String>,
}

#[tokio::main]
async fn main() {
    println!("Enter the URL to crawl or resume to resume the search: ");
    let mut url = String::new();
    io::stdin()
        .read_line(&mut url)
        .expect("Failed to read line");
    if url.trim() != "resume" {
        let mut urls_to_visit = VecDeque::new();
        urls_to_visit.push_back(url.trim().to_string());
        let url_list = UrlList {
            urls_to_visit,
            visited_urls: HashSet::new(),
        };
        let json_str = serde_json::to_string(&url_list).unwrap();
        fs::write("url_list.json", json_str).unwrap();
    }
    let url_list: UrlList = match fs::read_to_string("url_list.json") {
        Ok(json_str) => serde_json::from_str(&json_str).unwrap_or_else(|_| UrlList{
            urls_to_visit: VecDeque::new(),
            visited_urls: HashSet::new(),
        }),
        Err(_) => UrlList {
            urls_to_visit: VecDeque::new(),
            visited_urls: HashSet::new(),
        },
    };

    let client = Client::new();
    let visited_urls = Arc::new(Mutex::new(url_list.visited_urls));
    let urls_to_visit = Arc::new(Mutex::new(url_list.urls_to_visit));

    println!("Enter the number of tasks to use: ");
    let mut input_tasks = String::new();
    io::stdin()
        .read_line(&mut input_tasks)
        .expect("Failed to read line");
    let num_tasks:i32 = input_tasks.trim().parse().expect("Expected integer!");

    let start_url = url.clone();

    if start_url.trim() != "resume" {
        urls_to_visit.lock().unwrap().pop_front();
        crawl(&start_url.trim(), &client, &visited_urls, &urls_to_visit).await;
    }

    for _ in 0..num_tasks {
        let client = client.clone();
        let visited_urls = visited_urls.clone();
        let urls_to_visit = urls_to_visit.clone();
        tokio::spawn(async move {
            loop {
                let url = {
                    let mut urls_to_visit = urls_to_visit.lock().unwrap();
                    if let Some(url) = urls_to_visit.pop_front() {
                        url
                    } else {
                        break;
                    }
                };
                println!("thread id: {:?}, url {}",std::thread::current().id(), url);
                crawl(&url, &client, &visited_urls, &urls_to_visit).await;
            }
        });
    }
    println!("task completed");
    let signal = tokio::signal::ctrl_c();
    tokio::select! {
        _ = signal => {
            let visited_urls = visited_urls.lock().unwrap();
            let urls_to_visit = urls_to_visit.lock().unwrap();
            let url_list = UrlList {
                urls_to_visit: urls_to_visit.clone(),
                visited_urls: visited_urls.clone(),
            };
            let json_str = serde_json::to_string(&url_list).unwrap();
            fs::write("url_list.json", json_str).unwrap();
            println!("Received Ctrl-C, Exit successfully");
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(100)) => {
            let visited_urls = visited_urls.lock().unwrap();
            let urls_to_visit = urls_to_visit.lock().unwrap();
            let url_list = UrlList {
                urls_to_visit: urls_to_visit.clone(),
                visited_urls: visited_urls.clone(),
            };
            let json_str = serde_json::to_string(&url_list).unwrap();
            fs::write("url_list.json", json_str).unwrap();
            println!("Exiting..");
        }
    }

}

async fn crawl(
    url: &str,
    client: &Client,
    visited_urls: &Arc<Mutex<HashSet<String>>>,
    urls_to_visit: &Arc<Mutex<VecDeque<String>>>,
) {
    let resp = client.get(url).send().await;
    let resp = match resp {
        Ok(resp) => resp,
        Err(e) => {
            println!("Error visiting URL: {}", e);
            return;
        }
    };

    let body = resp.text().await.unwrap();

    let mut visited_urls = visited_urls.lock().unwrap();
    let mut urls_to_visit = urls_to_visit.lock().unwrap();

    let fragment = Html::parse_document(&body);

    let selector = Selector::parse("a").unwrap();

    visited_urls.insert(url.trim().to_string());

    for element in fragment.select(&selector) {
        let href = element.value().attr("href").unwrap_or("");
        if (href.starts_with("http") || href.starts_with("https") || href.starts_with("/") && href.len() > 1) && !visited_urls.contains(href){
            if href.starts_with("//"){
                let href = format!("https:{}", href);
                if visited_urls.contains(&href) || urls_to_visit.contains(&href){
                    continue;
                }
                urls_to_visit.push_back(href.trim().to_string());
                continue;
            }
            if href.starts_with("/"){
                let href = href.trim();
                let base_url = url.split("/").collect::<Vec<&str>>();
                let base_url = format!("{}//{}", base_url[0], base_url[2]);
                let href = format!("{}{}", base_url, href);
                if visited_urls.contains(&href) || urls_to_visit.contains(&href){
                    continue;
                }
                urls_to_visit.push_back(href.trim().to_string());
                continue;
            }
            if visited_urls.contains(href) || urls_to_visit.contains(&href.to_string()){
                continue;
            }
            urls_to_visit.push_back(href.trim().to_string());
        }
    }
}