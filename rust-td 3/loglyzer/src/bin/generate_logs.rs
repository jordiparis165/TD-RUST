use rand::seq::SliceRandom;
use rand::Rng;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {

    let args: Vec<String> = env::args().collect();

    let line_count: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100_000);

    let filename = args.get(2).cloned().unwrap_or_else(|| "generated.log".to_string());

    let file = File::create(&filename)?;
    let mut writer = BufWriter::new(file);

    let _levels = ["INFO", "WARNING", "ERROR", "DEBUG"];

    let info_messages = [
        "Application started",
        "User logged in",
        "User logged out",
        "Database connection established",
        "Job finished successfully",
        "Health check OK",
        "Cache warmed up",
        "Configuration loaded",
    ];

    let warning_messages = [
        "High memory usage detected",
        "Slow response time from external service",
        "Cache miss",
        "Retrying request after temporary failure",
        "Disk usage above 80%",
    ];

    let error_messages = [
        "Failed to connect to API: timeout",
        "Database query failed: syntax error",
        "Authentication failed for user",
        "Cannot write to log directory",
        "Payment service returned 500",
    ];

    let debug_messages = [
        "Loading configuration from config.yml",
        "SQL query executed",
        "Received HTTP 200 from upstream",
        "Parsed request headers",
        "Session token validated",
    ];

    let mut rng = rand::thread_rng();

    for i in 0..line_count {
        let base_seconds = 10 * 3600 + 30 * 60; // 10:30:00
        let t = base_seconds + (i as u32 % 86_400);
        let hour = t / 3600;
        let minute = (t % 3600) / 60;
        let second = t % 60;

        let timestamp = format!("2024-01-15 {:02}:{:02}:{:02}", hour, minute, second);

        let p: u8 = rng.gen_range(0..100);
        let level = if p < 55 {
            "INFO"
        } else if p < 75 {
            "WARNING"
        } else if p < 92 {
            "ERROR"
        } else {
            "DEBUG"
        };

        let message = match level {
            "INFO" => info_messages.choose(&mut rng).unwrap(),
            "WARNING" => warning_messages.choose(&mut rng).unwrap(),
            "ERROR" => error_messages.choose(&mut rng).unwrap(),
            "DEBUG" => debug_messages.choose(&mut rng).unwrap(),
            _ => "Unknown event",
        };

        writeln!(writer, "{timestamp} [{level}] {message}")?;
    }

    writer.flush()?;

    println!(
        "Generated {} log lines into '{}'",
        line_count, filename
    );

    Ok(())
}
