use std::time::Duration;

use gaming_cafe_api::cache::{CacheService, RedisCache};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires REDIS_URL"]
async fn token_bucket_is_shared_exact_and_refills() {
    let url = std::env::var("REDIS_URL").expect("REDIS_URL");
    let first = RedisCache::connect(&url).await.expect("first backend cache");
    let second = RedisCache::connect(&url).await.expect("second backend cache");
    let key = format!("test:rate-limit:{}", Uuid::new_v4());

    // A long window makes this an atomicity assertion rather than allowing
    // tokens to refill while 2,000 network round-trips are executed serially.
    let atomicity_window = Duration::from_secs(86_400);
    for index in 0..2_000 {
        let cache = if index % 2 == 0 { &first } else { &second };
        assert!(cache
            .consume_ip_token(&key, 2_000, atomicity_window)
            .await
            .unwrap()
            .unwrap()
            .allowed);
    }
    assert!(!second
        .consume_ip_token(&key, 2_000, atomicity_window)
        .await
        .unwrap()
        .unwrap()
        .allowed);

    let other_ip = format!("test:rate-limit:{}", Uuid::new_v4());
    assert!(first
        .consume_ip_token(&other_ip, 2_000, Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap()
        .allowed);

    let refill_key = format!("test:rate-limit:{}", Uuid::new_v4());
    for _ in 0..2 {
        assert!(first
            .consume_ip_token(&refill_key, 2, Duration::from_millis(100))
            .await
            .unwrap()
            .unwrap()
            .allowed);
    }
    assert!(!first
        .consume_ip_token(&refill_key, 2, Duration::from_millis(100))
        .await
        .unwrap()
        .unwrap()
        .allowed);
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(second
        .consume_ip_token(&refill_key, 2, Duration::from_millis(100))
        .await
        .unwrap()
        .unwrap()
        .allowed);
}
