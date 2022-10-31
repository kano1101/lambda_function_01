use lambda_dev::{bench, establish_connection_or_get_cache};
use lambda_http::{run, service_fn, Body, Request, Response};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use sqlx::Acquire as _;

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
    async fn db_setup(tx: &mut sqlx::Transaction<'_, sqlx::MySql>) -> anyhow::Result<()> {
        let (id, name) = (42, "Fumiya");
        let command = r#"INSERT INTO users VALUES (?, ?);"#;
        sqlx::query(command).bind(id).bind(name).execute(tx).await?;
        Ok(())
    }
    async fn db_teardown(tx: &mut sqlx::Transaction<'_, sqlx::MySql>) -> anyhow::Result<()> {
        let command = r#"DELETE FROM users WHERE name="Fumiya";"#;
        sqlx::query(command).execute(tx).await?;
        Ok(())
    }

    let event = Request::default();

    let pool = establish_connection_or_get_cache().await;
    let mut tx = pool.begin().await?;
    let mut tx = tx.begin().await?;
    db_setup(&mut tx).await?;
    let mut tx = tx.begin().await?;
    let succeed_response = func(&mut tx, event).await?;
    let mut tx = tx.begin().await?;
    db_teardown(&mut tx).await?;
    tx.commit().await?;

    let v: Vec<SqlxArticle> = match succeed_response.into_body() {
        Body::Text(t) => serde_json::from_str(&t)?,
        _ => todo!(),
    };

    assert_eq!(
        v,
        vec![
            SqlxArticle {
                id: 1,
                name: "Saki".to_string(),
            },
            SqlxArticle {
                id: 42,
                name: "Fumiya".to_string(),
            }
        ]
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

    let pool = establish_connection_or_get_cache().await;

    run(service_fn(|event| async move {
        let mut tx = pool.begin().await.unwrap();
        func(&mut tx, event).await
    }))
    .await
    .map_err(|_| core::fmt::Error)?;

    Ok(())
}

async fn func<'a>(
    tx: &mut sqlx::Transaction<'a, sqlx::MySql>,
    _event: Request,
) -> anyhow::Result<Response<Body>> {
    let query = r#"SELECT * FROM users;"#;
    let users = sqlx::query_as::<_, SqlxArticle>(query).fetch_all(tx);
    let users: Vec<SqlxArticle> = bench("select all users", users).await?;

    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(serde_json::to_string(&users)?.into())
        .map_err(Box::new)?;
    Ok(resp)
}
