use base64::Engine;
use tokio::{io::AsyncReadExt, sync::mpsc};
use tokio_stream::StreamExt;

mod tcs {
    tonic::include_proto!("tinkoff.cloud.stt.v1");
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let api_key = std::env::var("TCS_APIKEY").unwrap();
    let secret_key = base64::engine::general_purpose::STANDARD
        .decode(std::env::var("TCS_SECRET").unwrap())
        .unwrap();
    let jwt = generate_token(&api_key, &secret_key);
    println!("JWT={:?}", jwt);

    let mut client = grpc_client().await;
    let src = create_stream(
        tokio::fs::File::open(std::env::args().nth(1).unwrap())
            .await
            .unwrap(),
    );
    let mut req = tonic::Request::new(src);
    req.metadata_mut()
        .append("authorization", format!("Bearer {}", jwt).parse().unwrap());
    let resp = client.streaming_recognize(req).await.unwrap();
    println!("OK {:?} --- {:?}", client, resp);
    let mut answer_stream = resp.into_inner();
    while let Some(answer) = answer_stream.next().await {
        match answer {
            Ok(recog) => {
                let Some(result) = recog.results.first() else {
                    continue;
                };
                let Some(recognized) = &result.recognition_result else {
                    continue;
                };
                let (Some(start), Some(end)) = (
                    recognized.start_time.as_ref().map(convert_duration),
                    recognized.end_time.as_ref().map(convert_duration),
                ) else {
                    continue;
                };
                println!("\nRECOG: {:#?}\n", recognized);
                let diff = end - start;
                if diff > chrono::Duration::seconds(3) && !result.is_final {
                    println!("NOINPUT TIMEOUT.");
                    break;
                }
            }
            Err(e) => {
                println!("FAILED: {:?}", e);
            }
        }
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    println!("MAIN is over...");
}

fn generate_token(api_key: &str, secret_key: &[u8]) -> String {
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

fn create_stream<R>(mut r: R) -> impl tokio_stream::Stream<Item = tcs::StreamingRecognizeRequest>
where
    R: tokio::io::AsyncRead + Send + Sync + Unpin + std::fmt::Debug + 'static,
{
    let (tx, rx) = mpsc::channel(1024);
    tokio::spawn(async move {
        let req = tcs::StreamingRecognizeRequest {
            streaming_request: Some(
                tcs::streaming_recognize_request::StreamingRequest::StreamingConfig(
                    tcs::StreamingRecognitionConfig {
                        config: Some(tcs::RecognitionConfig {
                            encoding: tcs::AudioEncoding::Linear16.into(),
                            sample_rate_hertz: 8000,
                            language_code: String::from("ru-RU"),
                            max_alternatives: 1,
                            profanity_filter: false,
                            speech_contexts: vec![],
                            enable_automatic_punctuation: true,
                            model: String::new(),
                            num_channels: 1,
                            enable_denormalization: true,
                            enable_sentiment_analysis: true,
                            enable_gender_identification: true,
                            vad: Some(tcs::recognition_config::Vad::VadConfig(
                                tcs::VoiceActivityDetectionConfig {
                                    min_speech_duration: 0.0,
                                    max_speech_duration: 20.0,
                                    silence_duration_threshold: 1.,
                                    silence_prob_threshold: 0.5,
                                    aggressiveness: 0.5,
                                    silence_max: 2.0,
                                    silence_min: 1.0,
                                },
                            )),
                        }),
                        single_utterance: false,
                        interim_results_config: Some(tcs::InterimResultsConfig {
                            enable_interim_results: true,
                            interval: 1.0,
                        }),
                    },
                ),
            ),
        };
        tx.send(req).await.unwrap();
        for n in 0..5 {
            let req = tcs::StreamingRecognizeRequest {
                streaming_request: Some(
                    tcs::streaming_recognize_request::StreamingRequest::AudioContent(vec![
                        0;
                        16000
                    ]),
                ),
            };
            tx.send(req).await.unwrap();
            println!("{}. Sent silence", n);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        let mut buf = [0; 1024];
        let mut count = 0;
        loop {
            count += 1;
            let n = r.read(&mut buf[..]).await.unwrap();
            if n == 0 {
                break;
            }
            let req = tcs::StreamingRecognizeRequest {
                streaming_request: Some(
                    tcs::streaming_recognize_request::StreamingRequest::AudioContent(Vec::from(
                        &buf[..n],
                    )),
                ),
            };
            tx.send(req).await.unwrap();
            println!("{:6}. Sent {} bytes", count, n);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
        println!("Streaming the {:?} STOPPED.", r);
        for i in 0.. {
            let req = tcs::StreamingRecognizeRequest {
                streaming_request: Some(
                    tcs::streaming_recognize_request::StreamingRequest::AudioContent(vec![0; 1024]),
                ),
            };
            tx.send(req).await.unwrap();
            println!("{}. Sent zeroes", i);
            tokio::time::sleep(tokio::time::Duration::from_millis(256)).await;
        }
    });
    tokio_stream::wrappers::ReceiverStream::new(rx)
}

pub fn tls_config() -> tonic::transport::ClientTlsConfig {
    tonic::transport::ClientTlsConfig::new().with_native_roots()
}

async fn grpc_client() -> tcs::speech_to_text_client::SpeechToTextClient<tonic::transport::Channel>
{
    let channel = tonic::transport::Channel::from_static("https://api.tinkoff.ai:443")
        .tls_config(tls_config())
        .unwrap()
        .connect()
        .await
        .unwrap();
    tcs::speech_to_text_client::SpeechToTextClient::new(channel)
}

fn convert_duration(proto: &prost_types::Duration) -> chrono::Duration {
    chrono::Duration::new(proto.seconds, proto.nanos as _).unwrap()
}
