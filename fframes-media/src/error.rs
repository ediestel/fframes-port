use std::fmt;

#[derive(Debug, Clone)]
pub enum FFramesMediaError {
    MediaDirectoryProvided,
    DecodingError(String),
    IoError(String),
    SubtitleParseError(String),
    ImageError(String),
    FFmpegError(i32, String),
}

impl fmt::Display for FFramesMediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FFramesMediaError::MediaDirectoryProvided => {
                write!(f, "A directory was provided where a file was expected")
            }
            FFramesMediaError::DecodingError(msg) => write!(f, "Decoding error: {msg}"),
            FFramesMediaError::IoError(msg) => write!(f, "IO error: {msg}"),
            FFramesMediaError::SubtitleParseError(msg) => write!(f, "Subtitle parse error: {msg}"),
            FFramesMediaError::ImageError(msg) => write!(f, "Image error: {msg}"),
            FFramesMediaError::FFmpegError(code, msg) => {
                write!(f, "FFmpeg error (code {code}): {msg}")
            }
        }
    }
}

impl std::error::Error for FFramesMediaError {}

impl From<std::io::Error> for FFramesMediaError {
    fn from(e: std::io::Error) -> Self {
        FFramesMediaError::IoError(e.to_string())
    }
}

impl From<image::ImageError> for FFramesMediaError {
    fn from(e: image::ImageError) -> Self {
        FFramesMediaError::ImageError(e.to_string())
    }
}
