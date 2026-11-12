//! `oopz pull`: the WhatsApp backup off a phone with adb (USB or wireless debugging), with the same `oopz-app` code
//! as the app's "With USB or wireless debugging" screen. Only reads the phone; never prints the pairing code.

use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use oopz_app::adb::{self, Adb, Remote, Wanted};

use crate::say::say;

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
        println!("{}", say!(PairedWith, host));
        if args.connect.is_none() {
            println!("{}", say!(NextConnect));
        }
    }
    if let Some(host) = &args.connect {
        adb.connect(host)?;
        println!("{}", say!(ConnectedTo, host));
    }
    let Some(out_dir) = args.out_dir else { return Ok(()) };

    let devices = adb.devices()?;
    let device = adb::choose(&devices, args.serial.as_deref().or(args.connect.as_deref()))?;
    let model = device.model.clone().unwrap_or_else(|| say!(APhone));
    if device.wireless {
        println!("{}", say!(OverWireless, model, device.serial));
    } else {
        println!("{}", say!(OverUsb, model, device.serial));
    }
    let phone = adb.phone(&device.serial);
    let want = Wanted { business: args.business, media: args.media, remote_dir: args.remote_dir.as_deref() };
    let plan = adb::plan(&phone as &dyn Remote, &want)?;
    for folder in &plan.folders {
        let bytes: u64 = folder.files.iter().map(|f| f.bytes).sum();
        let path = format!("{}/{}", plan.root, folder.name);
        println!("{}", say!(FolderFiles, path, folder.files.len(), bytes.div_ceil(1_000_000)));
    }
    let (puller, info) = adb::start_in(&out_dir, phone, plan)?;
    println!("{}", say!(CopyingInto, info.dest.display(), info.free / 1_000_000_000));
    let status = loop {
        let status = puller.status();
        if !status.running {
            break status;
        }
        if let Some(folder) = status.folder {
            let speed = format!("{:.1}", status.bytes_per_sec as f64 / 1e6);
            let line = say!(Copying, folder, status.done / 1_000_000, status.total.div_ceil(1_000_000), speed);
            eprint!("\r{line}{}   ", crate::time_left(status.secs_left));
            let _ = io::stderr().flush();
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    eprintln!();
    for p in &status.copied {
        println!("{}", say!(CopiedFolder, p.name, p.files, p.bytes));
    }
    if let Some(e) = status.error {
        return Err(e.into());
    }
    println!("{}", say!(NextBackups, info.dest.join("Databases").display()));
    Ok(())
}
