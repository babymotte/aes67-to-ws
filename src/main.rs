use aes67_to_ws::poem::{self};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    poem::start().await
}
