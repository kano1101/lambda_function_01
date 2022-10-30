use aws_config::meta::region::RegionProviderChain;
use aws_sdk_secretsmanager::{output::GetSecretValueOutput, Client};
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use serde_json::Value;
use tracing::info;
use tracing_subscriber;

use futures::future::FutureExt as _;

async fn setup() {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}
async fn teardown() {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

pub async fn run_test_async<F1, F2, F3, R>(setup_async: F1, test: F2, teardown_async: F3) -> R
where
    F1: std::future::Future,
    F2: std::future::Future<Output = R>,
    F3: std::future::Future,
{
    setup_async.await;
    let result = std::panic::AssertUnwindSafe(test).catch_unwind().await;
    teardown_async.await;

    match result {
        Err(err) => {
            std::panic::resume_unwind(err);
        }
        Ok(ok) => return ok,
    }
}

#[tokio::test]
async fn test_func() -> Result<(), ()> {
    let request = Request::default();

    let succeed_response = run_test_async(setup(), func(request), teardown())
        .await
        .map_err(|_| ())?;
    println!("{:?}", succeed_response);

    Ok(())
}

use async_trait::async_trait;
#[async_trait]
trait LogWithMessage: Clone {
    async fn output<T>(
        &self,
        message: impl Into<&str> + Send,
        target_fn: impl std::future::Future<Output = T> + Send,
    ) -> T;
}
#[derive(Default, Clone)]
struct Bench {}
impl Bench {
    pub fn new() -> Bench {
        Bench {}
    }
}
#[async_trait]
impl LogWithMessage for Bench {
    async fn output<T>(
        &self,
        message: impl Into<&str> + Send,
        target_fn: impl std::future::Future<Output = T> + Send,
    ) -> T {
        let now = std::time::Instant::now();
        let r = target_fn.await;
        let duration_time = format!("{:?}", now.elapsed());
        let message = message.into();
        info!(duration_time, message);
        r
    }
}

#[derive(Default)]
struct GetUrlBySecretValue {
    url: Arc<Mutex<String>>,
    bench: Bench,
}
impl<'a> GetUrlBySecretValue {
    pub fn new() -> GetUrlBySecretValue {
        Self {
            url: Arc::new(Mutex::new("".to_string())),
            bench: Bench::new(),
        }
    }
}
#[async_trait]
trait GetUrl {
    async fn load_url(&self) -> Option<()>;
    async fn get_url(&self) -> Option<String>;
}
#[async_trait]
impl GetUrl for GetUrlBySecretValue {
    async fn load_url(&self) -> Option<()> {
        println!("テスト12");
        let region_provider = RegionProviderChain::default_provider().or_else("ap-northeast-3");
        println!("テスト13");

        let shared_config = self
            .bench
            .output("load shared config from env", async {
                aws_config::from_env().region(region_provider).load().await
            })
            .await;
        println!("テスト14");
        let client = self
            .bench
            .output("construct client", async { Client::new(&shared_config) })
            .await;
        println!("テスト15");

        let get_secret_value = client.get_secret_value();
        println!("テスト16");
        let secret_id = get_secret_value.secret_id("SecretsManager-02");
        println!("テスト17");

        let sent = self
            .bench
            .output("send", async { secret_id.send().await })
            .await;
        println!("テスト18");
        let resp = sent.unwrap_or(GetSecretValueOutput::builder().build());

        println!("テスト19");
        let value = resp.secret_string();

        println!("{:?}", value);
        let secret_info: Option<Value> = if let Some(value) = value {
            serde_json::from_str(value).ok()
        } else {
            None
        };

        let url = if let Some(secret_info) = secret_info {
            let host: &str = &secret_info["host_proxy"].as_str().unwrap_or("localhost");
            let username: &str = &secret_info["username"].as_str().unwrap_or("root");
            let password: &str = &secret_info["password"].as_str().unwrap_or("password");
            let database: &str = &secret_info["dbname"].as_str().unwrap_or("test_db");

            format!("mysql://{}:{}@{}/{}", username, password, host, database)
        } else {
            let host: &str = "localhost";
            let username: &str = "root";
            let password: &str = "password";
            let database: &str = "test_db";

            format!("mysql://{}:{}@{}/{}", username, password, host, database)
        };

        *(self.url.lock().await) = url;

        Some(())
    }
    async fn get_url(&self) -> Option<String> {
        let lock = self.url.lock().await;
        Some(lock.to_string())
    }
}

