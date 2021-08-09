use duino_miner::error::MinerError;

use tokio::net::TcpStream;

use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;

use clap::{AppSettings, Clap};
use tokio::io::{AsyncReadExt, AsyncWriteExt};


#[derive(Clap)]
#[clap(version = "0.1", author = "Black H. <encomblackhat@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "51.15.127.80")]
    host: String,
    #[clap(short, long, default_value = "2811")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let mut username = parity_wordlist::random_phrase(2);
    username.retain(|c| !c.is_whitespace());
    let password: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();

    let mut stream = TcpStream::connect(
        format!("{}:{}", opts.host, opts.port)).await.map_err(|_| MinerError::Connection)?;

    println!("Connected to pool {}:{}", opts.host, opts.port);

    let mut cmd_in: [u8; 200] = [0; 200];

    let n = stream.read(&mut cmd_in).await.map_err(|_| MinerError::RecvCommand)?;
    println!("version: {}", std::str::from_utf8(&cmd_in[..n])?);

    let cmd_job = format!("REGI,{},{},{}@gmail.com\n", username, password, username);
    stream.write(cmd_job.as_bytes()).await.map_err(|_| MinerError::SendCommand)?;
    let n = stream.read(&mut cmd_in).await.map_err(|_| MinerError::RecvCommand)?;
    let reg_status = std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?;

    if reg_status != "OK" {
        println!("register failed: {}", reg_status);
    } else {
        println!("registered {} with {}", username, password);
    }

    Ok(())
}