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

    let (tx, rx) = mpsc::channel(1024);
    let src_name = std::env::args().nth(1).unwrap();
    let mut src_file = tokio::fs::File::open(&src_name).await.unwrap();
    tokio::spawn(async move {
        let mut buf = [0; 1024];
        let mut count = 0;
        loop {
            count += 1;
            let n = src_file.read(&mut buf[..]).await.unwrap();
            if n == 0 {
                break;
            }
            tx.send(Some(Vec::from(&buf[..n]))).await.unwrap();
            println!("{:6}. Sent {} bytes", count, n);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        tx.send(None).await.unwrap();
        println!("File {:?} streaming STOPPED", src_name);
    });

    let mut client = grpc_client().await;
    let config = tcs::StreamingRecognizeRequest {
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
    let src = audio_stream(rx, config);
    let mut req = tonic::Request::new(src);
    req.metadata_mut()
        .append("authorization", format!("Bearer {}", jwt).parse().unwrap());
    let resp = client.streaming_recognize(req).await.unwrap();
    println!("OK {:?} --- {:?}", client, resp);
    let mut answer_stream = resp.into_inner();
    while let Some(answer) = answer_stream.next().await {
        match answer {
            Ok(recog) => {
                println!("\nRECOG: {:#?}\n", recog);
            }
            Err(e) => {
                println!("FAILED: {:?}", e);
            }
        }
    }
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

fn audio_stream(
    mut src: mpsc::Receiver<Option<Vec<u8>>>,
    config: tcs::StreamingRecognizeRequest,
) -> impl tokio_stream::Stream<Item = tcs::StreamingRecognizeRequest> {
    let (tx, rx) = mpsc::channel(1024);
    tokio::spawn(async move {
        tx.send(config).await.unwrap();
        while let Some(event) = src.recv().await {
            let Some(frame) = event else {
                break;
            };
            let req = tcs::StreamingRecognizeRequest {
                streaming_request: Some(
                    tcs::streaming_recognize_request::StreamingRequest::AudioContent(frame),
                ),
            };
            tx.send(req).await.unwrap();
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
