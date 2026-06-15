use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FailedImage {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StageTiming {
    pub stage: String,
    pub elapsed_ms: u128,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<StageTimingDetail>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StageTimingDetail {
    pub name: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressMessage {
    JobStarted {
        total: usize,
    },
    ImageProcessed {
        index: usize,
        total: usize,
        elapsed_ms: u128,
    },
    StageChanged {
        stage: Stage,
        message: String,
        elapsed_ms: u128,
    },
    StageFinished {
        stage: Stage,
        elapsed_ms: u128,
        total_elapsed_ms: u128,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        details: Vec<StageTimingDetail>,
    },
    Completed {
        outputs: Vec<String>,
        processed_count: usize,
        failed_images: Vec<FailedImage>,
        warnings: Vec<String>,
        elapsed_ms: u128,
        stage_timings: Vec<StageTiming>,
    },
    #[allow(dead_code)]
    Cancelled {
        message: String,
        partial_outputs: Vec<String>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    ProcessingImages,
    CreatingCollage,
    #[allow(dead_code)]
    AddingBorder,
    AddingWatermark,
    SavingOutput,
}

/// 向 stdout 写一行 JSON（NDJSON），供 Electron 主进程读取
pub fn send(msg: &ProgressMessage) {
    // eprintln! 用于调试日志（stderr），println! 是协议通道（stdout）
    if let Ok(json) = serde_json::to_string(msg) {
        println!("{}", json);
    }
}
