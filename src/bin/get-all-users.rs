use lambda_dev::establish_connection_or_get_cache;
use lambda_http::{run, service_fn, Body, Request, Response};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use async_trait::async_trait;

#[serde_as]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, sqlx::FromRow)]
struct User {
    pub id: i32,
    pub name: String,
}

#[derive(Debug)]
enum RepositoryError {
    CannotEstablish,
}
impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CannotEstablish => write!(f, "cannot establish"),
        }
    }
}
impl std::error::Error for RepositoryError {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let pool = establish_connection_or_get_cache().await.ok_or_else(|| {
        tracing::info!("repository error cannot establish");
        RepositoryError::CannotEstablish
    })?;

    run(service_fn(|event| async move { func(pool, &event).await }))
        .await
        .map_err(|_| core::fmt::Error)?;

    Ok(())
}

async fn func<'a>(pool: &MySqlPool, _event: &Request) -> anyhow::Result<Response<Body>> {
    let users = pool.all_users().await?;

    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&users)?.into())
        .map_err(Box::new)?;

    Ok(resp)
}

use sqlx::{Acquire, MySql, MySqlPool};

/// `Acquire<'_, Database = MySql>`のエイリアス
pub trait MySqlAcquire<'c>: Acquire<'c, Database = MySql> + Send {}
impl<'c, T> MySqlAcquire<'c> for T where T: Acquire<'c, Database = MySql> + Send {}

#[async_trait]
trait IUserRepository {
    async fn clear_users(
        &self,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<()>;
    async fn insert_user(
        &self,
        name: &str,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<()>;
    async fn select_user(
        &self,
        name: &str,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<Option<User>>;
    async fn select_all_users(
        &self,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<Vec<User>>;
}

struct UserRepository;
#[async_trait]
impl IUserRepository for UserRepository {
    async fn clear_users(
        &self,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<()> {
        let mut conn = executor.acquire().await?;
        sqlx::query!("TRUNCATE TABLE users")
            .execute(&mut *conn)
            .await?;
        Ok(())
    }
    async fn insert_user(
        &self,
        name: &str,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<()> {
        let mut conn = executor.acquire().await?;
        sqlx::query!("INSERT INTO users (id, name) VALUES (42, ?)", name) // TODO: 42
            .execute(&mut *conn)
            .await?;
        Ok(())
    }
    async fn select_user(
        &self,
        name: &str,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<Option<User>> {
        let mut conn = executor.acquire().await?;
        let user = sqlx::query_as("SELECT * FROM users WHERE name = ?")
            .bind(name)
            .fetch_optional(&mut *conn);
        let user = user.await;
        Ok(user?)
    }
    async fn select_all_users(
        &self,
        executor: impl MySqlAcquire<'_> + 'async_trait,
    ) -> anyhow::Result<Vec<User>> {
        let mut conn = executor.acquire().await?;
        let users = sqlx::query_as("SELECT * FROM users").fetch_all(&mut *conn);
        let users = users.await;
        Ok(users?)
    }
}

#[async_trait]
trait IUserService {
    async fn clear_users(&self) -> anyhow::Result<()>;
    async fn save_user(&self, name: &str) -> anyhow::Result<()>;
    async fn all_users(&self) -> anyhow::Result<Vec<User>>;
}

#[async_trait]
impl IUserService for MySqlPool {
    async fn clear_users(&self) -> anyhow::Result<()> {
        let repo = UserRepository;
        let mut tx = self.begin().await?;
        // トランザクションでUserRepoの関数を実行できる
        let succeed_or_not = repo.clear_users(&mut tx).await;
        if succeed_or_not.is_err() {
            tx.rollback().await?;
            anyhow::bail!("テーブルのクリアに失敗しました。")
        }
        tx.commit().await?;
        Ok(())
    }
    async fn save_user(&self, name: &str) -> anyhow::Result<()> {
        let repo = UserRepository;
        let mut tx = self.begin().await?;
        // トランザクションでUserRepoの関数を実行できる
        let user = repo.select_user(name, &mut tx).await?;
        if user.is_some() {
            tx.rollback().await?;
            anyhow::bail!("既に使用されている名前です。")
        }
        repo.insert_user(name, &mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
    async fn all_users(&self) -> anyhow::Result<Vec<User>> {
        let repo = UserRepository;
        // トランザクションを開始せずに実行できる
        let users = repo.select_all_users(self).await?;
        Ok(users)
    }
}

#[cfg(test)]
mod tests {
    use super::{func, IUserService, User};
    use async_trait::async_trait;
    use lambda_dev::establish_connection_or_get_cache;
    use lambda_http::{Body, Request};
    #[async_trait]
    trait UserServiceForTest: IUserService {
        async fn setup(&self) -> anyhow::Result<()>;
        async fn teardown(&self) -> anyhow::Result<()>;
    }

    #[async_trait]
    impl UserServiceForTest for sqlx::mysql::MySqlPool {
        async fn setup(&self) -> anyhow::Result<()> {
            self.clear_users().await?;
            self.save_user("Akira").await?;
            Ok(())
        }
        async fn teardown(&self) -> anyhow::Result<()> {
            self.clear_users().await?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_system_get() -> anyhow::Result<()> {
        let pool = establish_connection_or_get_cache()
            .await
            .ok_or(core::fmt::Error)?;

        assert!(pool.setup().await.is_ok());

        let users: String = reqwest::get("http://localhost:9000/lambda-url/get-all-users")
            .await?
            .text()
            .await?;

        assert!(pool.teardown().await.is_ok());

        let v: Vec<User> = serde_json::from_str(&users)?;

        assert_eq!(
            v,
            vec![User {
                id: 42, // TODO: 42
                name: "Akira".to_string(),
            }]
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_unit_all_func() -> anyhow::Result<()> {
        let event = Request::default();

        let pool = &sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect("mysql://root:password@localhost/test_db")
            .await?;
        assert!(pool.setup().await.is_ok());
        assert_eq!(
            func(pool, &event).await.unwrap().body(),
            &Body::Text(r#"[{"id":42,"name":"Akira"}]"#.into()) // TODO: 42
        );
        assert!(pool.teardown().await.is_ok());

        Ok(())
    }
}
