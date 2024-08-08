use base64::Engine;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let api_key = std::env::var("TCS_APIKEY").unwrap();
    let secret_key = base64::engine::general_purpose::STANDARD
        .decode(std::env::var("TCS_SECRET").unwrap())
        .unwrap();
    let jwt = generate(&api_key, &secret_key);

    println!("JWT={:?}", jwt);
}

fn generate(api_key: &str, secret_key: &[u8]) -> String {
    let claims = serde_json::json!({
        "iss": "recog",
        "sub": "akmitrich",
        "aud": "tinkoff.cloud.stt",
        "exp": chrono::Local::now().timestamp() + 60,
    });
    let header = jsonwebtoken::Header {
        kid: Some(api_key.to_owned()),
        alg: jsonwebtoken::Algorithm::HS256,
        ..Default::default()
    };
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret_key),
    )
    .unwrap()
}
