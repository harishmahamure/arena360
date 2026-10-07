use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use futures::future::BoxFuture;
use gaming_cafe_api::cache::create_cache;
use gaming_cafe_api::config::{Roles, Settings};
use gaming_cafe_api::dto::{KioskRegisterDto, LoginDto};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{AssignPlanDto, Device, UpdateUserDto, UserFilterDto};
use gaming_cafe_api::repositories::{
    StaffProjectionResult, TenantCreatePlayer, TenantLocationRoleGrant, TenantStaffProjection,
    TenantUserRepository,
};
use gaming_cafe_api::services::{AuthService, BalanceService, PlayerPlanService, UserService};
use gaming_cafe_api::tenancy::{
    tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease,
};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

#[derive(Default)]
struct Lease {
    generations: RwLock<HashMap<Uuid, i64>>,
    ensure_calls: AtomicUsize,
    fail_on_call: AtomicUsize,
}

impl Lease {
    fn fail_next_commit(&self) {
        self.ensure_calls.store(0, Ordering::SeqCst);
        self.fail_on_call.store(3, Ordering::SeqCst);
    }
}

impl TenantLease for Lease {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.generations
            .read()
            .map_err(|_| AppError::Internal("lease lock poisoned".into()))?
            .get(&tenant_id)
            .copied()
            .ok_or_else(|| AppError::Forbidden("tenant lease is not writable".into()))
    }

    fn ensure_writable(&self, tenant_id: Uuid, generation: i64) -> Result<(), AppError> {
        let call = self.ensure_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_on_call.load(Ordering::SeqCst) == call {
            self.generations
                .write()
                .map_err(|_| AppError::Internal("lease lock poisoned".into()))?
                .insert(tenant_id, generation + 1);
        }
        if self.writable_generation(tenant_id)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "tenant lease generation changed".into(),
            ))
        }
    }
}

