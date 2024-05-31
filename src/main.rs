extern crate cfonts;
extern crate chrono;
extern crate chrono_tz;

use std::process::Command;
use std::io::{Read, stdin};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;

use cfonts::{Align, BgColors, Colors, Env, Fonts, Options, say};
use chrono::{Datelike, DateTime, Duration as ChronoDuration, NaiveTime, Timelike, TimeZone, Utc};
use chrono_tz::Tz;
use clap::Parser;
use termion::terminal_size;
use termios::{ECHO, ICANON, TCSANOW, tcsetattr, Termios};

/// CLI tool for displaying a clock or a countdown timer
#[derive(Parser)]
#[command(name = "Clock or Timer", about = "Displays a clock or a countdown timer with cfonts")]
struct Cli {
    /// Color index (1-9) for the font
    #[arg(short, long, default_value_t = 2)]
    color: u8,

    /// Timezone for the clock
    #[arg(short, long, default_value = "Europe/Belgrade")]
    timezone: String,

    /// Countdown duration (e.g., '3h', '125m', '3:12:15')
    #[arg(short, long)]
    duration: Option<String>,

    #[arg(short = 'D', long)]
    date: bool,

    /// Command to execute when the timer completes
    #[arg(short = 'x', long)]
    command: Option<String>,

    /// Target time for countdown (e.g., '2024-12-31T23:59:59Z')
    #[arg(short = 'T', long)]
    target_time: Option<String>,
}

fn get_color(index: u8) -> Colors {
    match index {
        1 => Colors::Red,
        2 => Colors::Green,
        3 => Colors::Yellow,
        4 => Colors::Blue,
        5 => Colors::Magenta,
        6 => Colors::Cyan,
        7 => Colors::White,
        8 => Colors::Gray,
        9 => Colors::Black,
        _ => Colors::White,
    }
}

fn parse_duration(s: &str) -> Result<ChronoDuration, &'static str> {
    if let Some(stripped) = s.strip_suffix('h') {
        let hours = stripped.parse::<i64>().map_err(|_| "Invalid number for hours")?;
        return Ok(ChronoDuration::hours(hours));
    }
    if let Some(stripped) = s.strip_suffix('m') {
        let minutes = stripped.parse::<i64>().map_err(|_| "Invalid number for minutes")?;
        return Ok(ChronoDuration::minutes(minutes));
    }
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 3 {
        let hours = parts[0].parse::<i64>().map_err(|_| "Invalid number for hours")?;
        let minutes = parts[1].parse::<i64>().map_err(|_| "Invalid number for minutes")?;
        let seconds = parts[2].parse::<i64>().map_err(|_| "Invalid number for seconds")?;
        return Ok(ChronoDuration::hours(hours) + ChronoDuration::minutes(minutes) + ChronoDuration::seconds(seconds));
    }
    Err("Invalid duration format")
}

fn display_time(time: String, color: Colors) {
    // Clear the terminal
    print!("{esc}c", esc = 27 as char);

    // Display time using cfonts
    say(Options {
        text: time,
        font: Fonts::FontBlock,
        colors: vec![color],
        background: BgColors::Transparent,
        align: Align::Center,
        letter_spacing: 1,
        line_height: 1,
        spaceless: false,
        max_length: 0,
        gradient: Vec::new(),
        independent_gradient: false,
        transition_gradient: false,
        env: Env::Cli,
        ..Options::default()
    });
}

