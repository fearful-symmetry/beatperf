use serde_json::{Map, Value};
use tokio::{sync::broadcast::Sender, task::JoinSet};
use tracing::{debug, error, info};

use crate::groups::Watcher;

pub fn run_watch<T: Watcher + Send + 'static>( set: &mut JoinSet<()>, broadcaster: &Sender<Map<String, Value>>) {
    let mut rx2 = broadcaster.subscribe();
    set.spawn(async move {
        let mut watch = T::new();
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

            if count % 5 == 0 {
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
}