#[tokio::test]
async fn players_are_isolated_and_support_safe_lifecycle() {
    let first = Fixture::new().await;
    let second = Fixture::new().await;
    let first_repo = TenantUserRepository::new(first.db.clone());
    let second_repo = TenantUserRepository::new(second.db.clone());
    let password_hash = hash("correct horse", DEFAULT_COST).unwrap();
    let player = first_repo
        .create_player(TenantCreatePlayer {
            username: "CasePlayer".into(),
            password_hash,
            phone_number: "9999999999".into(),
            first_name: Some("Case".into()),
            last_name: Some("Player".into()),
            actor_id: None,
        })
        .await
        .unwrap();

    assert!(second_repo.find_by_id(player.id).await.unwrap().is_none());
    assert_eq!(
        first_repo
            .list(&UserFilterDto {
                username: Some("player".into()),
                sort_by: Some("username".into()),
                sort_order: Some("ASC".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .total,
        1
    );
    assert!(first_repo
        .find_by_id(player.id)
        .await
        .unwrap()
        .unwrap()
        .password_hash
        .is_none());
    assert!(first_repo
        .find_player_for_auth("caseplayer")
        .await
        .unwrap()
        .unwrap()
        .password_hash
        .is_some());

    let updated = first_repo
        .update(
            player.id,
            &UpdateUserDto {
                username: Some("RenamedPlayer".into()),
                phone_number: Some("8888888888".into()),
                first_name: Some("Renamed".into()),
                last_name: None,
                role: Some("admin".into()),
                is_active: Some(true),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(updated.role.as_deref(), Some("player"));
    first_repo
        .set_avatar(player.id, Some("https://cdn.invalid/avatar.png"))
        .await
        .unwrap();
    let replacement = hash("new password", DEFAULT_COST).unwrap();
    first_repo
        .update_password(player.id, &replacement)
        .await
        .unwrap();
    let auth = first_repo
        .find_player_for_auth("renamedplayer")
        .await
        .unwrap()
        .unwrap();
    assert!(verify("new password", auth.password_hash.as_deref().unwrap()).unwrap());

    first_repo.soft_delete(player.id).await.unwrap();
    assert!(first_repo.find_by_id(player.id).await.unwrap().is_none());
    assert!(first_repo
        .find_player_for_auth("renamedplayer")
        .await
        .unwrap()
        .is_none());
    first_repo
        .create_player(TenantCreatePlayer {
            username: "renamedplayer".into(),
            password_hash: hash("another password", DEFAULT_COST).unwrap(),
            phone_number: "7777777777".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();

    first.close().await;
    second.close().await;
}

#[tokio::test]
async fn live_names_conflict_and_require_methods_enforce_role_and_state() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let staff = projection(
        fixture.staff_id,
        "SharedName",
        "staff",
        1,
        true,
        false,
        vec![],
    );
    repo.project_staff(staff).await.unwrap();
    let result = repo
        .create_player(TenantCreatePlayer {
            username: "sharedname".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert!(repo.require_active_staff(fixture.staff_id).await.is_ok());
    assert!(repo.require_active_player(fixture.staff_id).await.is_err());

    repo.project_staff(projection(
        fixture.staff_id,
        "SharedName",
        "staff",
        2,
        false,
        false,
        vec![],
    ))
    .await
    .unwrap();
    assert!(repo.require_active_staff(fixture.staff_id).await.is_err());
    fixture.close().await;
}

#[tokio::test]
async fn staff_projection_never_reuses_a_soft_deleted_player_id() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "deleted-player-id".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    repo.soft_delete(player.id).await.unwrap();
    let before = counts(&fixture).await;
    let result = repo
        .project_staff(projection(
            player.id,
            "replacement-staff",
            "staff",
            2,
            true,
            false,
            vec![],
        ))
        .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn staff_projection_is_revisioned_atomic_and_exactly_scoped() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let initial = projection(
        fixture.staff_id,
        "operator",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    );
    assert_eq!(
        repo.project_staff(initial.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_first = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into(), "staff".into()],
            location_grants: vec![
                TenantLocationRoleGrant {
                    system_key: "staff".into(),
                    location_id: fixture.location_a,
                },
                TenantLocationRoleGrant {
                    system_key: "staff".into(),
                    location_id: fixture.location_a,
                },
            ],
            ..initial.clone()
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, after_first);
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 0,
            ..initial.clone()
        })
        .await,
        Err(AppError::BadRequest(_))
    ));
    assert_eq!(
        repo.location_ids_for_permission(fixture.staff_id, "sessions:write")
            .await
            .unwrap(),
        vec![fixture.location_a]
    );
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "finance:read")
        .await
        .unwrap()
        .is_empty());

    let replaced = TenantStaffProjection {
        member_revision: 2,
        first_name: Some("Updated".into()),
        global_access_role_system_keys: vec!["finance".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "finance".into(),
            location_id: fixture.location_b,
        }],
        ..initial
    };
    repo.project_staff(replaced).await.unwrap();
    assert_eq!(
        repo.location_ids_for_permission(fixture.staff_id, "finance:read")
            .await
            .unwrap(),
        vec![fixture.location_b]
    );
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "sessions:write")
        .await
        .unwrap()
        .is_empty());

    let before = counts(&fixture).await;
    let unknown = repo
        .project_staff(projection(
            fixture.staff_id,
            "operator",
            "staff",
            3,
            true,
            false,
            vec![TenantLocationRoleGrant {
                system_key: "unknown-custom-role".into(),
                location_id: fixture.location_a,
            }],
        ))
        .await;
    assert!(matches!(unknown, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn projection_rejects_non_global_or_template_location_roles_atomically() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let before = counts(&fixture).await;
    let subset = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "finance".into(),
                location_id: fixture.location_a,
            }],
            ..projection(
                fixture.staff_id,
                "subset-staff",
                "staff",
                1,
                true,
                false,
                vec![],
            )
        })
        .await;
    assert!(matches!(subset, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);

    let template = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["template-only".into()],
            ..projection(
                fixture.staff_id,
                "template-staff",
                "staff",
                1,
                true,
                false,
                vec![],
            )
        })
        .await;
    assert!(matches!(template, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn equal_revision_requires_exact_canonical_projection_state() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let initial = projection(
        fixture.staff_id,
        "revision-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    );
    repo.project_staff(initial.clone()).await.unwrap();
    let before = counts(&fixture).await;
    let divergent_profile = repo
        .project_staff(TenantStaffProjection {
            first_name: Some("Divergent".into()),
            ..initial.clone()
        })
        .await;
    assert!(matches!(divergent_profile, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    assert_eq!(
        repo.find_by_id(fixture.staff_id)
            .await
            .unwrap()
            .unwrap()
            .first_name
            .as_deref(),
        Some("Projected")
    );

    let divergent_assignments = repo
        .project_staff(TenantStaffProjection {
            global_access_role_system_keys: vec!["staff".into(), "finance".into()],
            ..initial
        })
        .await;
    assert!(matches!(divergent_assignments, Err(AppError::Conflict(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn admin_visibility_and_inactive_or_deleted_staff_access_are_safe() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let admin_id = Uuid::now_v7();
    repo.project_staff(projection(
        admin_id,
        "administrator",
        "admin",
        1,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    assert_eq!(
        repo.location_ids_for_permission(admin_id, "anything:read")
            .await
            .unwrap(),
        vec![fixture.location_a, fixture.location_b]
    );

    repo.project_staff(projection(
        fixture.staff_id,
        "operator",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    ))
    .await
    .unwrap();
    repo.project_staff(projection(
        fixture.staff_id,
        "operator",
        "staff",
        2,
        true,
        true,
        vec![],
    ))
    .await
    .unwrap();
    assert!(repo
        .location_ids_for_permission(fixture.staff_id, "sessions:write")
        .await
        .unwrap()
        .is_empty());
    let event: (bool, String) = sqlx::query_as(
        "SELECT deleted,payload FROM outbox_events WHERE aggregate_id=? \
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert!(event.0);
    let payload: Value = serde_json::from_str(&event.1).unwrap();
    assert_eq!(payload["deleted"], true);
    fixture.close().await;
}

#[tokio::test]
async fn higher_revision_revocation_ignores_stale_grants_and_profile_collisions() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    repo.project_staff(projection(
        fixture.staff_id,
        "revoked-staff",
        "staff",
        1,
        true,
        false,
        vec![TenantLocationRoleGrant {
            system_key: "staff".into(),
            location_id: fixture.location_a,
        }],
    ))
    .await
    .unwrap();
    repo.create_player(TenantCreatePlayer {
        username: "profile-collision".into(),
        password_hash: hash("password 123", DEFAULT_COST).unwrap(),
        phone_number: "9999999999".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();
    let location_a = fixture.location_a;
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query("UPDATE venue_locations SET is_active=0 WHERE id=?")
                    .bind(location_a.to_string())
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    let before_deactivate = counts(&fixture).await;
    repo.project_staff(TenantStaffProjection {
        username: "profile-collision".into(),
        member_revision: 2,
        is_active: false,
        global_access_role_system_keys: vec!["stale-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "other-stale-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            fixture.staff_id,
            "ignored",
            "staff",
            2,
            false,
            false,
            vec![],
        )
    })
    .await
    .unwrap();
    let after_deactivate = counts(&fixture).await;
    assert_eq!(after_deactivate.1, 0);
    assert_eq!(after_deactivate.2, 0);
    assert_eq!(after_deactivate.3, before_deactivate.3 + 1);
    let state: (String, bool, i64, Option<String>) = sqlx::query_as(
        "SELECT username,is_active,member_revision,deleted_at FROM users WHERE id=?",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(state, ("revoked-staff".into(), false, 2, None));
    let deactivated_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "different-stale-profile".into(),
            first_name: Some("Ignored".into()),
            permissions: vec!["ignored:permission".into()],
            member_revision: 2,
            is_active: false,
            global_access_role_system_keys: vec!["another-stale-role".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "another-stale-role".into(),
                location_id: Uuid::now_v7(),
            }],
            ..projection(
                fixture.staff_id,
                "ignored",
                "staff",
                2,
                false,
                false,
                vec![],
            )
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deactivated_counts);

    repo.project_staff(TenantStaffProjection {
        member_revision: 3,
        global_access_role_system_keys: vec!["finance".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "finance".into(),
            location_id: fixture.location_b,
        }],
        ..projection(
            fixture.staff_id,
            "revoked-staff",
            "staff",
            3,
            true,
            false,
            vec![],
        )
    })
    .await
    .unwrap();
    let before_delete = counts(&fixture).await;
    repo.project_staff(TenantStaffProjection {
        username: "profile-collision".into(),
        member_revision: 4,
        global_access_role_system_keys: vec!["missing-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "missing-location-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(fixture.staff_id, "ignored", "staff", 4, true, true, vec![])
    })
    .await
    .unwrap();
    let after_delete = counts(&fixture).await;
    assert_eq!(after_delete.1, 0);
    assert_eq!(after_delete.2, 0);
    assert_eq!(after_delete.3, before_delete.3 + 1);
    let deleted: (bool, i64, bool) = sqlx::query_as(
        "SELECT is_active,member_revision,deleted_at IS NOT NULL FROM users WHERE id=?",
    )
    .bind(fixture.staff_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(deleted, (false, 4, true));
    let deleted_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "different-deleted-profile".into(),
            last_name: Some("Ignored".into()),
            permissions: vec!["ignored:deleted".into()],
            member_revision: 4,
            deleted: true,
            global_access_role_system_keys: vec!["deleted-stale-role".into()],
            location_grants: vec![TenantLocationRoleGrant {
                system_key: "other-deleted-stale-role".into(),
                location_id: Uuid::now_v7(),
            }],
            ..projection(fixture.staff_id, "ignored", "admin", 4, true, true, vec![],)
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deleted_counts);
    fixture.close().await;
}

#[tokio::test]
async fn first_seen_revocations_ignore_stale_access_payloads_and_fence_older_activation() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    repo.create_player(TenantCreatePlayer {
        username: "live-name-reused-by-tombstone".into(),
        password_hash: hash("password 123", DEFAULT_COST).unwrap(),
        phone_number: "9999999999".into(),
        first_name: None,
        last_name: None,
        actor_id: None,
    })
    .await
    .unwrap();

    let deleted_id = Uuid::now_v7();
    let before_deleted = counts(&fixture).await;
    let deleted = TenantStaffProjection {
        global_access_role_system_keys: vec!["unknown-template-or-stale".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "different-unknown-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            deleted_id,
            "live-name-reused-by-tombstone",
            "staff",
            5,
            true,
            true,
            vec![],
        )
    };
    assert_eq!(
        repo.project_staff(deleted.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_deleted = counts(&fixture).await;
    assert_eq!(after_deleted.0, before_deleted.0 + 1);
    assert_eq!(after_deleted.1, 0);
    assert_eq!(after_deleted.2, 0);
    assert_eq!(after_deleted.3, before_deleted.3 + 1);
    let deleted_state: (bool, i64, bool) = sqlx::query_as(
        "SELECT is_active,member_revision,deleted_at IS NOT NULL FROM users WHERE id=?",
    )
    .bind(deleted_id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(deleted_state, (false, 5, true));
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 4,
            is_active: true,
            deleted: false,
            global_access_role_system_keys: vec!["staff".into()],
            location_grants: vec![],
            ..deleted.clone()
        })
        .await,
        Err(AppError::Conflict(_))
    ));
    let deleted_replay_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "ignored-deleted-replay".into(),
            first_name: Some("Changed".into()),
            permissions: vec!["changed:permission".into()],
            ..deleted
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, deleted_replay_counts);

    let inactive_id = Uuid::now_v7();
    let before_inactive = counts(&fixture).await;
    let inactive = TenantStaffProjection {
        global_access_role_system_keys: vec!["template-only".into(), "missing-role".into()],
        location_grants: vec![TenantLocationRoleGrant {
            system_key: "missing-location-role".into(),
            location_id: Uuid::now_v7(),
        }],
        ..projection(
            inactive_id,
            "first-seen-inactive",
            "admin",
            7,
            false,
            false,
            vec![],
        )
    };
    assert_eq!(
        repo.project_staff(inactive.clone()).await.unwrap(),
        StaffProjectionResult::Applied
    );
    let after_inactive = counts(&fixture).await;
    assert_eq!(after_inactive.0, before_inactive.0 + 1);
    assert_eq!(after_inactive.1, 0);
    assert_eq!(after_inactive.2, 0);
    assert_eq!(after_inactive.3, before_inactive.3 + 1);
    let inactive_state: (bool, i64, Option<String>) =
        sqlx::query_as("SELECT is_active,member_revision,deleted_at FROM users WHERE id=?")
            .bind(inactive_id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(inactive_state, (false, 7, None));
    assert!(matches!(
        repo.project_staff(TenantStaffProjection {
            member_revision: 6,
            is_active: true,
            global_access_role_system_keys: vec!["admin".into()],
            location_grants: vec![],
            ..inactive.clone()
        })
        .await,
        Err(AppError::Conflict(_))
    ));
    let inactive_replay_counts = counts(&fixture).await;
    assert_eq!(
        repo.project_staff(TenantStaffProjection {
            username: "ignored-inactive-replay".into(),
            role: "staff".into(),
            permissions: vec!["changed:inactive".into()],
            ..inactive
        })
        .await
        .unwrap(),
        StaffProjectionResult::Unchanged
    );
    assert_eq!(counts(&fixture).await, inactive_replay_counts);
    fixture.close().await;
}

#[tokio::test]
async fn user_outbox_is_secret_free_and_lease_fencing_rolls_back_everything() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "safe-player".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: Some("Safe".into()),
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    let payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events WHERE aggregate_id=? ORDER BY sequence DESC LIMIT 1",
    )
    .bind(player.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    for forbidden in [
        "password",
        "email",
        "phone",
        "permissions",
        "secret",
        "token",
        "otp",
    ] {
        assert!(!payload.to_ascii_lowercase().contains(forbidden));
    }

    let before = counts(&fixture).await;
    fixture.lease.fail_next_commit();
    let result = repo
        .project_staff(projection(
            fixture.staff_id,
            "fenced-staff",
            "staff",
            1,
            true,
            false,
            vec![TenantLocationRoleGrant {
                system_key: "staff".into(),
                location_id: fixture.location_a,
            }],
        ))
        .await;
    assert!(matches!(result, Err(AppError::Forbidden(_))));
    assert_eq!(counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn staged_services_do_not_connect_to_unreachable_lazy_postgres_and_reject_staff_plans() {
    let fixture = Fixture::new().await;
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let users = UserService::new(postgres.clone(), create_cache(None).await);
    let registered = users
        .register_from_kiosk_tenant(
            fixture.db.clone(),
            KioskRegisterDto {
                username: "service-player".into(),
                password: "password 123".into(),
                phoneNumber: "9999999999".into(),
                firstName: None,
                lastName: None,
            },
        )
        .await
        .unwrap();
    assert!(TenantUserRepository::new(fixture.db.clone())
        .find_player_for_auth(&registered.username)
        .await
        .unwrap()
        .is_some());
    let registered_id = Uuid::parse_str(&registered.userId).unwrap();
    users
        .update_tenant(
            fixture.db.clone(),
            registered_id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: Some("Updated".into()),
                last_name: None,
                role: Some("player".into()),
                is_active: None,
            },
            None,
        )
        .await
        .unwrap();
    let invalid_role = users
        .update_tenant(
            fixture.db.clone(),
            registered_id,
            UpdateUserDto {
                username: None,
                phone_number: None,
                first_name: None,
                last_name: None,
                role: Some("staff".into()),
                is_active: None,
            },
            None,
        )
        .await;
    assert!(matches!(invalid_role, Err(AppError::BadRequest(_))));

    TenantUserRepository::new(fixture.db.clone())
        .project_staff(projection(
            fixture.staff_id,
            "plan-staff",
            "staff",
            1,
            true,
            false,
            vec![],
        ))
        .await
        .unwrap();
    let plan_id = Uuid::now_v7();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO plans(id,name,price,plan_type,validity_days,time_credits,is_active,\
                     created_at,updated_at) VALUES(?, 'Test plan', 10000, 'time_based', 30, 60, 1, ?, ?)",
                )
                .bind(plan_id.to_string())
                .bind(&at)
                .bind(&at)
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    let result = PlayerPlanService::new(postgres)
        .assign_plan_to_player_tenant(
            fixture.db.clone(),
            AssignPlanDto {
                player_id: fixture.staff_id,
                plan_id,
                transaction_id: None,
                purchase_date: None,
            },
            None,
        )
        .await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
    fixture.close().await;
}

#[tokio::test]
async fn staged_tenant_login_authenticates_only_tenant_players() {
    let fixture = Fixture::new().await;
    let repo = TenantUserRepository::new(fixture.db.clone());
    let player = repo
        .create_player(TenantCreatePlayer {
            username: "login-player".into(),
            password_hash: hash("password 123", DEFAULT_COST).unwrap(),
            phone_number: "9999999999".into(),
            first_name: None,
            last_name: None,
            actor_id: None,
        })
        .await
        .unwrap();
    let balance_id = Uuid::now_v7();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    let expiry =
        gaming_cafe_api::time::format_sqlite_timestamp(&(Utc::now() + chrono::Duration::days(1)))
            .unwrap();
    fixture
        .db
        .with_immediate_writer(move |connection| -> BoxFuture<'_, Result<(), AppError>> {
            Box::pin(async move {
                sqlx::query(
                    "INSERT INTO player_plan_balances(id,player_id,device_type,device_sub_type,\
                     kind,remaining_minutes,expiry_date,status,created_at,updated_at) \
                     VALUES(?,?,'PC','HIGH_END_PCS','time',60,?,'active',?,?)",
                )
                .bind(balance_id.to_string())
                .bind(player.id.to_string())
                .bind(expiry)
                .bind(&at)
                .bind(&at)
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

    repo.project_staff(projection(
        fixture.staff_id,
        "login-staff",
        "staff",
        1,
        true,
        false,
        vec![],
    ))
    .await
    .unwrap();
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let cache = create_cache(None).await;
    let users = Arc::new(UserService::new(postgres.clone(), cache.clone()));
    let balances = Arc::new(BalanceService::new(postgres.clone(), cache));
    let auth = AuthService::new(postgres, test_settings(), balances, users);
    let now = Utc::now();
    let device = Device {
        id: Uuid::now_v7(),
        organization_id: fixture.tenant_id,
        location_id: fixture.location_a,
        name: "Login kiosk".into(),
        serial_number: None,
        local_ip_address: None,
        device_type: "PC".into(),
        device_sub_type: "HIGH_END_PCS".into(),
        location: None,
        status: "available".into(),
        registered_kiosk: None,
        registration_status: "registered".into(),
        created_by: None,
        updated_by: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };
    let response = auth
        .login_player_tenant(
            fixture.db.clone(),
            &device,
            LoginDto {
                username: "LOGIN-PLAYER".into(),
                password: "password 123".into(),
            },
            "UTC".into(),
        )
        .await
        .unwrap();
    assert_eq!(response.user.username, "login-player");
    for (username, password) in [
        ("login-player", "wrong password"),
        ("login-staff", "password 123"),
        ("missing-player", "password 123"),
    ] {
        let error = auth
            .login_player_tenant(
                fixture.db.clone(),
                &device,
                LoginDto {
                    username: username.into(),
                    password: password.into(),
                },
                "UTC".into(),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::Api { ref code, .. } if code == "AUTH_INVALID_CREDENTIALS"
        ));
    }
    fixture.close().await;
}

fn test_settings() -> Arc<Settings> {
    Arc::new(Settings {
        roles: Roles::ALL,
        cell_id: None,
        tenant_data_dir: PathBuf::from("unused"),
        database_url: "postgres://unused:unused@127.0.0.1:1/unused".into(),
        control_database_url: None,
        database_listener_url: "postgres://unused:unused@127.0.0.1:1/unused".into(),
        database_min_connections: 0,
        database_max_connections: 1,
        database_acquire_timeout_seconds: 1,
        database_idle_timeout_seconds: 1,
        database_max_lifetime_seconds: 1,
        redis_url: None,
        jwt_secret: "tenant-login-test-secret-at-least-32-characters".into(),
        jwt_access_expiration: "15m".into(),
        jwt_player_expiration: "24h".into(),
        jwt_device_expiration: "365d".into(),
        bcrypt_salt_rounds: 10,
        port: 3000,
        cafe_timezone: "UTC".into(),
        zeptomail_token: None,
        legacy_rest_enabled: false,
        trusted_proxy_cidrs: vec![],
        max_concurrent_requests: 16,
    })
}

fn projection(
    user_id: Uuid,
    username: &str,
    role: &str,
    member_revision: i64,
    is_active: bool,
    deleted: bool,
    location_grants: Vec<TenantLocationRoleGrant>,
) -> TenantStaffProjection {
    TenantStaffProjection {
        user_id,
        email: Some(format!("{username}@example.invalid")),
        username: username.into(),
        first_name: Some("Projected".into()),
        last_name: Some("Staff".into()),
        phone_number: Some("9999999999".into()),
        avatar_url: None,
        role: role.into(),
        permissions: vec!["safe:profile".into()],
        is_active,
        deleted,
        member_revision,
        global_access_role_system_keys: vec![role.into()],
        location_grants,
    }
}

async fn counts(fixture: &Fixture) -> (i64, i64, i64, i64) {
    let pool = fixture.db.read_pool().unwrap();
    let users = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let access = sqlx::query_scalar("SELECT COUNT(*) FROM access_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let locations = sqlx::query_scalar("SELECT COUNT(*) FROM location_role_assignments")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outbox = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    (users, access, locations, outbox)
}

struct Fixture {
    root: PathBuf,
    tenant_id: Uuid,
    db: Arc<TenantDb>,
    lease: Arc<Lease>,
    staff_id: Uuid,
    location_a: Uuid,
    location_b: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("arena360-user-repo-{}", Uuid::now_v7()));
        let tenant_id = Uuid::now_v7();
        let path = tenant_path(&root, tenant_id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
        let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
        let location_a = Uuid::from_u128(1);
        let location_b = Uuid::from_u128(2);
        for (id, slug) in [(location_a, "alpha"), (location_b, "beta")] {
            sqlx::query(
                "INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)",
            )
            .bind(id.to_string())
            .bind(slug)
            .bind(slug)
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
        }
        for (id, key, permissions, is_template) in [
            (
                Uuid::from_u128(10),
                "staff",
                r#"["sessions:read","sessions:write"]"#,
                false,
            ),
            (Uuid::from_u128(11), "admin", r#"["access:manage"]"#, false),
            (Uuid::from_u128(12), "finance", r#"["finance:read"]"#, false),
            (
                Uuid::from_u128(13),
                "template-only",
                r#"["finance:read"]"#,
                true,
            ),
        ] {
            sqlx::query(
                "INSERT INTO access_roles(id,system_key,name,permissions,is_template,created_at,updated_at) \
                 VALUES(?,?,?,?,?,?,?)",
            )
            .bind(id.to_string())
            .bind(key)
            .bind(key)
            .bind(permissions)
            .bind(is_template)
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
        }
        pool.close().await;
        let lease = Arc::new(Lease::default());
        lease.generations.write().unwrap().insert(tenant_id, 1);
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(250),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            lease.clone(),
        )
        .unwrap();
        let db = manager.open(tenant_id).await.unwrap();
        Self {
            root,
            tenant_id,
            db,
            lease,
            staff_id: Uuid::now_v7(),
            location_a,
            location_b,
        }
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}
