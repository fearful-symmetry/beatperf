use std::{fs::{read_to_string, File, OpenOptions}, time::Duration};

use anyhow::Context;
use clap::{ArgGroup, Parser};
use groups::{custom::CustomMetrics, kernel_tracing::KernelTracing, memory::MemoryMetrics, output::Output, pipeline::Pipeline, processdb::ProcessDB};
use reqwest::IntoUrl;
use serde_json::{Map, Value};
use spinners::{Spinner, Spinners};
use tokio::{signal, sync::broadcast::{self, Sender}, task::JoinSet, time};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use watchers::run_watch;
use std::io::prelude::*;

mod groups;
mod watchers;


#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(group(
    ArgGroup::new("fields")
        .args(&["metrics", "memory", "cpu", "processdb", "pipeline", "output", "ndjson", "kernel_tracing"]) // if you're adding new metric groups, be sure to add them here
        .multiple(true)
        .required(true)
))]
#[clap(group(
    ArgGroup::new("reader")
    .required(false)
    .args(&["read"])
    .conflicts_with("ndjson"),
))]
struct Cli {
    /// the hostname:port combination of the beat stat endpoint
    #[arg(default_value_t = default_endpoint() )]
    endpoint: String,

    /// How often to fetch stats, in seconds.
    #[arg(long, short, default_value_t = 5 )]
    interval: u64,

    /// A list of custom metrics to monitor, in dot-notation
    #[arg(long, short)]
    metrics: Option<Vec<String>>,

    /// report memory metrics
    #[arg(long)]
    memory: bool,

    /// report CPU metrics
    #[arg(long)]
    cpu: bool,

    /// report add_session_metadata's processDB metrics
    #[arg(long)]
    processdb: bool,

    /// report libbeat pipeline metrics
    #[arg(long)]
    pipeline: bool,

    /// report add_sesson_metadata's kernel_tracing metrics
    #[arg(long)]
    kernel_tracing: bool,


    /// Report output event metrics
    #[arg(long)]
    output: bool,

    /// Debug logging
    #[arg(long, short)]
    verbose: bool,

    /// dump all beat metrics to an ndjson file
    #[arg(long)]
    ndjson: Option<String>,

    ///Read metrics from an file, instead of from a a beat http endpoint.
    #[arg(long)]
    read: Option<String>

}

fn default_endpoint() -> String {
    "localhost:5066".to_string()
}

/// start up tasks for every configured watcher
fn generate_readers(args: &Cli, tx: &mut Sender<Map<String, Value>>, realtime: bool) -> JoinSet<()> {
    let mut set = JoinSet::new();
    if args.memory {
        run_watch::<MemoryMetrics>(&mut set, tx, None, realtime);
    }
    if args.processdb {
        run_watch::<ProcessDB>(&mut set, tx, None, realtime);
    }

    if args.pipeline {
        run_watch::<Pipeline>(&mut set, tx, None, realtime);
    }

    if args.output {
        run_watch::<Output>(&mut set, tx, None, realtime);
    }

    if args.kernel_tracing {
        run_watch::<KernelTracing>(&mut set, tx, None, realtime);
    }

    if  args.metrics.is_some() {
        run_watch::<CustomMetrics>(&mut set, tx, args.metrics.clone(), realtime);
    }

    set
}

/// Sit and read events
async fn watch(stat_path: String, args: Cli) -> anyhow::Result<()> {
    let token = CancellationToken::new();
    let cloned_token = token.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("failed to listen for event");
        token.cancel();
    });

    let mut nd_file: Option<File> = match &args.ndjson {
        Some(fname) => {
            let file = OpenOptions::new().append(true).create(true).open(fname)?;
            Some(file)
        },
        None => None
    };


    // ======= init metrics channels
    let (mut tx,  _) = broadcast::channel(100);
    let _readers_handle = generate_readers(&args, &mut tx, true);

    let mut interval = time::interval(Duration::from_secs(args.interval));
    info!("starting watch of beat stats...");

    loop {
        let mut sp = Spinner::new(Spinners::Dots9, "Watching...".into());
        
        tokio::select! {
            _ = cloned_token.cancelled() => {
                sp.stop_with_message("shutting down!".to_string());
                    
                return Ok(());
            }
            _ = interval.tick() => {
                let res = get_stat(&stat_path, &mut nd_file).await;
                if tx.receiver_count() > 0 {
                    match  res {
                        Ok(res) => {
                           match tx.send(res){
                            Ok(c) => {
                                debug!("sent to {} monitors", c);
                            }, 
                            Err(e) => {
                                error!("error sending event: {}", e);
                            }
                           }
                        },
                        Err(e) => {
                            error!("got error fetching stats: {}", e)
                        }
                    }
                }

            }
        }
    }

}


async fn get_stat<T: IntoUrl>(stat_path: T, fname: &mut Option<File>) -> anyhow::Result<serde_json::Map<String, serde_json::Value>>{
    let test_get = reqwest::get(stat_path)
    .await.context("error fetching URL")?.error_for_status()?.text().await?;

    if let Some(file) = fname {
        writeln!(file, "{}", test_get)?;
    }

    let result: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&test_get)?;

    Ok(result)
}

/// ingest all metrics from a file
async fn read_file<T: AsRef<str>>(path: T, args: Cli) -> anyhow::Result<()> {
    let raw = read_to_string(path.as_ref()).context("error reading file to string")?;
    let (mut tx,  _) = broadcast::channel(100);
    let mut readers_handle = generate_readers(&args, &mut tx, false);
    for point in raw.split('\n') {
        if point.is_empty() {
            continue;
        }

        let result: serde_json::Map<String, serde_json::Value> = serde_json::from_str(point).context("error parsing JSON")?;
       tx.send(result)?;
    };
    drop(tx);

    while readers_handle.join_next().await.is_some() {
        info!("watcher done....")
    }
    

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let mut level = LevelFilter::INFO;
    if args.verbose {
        level = LevelFilter::DEBUG;
    }

    tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::builder().with_default_directive(level.into()).from_env_lossy()) 
    .init();


    if let Some(path) = args.read.clone() {
        read_file(path, args).await?;
    } else {
        let stats_endpoint = format!("http://{}/stats", args.endpoint);
        info!("using endpoint {}", stats_endpoint);
    
        // do initial get to make sure the endpoint is okay.
        let _test_get = reqwest::get(&stats_endpoint)
        .await.context("error fetching URL. Is is correct, and is the beat running?")?.error_for_status()?.text().await?;
    
        
        watch(stats_endpoint, args).await?;
    }

    Ok(())
}
