pub mod json_rpc;
use async_trait::async_trait;
use jsonrpsee::client_transport::ws::WsHandshakeError;
use jsonrpsee::types::{error::ErrorCode, ResponsePayload};
use jsonrpsee::IntoResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PluginError {
    #[error("json-rpc client error")]
    ClientError(#[from] jsonrpsee::core::ClientError),
    #[error("JSON RPC returns an invalid response")]
    APIBadResponse(#[from] serde_json::Error),
    #[error("Unable to parse the url")]
    ParseError(#[from] url::ParseError),
    #[error("Unable to perform the ws handshake")]
    WsHandshakeError(#[from] WsHandshakeError),
    #[error("unknown data store error")]
    Unknown,
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq)]
pub enum PluginType {
    #[default]
    JSONRPC,
}

type Result<T> = std::result::Result<T, PluginError>;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct PluginMetaData {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TestDescriptor {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DataTransformDescriptor {
    pub name: String,
    pub accpeted_scheme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProtocolDescriptor {
    pub name: String,
    pub content: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionDescriptor {
    pub http: Option<String>,
    pub socks5: Option<String>,
    pub tun: bool,
}

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

#[async_trait]
pub trait Plugin {
    async fn configure(
        &mut self,
        proxy: serde_json::Value,
        config: serde_json::Value,
    ) -> Result<ConnectionDescriptor>;
    async fn metadata(&self) -> Result<PluginMetaData>;
    async fn tests(&self) -> Result<Vec<TestDescriptor>>;
    async fn run_test(
        &mut self,
        test: &TestDescriptor,
        proxy: &ConnectionDescriptor,
    ) -> Result<serde_json::Value>;
    async fn data_transforms(&self) -> Result<Vec<DataTransformDescriptor>>;
    async fn parse_protocol(&self, connection_string: &str) -> Result<Option<ProtocolDescriptor>>;
}

#[async_trait]
impl<T, ImplPlugin> Plugin for T
where
    T: std::ops::Deref<Target = ImplPlugin> + Send + Sync,
    ImplPlugin: Plugin + Send + Sync,
{
    async fn configure(
        &mut self,
        proxy: serde_json::Value,
        config: serde_json::Value,
    ) -> Result<ConnectionDescriptor> {
        self.deref().configure(proxy, config).await
    }
    async fn metadata(&self) -> Result<PluginMetaData> {
        self.deref().metadata().await
    }
    async fn tests(&self) -> Result<Vec<TestDescriptor>> {
        self.deref().tests().await
    }
    async fn run_test(
        &mut self,
        test: &TestDescriptor,
        proxy: &ConnectionDescriptor,
    ) -> Result<serde_json::Value> {
        self.deref().run_test(test, proxy).await
    }
    async fn data_transforms(&self) -> Result<Vec<DataTransformDescriptor>> {
        self.deref().data_transforms().await
    }
    async fn parse_protocol(&self, connection_string: &str) -> Result<Option<ProtocolDescriptor>> {
        self.deref().parse_protocol(connection_string).await
    }
}
