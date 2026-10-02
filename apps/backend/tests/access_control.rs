//! DATABASE_URL=<isolated migrated database> cargo test --test access_control -- --ignored
use gaming_cafe_api::access::{effective, MANAGED};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
#[tokio::test]
#[ignore = "requires isolated migrated DATABASE_URL; fixtures roll back"]
async fn roles_are_scoped_templates_do_not_grant_and_modules_override() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let org = Uuid::new_v4();
    let user = Uuid::new_v4();
    let role = Uuid::new_v4();
    sqlx::query("INSERT INTO organizations(id,slug,name) VALUES($1,$2,'Access test')")
        .bind(org)
        .bind(org.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'unused','staff')")
        .bind(user)
        .bind(user.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(r#"INSERT INTO organization_memberships("organizationId","userId",role) VALUES($1,$2,'staff')"#).bind(org).bind(user).execute(&mut *tx).await.unwrap();
    sqlx::query("DELETE FROM access_assignments WHERE organization_id=$1 AND user_id=$2")
        .bind(org)
        .bind(user)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO access_roles(id,organization_id,name,permissions) VALUES($1,$2,'Finance only','[\"finance:read\"]')").bind(role).bind(org).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO access_assignments(organization_id,user_id,role_id) VALUES($1,$2,$3)")
        .bind(org)
        .bind(user)
        .bind(role)
        .execute(&mut *tx)
        .await
        .unwrap();
    let grants = effective(&mut *tx, org, user).await.unwrap();
    assert_eq!(grants, vec!["finance:read", MANAGED]);
    sqlx::query("SAVEPOINT wrong_organization")
        .execute(&mut *tx)
        .await
        .unwrap();
    let wrong = sqlx::query(
        "INSERT INTO access_assignments(organization_id,user_id,role_id) VALUES($1,$2,$3)",
    )
    .bind(gaming_cafe_api::models::DEFAULT_ORGANIZATION_ID)
    .bind(user)
    .bind(role)
    .execute(&mut *tx)
    .await;
    assert!(wrong.is_err());
    sqlx::query("ROLLBACK TO wrong_organization")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO access_modules(organization_id,module,enabled) VALUES($1,'finance',false)",
    )
    .bind(org)
    .execute(&mut *tx)
    .await
    .unwrap();
    assert_eq!(effective(&mut *tx, org, user).await.unwrap(), vec![MANAGED]);
    sqlx::query("UPDATE access_modules SET enabled=true WHERE organization_id=$1")
        .bind(org)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE access_roles SET is_template=true WHERE id=$1")
        .bind(role)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(effective(&mut *tx, org, user).await.unwrap(), vec![MANAGED]);
    tx.rollback().await.unwrap();
}
