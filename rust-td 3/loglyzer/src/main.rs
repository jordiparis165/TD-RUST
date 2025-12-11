
// PARTIE 1 
use clap::Parser;
use colored::*;
use once_cell::sync::Lazy;
use prettytable::{Cell, Row, Table};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// CLI du projet (options utilisateur)
#[derive(Parser, Debug)]
#[command(name = "loglyzer")]
#[command(version = "1.0")]
#[command(about = "Analyze log files and extract patterns", long_about = None)]
struct Cli {
    #[arg(value_name = "FILE")]
    input: PathBuf,

    #[arg(short, long, value_enum, default_value = "text")]
    format: OutputFormat,

    #[arg(short, long)]
    errors_only: bool,

    #[arg(short, long)]
    verbose: bool,

    #[arg(short, long, value_name = "N")]
    top: Option<usize>,

    #[arg(short, long, value_name = "TEXT")]
    search: Option<String>,

    #[arg(long, value_name = "FILE")]
    output: Option<PathBuf>,

    #[arg(long)]
    parallel: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum OutputFormat {
    Text,
    Json,
    Csv,
}


//PARTIE 2 — PARSING DU FICHIER DE LOGS

//Modèle pour une entrée de log
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp: String,
    level: LogLevel,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum LogLevel {
    Info,
    Warning,
    Error,
    Debug,
}

impl LogLevel {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "INFO" => Some(LogLevel::Info),
            "WARN" | "WARNING" => Some(LogLevel::Warning),
            "ERROR" => Some(LogLevel::Error),
            "DEBUG" => Some(LogLevel::Debug),
            _ => None,
        }
    }
}

// Regex compilée une seule fois
static LOG_LINE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\s+\[(\w+)\]\s+(.+)$").unwrap()
});

fn parse_log_line(line: &str) -> Option<LogEntry> {
    LOG_LINE_RE.captures(line).and_then(|caps| {
        Some(LogEntry {
            timestamp: caps.get(1)?.as_str().to_string(),
            level: LogLevel::from_str(caps.get(2)?.as_str())?,
            message: caps.get(3)?.as_str().to_string(),
        })
    })
}

