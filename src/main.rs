#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
use clap::Parser;
use crossterm::{
    cursor::MoveTo,
    style::{Color, Print, SetForegroundColor},
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, ScrollUp},
    QueueableCommand,
};
use itertools::Itertools;
use std::{
    cmp::Ordering,
    collections::HashMap,
    io::{BufReader, Write},
    path::PathBuf,
    string::ToString,
    time::Duration,
};

#[derive(Parser)]
struct Args {
    #[arg(short)]
    pub filter: Vec<String>,
    #[arg(short = 'm', default_value = "0.0")]
    pub min: f64,
    #[arg(short = 'M', default_value = "100.0")]
    pub max: f64,
    #[arg(short)]
    pub wait: Option<f64>,
    #[arg(default_value = "/dev/stdin")]
    pub input: PathBuf,
    #[arg(short = 'k')]
    pub no_alternate_screen: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let wait = args.wait.map_or(Duration::ZERO, Duration::from_secs_f64);
    ctrlc::set_handler(|| {
        let mut stdout = std::io::stdout();
        stdout.queue(LeaveAlternateScreen).unwrap();
        stdout.flush().unwrap();
        eprintln!("Got SIGINT");
        std::process::exit(0);
    })?;
    let mut stdout = std::io::stdout();
    if !args.no_alternate_screen {
        stdout.queue(EnterAlternateScreen)?;
    }
    stdout.flush()?;

    let (send_line, recv_line) = std::sync::mpsc::channel();

    std::thread::spawn({
        let send_line = send_line.clone();
        move || {
            let file = std::fs::File::open(args.input).unwrap();
            let mut reader = csv::Reader::from_reader(BufReader::new(file));
            let names = reader
                .headers()
                .unwrap()
                .iter()
                .map(ToString::to_string)
                .collect_vec();
            reader.records().flatten().for_each(|r| {
                let row = r
                    .iter()
                    .enumerate()
                    .map(|(n, val)| (names[n].clone(), val.trim().parse().unwrap_or_default()))
                    .collect::<HashMap<_, f64>>();
                send_line.send(row).unwrap();
            });
        }
    });
    let old_values: HashMap<String, f64> = recv_line.recv()?;
    send_line.send(old_values.clone())?; // Read the first set of values twice
    let mut old_values: HashMap<String, f64> = old_values
        .into_iter()
        .filter(|(k, _)| args.filter.is_empty() || args.filter.contains(k))
        .collect();
    loop {
        let mut stdout = std::io::stdout();
        let (width, height) = crossterm::terminal::size()?;
        let values: HashMap<String, f64> = recv_line
            .recv()?
            .into_iter()
            .filter(|(k, _)| args.filter.is_empty() || args.filter.contains(k))
            .collect();

        stdout.queue(ScrollUp(1))?;
        stdout.queue(MoveTo(0, height - (values.len() as u16 + 1)))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
        for (i, (name, value)) in values
            .iter()
            .sorted_by(|(a, _), (b, _)| a.cmp(b))
            .enumerate()
        {
            let old_position = (old_values.get(name).unwrap() - args.min) / (args.max - args.min);
            let old_position = (old_position * (width as f64 - 1.0)) as u16;
            let position = (value - args.min) / (args.max - args.min);
            let position = (position * (width as f64 - 1.0)) as u16;
            let ordering = position.cmp(&old_position);
            stdout.queue(MoveTo(
                position.min(old_position),
                height - (values.len() as u16 + 1),
            ))?;
            stdout.queue(SetForegroundColor(Color::AnsiValue(i as u8)))?;
            stdout.queue(Print(match ordering {
                Ordering::Less => {
                    format!("/{}", "‾".repeat((old_position - position) as usize - 1))
                }
                Ordering::Equal => '|'.to_string(),
                Ordering::Greater => {
                    format!(" {}\\", "‾".repeat((position - old_position) as usize - 1))
                }
            }))?;
            let label = format!("{name}: {value}");
            stdout.queue(MoveTo(
                ((position + old_position) / 2).min(width - label.len() as u16),
                height - (values.len() - i) as u16,
            ))?;
            stdout.queue(Clear(ClearType::CurrentLine))?;
            stdout.queue(Print(label))?;
            stdout.queue(MoveTo(0, height - 1))?;
        }

        old_values = values;
        stdout.flush()?;
        std::thread::sleep(wait);
    }
}
