# Lambda function 01

## 確認した環境
```
% $SHELL --version
zsh 5.8.1 (x86_64-apple-darwin21.0)
% git --version
git version 2.37.0 (Apple Git-136)
% mysql --version
mysql  Ver 8.0.31 for macos12.6 on arm64 (Homebrew)
% cargo --version
cargo 1.65.0 (4bc8f24d3 2022-10-20)
```

## コマンドの実行手順

### 環境の構築(構築は一度完了すれば二度はしなくて良い)
```
cargo install sqlx-cli
cargo install cargo-lambda
```

### git clone 手順(多くの場合一度クローンすれば二度しなくて良い)
```
cd ~
mkdir -p development/rust/marble/lambda_test/
cd development/rust/marble/lambda_test/
git clone git@github.com:kano1101/lambda_function_01.git
```

### プロジェクトディレクトリに移動
```
cd ~/development/rust/marble/lambda_test/lambda_function_01/
```

### データベースのセットアップ(一度セットアップすれば二度は行わなくて良い)
※下記コマンド中の`password`は好きに変更して良い
```
export PASSWORD=password
echo "DATABASE_URL=mysql://root:${PASSWORD}@localhost:3306/test_db" >> .env
sqlx database create
```

### マイグレーション
```
sqlx migrate run
```

### ローカルにバックグラウンドでLambdaを起動
```
cargo lambda watch &
```

### テストの実行
```
curl -L http://localhost:9000/lambda-url/lambda_function_01
cargo lambda invoke lambda_function_01 --data-ascii "{}"
cargo test -- --test-threads=1
```

### バックグラウンドに起動したLambdaを終了
```
ps aux | grep "watch" | grep -v grep | awk '{ print "kill -9", $2 }' | sudo sh
```

## デプロイ用ビルド

### 実行コマンド
```
cargo lambda build --output-format zip --release
```
上記コマンドを実行することで生成された`./target/lambda/lambda_function_01/bootstrap.zip`をAWS Lambdaにデプロイします。