//Lecture séquentielle
fn read_logs(path: &Path) -> Result<Vec<LogEntry>, std::io::Error> {
    let reader = BufReader::new(File::open(path)?);
    let mut entries = Vec::new();

    for line in reader.lines() {
        if let Some(entry) = parse_log_line(&line?) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

//Lecture parallèle
fn read_logs_parallel(path: &Path) -> Result<Vec<LogEntry>, std::io::Error> {
    let reader = BufReader::new(File::open(path)?);

    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let entries: Vec<LogEntry> = lines
        .par_iter()
        .filter_map(|line| parse_log_line(line))
        .collect();

    Ok(entries)
}


/// PARTIE 3 — ANALYSE DES LOGS 

#[derive(Debug, Serialize)]
struct LogStats {
    total_entries: usize,
    by_level: HashMap<String, usize>,
    top_errors: Vec<ErrorFrequency>,
    errors_by_hour: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ErrorFrequency {
    message: String,
    count: usize,
}

fn analyze_logs(entries: &[LogEntry], top_n: Option<usize>) -> LogStats {
    let mut by_level = HashMap::new();
    let mut error_messages = HashMap::new();
    let mut errors_by_hour = HashMap::new();

    for entry in entries {
        let level_name = format!("{:?}", entry.level);
        *by_level.entry(level_name.clone()).or_insert(0) += 1;

        if entry.level == LogLevel::Error {
            *error_messages.entry(entry.message.clone()).or_insert(0) += 1;

            if let Some(timepart) = entry.timestamp.split_whitespace().nth(1) {
                let hour = &timepart[0..2];
                *errors_by_hour.entry(hour.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut top_errors: Vec<_> = error_messages
        .into_iter()
        .map(|(msg, count)| ErrorFrequency { message: msg, count })
        .collect();

    top_errors.sort_by(|a, b| b.count.cmp(&a.count));

    let limit = top_n.unwrap_or(5);
    if top_errors.len() > limit {
        top_errors.truncate(limit);
    }

    LogStats {
        total_entries: entries.len(),
        by_level,
        top_errors,
        errors_by_hour,
    }
}

/// Analyse parallèle 
fn analyze_logs_parallel(entries: &[LogEntry], top_n: Option<usize>) -> LogStats {
    use std::sync::Mutex;

    let by_level = Mutex::new(HashMap::new());
    let error_messages = Mutex::new(HashMap::new());
    let errors_by_hour = Mutex::new(HashMap::new());

    entries.par_iter().for_each(|entry| {
        let mut bl = by_level.lock().unwrap();
        *bl.entry(format!("{:?}", entry.level)).or_insert(0) += 1;

        if entry.level == LogLevel::Error {
            let mut em = error_messages.lock().unwrap();
            *em.entry(entry.message.clone()).or_insert(0) += 1;

            let mut eb = errors_by_hour.lock().unwrap();
            if let Some(tp) = entry.timestamp.split_whitespace().nth(1) {
                let hour = &tp[0..2];
                *eb.entry(hour.to_string()).or_insert(0) += 1;
            }
        }
    });

    let mut top_errors: Vec<_> = error_messages
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|(msg, count)| ErrorFrequency { message: msg, count })
        .collect();

    top_errors.sort_by(|a, b| b.count.cmp(&a.count));

    let limit = top_n.unwrap_or(5);
    if top_errors.len() > limit {
        top_errors.truncate(limit);
    }

    LogStats {
        total_entries: entries.len(),
        by_level: by_level.into_inner().unwrap(),
        top_errors,
        errors_by_hour: errors_by_hour.into_inner().unwrap(),
    }
}


// PARTIE 3 — FORMATS DE SORTIE

fn output_text(stats: &LogStats) -> String {
    let mut out = String::new();

    out.push_str("\nLog Analysis Results\n");
    out.push_str("========================\n\n");

    out.push_str(&format!("Total entries: {}\n\n", stats.total_entries));

    // petit tableau
    let mut table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Level"),
        Cell::new("Count"),
        Cell::new("Percentage"),
    ]));

    for (level, count) in &stats.by_level {
        let percent = (*count as f64 / stats.total_entries as f64) * 100.0;
        let colored_level = match level.as_str() {
            "Error" => level.red().bold().to_string(),
            "Warning" => level.yellow().bold().to_string(),
            _ => level.to_string(),
        };
        table.add_row(Row::new(vec![
            Cell::new(&colored_level),
            Cell::new(&count.to_string()),
            Cell::new(&format!("{:.1}%", percent)),
        ]));
    }

    let mut tmp = Vec::new();
    table.print(&mut tmp).unwrap();
    out.push_str(&String::from_utf8(tmp).unwrap());
    out.push('\n');

    // top erreurs
    if !stats.top_errors.is_empty() {
        out.push_str("\nTop errors:\n");
        let mut t = Table::new();
        t.add_row(Row::new(vec![
            Cell::new("Error Message"),
            Cell::new("Occurrences"),
        ]));

        for e in &stats.top_errors {
            t.add_row(Row::new(vec![
                Cell::new(&e.message),
                Cell::new(&e.count.to_string()),
            ]));
        }

        let mut tmp = Vec::new();
        t.print(&mut tmp).unwrap();
        out.push_str(&String::from_utf8(tmp).unwrap());
    }

    out
}

fn output_json(stats: &LogStats) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(stats)
}

fn output_csv(stats: &LogStats) -> String {
    let mut out = String::new();
    out.push_str("metric,category,value\n");

    out.push_str(&format!("total,all,{}\n", stats.total_entries));

    for (lvl, cnt) in &stats.by_level {
        out.push_str(&format!("level,{},{}\n", lvl, cnt));
    }

    for (hour, cnt) in &stats.errors_by_hour {
        out.push_str(&format!("error_by_hour,{},{}\n", hour, cnt));
    }

    for err in &stats.top_errors {
        out.push_str(&format!("top_error,\"{}\",{}\n", err.message, err.count));
    }

    out
}

/// PARTIE 4

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.verbose {
        println!("File: {:?}", cli.input);
        println!("Parallel forced: {}", cli.parallel);
    }

    let start = Instant::now();

    let file_size = std::fs::metadata(&cli.input)?.len();
    let use_parallel = cli.parallel || file_size > 10_000_000;

    if cli.verbose {
        println!("File size: {} bytes", file_size);
        println!("Mode: {}", if use_parallel { "Parallel" } else { "Sequential" });
    }

    let entries = if use_parallel {
        read_logs_parallel(&cli.input)?
    } else {
        read_logs(&cli.input)?
    };

    let parse_time = start.elapsed();

    //filtres
    let filtered: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            if cli.errors_only && e.level != LogLevel::Error {
                return false;
            }
            if let Some(txt) = &cli.search {
                if !e.message.contains(txt) && !e.timestamp.contains(txt) {
                    return false;
                }
            }
            true
        })
        .collect();

    let stats = if use_parallel {
        analyze_logs_parallel(&filtered, cli.top)
    } else {
        analyze_logs(&filtered, cli.top)
    };

    let total_time = start.elapsed();

    // formats d’output
    let output = match cli.format {
        OutputFormat::Text => output_text(&stats),
        OutputFormat::Json => output_json(&stats)?,
        OutputFormat::Csv => output_csv(&stats),
    };

    if let Some(path) = cli.output {
        std::fs::write(path, output)?;
    } else {
        print!("{}", output);
    }

    if cli.verbose {
        eprintln!("\nPerformance:");
        eprintln!("  Parsing: {:?}", parse_time);
        eprintln!("  Total:   {:?}", total_time);
    }

    Ok(())
}
