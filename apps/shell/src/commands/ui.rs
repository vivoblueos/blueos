// Copyright (c) 2026 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A minimal text user interface for the interactive shell.
//!
//! `ui` draws a boxed dashboard (system, process and file panels) with Unicode
//! box-drawing characters and offers a small menu to inspect each panel alone.

use std::{
    env, fs,
    io::{self, Write},
    path::Path,
};

/// Characters between the left and right frame borders.
const WIDTH: usize = 74;
/// Process rows shown in the compact dashboard.
const DASHBOARD_PROCESS_ROWS: usize = 8;
/// File rows shown in the compact dashboard.
const DASHBOARD_FILE_ROWS: usize = 6;
/// Process rows shown in the dedicated process panel.
const PANEL_PROCESS_ROWS: usize = 16;
/// File rows shown in the dedicated file panel.
const PANEL_FILE_ROWS: usize = 16;

pub fn command(args: &[&str]) -> Result<(), String> {
    match args.first() {
        None => interactive(),
        Some(&"once") => {
            draw_dashboard();
            Ok(())
        }
        Some(&"help") => {
            print_menu();
            Ok(())
        }
        Some(other) => Err(format!("ui: unknown option '{}', try 'ui help'", other)),
    }
}

/// Run the menu loop until the user quits or the input stream ends.
fn interactive() -> Result<(), String> {
    draw_dashboard();
    loop {
        print!("\nui> ");
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut input = String::new();
        if io::stdin()
            .read_line(&mut input)
            .map_err(|e| e.to_string())?
            == 0
        {
            println!();
            break;
        }

        match input.trim() {
            "" => continue,
            "1" | "sys" | "system" | "mem" => draw_box("System", &system_lines()),
            "2" | "ps" | "proc" | "processes" => {
                draw_box("Processes", &process_lines(PANEL_PROCESS_ROWS))
            }
            "3" | "ls" | "files" => draw_box("Files", &file_lines(PANEL_FILE_ROWS)),
            "4" | "help" | "?" => print_menu(),
            "r" | "refresh" => draw_dashboard(),
            "q" | "quit" | "exit" => break,
            other => println!("Unknown choice: {} (type 'help' for options)", other),
        }
    }
    Ok(())
}

fn print_menu() {
    println!("ui options:");
    println!("  1 | sys      show the system panel");
    println!("  2 | ps       show the process panel");
    println!("  3 | ls       show the current directory");
    println!("  4 | help     show this help");
    println!("  r | refresh  redraw the dashboard");
    println!("  q | quit     leave the text interface");
}

fn draw_dashboard() {
    println!();
    println!("{}", titled("BlueOS Text UI"));
    print_row("");
    print_row("  Welcome to the BlueOS text interface. Pick an item from the menu.");
    print_row("");

    println!("{}", sep("System"));
    for line in system_lines() {
        print_row(&format!("  {}", line));
    }

    println!("{}", sep("Processes"));
    for line in process_lines(DASHBOARD_PROCESS_ROWS) {
        print_row(&format!("  {}", line));
    }

    println!("{}", sep("Files"));
    for line in file_lines(DASHBOARD_FILE_ROWS) {
        print_row(&format!("  {}", line));
    }
    print_row("");

    println!("{}", sep("Menu"));
    print_row("  [1] System   [2] Processes   [3] Files   [4] Help");
    print_row("  [r] Refresh  [q] Quit");

    println!("{}", line("└", "┘"));
}

/// Draw a stand-alone titled panel.
fn draw_box(title: &str, lines: &[String]) {
    println!();
    println!("{}", titled(title));
    if lines.is_empty() {
        print_row("");
    } else {
        for line in lines {
            print_row(&format!("  {}", line));
        }
    }
    println!("{}", line("└", "┘"));
}

fn system_lines() -> Vec<String> {
    let cwd = env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "?".to_string());
    vec![
        memory_line(),
        format!("Cwd      : {}", cwd),
        format!("Processes: {}", read_processes().len()),
    ]
}

fn memory_line() -> String {
    match read_meminfo() {
        Some((total, used, _available)) if total > 0 => {
            let percent = (used.saturating_mul(100) / total).min(100) as usize;
            format!(
                "Memory   : {} {:>3}%  used {} / total {}",
                usage_bar(percent, 20),
                percent,
                format_bytes(used.saturating_mul(1024)),
                format_bytes(total.saturating_mul(1024))
            )
        }
        _ => "Memory   : unavailable".to_string(),
    }
}

