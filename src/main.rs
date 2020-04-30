use log::info;

#[async_std::main]
async fn main() -> Result<(), String> {
    env_logger::init();

    let config = config::load().map_err(|err| format!("Failed to load config: {}", err))?;
    info!("App config: {:?}", config);

    app::run(&config).await
}

mod app;
mod config;
mod event;
