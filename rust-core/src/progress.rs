use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressMessage {
    ImageProcessed {
        index: usize,
        total: usize,
    },
    StageChanged {
        stage: Stage,
        message: String,
    },
    Completed {
        outputs: Vec<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    ProcessingImages,
    CreatingCollage,
    AddingBorder,
    AddingWatermark,
}

/// 向 stdout 写一行 JSON（NDJSON），供 Electron 主进程读取
pub fn send(msg: &ProgressMessage) {
    // eprintln! 用于调试日志（stderr），println! 是协议通道（stdout）
    if let Ok(json) = serde_json::to_string(msg) {
        println!("{}", json);
    }
}
