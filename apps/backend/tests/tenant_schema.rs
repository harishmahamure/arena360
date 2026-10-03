//! Run with DATABASE_URL pointing at a migrated test database:
//! cargo test --test tenant_schema -- --ignored

use gaming_cafe_api::config::load_dotenv;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a database with tenant migrations applied"]
async fn resources_and_memberships_stay_in_their_organization() {
    load_dotenv();
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        return;
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("test database");
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *tx)
        .await
        .expect("immediate tenant constraints");

    let org = Uuid::new_v4();
    let venue = Uuid::new_v4();
    let name = format!("tenant-test-{}", org.simple());
    let user: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&mut *tx)
        .await
        .expect("seeded user");
    sqlx::query("INSERT INTO organizations(id,slug,name) VALUES($1,$2,'Tenant schema test')")
        .bind(org)
        .bind(&name)
        .execute(&mut *tx)
        .await
        .expect("organization");
    sqlx::query(
        r#"INSERT INTO venue_locations(id,"organizationId",slug,name) VALUES($1,$2,'main','Main')"#,
    )
    .bind(venue)
    .bind(org)
    .execute(&mut *tx)
    .await
    .expect("venue");

    let default_units: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM units WHERE "organizationId"=$1 AND "deletedAt" IS NULL"#,
    )
    .bind(gaming_cafe_api::models::DEFAULT_ORGANIZATION_ID)
    .fetch_one(&mut *tx)
    .await
    .expect("default units");
    let tenant_units: i64 =
        sqlx::query_scalar(r#"SELECT count(*) FROM units WHERE "organizationId"=$1"#)
            .bind(org)
            .fetch_one(&mut *tx)
            .await
            .expect("tenant units");
    assert!(default_units > 0);
    assert_eq!(tenant_units, default_units);

    sqlx::query("INSERT INTO devices(name) VALUES($1)")
        .bind(&name)
        .execute(&mut *tx)
        .await
        .expect("default device");
    sqlx::query(r#"INSERT INTO devices(name,"organizationId","locationId") VALUES($1,$2,$3)"#)
        .bind(&name)
        .bind(org)
        .bind(venue)
        .execute(&mut *tx)
        .await
        .expect("same name in another organization");

    sqlx::query("SAVEPOINT wrong_venue")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let cross_venue =
        sqlx::query(r#"INSERT INTO devices(name,"organizationId","locationId") VALUES($1,$2,$3)"#)
            .bind(format!("{name}-wrong"))
            .bind(org)
            .bind(gaming_cafe_api::models::DEFAULT_VENUE_LOCATION_ID)
            .execute(&mut *tx)
            .await
            .expect_err("another organization's venue must be rejected");
    assert_eq!(
        cross_venue
            .as_database_error()
            .and_then(|e| e.code())
            .as_deref(),
        Some("23503")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT wrong_venue")
        .execute(&mut *tx)
        .await
        .expect("restore transaction");

    sqlx::query("SAVEPOINT missing_membership")
        .execute(&mut *tx)
        .await
        .expect("savepoint");
    let missing_member = sqlx::query(
        r#"INSERT INTO shifts("userId","venueLocationId","organizationId") VALUES($1,$2,$3)"#,
    )
    .bind(user)
    .bind(venue)
    .bind(org)
    .execute(&mut *tx)
    .await
    .expect_err("a shift requires membership in its organization");
    assert_eq!(
        missing_member
            .as_database_error()
            .and_then(|e| e.code())
            .as_deref(),
        Some("23503")
    );
    sqlx::query("ROLLBACK TO SAVEPOINT missing_membership")
        .execute(&mut *tx)
        .await
        .expect("restore transaction");
    sqlx::query(
        r#"INSERT INTO organization_memberships("organizationId","userId",role) VALUES($1,$2,'staff')"#,
    )
    .bind(org)
    .bind(user)
    .execute(&mut *tx)
    .await
    .expect("membership");
    sqlx::query(
        r#"INSERT INTO shifts("userId","venueLocationId","organizationId") VALUES($1,$2,$3)"#,
    )
    .bind(user)
    .bind(venue)
    .bind(org)
    .execute(&mut *tx)
    .await
    .expect("member shift");
    tx.rollback().await.expect("leave database unchanged");
}
