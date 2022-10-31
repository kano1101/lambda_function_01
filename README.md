# Lambda function 01

## コマンド紹介

### セットアップ手順(わからない人向け)
```
cd ~
mkdir -p development/rust/marble/lambda_test/
cd development/rust/marble/lambda_test/
git clone git@github.com:kano1101/lambda_function_01.git
cd lambda_function_01/
```

### ローカルにバックグラウンドでLambdaを起動
```
cargo lambda watch &
```

### テストの実行
```
cargo lambda invoke lambda_function_01 --data-ascii "{}"
cargo test -- --test-threads=1
curl -L http://localhost:9000/lambda-url/lambda_function_01
```

### バックグラウンドに起動したLambdaを終了
```
ps aux | grep "watch" | grep -v grep | awk '{ print "kill -9", $2 }' | sh
```

## デプロイ用ビルド

### 実行コマンド
```
cargo lambda build --output-format zip --release
```
上記コマンドを実行することで生成された`./target/lambda/lambda_function_01/bootstrap.zip`をAWS Lambdaにデプロイします。