use std::sync::Arc;
use tokio::sync::Mutex;
#[derive(Default)]
struct MySqlPoolByGetSecretValue {
    // url: GetUrlBySecretValue,
    pool: Arc<Mutex<Option<sqlx::MySqlPool>>>,
    bench: Bench,
}
impl MySqlPoolByGetSecretValue {
    pub async fn new() -> MySqlPoolByGetSecretValue {
        println!("テスト8");
        // let rt = tokio::runtime::Runtime::new().expect("ランタイム起動エラー");
        println!("テスト9");
        let url = GetUrlBySecretValue::new();
        println!("テスト10");
        // rt.block_on(async { url.load_url().await });
        url.load_url().await;
        println!("テスト11");

        let c = async {
            let url = url.get_url().await.unwrap();
            println!("{:?}", &url);
            sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(5)
                .connect(&url)
                .await
                .unwrap()
        };

        let default = MySqlPoolByGetSecretValue::default();
        let pool = default.establish_connection(c).await;
        let pool = Some(pool);
        let pool = Arc::new(Mutex::new(pool));
        Self {
            // url: url,
            pool: pool,
            bench: Bench::new(),
        }
    }
    async fn establish_connection(
        &self,
        connect_fn: impl std::future::Future<Output = sqlx::MySqlPool> + Send,
    ) -> sqlx::MySqlPool {
        let pool = self.bench.output("establish connection", connect_fn).await;
        pool
        // self.set_pool(pool).await;
        // return self.get_pool().await;
    }
}
#[async_trait]
trait HoldPoolFromUrl<T> {
    async fn get(&self) -> Arc<Mutex<Option<T>>>;
    // async fn set_pool(&self, pool: T) -> Option<()> {
    //     let lock = self.get().await;
    //     let mut n = lock.lock().await;
    //     *n = Some(pool);
    //     Some(())
    // }
    async fn get_pool(&self) -> Arc<Mutex<Option<T>>> {
        Arc::clone(&self.get().await)
    }
}
#[async_trait]
impl HoldPoolFromUrl<sqlx::MySqlPool> for MySqlPoolByGetSecretValue {
    async fn get(&self) -> Arc<Mutex<Option<sqlx::MySqlPool>>> {
        Arc::clone(&self.pool)
    }
}
async fn establish_connection_or_get_cache() -> Arc<Mutex<Option<sqlx::MySqlPool>>> {
    // static mut POOL: Option<Box<sqlx::MySqlPool>> = None;
    println!("テスト3");
    static mut POOL: Option<MySqlPoolByGetSecretValue> = None;
    println!("テスト4");
    unsafe {
        println!("テスト5");
        if POOL.is_none() {
            println!("テスト6");
            POOL = Some(MySqlPoolByGetSecretValue::new().await);
            println!("テスト7");
        }
    }
    let result = match unsafe { POOL.as_ref() } {
        None => todo!(),
        Some(pool) => pool.get_pool(),
    };
    result.await
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct SqlxArticle {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    run(service_fn(func)).await?;
    Ok(())
}

async fn func(event: Request) -> Result<Response<Body>, Error> {
    println!("テスト1");
    let pool = establish_connection_or_get_cache().await;
    println!("テスト2");
    let pool = pool.lock().await;
    let pool: &Option<sqlx::MySqlPool> = &(*pool);
    let pool = pool.as_ref().unwrap();

    let (_parts, body) = event.into_parts();

    if let Body::Text(body) = body {
        let json: Value = serde_json::from_str(&body).unwrap();
        let id = json["id"].as_str();
        let name = json["name"].as_str();

        if let (Some(id), Some(name)) = (id, name) {
            let mut transaction = pool.begin().await?;
            let command = r#"CREATE TABLE IF NOT EXISTS users (id int, name varchar(64));"#;
            sqlx::query(command).execute(&mut transaction).await?;
            transaction.commit().await?;

            let mut transaction = pool.begin().await?;
            let command = r#"INSERT INTO users VALUES (?, ?);"#;
            sqlx::query(command)
                .bind(id)
                .bind(name)
                .execute(&mut transaction)
                .await?;
            transaction.commit().await?;
        }
    }

    let bench = Bench::new();
    let query = r#"SELECT * FROM users;"#;
    let users: Vec<SqlxArticle> = bench
        .output("select all users", async {
            sqlx::query_as::<_, SqlxArticle>(query)
                .fetch_all(pool)
                .await
                .unwrap()
        })
        .await;

    let resp = Response::builder()
        .status(200)
        .header("content-type", "application/json")
        .body(format!("{:?}", users).into())
        .map_err(Box::new)?;
    Ok(resp)
}
