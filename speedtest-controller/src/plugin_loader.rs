use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginLoaderError {
    #[error("Invalid plugin source `{0}`, scheme is unexpected")]
    UnexpectedScheme(String),

    #[error("Unable to load the plugin")]
    PluginError(#[from] crate::plugin::PluginError),
}

pub type Result<T> = std::result::Result<T, PluginLoaderError>;
