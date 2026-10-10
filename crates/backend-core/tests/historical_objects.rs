use gaming_cafe_api::{historical::objects, replication::snapshot};
#[tokio::test]
async fn large_encrypted_objects_stream_multipart_verify_and_refuse_overwrite() {
    use std::io::Write;
    let root = std::env::temp_dir().join(format!("arena-object-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("large.bin");
    let mut file = std::fs::File::create(&source).unwrap();
    let mut state = 123456789u32;
    for _ in 0..20 {
        let mut bytes = vec![0u8; 1024 * 1024];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        file.write_all(&bytes).unwrap();
    }
    file.sync_all().unwrap();
    drop(file);
    let store = object_store::memory::InMemory::new();
    let key = [31u8; 32];
    let hash = snapshot::hash_file(&source).unwrap();
    let object = objects::upload(
        &store,
        &key,
        "tenants/fixture/exports/large.bin".into(),
        "transport".into(),
        0,
        hash.clone(),
        &source,
    )
    .await
    .unwrap();
    assert!(
        object.bytes > 16 * 1024 * 1024,
        "fixture must exercise multipart upload"
    );
    assert!(objects::upload(
        &store,
        &key,
        object.key.clone(),
        "transport".into(),
        0,
        hash.clone(),
        &source
    )
    .await
    .is_err());
    let restored = root.join("restored.bin");
    objects::download(&store, &key, &object, &restored)
        .await
        .unwrap();
    assert_eq!(snapshot::hash_file(&restored).unwrap(), hash);
    assert!(
        objects::download(&store, &[32; 32], &object, &root.join("wrong-key.bin"))
            .await
            .is_err()
    );
    std::fs::remove_dir_all(root).unwrap();
}
