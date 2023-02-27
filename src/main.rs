use std::{
    collections::HashMap,
    process::{Command, Stdio}
};

use clap::Parser;
use env_logger::Env;
use log::info;
use serde::{Serialize, Deserialize};
use reqwest;
use tokio_cron_scheduler::{JobScheduler, Job};
use sysinfo::{ProcessExt, System, SystemExt};



#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct PageInfo {
    #[serde(alias = "totalResults")]
    total_results: i32,
    #[serde(alias = "resultsPerPage")]
    results_per_page: i32
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct UserRespItem {
    kind: String,
    etag: String,
    id: String
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct UserResponse {
    kind: String,
    etag: String,
    #[serde(alias = "pageInfo")]
    page_info: PageInfo,
    items: Vec<UserRespItem>
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct Thumbnail {
    url: String,
    width: i32,
    height: i32
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct Snippet {
    #[serde(alias = "publishedAt")]
    published_at: String,
    #[serde(alias = "channelId")]
    channel_id: String,
    title: String,
    description: String,
    thumbnails: HashMap<String, Thumbnail>,
    #[serde(alias = "channelTitle")]
    channel_title: String,
    #[serde(alias = "liveBroadcastContent")]
    live_broadcast_content: String,
    #[serde(alias = "publishTime")]
    publish_time: String
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct Id {
    kind: String,
    #[serde(alias = "videoId")]
    video_id: String
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct Item {
    kind: String,
    etag: String,
    id: Id,
    snippet: Snippet
}

#[derive(
    Serialize,
    Deserialize,
    Debug
)]
struct YoutubeSearchListResponse {
    kind: String,
    etag: String,
    #[serde(alias = "pageInfo")]
    page_info: PageInfo,
    items: Vec<Item>
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    api_key: String,
    #[arg(short, long)]
    channel: String,
    #[arg(short, long, default_value_t = false)]
    quiet: bool
}

fn user_search(api_key: &String, channel: &String) -> String {
    format!("https://www.googleapis.com/youtube/v3/channels?key={}&forUsername={}&part=id", api_key, channel)
}

fn video_search(api_key: &String, user_id: &String) -> String {
    format!("https://www.googleapis.com/youtube/v3/search?part=snippet&channelId={}&type=video&eventType=live&key={}", user_id, api_key)
}

fn youtube_live_link(video_id: &String) -> String {
    format!("https://www.youtube.com/watch?v={}", video_id)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _args = Args::parse(); // for --help
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let mut scheduler = JobScheduler::new().await?;
    
    scheduler.add(Job::new_async("1/10 * * * * *", |uuid, _l| Box::pin( async move {
        info!("job is running as {}", uuid);
        let args = Args::parse();
        let channel = args.channel;
        let api_key = args.api_key;
        let user = 
            reqwest::get(user_search(&api_key, &channel))
                .await
                .unwrap()
                .json::<UserResponse>()
                .await
                .unwrap();
    
        let search = 
            reqwest::get(video_search(&api_key, &user.items[0].id))
                .await
                .unwrap()
                .json::<YoutubeSearchListResponse>()
                .await
                .unwrap();
    
        let mut yt_dlp: String = "yt-dlp".to_owned();

        if cfg!(windows) {
            yt_dlp.push_str(".exe");
        }

        let is_running = System::new_all()        
            .processes_by_exact_name("yt-dlp.exe")
            .any(|process| process.cmd()[1] == youtube_live_link(&search.items[0].id.video_id));

        

        if search.items.len() >= 1 && !is_running {
            info!("Recording...");
            let mut cmd = Command::new("yt-dlp")
            .args(&[youtube_live_link(&search.items[0].id.video_id)])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();

            let status = cmd.wait();
            info!("Exited with status {:?}", status);
        }
    })).unwrap()).await?;

    #[cfg(feature = "signal")]
    scheduler.shutdown_on_ctrl_c();
    scheduler.set_shutdown_handler(Box::new(|| {
      Box::pin(async move {
        println!("Exiting...");
      })
    }));

    scheduler.start().await.unwrap();
    tokio::time::sleep(core::time::Duration::from_secs(10)).await;
    Ok(())
}
