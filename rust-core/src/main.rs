mod border;
mod collage;
mod color;
mod config;
mod dpi;
mod error;
mod fonts;
mod image_loader;
mod image_proc;
mod jpeg_output;
mod metadata;
mod pipeline;
mod progress;
mod text_block;
mod ultrahdr_output;
mod watermark;

use std::io::{self, BufRead};
use std::path::PathBuf;

use config::CollageConfig;
use progress::{send, ProgressMessage};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--list-fonts") {
        match serde_json::to_string(&fonts::list_system_fonts()) {
            Ok(json) => {
                println!("{}", json);
                return;
            }
            Err(e) => {
                eprintln!("failed to serialize font list: {}", e);
                std::process::exit(1);
            }
        }
    }
    let preview_request = match args.get(1).map(String::as_str) {
        Some("--render-preview") => {
            let output_path = match args.get(2) {
                Some(path) => PathBuf::from(path),
                None => {
                    send(&ProgressMessage::Error {
                        message: "missing preview output path".into(),
                    });
                    std::process::exit(1);
                }
            };
            let preview_long_edge = match args.get(3).and_then(|value| value.parse::<u32>().ok()) {
                Some(value) if value > 0 => value,
                _ => {
                    send(&ProgressMessage::Error {
                        message: "preview long edge must be a positive integer".into(),
                    });
                    std::process::exit(1);
                }
            };
            Some((output_path, preview_long_edge))
        }
        _ => None,
    };

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

    if let Some((output_path, preview_long_edge)) = preview_request {
        match pipeline::render_preview(&config, &output_path, preview_long_edge) {
            Ok(report) => {
                send(&ProgressMessage::PreviewCompleted {
                    output_path: report.output_path.to_string_lossy().into_owned(),
                    width: report.width,
                    height: report.height,
                    final_width: report.final_width,
                    final_height: report.final_height,
                    processed_count: report.processed_count,
                    failed_images: report.failed_images,
                    warnings: report.warnings,
                    elapsed_ms: report.elapsed_ms,
                    stage_timings: report.stage_timings,
                });
            }
            Err(e) => {
                send(&ProgressMessage::Error {
                    message: e.to_string(),
                });
                std::process::exit(1);
            }
        }
        return;
    }

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
                elapsed_ms: report.elapsed_ms,
                stage_timings: report.stage_timings,
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
