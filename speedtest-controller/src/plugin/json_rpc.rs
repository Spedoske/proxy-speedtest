use jsonrpsee::async_client::ClientBuilder;
use jsonrpsee::client_transport::ws::{Url, WsTransportClientBuilder};
use jsonrpsee::rpc_params;
use jsonrpsee::{async_client::Client, core::client::ClientT};

use super::{ConnectionDescriptor, Plugin, Result, TestDescriptor};
pub struct JSONRPCPlugin {
    client: Client,
}

#[async_trait::async_trait]
impl Plugin for JSONRPCPlugin {
    async fn configure(
        &mut self,
        proxy: serde_json::Value,
        config: serde_json::Value,
    ) -> Result<ConnectionDescriptor> {
        let result = self
            .client
            .request("configure", rpc_params![proxy, config])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn metadata(&self) -> Result<super::PluginMetaData> {
        let result = self.client.request("metadata", rpc_params![]).await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn tests(&self) -> Result<Vec<super::TestDescriptor>> {
        let result = self.client.request("tests", rpc_params![]).await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn run_test(
        &mut self,
        test: &TestDescriptor,
        proxy: &ConnectionDescriptor,
    ) -> Result<serde_json::Value> {
        let result = self
            .client
            .request("run_test", rpc_params![&test.name, proxy])
            .await?;
        Ok(result)
    }

    async fn data_transforms(&self) -> Result<Vec<super::DataTransformDescriptor>> {
        let result = self
            .client
            .request("data_transforms", rpc_params![])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    async fn parse_protocol(
        &self,
        connection_string: &str,
    ) -> Result<Option<super::ProtocolDescriptor>> {
        let result = self
            .client
            .request("parse_protocol", rpc_params![connection_string])
            .await?;
        Ok(serde_json::from_value(result)?)
    }
}

impl JSONRPCPlugin {
    pub async fn new(endpoint: &str) -> Result<Self> {
        let uri = Url::parse(&format!("ws://{}", endpoint))?;

        let (tx, rx) = WsTransportClientBuilder::default().build(uri).await?;
        let client: Client = ClientBuilder::default().build_with_tokio(tx, rx);
        Ok(JSONRPCPlugin { client })
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use crate::plugin::PluginMetaData;
    use jsonrpsee::{
        server::{RpcModule, Server},
        types::ErrorObject,
    };

    use super::*;

    async fn create_rpc_service() -> anyhow::Result<SocketAddr> {
        let server = Server::builder().build("127.0.0.1:0").await?;
        let mut module = RpcModule::new(());
        module.register_method("metadata", |_, _| PluginMetaData {
            name: "foo".to_owned(),
        })?;
        module.register_method(
            "configure",
            |params, _| -> std::result::Result<_, ErrorObject> {
                let params: (ConnectionDescriptor,) = params.parse()?;
                println!("{:?}", params);
                Ok(ConnectionDescriptor {
                    http: Some("http://127.0.0.1:1234".to_owned()),
                    socks5: Some("socks5://127.0.0.1:2345".to_owned()),
                    tun: false,
                })
            },
        )?;
        let addr = server.local_addr()?;

        let handle = server.start(module);

        // In this example we don't care about doing shutdown so let's it run forever.
        // You may use the `ServerHandle` to shut it down or manage it yourself.
        tokio::spawn(handle.stopped());
        Ok(addr)
    }

    #[tokio::test]
    async fn it_works() {
        let addr = create_rpc_service().await.unwrap();
        let mut plugin = JSONRPCPlugin::new(&format!("{}", addr)).await.unwrap();
        assert_eq!(plugin.metadata().await.unwrap().name, "foo");
        plugin
            .configure(
                serde_json::to_value(ConnectionDescriptor {
                    http: Some("http://127.0.0.1:1234".to_owned()),
                    socks5: Some("socks5://127.0.0.1:2345".to_owned()),
                    tun: false,
                })
                .unwrap(),
                serde_json::Value::Null,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_works_on_gost() {
        let addr = "127.0.0.1:54040";
        let mut plugin = JSONRPCPlugin::new(&format!("{}", addr)).await.unwrap();
        println!("{}", plugin.metadata().await.unwrap().name);
        println!(
            "{:?}",
            plugin
                .configure(serde_json::Value::Null, serde_json::Value::Null)
                .await
                .unwrap()
        );
    }
}
