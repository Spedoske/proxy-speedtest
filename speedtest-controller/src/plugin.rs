/// This module contains the definition of the `Plugin` trait and its associated types and implementations.
/// The `Plugin` trait defines the interface for a plugin that can be used in the speedtest controller.
/// It provides methods for configuring the plugin, retrieving metadata, running tests, and performing data transformations.
/// The module also includes various supporting types and macros used by the `Plugin` trait and its implementations.
pub mod json_rpc;

use async_trait::async_trait;
use jsonrpsee::client_transport::ws::WsHandshakeError;
use jsonrpsee::types::{error::ErrorCode, ResponsePayload};
use jsonrpsee::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// An error type representing various plugin-related errors.
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("json-rpc client error")]
    ClientError(#[from] jsonrpsee::core::ClientError),
    #[error("json-rpc returns an invalid response")]
    APIBadResponse(#[from] serde_json::Error),
    #[error("Unable to parse the url")]
    ParseError(#[from] url::ParseError),
    #[error("Unable to perform the ws handshake")]
    WsHandshakeError(#[from] WsHandshakeError),
}

/// An enum representing the type of a plugin.
#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
pub enum PluginType {
    #[default]
    JSONRPC,
}

/// A type alias for the result of plugin operations.
type Result<T> = std::result::Result<T, PluginError>;

/// Metadata associated with a plugin.
#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct PluginMetaData {
    pub name: String,
}

/// Descriptor for a test.
#[derive(Debug, Deserialize, Clone)]
pub struct TestDescriptor {
    pub name: String,
}

/// Descriptor for a data transformation.
#[derive(Debug, Deserialize)]
pub struct DataTransformDescriptor {
    pub name: String,
    pub accpeted_scheme: String,
}

/// Descriptor for a protocol.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProtocolDescriptor {
    pub name: String,
    pub content: Value,
}

/// Descriptor for a connection.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionDescriptor {
    pub http: Option<String>,
    pub socks5: Option<String>,
    pub tun: bool,
}

/// A macro for implementing the `IntoResponse` trait for a given type.
#[macro_export]
macro_rules! impl_into_response {
    ($t:tt) => {
        impl IntoResponse for $t {
            type Output = Value;

            fn into_response(self) -> ResponsePayload<'static, Self::Output> {
                let value = serde_json::to_value(self);
                match value {
                    Ok(v) => ResponsePayload::result(v),
                    Err(_) => ResponsePayload::Error(ErrorCode::InternalError.into()),
                }
            }
        }
    };
}

impl_into_response!(PluginMetaData);
impl_into_response!(ConnectionDescriptor);

/// The `Plugin` trait defines the interface for a plugin that can be used in the speedtest controller.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Configures the plugin with the given proxy configuration.
    async fn setup_proxy(&self, proxy: serde_json::Value) -> Result<ConnectionDescriptor>;

    /// Initialize the plugin
    async fn init(&self) -> Result<()>;

    /// Retrieves the metadata associated with the plugin.
    async fn metadata(&self) -> Result<PluginMetaData>;

    /// Retrieves the list of tests supported by the plugin.
    async fn tests(&self) -> Result<Vec<TestDescriptor>>;

    /// Runs the specified test using the given proxy configuration.
    async fn run_test(
        &self,
        test: &TestDescriptor,
        proxy: &ConnectionDescriptor,
    ) -> Result<serde_json::Value>;

    /// Retrieves the list of data transformations supported by the plugin.
    async fn data_transforms(&self) -> Result<Vec<DataTransformDescriptor>>;

    /// Parses the given connection string and returns a list of supported protocols.
    async fn parse_protocol(&self, connection_string: &str) -> Result<Vec<ProtocolDescriptor>>;
}

#[async_trait]
impl<T, ImplPlugin> Plugin for T
where
    T: std::ops::Deref<Target = ImplPlugin> + Send + Sync,
    ImplPlugin: Plugin,
{
    async fn setup_proxy(&self, proxy: serde_json::Value) -> Result<ConnectionDescriptor> {
        self.deref().setup_proxy(proxy).await
    }

    async fn init(&self) -> Result<()> {
        self.deref().init().await
    }

    async fn metadata(&self) -> Result<PluginMetaData> {
        self.deref().metadata().await
    }

    async fn tests(&self) -> Result<Vec<TestDescriptor>> {
        self.deref().tests().await
    }

    async fn run_test(
        &self,
        test: &TestDescriptor,
        proxy: &ConnectionDescriptor,
    ) -> Result<serde_json::Value> {
        self.deref().run_test(test, proxy).await
    }

    async fn data_transforms(&self) -> Result<Vec<DataTransformDescriptor>> {
        self.deref().data_transforms().await
    }

    async fn parse_protocol(&self, connection_string: &str) -> Result<Vec<ProtocolDescriptor>> {
        self.deref().parse_protocol(connection_string).await
    }
}
