use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use headless_chrome::{Browser, LaunchOptionsBuilder};
use std::{ffi::OsStr, fs::OpenOptions, io::Write, path::PathBuf, thread, time::Duration};

fn format_id(prefix: &str, n: u64, width: usize) -> String {
    format!("{}{:0width$}", prefix, n, width = width)
}

fn make_url(prefix_id: &str, total_id: u64, width: usize) -> String {
    let full_id = format_id(prefix_id, total_id, width);

    format!("https://pddikti.kemdiktisaintek.go.id/search/{}?", full_id) //CONFIGURASI SENDIRI
}

fn worker_loop(
    rx: Receiver<(String, u64)>,
    result_tx: Sender<String>,
    id_width: usize,
) -> Result<()> {
    let options = LaunchOptionsBuilder::default()
        .path(Some(PathBuf::from("/usr/bin/google-chrome"))) // UNTUK WINDOWS, HAPUS SETTINGAN UBAH MENJADI
        .headless(false)                                     // let browser = Browser::default()?;
        .args(vec![
            OsStr::new("--no-sandbox"),
            OsStr::new("--disable-dev-shm-usage"),
        ])
        .build()?;

    let browser = Browser::new(options)?; // UNTUK WINDOWS, HAPUS INI JUGA
    let tab = browser.new_tab()?;
    tab.set_default_timeout(Duration::from_secs(60));

    while let Ok((prefix_id, total_id)) = rx.recv() {
        let url = make_url(&prefix_id, total_id, id_width);

        if tab.navigate_to(&url).is_err() { continue; }
        if tab.wait_until_navigated().is_err() { continue; }

        if tab.wait_for_element("td.px-2").is_err() { continue; }

        let rows = match tab.find_elements("tbody tr") {
            Ok(r) => r,
            Err(_) => continue,
        };

        for row in rows {
            let px4 = row.find_elements("td.px-4").unwrap_or_default();
            let px2 = row.find_elements("td.px-2").unwrap_or_default();

            if px4.len() >= 2 && px2.len() >= 2 {
                let px4_1 = px4[0].get_inner_text().unwrap_or_default().trim().to_string();
                let px4_2 = px4[1].get_inner_text().unwrap_or_default().trim().to_string();
                let px2_1 = px2[0].get_inner_text().unwrap_or_default().trim().to_string();
                let px2_2 = px2[1].get_inner_text().unwrap_or_default().trim().to_string();

                let line = format!("{}\t{}\t{}\t{}\n", px4_1, px4_2, px2_1, px2_2);
                let _ = result_tx.send(line);
                break;
            }
        }

        thread::sleep(Duration::from_millis(200));
    }

    Ok(())
}

fn main() -> Result<()> {
    // ====== CONFIG ======
    let tab_max: usize = 20;
    let prefix_id = "IsiSendiri";
    let start_id: u64 = 1;
    let end_id: u64 = 400;
    let id_width: usize = 3; // mis. 3 mengisi suffix 0001 sampai 0400
    // ====================

    let (job_tx, job_rx) = unbounded::<(String, u64)>();
    let (result_tx, result_rx) = unbounded::<String>();

    let writer = thread::spawn(move || -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("Mahasiswa.txt")?;

        while let Ok(line) = result_rx.recv() {
            file.write_all(line.as_bytes())?;
        }
        Ok(())
    });

    let mut workers = Vec::new();
    for _ in 0..tab_max {
        let rx = job_rx.clone();
        let tx = result_tx.clone();
        workers.push(thread::spawn(move || worker_loop(rx, tx, id_width)));
    }

    for id in start_id..=end_id {
        job_tx.send((prefix_id.to_string(), id))?;
    }

    drop(job_tx);
    drop(result_tx);

    for w in workers {
        w.join().expect("worker panic")?;
    }

    writer.join().expect("writer panic")?;
    println!("Scraping selesai");

    Ok(())
}
