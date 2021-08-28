use duino_miner::error::MinerError;

use serde::{Deserialize, Serialize};

use std::io::{Read, Write};
use std::net::TcpStream;

use clap::{AppSettings, Clap};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Config {
    host: String,
    port: u16,

    main_account: String,
    accounts: Vec<Account>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Account {
    username: String,
    password: String,
}

#[derive(Clap)]
#[clap(version = "0.1", author = "Black H. <encomblackhat@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short, long, default_value = "accounts.yaml")]
    config_file: String,
    #[clap(short)]
    force: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts: Opts = Opts::parse();

    let c_serial = std::fs::read_to_string(opts.config_file)?;
    let c: Config = serde_yaml::from_str(c_serial.as_str())?;

    let mut stream =
        TcpStream::connect(format!("{}:{}", c.host, c.port)).map_err(|_| MinerError::Connection)?;

    println!("Connected to pool {}:{}", c.host, c.port);

    let mut cmd_in: [u8; 200] = [0; 200];
    let n = stream
        .read(&mut cmd_in)
        .map_err(|_| MinerError::RecvCommand)?;
    println!("version: {}", std::str::from_utf8(&cmd_in[..n])?);

    for account in c.accounts {
        let cmd_job = format!("LOGI,{},{}\n", account.username, account.password);
        stream
            .write(cmd_job.as_bytes())
            .map_err(|_| MinerError::SendCommand)?;
        let n = stream
            .read(&mut cmd_in)
            .map_err(|_| MinerError::RecvCommand)?;
        let login_status =
            std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?;

        if login_status != "OK" {
            println!("{} login failed", account.username);
            continue;
        }

        stream
            .write("BALA".as_bytes())
            .map_err(|_| MinerError::SendCommand)?;
        let n = stream
            .read(&mut cmd_in)
            .map_err(|_| MinerError::RecvCommand)?;
        let balance: f32 = std::str::from_utf8(&cmd_in[..n])
            .map_err(|_| MinerError::InvalidUTF8)?
            .parse()?;
        println!("account {} has balance {}", account.username, balance);

        if account.username == c.main_account {
            continue;
        }

        let balance = balance as u32;

        if balance > 100 || (opts.force && balance > 0) {
            let cmd_job = format!("SEND,-,{},{}\n", c.main_account, balance);
            stream
                .write(cmd_job.as_bytes())
                .map_err(|_| MinerError::SendCommand)?;
            let n = stream
                .read(&mut cmd_in)
                .map_err(|_| MinerError::RecvCommand)?;
            let transfer_status =
                std::str::from_utf8(&cmd_in[..n]).map_err(|_| MinerError::InvalidUTF8)?;

            println!(
                "transfer of {} coins to {} exited with status {}",
                balance, c.main_account, transfer_status
            );
        }
    }

    Ok(())
}
