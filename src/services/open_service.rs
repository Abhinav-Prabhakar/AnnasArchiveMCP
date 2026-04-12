use std::path::Path;

use crate::errors::{AppError, AppResult};

#[derive(Debug, Clone, Default)]
pub struct OpenService;

impl OpenService {
    pub fn new() -> Self {
        Self
    }

    pub async fn open(&self, path: &Path) -> AppResult<()> {
        if !path.exists() {
            return Err(AppError::OpenFile(crate::errors::OpenFileError::Failed {
                path: path.to_path_buf(),
            }));
        }

        if !path.is_file() {
            return Err(AppError::OpenFile(crate::errors::OpenFileError::Failed {
                path: path.to_path_buf(),
            }));
        }

        opener::open(path).map_err(|_| {
            AppError::OpenFile(crate::errors::OpenFileError::Failed {
                path: path.to_path_buf(),
            })
        })?;

        Ok(())
    }
}
