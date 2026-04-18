use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("图像处理错误: {0}")]
    Image(#[from] image::ImageError),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 解析错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("没有成功处理任何图片")]
    NoImagesProcessed,

    #[error("处理失败: {0}")]
    Processing(String),
}
