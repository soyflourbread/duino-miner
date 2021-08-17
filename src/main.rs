use duino_miner::error::MinerError;

use serde::{Deserialize, Serialize};

use std::time::{Duration, SystemTime};

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use log::{error, info, warn};

use rand::Rng;
use sha1::{Digest, Sha1};

use clap::{AppSettings, Clap, Subcommand};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub connections: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub username: String,
    pub device_name: String,
    pub device_type: String,
    pub chip_id: String,
    pub firmware: String,
    pub target_rate: u32,
}

#[derive(Clap)]
#[clap(version = "0.1", author = "Black H. <encomblackhat@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "config.yaml")]
    config_file: String,
    #[clap(subcommand)]
    sub_command: SubCommands,
}

#[derive(Subcommand)]
enum SubCommands {
    #[clap(version = "0.1", author = "Black H. <encomblackhat@gmail.com>")]
    Generate(Generate),
    Run(Run),
}

#[derive(Clap)]
struct Generate {
    #[clap(short, long, default_value = "my_username")]
    username: String,
    #[clap(long, default_value = "16")]
    device_count: u32,
    #[clap(long, default_value = "avr-")]
    device_name_prefix: String,
    #[clap(long, default_value = "AVR")]
    device_type: String,
    #[clap(long, default_value = "Official AVR Miner v2.6")]
    firmware: String,
    #[clap(long, default_value = "190")]
    target_rate: u32,
}

#[derive(Clap)]
struct Run {
    #[clap(short, long)]
    pool: Option<String>,
}

fn generate_8hex() -> String {
    const HEX_ARRAY: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    let mut result = String::new();

    for _ in 0..8 {
        let n: usize = rand::thread_rng().gen_range(0..16);
        result.push(HEX_ARRAY[n]);
    }

    result
}

async fn generate_config(
    file_path: String,
    gen: &Generate,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut device_vec: Vec<Device> = Vec::new();

    for i in 0..gen.device_count {
        let device = Device {
            username: gen.username.clone(),
            device_name: format!("{}{}", gen.device_name_prefix, i + 1),
            device_type: gen.device_type.clone(),
            chip_id: format!("DUCOID{}", generate_8hex()),
            firmware: gen.firmware.clone(),
            target_rate: gen.target_rate,
        };

        device_vec.push(device);
    }

    let c = Config {
        devices: device_vec,
    };
    let c_serial = serde_yaml::to_string(&c)?;

    let mut f = File::create(file_path).await?;
    f.write_all(c_serial.as_bytes()).await?;

    Ok(())
}

fn sha1_digest(input: &str) -> String {
    let mut hasher = Sha1::new();
    sha1::Digest::update(&mut hasher, input.as_bytes());

    let h = hasher.finalize();
    format!("{:x}", h)
}

async fn get_pool_info() -> Result<Pool, MinerError> {
    let pool: Pool = reqwest::get("http://51.15.127.80:4242/getPool")
        .await
        .map_err(|_| MinerError::Connection)?
        .json()
        .await
        .map_err(|_| MinerError::Connection)?;

    Ok(pool)
}

