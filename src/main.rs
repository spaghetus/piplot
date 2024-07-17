use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    io::{stdin, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use clap::Parser;
use crossterm::{
    cursor::MoveTo,
    style::{Color, Print, SetForegroundColor},
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, ScrollDown, ScrollUp,
    },
    QueueableCommand,
};
use itertools::Itertools;

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
    let wait = args
        .wait
        .map(Duration::from_secs_f64)
        .unwrap_or(Duration::ZERO);
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
                .map(|h| h.to_string())
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
        let (width, height) = (width as f64, height as f64);
        let values: HashMap<String, f64> = recv_line
            .recv()?
            .into_iter()
            .filter(|(k, _)| args.filter.is_empty() || args.filter.contains(k))
            .collect();

        stdout.queue(ScrollUp(1))?;
        stdout.queue(MoveTo(0, height as u16 - (values.len() as u16 + 1)))?;
        stdout.queue(Clear(ClearType::CurrentLine))?;
        for (i, (name, value)) in values
            .iter()
            .sorted_by(|(a, _), (b, _)| a.cmp(b))
            .enumerate()
        {
            let old_position = (old_values.get(name).unwrap() - args.min) / (args.max - args.min);
            let old_position = (old_position * (width - 1.0)) as u16;
            let position = (value - args.min) / (args.max - args.min);
            let position = (position * (width - 1.0)) as u16;
            let ordering = position.cmp(&old_position);
            stdout.queue(MoveTo(
                position.min(old_position),
                height as u16 - (values.len() as u16 + 1),
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
                ((position + old_position) / 2).min(width as u16 - label.len() as u16),
                height as u16 - (values.len() - i) as u16,
            ))?;
            stdout.queue(Clear(ClearType::CurrentLine))?;
            stdout.queue(Print(label))?;
            stdout.queue(MoveTo(0, height as u16 - 1))?;
        }

        old_values = values;
        stdout.flush()?;
        std::thread::sleep(wait);
    }
}