fn process_lines(limit: usize) -> Vec<String> {
    let processes = read_processes();
    let mut lines = vec![format!(
        "{:<8} {:<12} {:<10} {}",
        "TID", "STATE", "PRIORITY", "NAME"
    )];
    if processes.is_empty() {
        lines.push("(no process found)".to_string());
        return lines;
    }
    for process in processes.iter().take(limit) {
        lines.push(format!(
            "{:<8} {:<12} {:<10} {}",
            process.tid, process.state, process.priority, process.name
        ));
    }
    if processes.len() > limit {
        lines.push(format!("... ({} more)", processes.len() - limit));
    }
    lines
}

fn file_lines(max_lines: usize) -> Vec<String> {
    let cwd = match env::current_dir() {
        Ok(cwd) => cwd,
        Err(_) => return vec!["(unable to get current directory)".to_string()],
    };
    let mut names: Vec<String> = match fs::read_dir(&cwd) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if entry.path().is_dir() {
                    format!("{}/", name)
                } else {
                    name
                }
            })
            .collect(),
        Err(_) => return vec!["(unable to read directory)".to_string()],
    };
    names.sort();
    if names.is_empty() {
        return vec!["(empty)".to_string()];
    }

    let limit = WIDTH.saturating_sub(2);
    let mut lines = Vec::new();
    let mut index = 0;
    while index < names.len() && lines.len() < max_lines {
        let mut current = String::new();
        while index < names.len() {
            let name = &names[index];
            let separator = if current.is_empty() { 0 } else { 2 };
            if !current.is_empty()
                && current.chars().count() + separator + name.chars().count() > limit
            {
                break;
            }
            if !current.is_empty() {
                current.push_str("  ");
            }
            current.push_str(name);
            index += 1;
        }
        lines.push(current);
    }
    if index < names.len() {
        lines.push(format!("... ({} more)", names.len() - index));
    }
    lines
}

/// Read `/proc/meminfo`. The returned values are in kilobytes.
fn read_meminfo() -> Option<(u64, u64, u64)> {
    let content = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total = 0;
    let mut used = 0;
    let mut available = 0;
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let number = value
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            match key.trim() {
                "MemTotal" => total = number,
                "MemUsed" => used = number,
                "MemAvailable" => available = number,
                _ => {}
            }
        }
    }
    Some((total, used, available))
}

struct Process {
    tid: u32,
    state: String,
    priority: String,
    name: String,
}

fn read_processes() -> Vec<Process> {
    let mut processes = Vec::new();
    let entries = match fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(_) => return processes,
    };
    for entry in entries.filter_map(|entry| entry.ok()) {
        let tid = match entry.file_name().to_string_lossy().parse::<u32>() {
            Ok(tid) => tid,
            Err(_) => continue,
        };
        let (name, state, priority) = read_status(&entry.path());
        processes.push(Process {
            tid,
            state,
            priority,
            name,
        });
    }
    processes.sort_by_key(|process| process.tid);
    processes
}

fn read_status(proc_path: &Path) -> (String, String, String) {
    let mut name = "?".to_string();
    let mut state = "?".to_string();
    let mut priority = "?".to_string();
    if let Ok(content) = fs::read_to_string(proc_path.join("status")) {
        for line in content.lines() {
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "Name" => name = value.trim().to_string(),
                    "State" => state = value.split_whitespace().next().unwrap_or("?").to_string(),
                    "Priority" => priority = value.trim().to_string(),
                    _ => {}
                }
            }
        }
    }
    (name, state, priority)
}

fn usage_bar(percent: usize, width: usize) -> String {
    let filled = ((percent * width + 50) / 100).min(width);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    }
}

// -- Frame drawing helpers --------------------------------------------------

fn line(left: &str, right: &str) -> String {
    format!("{}{}{}", left, "─".repeat(WIDTH), right)
}

fn titled(title: &str) -> String {
    let label = format!("─ {} ", title);
    let used = label.chars().count();
    format!("┌{}{}┐", label, "─".repeat(WIDTH.saturating_sub(used)))
}

fn sep(title: &str) -> String {
    let label = format!("─ {} ", title);
    let used = label.chars().count();
    format!("├{}{}┤", label, "─".repeat(WIDTH.saturating_sub(used)))
}

fn print_row(content: &str) {
    println!("│{}│", pad(content));
}

fn pad(content: &str) -> String {
    let length = content.chars().count();
    if length >= WIDTH {
        content.chars().take(WIDTH).collect()
    } else {
        format!("{}{}", content, " ".repeat(WIDTH - length))
    }
}
