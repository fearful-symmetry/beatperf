use serde_json::{Map, Value};
use tokio::{sync::broadcast::Sender, task::JoinSet};
use tracing::{debug, error, info};

use crate::groups::Watcher;

/// Start a watcher for a single group of metrics
pub fn run_watch<T: Watcher + Send + 'static>( set: &mut JoinSet<()>, broadcaster: &Sender<Map<String, Value>>, added_metrics: Option<Vec<String>>, realtime: bool) {
    let mut rx2 = broadcaster.subscribe();
    set.spawn(async move {
        let mut watch = T::new(added_metrics);
        let mut count = 0;
        loop {
            tokio::select! {
                Ok(dat) = rx2.recv() => {
                    watch.update(&dat);
                    count+=1;
                }
                else => {
                    break
                }
            }

            if realtime && count % 5 == 0{
                debug!("updating plot...");
                if let Err(e) = watch.plot() {
                    error!("error updating plot: {}", e)
                }
            }

        }

        info!("rendering final plot");
        if let Err(e) = watch.plot() {
            error!("error rendering plot: {}", e)
        }
    });
} // 75-140