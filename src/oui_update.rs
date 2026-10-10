//! IEEE OUI database maintenance; downloads never transmit the local ARP cache.
use std::{
    env, fs,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};
const AGE: Duration = Duration::from_secs(90 * 24 * 60 * 60);
const URL: &str = "https://standards-oui.ieee.org/oui/oui.csv";

fn path() -> io::Result<PathBuf> {
    if let Some(p) = env::var_os("PONG_OUI_FILE") {
        return Ok(PathBuf::from(p));
    }
    env::var_os("HOME")
        .map(|h| PathBuf::from(h).join(".local/share/pong/oui.csv"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME or PONG_OUI_FILE required"))
}

fn age(path: &Path) -> io::Result<Option<Duration>> {
    match fs::metadata(path) {
        Ok(meta) => Ok(Some(
            SystemTime::now()
                .duration_since(meta.modified()?)
                .unwrap_or(Duration::ZERO),
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

fn valid_csv(p: &Path) -> io::Result<bool> {
    let bytes = fs::read(p)?;
    if bytes.len() < 500_000 {
        return Ok(false);
    }
    let contents = String::from_utf8(bytes).map_err(io::Error::other)?;
    let mut lines = contents.lines();
    let header = lines.next().unwrap_or("").trim_start_matches('\u{feff}');
    if !header.starts_with("Registry,Assignment,Organization Name") {
        return Ok(false);
    }
    Ok(lines.filter(|l| l.starts_with("MA-L,")).count() > 10_000)
}

pub fn check() -> io::Result<()> {
    let p = path()?;
    println!("OUI database: {}", p.display());
    match age(&p)? {
        None => println!("Status: missing; update required"),
        Some(d) => {
            println!("Age: {} days", d.as_secs() / 86400);
            println!(
                "Status: {}",
                if d >= AGE {
                    "update due (90 days)"
                } else {
                    "current"
                }
            );
        }
    }
    Ok(())
}

pub fn update(force: bool, quiet: bool) -> io::Result<()> {
    let p = path()?;
    if !force && age(&p)?.is_some_and(|d| d < AGE) {
        if !quiet {
            println!("OUI database current; update not required");
        }
        return Ok(());
    }
    let parent = p
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid database path"))?;
    fs::create_dir_all(parent)?;
    // Avoid collisions across concurrent runs. Create a new private temp file in the same directory.
    let mut temp = None;
    for n in 0..32u32 {
        let candidate = parent.join(format!(
            ".oui-{}-{}-{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            n
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(_) => {
                temp = Some(candidate);
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let temp = temp.ok_or_else(|| io::Error::other("could not create temporary OUI file"))?;
    let outcome = (|| -> io::Result<()> {
        let status = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--retry",
                "1",
                "--connect-timeout",
                "3",
                "--max-time",
                "12",
                "--output",
            ])
            .arg(&temp)
            .arg(URL)
            .status()
            .map_err(|e| io::Error::new(e.kind(), format!("curl unavailable: {e}")))?;
        if !status.success() {
            return Err(io::Error::other(format!("OUI download failed: {status}")));
        }
        if !valid_csv(&temp)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "downloaded OUI CSV failed validation",
            ));
        }
        fs::rename(&temp, &p)?;
        if !quiet {
            println!("OUI database updated: {}", p.display());
        }
        Ok(())
    })();
    if outcome.is_err() {
        let _ = fs::remove_file(&temp);
    }
    outcome
}

/// Read-only reminder for ARP mode. Never performs network I/O or updates files.
/// Only show the reminder for interactive text output; never corrupt JSON.
pub fn reminder_for_arp(json: bool) {
    if json || !io::stdout().is_terminal() {
        return;
    }
    let Ok(p) = path() else {
        return;
    };
    match age(&p) {
        Ok(Some(d)) if d >= AGE => {
            eprintln!(
                "\nIEEE OUI database is {} days old; run `pong --oui-update` to refresh.",
                d.as_secs() / 86400
            );
        }
        Ok(None) => {
            eprintln!("\nIEEE OUI database is missing; run `pong --oui-update` to install it.")
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interval_is_ninety_days() {
        assert_eq!(AGE.as_secs(), 7_776_000);
    }
}
