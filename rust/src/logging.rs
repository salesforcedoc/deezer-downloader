use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

const MAX_LINES: usize = 500;

static LOG: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn buffer() -> &'static Mutex<VecDeque<String>> {
    LOG.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn log(message: impl AsRef<str>) {
    let line = message.as_ref().to_string();
    println!("{line}");
    let mut buffer = buffer().lock().unwrap();
    buffer.push_back(line);
    while buffer.len() > MAX_LINES {
        buffer.pop_front();
    }
}

pub fn error(message: impl AsRef<str>) {
    log(format!("ERROR: {}", message.as_ref()));
}

pub fn tail() -> String {
    buffer()
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}