fn run_timer(duration: ChronoDuration, color: Colors, running: Arc<AtomicBool>, paused: Arc<AtomicBool>, command: Option<String>) {
    // Execute "timew continue" when the timer starts
    Command::new("timew")
        .arg("continue")
        .output()
        .expect("Failed to execute command");

    let mut seconds_left = duration.num_seconds();
    let mut was_paused = false;
    let mut command_executed = false;

    while running.load(Ordering::SeqCst) {
        let hours = seconds_left.abs() / 3600;
        let minutes = (seconds_left.abs() % 3600) / 60;
        let seconds = seconds_left.abs() % 60;
        let time = if seconds_left < 0 {
            format!("-{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        };

        if paused.load(Ordering::SeqCst) {
            display_time(time, Colors::BlueBright);

            if !was_paused {
                // Execute "timew stop" when the timer is paused
                Command::new("timew")
                    .arg("stop")
                    .output()
                    .expect("Failed to execute command");

                was_paused = true;
            }

            sleep(Duration::from_millis(1000));
            continue;
        } else if was_paused {
            // Execute "timew continue" when resuming from pause
            Command::new("timew")
                .arg("continue")
                .output()
                .expect("Failed to execute command");

            was_paused = false;
        }

        display_time(time, color.clone());
        sleep(Duration::from_secs(1));
        if !paused.load(Ordering::SeqCst) {
            seconds_left -= 1;
        }

        if seconds_left == 0 && !command_executed {
            if let Some(cmd) = &command {
                if running.load(Ordering::SeqCst) {
                    println!("Executing command: {}", cmd);
                    let mut parts = cmd.split_whitespace();
                    if let Some(program) = parts.next() {
                        let args: Vec<&str> = parts.collect();
                        Command::new(program)
                            .args(&args)
                            .spawn()
                            .expect("Failed to execute command");
                    }
                }
            }
            command_executed = true;
        }
    }

    // Execute "timew stop" when the timer completes or exits
    Command::new("timew")
        .arg("stop")
        .output()
        .expect("Failed to execute command");
}
fn run_clock(timezone: Tz, color: Colors) {
    loop {
        let (width, _) = terminal_size().unwrap();
        let now: DateTime<Tz> = timezone.from_utc_datetime(&Utc::now().naive_utc());
        let time = now.format("%H:%M").to_string();
        let date = now.format("%m.%d %a").to_string();
        let padding = (width as usize - date.len()) / 2;

        display_time(time, color.clone());
        println!("{:padding$}{}", "", date, padding = padding);
        let seconds_until_next_minute = 60 - now.second();
        sleep(Duration::from_secs(seconds_until_next_minute as u64));
    }
}

fn handle_input(running: Arc<AtomicBool>, paused: Arc<AtomicBool>) {
    let mut stdin = stdin();
    let mut termios = Termios::from_fd(0).unwrap();
    let original_termios = termios.clone();
    termios.c_lflag &= !(ICANON | ECHO);
    tcsetattr(0, TCSANOW, &termios).unwrap();

    std::thread::spawn(move || {
        let mut input = [0];
        while running.load(Ordering::SeqCst) {
            if stdin.read(&mut input).is_ok() {
                match input[0] {
                    b' ' => {
                        let is_paused = paused.load(Ordering::SeqCst);
                        paused.store(!is_paused, Ordering::SeqCst);
                    }
                    b'q' => {
                        running.store(false, Ordering::SeqCst);
                    }
                    _ => {}
                }
            }
        }
        tcsetattr(0, TCSANOW, &original_termios).unwrap();
    });
}

fn run_countdown_to_time(target_time: DateTime<Tz>, color: Colors, command: Option<String>) {
    loop {
        let now = Utc::now().with_timezone(&target_time.timezone());
        let remaining = target_time - now;
        if remaining.num_seconds() <= 0 {
            break;
        }

        let hours = remaining.num_hours();
        let minutes = remaining.num_minutes() % 60;
        let seconds = remaining.num_seconds() % 60;
        let time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

        display_time(time, color.clone());
        sleep(Duration::from_secs(1));
    }

    if let Some(cmd) = command {
        println!("Executing command: {}", cmd);
        let mut parts = cmd.split_whitespace();
        if let Some(program) = parts.next() {
            let args: Vec<&str> = parts.collect();
            Command::new(program)
                .args(&args)
                .spawn()
                .expect("Failed to execute command");
        }
    }
}

fn run_date_and_weekday(timezone: Tz, color: Colors) {
    loop {
        let now: DateTime<Tz> = Utc::now().with_timezone(&timezone);
        let date = now.format("%m.%d").to_string();
        let weekday = now.format("%a").to_string();
        let display_text = format!("{} {}", date, weekday);

        display_time(display_text, color.clone());
        sleep(Duration::from_secs(60)); // Update every minute
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let color = get_color(cli.color);
    let timezone: Tz = cli.timezone.parse().unwrap_or_else(|_| "Asia/Manila".parse().unwrap());

    let running = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));
    handle_input(running.clone(), paused.clone());

    match (cli.target_time, cli.duration, cli.date) {
        (Some(target_time_str), _, _) => {
            let target_time = NaiveTime::parse_from_str(&target_time_str, "%H:%M:%S")
                .map_err(|_| "Invalid target time format")?;
            let now = Utc::now().with_timezone(&timezone);
            let target_datetime = timezone
                .with_ymd_and_hms(now.year(), now.month(), now.day(), target_time.hour(), target_time.minute(), target_time.second())
                .single()
                .ok_or("Invalid target time format")?;
            run_countdown_to_time(target_datetime, color.clone(), cli.command);
        },
        (_, Some(duration_str), _) => {
            let duration = parse_duration(&duration_str).expect("Invalid duration format");
            run_timer(duration, color.clone(), running.clone(), paused.clone(), cli.command);
        },
        (_, _, true) => {
            run_date_and_weekday(timezone.clone(), color.clone());
        },
        _ => run_clock(timezone.clone(), color.clone()),
    }

    println!("Got it! Exiting...");

    Ok(())
}

