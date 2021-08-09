use duino_miner::error::MinerError;

use serde::{Serialize, Deserialize};

use std::time::{SystemTime, Duration};

use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::fs::File;

use sha1::{Sha1, Digest};
use rand::Rng;

use clap::{AppSettings, Clap, Subcommand};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub devices: Vec<Device>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub host: String,
    pub port: u16,
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
    #[clap(short, long, default_value = "149.91.88.18")]
    host: String,
    #[clap(short, long, default_value = "6000")]
    port: u16,
    #[clap(short, long, default_value = "my_username")]
    username: String,
    #[clap(long, default_value = "16")]
    device_count: u32,
    #[clap(long, default_value = "esp-")]
    device_name_prefix: String,
    #[clap(long, default_value = "ESP8266")]
    device_type: String,
    #[clap(long, default_value = "ESP8266 Miner v2.55")]
    firmware: String,
    #[clap(long, default_value = "9200")]
    target_rate: u32,
}

#[derive(Clap)]
struct Run {}


fn generate_5hex() -> String {
    const HEX_ARRAY: [char; 16] = ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'];

    let mut result = String::new();

    for _ in 0..5 {
        let n: usize = rand::thread_rng().gen_range(0..16);
        result.push(HEX_ARRAY[n]);
    }

    result
}

async fn generate_config(file_path: String, gen: &Generate) -> Result<(), Box<dyn std::error::Error>> {
    let mut device_vec: Vec<Device> = Vec::new();

    for i in 0..gen.device_count {
        let device = Device {
            host: gen.host.clone(),
            port: gen.port,
            username: gen.username.clone(),
            device_name: format!("{}{}", gen.device_name_prefix, i + 1),
            device_type: gen.device_type.clone(),
            chip_id: generate_5hex(),
            firmware: gen.firmware.clone(),
            target_rate: gen.target_rate,
        };

        device_vec.push(device);
    }

    let c = Config { devices: device_vec };
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

async fn start_miner(device: Device) -> Result<(), MinerError> {
    let heatup_duration: u64 = rand::thread_rng().gen_range(0..10000);
    tokio::time::sleep(Duration::from_millis(heatup_duration)).await;

    let mut stream = TcpStream::connect(
        format!("{}:{}", device.host, device.port)).await.map_err(|_| MinerError::Connection)?;

    // println!("{} connected to pool {}:{}", device.device_name, device.host, device.port);

    let mut cmd_in: [u8; 200] = [0; 200];
    let n = stream.read(&mut cmd_in).await.map_err(|_| MinerError::RecvCommand)?;
    // println!("version: {}", std::str::from_utf8(&cmd_in[..n])?);

    let expected_interval = 1000000u128 / device.target_rate as u128;

    loop {
        let cmd_job = format!("JOB,{},{}\n", device.username, device.device_type);
        stream.write(cmd_job.as_bytes()).await.map_err(|_| MinerError::SendCommand)?;

        let n = stream.read(&mut cmd_in).await.map_err(|_| MinerError::RecvCommand)?;
        let job = std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?.trim();

        let args: Vec<&str> = job.split(',').collect();
        if args.len() < 3 {
            return Err(MinerError::MalformedJob(job.to_string()));
        }

        let last_block_hash = args[0];
        let expected_hash = args[1];
        let diff = args[2].parse::<u32>().map_err(|_| MinerError::MalformedJob(job.to_string()))? * 100 + 1;

        // println!("last: {}, expected: {}, diff: {}", last_block_hash, expected_hash, diff);

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
                    tokio::time::sleep(Duration::from_micros(
                        (expected_duration - duration) as u64)).await;
                    // println!("Waited {} micro sec", expected_duration - duration);
                }

                let end = SystemTime::now();
                let duration = end.duration_since(start).unwrap().as_micros();
                let emu_rate = duco_numeric_result as f64 / duration as f64 * 1000000f64;

                let cmd_out = format!("{},{:.2},{},{},{}\n",
                                      duco_numeric_result, emu_rate, device.firmware, device.device_name, device.chip_id);
                stream.write(cmd_out.as_bytes()).await.map_err(|_| MinerError::SendCommand)?;

                let n = stream.read(&mut cmd_in).await.map_err(|_| MinerError::RecvCommand)?;
                let resp = std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?.trim();

                // println!("resp: {}, result: {}, rate: {:.2}, real: {:.2}",
                //          resp, duco_numeric_result, emu_rate, real_rate);
                if resp != "GOOD" {
                    println!("resp: {}, result: {}, rate: {:.2}, real: {:.2}",
                             resp, duco_numeric_result, emu_rate, real_rate);
                }

                break;
            }
        }
    }
}

async fn start_miners(devices: Vec<Device>) -> Result<(), MinerError> {
    let mut futures_vec = Vec::new();

    for device in devices {
        let f = start_miner(device);
        futures_vec.push(f);
    }

    futures::future::try_join_all(futures_vec).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    match opts.sub_command {
        SubCommands::Generate(gen) => {
            generate_config(opts.config_file, &gen).await?;
        }
        SubCommands::Run(_) => {
            let c_serial = tokio::fs::read_to_string(opts.config_file).await?;
            let c: Config = serde_yaml::from_str(c_serial.as_str())?;

            println!("Running with {} miners", c.devices.len());

            loop {
                match start_miners(c.devices.clone()).await {
                    Ok(_) => break,
                    Err(e) => {
                        println!("Exited with error: {:?}", e);
                        tokio::time::sleep(Duration::from_secs(300u64)).await;
                    }
                }
            }
        }
    }

    Ok(())
}
