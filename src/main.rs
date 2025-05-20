mod link_gatherer;
mod link_map;
mod site_tracer;

use link_gatherer::Page;
use site_tracer::SiteTracer;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version)]
pub struct Cli {
    /// The base URL to begin from
    #[arg(short, long)]
    url: String,
    #[arg(short, long)]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Cli::try_parse();
    match args {
        Ok(args) => {
            if let Some(log_level) = args.log_level {
                tracing_subscriber::fmt()
                    .with_env_filter(format!("aleister_crawly={}", log_level))
                    .compact()
                    .init();
            }

            let st = SiteTracer {
                link_getter: Page::new(reqwest::Client::new()),
                worker_pool_size: 100,
                max_retries: 3,
                initial_retry_delay_ms: 250,
            };

            let link_map = st.trace(&args.url).await;
            println!("\n{}", link_map.to_tree());
        }
        Err(e) => println!("{}", e.to_string()),
    }
}
