use aws_config::meta::region::RegionProviderChain;
use aws_sdk_secretsmanager::{output::GetSecretValueOutput, Client};
use lambda_runtime::{service_fn, Context, Error, LambdaEvent};
use serde_json::{json, Value};
use tracing::info;
use tracing_subscriber;

#[test]
fn test_func() -> Result<(), ()> {
    let context = Context::default();
    let lambda_event = LambdaEvent::<Value>::new(
        serde_json::from_str(r#"{ "body": { "firstName": "Fumiya" } }"#).unwrap(),
        context,
    );
    let rt = tokio::runtime::Runtime::new().unwrap();
    let succeed_response = rt.block_on(async { func(lambda_event).await });
    println!("{:?}", succeed_response);
    assert_ne!(succeed_response.unwrap(), Value::Null);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct SqlxArticle {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();
    let func = service_fn(func);
    lambda_runtime::run(func).await?;
    Ok(())
}

async fn bench<T>(name: &str, f: impl std::future::Future<Output = T>) -> T {
    let now = std::time::Instant::now();
    let r = f.await;
    let duration_time = format!("{:?}", now.elapsed());
    info!(duration_time, name);
    r
}

fn get_url(value: Option<&str>) -> Result<String, Error> {
    let secret_info: Value = if let Some(value) = value {
        serde_json::from_str(value)?
    } else {
        Value::Null
    };

    let host: &str = &secret_info["host_proxy"].as_str().unwrap_or("localhost");
    let username: &str = &secret_info["username"].as_str().unwrap_or("root");
    let password: &str = &secret_info["password"].as_str().unwrap_or("password");
    let database: &str = &secret_info["dbname"].as_str().unwrap_or("test_db");

    let url = format!("mysql://{}:{}@{}/{}", username, password, host, database);

    Ok(url)
}

async fn func(event: LambdaEvent<Value>) -> Result<Value, Error> {
    let region_provider = RegionProviderChain::default_provider().or_else("ap-northeast-3");

    let shared_config = bench("load shared config from env", async {
        aws_config::from_env().region(region_provider).load().await
    })
    .await;
    let client = bench("construct client", async { Client::new(&shared_config) }).await;

    let get_secret_value = client.get_secret_value();
    let secret_id = get_secret_value.secret_id("SecretsManager-02");

    let sent = bench("send", async { secret_id.send().await }).await;
    let resp = sent.unwrap_or(GetSecretValueOutput::builder().build());

    let value = resp.secret_string();

    let url = get_url(value)?;

    let pool: sqlx::MySqlPool = bench(
        "establish connection",
        sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&url),
    )
    .await?;

    let query = r#"SELECT * FROM users;"#;
    let users: Vec<SqlxArticle> = bench("select all users", async {
        sqlx::query_as::<_, SqlxArticle>(query)
            .fetch_all(&pool)
            .await
            .unwrap()
    })
    .await;

    Ok(json!({
        "statusCode": 200,
        "headers": { "content-type": "application/json" },
        "body": format!("{:?}", users),
    }))
}
