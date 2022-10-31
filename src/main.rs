use lambda_dev::{bench, establish_connection_or_get_cache};
use lambda_http::{run, service_fn, Body, Request, Response};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[tokio::test]
async fn test_system_get() -> anyhow::Result<()> {
    let test: String = reqwest::get("http://localhost:9000/lambda-url/lambda_function_01")
        .await?
        .text()
        .await?;

    let v: Vec<SqlxArticle> = serde_json::from_str(&test)?;

    assert_eq!(
        v,
        vec![SqlxArticle {
            id: 1,
            name: "Saki".to_string(),
        }]
    );

    Ok(())
}
#[tokio::test]
async fn test_unit_func() -> anyhow::Result<()> {
    async fn db_setup(transaction: &mut sqlx::Transaction<'_, sqlx::MySql>) -> anyhow::Result<()> {
        // let command = r#"CREATE TABLE IF NOT EXISTS users (id int, name varchar(64));"#;
        // sqlx::query(command).execute(transaction).await?;

        // let (id, name) = (42, "Fumiya");
        // let command = r#"INSERT INTO users VALUES (?, ?);"#;
        // sqlx::query(command)
        //     .bind(id)
        //     .bind(name)
        //     .execute(transaction)
        //     .await?;
        Ok(())
    }
    async fn db_teardown() {
        // tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let request = Request::default();

    let pool = establish_connection_or_get_cache().await;
    let mut transaction = pool.begin().await?;
    let succeed_response =
        lambda_dev::run_test_async(db_setup(&mut transaction), func(request), db_teardown())
            .await?;
    transaction.commit().await?;

    let v: Vec<SqlxArticle> = match succeed_response.into_body() {
        Body::Text(t) => serde_json::from_str(&t)?,
        _ => todo!(),
    };

    assert_eq!(
        v,
        vec![SqlxArticle {
            id: 1,
            name: "Saki".to_string(),
        }]
    );

    Ok(())
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, sqlx::FromRow)]
struct SqlxArticle {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    run(service_fn(func)).await.map_err(|_| core::fmt::Error)?;
    Ok(())
}

async fn func(event: Request) -> anyhow::Result<Response<Body>> {
    let pool = establish_connection_or_get_cache().await;

    let query = r#"SELECT * FROM users;"#;
    async fn select(pool: &sqlx::MySqlPool, query: &str) -> anyhow::Result<Vec<SqlxArticle>> {
        let users = sqlx::query_as::<_, SqlxArticle>(query)
            .fetch_all(pool)
            .await?;
        Ok(users)
    }
    let users: Vec<SqlxArticle> = bench("select all users", select(pool, query)).await?;

    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&users)?.into())
        .map_err(Box::new)?;
    Ok(resp)
}
