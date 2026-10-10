#[tokio::main]
async fn main() {
    gaming_cafe_api::runtime::run(gaming_cafe_api::runtime::Service::Gateway).await;
}
