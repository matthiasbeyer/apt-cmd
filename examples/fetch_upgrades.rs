use anyhow::Context;
use apt_cmd::{
    fetch::{EventKind, PackageFetcher},
    AptGet,
};

use async_global_executor::block_on;
use futures::{future::FutureExt, stream::StreamExt};
use std::{path::Path, sync::Arc};

fn main() -> anyhow::Result<()> {
    const CONCURRENT_FETCHES: usize = 4;
    const DELAY_BETWEEN: u64 = 100;
    const RETRIES: u32 = 3;

    let future = async move {
        let client = surf::Client::new();
        let path = Path::new("./packages/");
        let (fetch_tx, fetch_rx) = flume::bounded(CONCURRENT_FETCHES);

        if !path.exists() {
            async_fs::create_dir_all(path).await.unwrap();
        }

        let (fetcher, mut events) = PackageFetcher::new(client)
            .concurrent(CONCURRENT_FETCHES)
            .delay_between(DELAY_BETWEEN)
            .retries(RETRIES)
            .fetch(fetch_rx.into_stream(), Arc::from(path));

        // Fetch a list of packages that need to be fetched, and send them on their way
        let sender = async move {
            let packages = AptGet::new()
                .noninteractive()
                .upgrade_uris()
                .await
                .context("failed to spawn apt-get command")?
                .context("failed to fetch package URIs from apt-get")?;

            for package in packages {
                let _ = fetch_tx.send_async(Arc::new(package)).await;
            }

            Ok::<(), anyhow::Error>(())
        };

        // Begin listening for packages to fetch
        let receiver = async move {
            while let Some(event) = events.next().await {
                println!("Event: {:#?}", event);

                if let EventKind::Error(why) = event.kind {
                    return Err(why).context("package fetching failed");
                }
            }

            Ok::<(), anyhow::Error>(())
        };

        futures::future::try_join3(fetcher.map(Ok), sender, receiver).await?;

        println!("returning");

        Ok(())
    };

    block_on(future)
}