async fn start_miner(device: Device, pool: String) -> Result<(), MinerError> {
    let heatup_duration: u64 = rand::thread_rng().gen_range(10..10000);
    tokio::time::sleep(Duration::from_millis(heatup_duration)).await;

    let mut stream = TcpStream::connect(&pool)
        .await
        .map_err(|_| MinerError::Connection)?;

    info!("{} connected to pool {}", device.device_name, pool);

    let mut cmd_in: [u8; 200] = [0; 200];
    let n = stream
        .read(&mut cmd_in)
        .await
        .map_err(|_| MinerError::RecvCommand)?;
    info!(
        "version: {}",
        std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?
    );

    let expected_interval = 1000000u128 / device.target_rate as u128;

    loop {
        let cmd_job = format!("JOB,{},{}\n", device.username, device.device_type);
        stream
            .write(cmd_job.as_bytes())
            .await
            .map_err(|_| MinerError::SendCommand)?;

        let n = stream
            .read(&mut cmd_in)
            .await
            .map_err(|_| MinerError::RecvCommand)?;
        let job = std::str::from_utf8(&cmd_in[..n])
            .map_err(|_| MinerError::InvalidUTF8)?
            .trim();

        let args: Vec<&str> = job.split(',').collect();
        if args.len() < 3 {
            return Err(MinerError::MalformedJob(job.to_string()));
        }

        let last_block_hash = args[0];
        let expected_hash = args[1];
        let diff = args[2]
            .parse::<u32>()
            .map_err(|_| MinerError::MalformedJob(job.to_string()))?
            * 100
            + 1;

        info!(
            "last: {}, expected: {}, diff: {}",
            last_block_hash, expected_hash, diff
        );

        let start = SystemTime::now();

        for duco_numeric_result in 0..diff {
            let h = format!("{}{}", last_block_hash, duco_numeric_result);
            let result = sha1_digest(h.as_str());

            if result == expected_hash {
                let end = SystemTime::now();
                let duration = end.duration_since(start).unwrap().as_micros();
                let real_rate = duco_numeric_result as f64 / duration as f64 * 1000000f64;

                let expected_duration = expected_interval * duco_numeric_result as u128;

                if duration < expected_duration {
                    let wait_duration = (expected_duration - duration) as u64;
                    tokio::time::sleep(Duration::from_micros(wait_duration)).await;
                    info!("waited {} micro sec", wait_duration);
                } else {
                    warn!(
                        "system too slow, lag {} micro sec",
                        duration - expected_duration
                    );
                }

                let end = SystemTime::now();
                let duration = end.duration_since(start).unwrap().as_micros();
                let emu_rate = duco_numeric_result as f64 / duration as f64 * 1000000f64;

                let lag_duration: u64 = rand::thread_rng().gen_range(0..100);
                tokio::time::sleep(Duration::from_millis(lag_duration)).await;

                let cmd_out = format!(
                    "{},{:.2},{},{},{}\n",
                    duco_numeric_result,
                    emu_rate,
                    device.firmware,
                    device.device_name,
                    device.chip_id
                );
                stream
                    .write(cmd_out.as_bytes())
                    .await
                    .map_err(|_| MinerError::SendCommand)?;

                let n = stream
                    .read(&mut cmd_in)
                    .await
                    .map_err(|_| MinerError::RecvCommand)?;
                let resp = std::str::from_utf8(&cmd_in[..n])
                    .map_err(|_| MinerError::InvalidUTF8)?
                    .trim();

                if resp == "GOOD" {
                    info!(
                        "result good, result: {}, rate: {:.2}, real: {:.2}",
                        duco_numeric_result, emu_rate, real_rate
                    );
                } else if resp == "BLOCK" {
                    info!(
                        "FOUND BLOCK!, result: {}, rate: {:.2}, real: {:.2}",
                        duco_numeric_result, emu_rate, real_rate
                    );
                } else {
                    warn!(
                        "resp: {}, result: {}, rate: {:.2}, real: {:.2}",
                        resp, duco_numeric_result, emu_rate, real_rate
                    );
                }

                break;
            }
        }
    }
}

async fn start_miners(devices: Vec<Device>, pool: Option<String>) {
    loop {
        let pool = if let Some(pool) = pool.clone() {
            pool
        } else {
            let pool = get_pool_info().await.unwrap_or(Pool {
                name: "Default pool".to_string(),
                ip: "server.duinocoin.com".to_string(),
                port: 2813,
                connections: 1,
            });

            format!("{}:{}", pool.ip, pool.port)
        };

        let mut futures_vec = Vec::new();

        for device in &devices {
            let f = start_miner(device.clone(), pool.clone());
            futures_vec.push(f);
        }

        match futures::future::try_join_all(futures_vec).await {
            Ok(_) => error!("exited without error"),
            Err(e) => error!("exited with error: {:?}", e),
        }

        let hiatus_duration: u64 = rand::thread_rng().gen_range(30..200);
        tokio::time::sleep(Duration::from_secs(hiatus_duration)).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let opts: Opts = Opts::parse();

    match opts.sub_command {
        SubCommands::Generate(gen) => {
            generate_config(opts.config_file, &gen).await?;
        }
        SubCommands::Run(run) => {
            let c_serial = tokio::fs::read_to_string(opts.config_file).await?;
            let c: Config = serde_yaml::from_str(c_serial.as_str())?;

            info!("running with {} miners", c.devices.len());

            start_miners(c.devices, run.pool).await;
        }
    }

    Ok(())
}
