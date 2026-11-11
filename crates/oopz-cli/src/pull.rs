//! `oopz pull`: the WhatsApp backup off a phone with adb (USB or wireless debugging), with the same `oopz-app` code
//! as the app's "With USB or wireless debugging" screen. Only reads the phone; never prints the pairing code.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use oopz_app::adb::{self, Adb, Remote, Wanted};

#[derive(clap::Args)]
pub struct Pull {
    /// A folder; the copy goes in a new `oopz-pulled-N` folder in it.
    #[arg(long, required_unless_present_any = ["pair", "connect"])]
    out_dir: Option<PathBuf>,
    /// The phone to copy from, as `adb devices` names it; needed when several are connected.
    #[arg(long)]
    serial: Option<String>,
    /// WhatsApp Business (com.whatsapp.w4b) instead of WhatsApp.
    #[arg(long)]
    business: bool,
    /// Also copy the Media folder (photos, videos, voice notes: often many GB).
    #[arg(long)]
    media: bool,
    /// The phone's folder holding Databases (and Media), instead of WhatsApp's own.
    #[arg(long, value_name = "PATH")]
    remote_dir: Option<String>,
    /// Wireless debugging: pair first, with the address and port on the phone's "Pair device with pairing code"
    /// screen.
    #[arg(long, value_name = "HOST:PORT", requires = "code")]
    pair: Option<String>,
    /// The 6-digit code on that screen.
    #[arg(long, requires = "pair")]
    code: Option<String>,
    /// Wireless debugging: connect to the address and port under "IP address & port" on the phone's Wireless
    /// debugging screen (not the pairing one). Then copies from it unless --serial names another.
    #[arg(long, value_name = "HOST:PORT")]
    connect: Option<String>,
}

pub fn run(args: Pull) -> Result<()> {
    let adb = Adb::find()?;
    if let (Some(host), Some(code)) = (&args.pair, &args.code) {
        adb.pair(host, code)?;
        println!("paired with {host}");
        if args.connect.is_none() {
            println!(
                "next: oopz pull --connect IP:PORT (under \"IP address & port\" on the Wireless debugging screen)"
            );
        }
    }
    if let Some(host) = &args.connect {
        adb.connect(host)?;
        println!("connected to {host}");
    }
    let Some(out_dir) = args.out_dir else { return Ok(()) };

    let devices = adb.devices()?;
    let device = adb::choose(&devices, args.serial.as_deref().or(args.connect.as_deref()))?;
    let model = device.model.as_deref().unwrap_or("phone");
    let how = if device.wireless { "wireless debugging" } else { "USB" };
    println!("{model} ({}, over {how})", device.serial);
    let phone = adb.phone(&device.serial);
    let want = Wanted { business: args.business, media: args.media, remote_dir: args.remote_dir.as_deref() };
    let plan = adb::plan(&phone as &dyn Remote, &want)?;
    for folder in &plan.folders {
        let bytes: u64 = folder.files.iter().map(|f| f.bytes).sum();
        println!("{}/{}: {} files, {} MB", plan.root, folder.name, folder.files.len(), bytes.div_ceil(1_000_000));
    }
    let (puller, info) = adb::start_in(&out_dir, phone, plan)?;
    println!("Copying into {} ({} GB free). Ctrl-C stops.", info.dest.display(), info.free / 1_000_000_000);
    let status = loop {
        let status = puller.status();
        if !status.running {
            break status;
        }
        if let Some(folder) = status.folder {
            let left = status.secs_left.map_or_else(String::new, |s| format!(", {}:{:02} left", s / 60, s % 60));
            eprint!(
                "\rcopying {folder}: {} / {} MB, {:.1} MB/s{left}   ",
                status.done / 1_000_000,
                status.total.div_ceil(1_000_000),
                status.bytes_per_sec as f64 / 1e6
            );
            let _ = io::stderr().flush();
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    eprintln!();
    for p in &status.copied {
        println!("copied {}: {} files, {} bytes, every size checked", p.name, p.files, p.bytes);
    }
    if let Some(e) = status.error {
        return Err(e.into());
    }
    println!("next: oopz backups --dir {}", info.dest.join("Databases").display());
    Ok(())
}
