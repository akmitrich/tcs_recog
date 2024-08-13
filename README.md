# T-bank Voicekit recognition example in Rust

## Before start
Make sure u have two environment variables with your t-bank authentication/authorization secrets:
```bash
TCS_APIKEY="..."
TCS_SECRET="..."
```

For your convinience I added `dotenv` crate into the utility so you may place them in `.env` file.

## Usage
The utility takes an argument with audio file path. The argument is mandatory one. By now the utility accepts only raw `linear16` PCM files. Some of such files are in `audio` folder. So with `cargo`:
```bash
cargo run -- audio/repeat.pcm
```
If you take binary from `target` folder:
```bash
tcs_recog audio/hello.wav
```