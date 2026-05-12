mod border;
mod collage;
mod config;
mod dpi;
mod error;
mod image_proc;
mod pipeline;
mod progress;
mod watermark;

use std::io::{self, BufRead};

use config::CollageConfig;
use progress::{send, ProgressMessage};

fn main() {
    // 从 stdin 读取一行 JSON 配置
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    let config_line = match lines.next() {
        Some(Ok(line)) => line,
        Some(Err(e)) => {
            send(&ProgressMessage::Error {
                message: format!("读取配置失败: {}", e),
            });
            std::process::exit(1);
        }
        None => {
            send(&ProgressMessage::Error {
                message: "stdin 为空，未收到配置".into(),
            });
            std::process::exit(1);
        }
    };

    let config: CollageConfig = match serde_json::from_str(&config_line) {
        Ok(c) => c,
        Err(e) => {
            send(&ProgressMessage::Error {
                message: format!("配置 JSON 解析失败: {}", e),
            });
            std::process::exit(1);
        }
    };

    match pipeline::run(&config) {
        Ok(report) => {
            let outputs = report
                .outputs
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            send(&ProgressMessage::Completed {
                outputs,
                processed_count: report.processed_count,
                failed_images: report.failed_images,
                warnings: report.warnings,
            });
        }
        Err(e) => {
            send(&ProgressMessage::Error {
                message: e.to_string(),
            });
            std::process::exit(1);
        }
    }
}
