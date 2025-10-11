use std::time::Duration;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient, Interceptor};
use crate::common::AnyResult;
use crate::globals::constants::*;

/// gRPC连接池 - 简化版本
pub struct GrpcConnectionPool {
    endpoint: String,
    x_token: Option<String>,
}

impl GrpcConnectionPool {
    pub fn new(endpoint: String, x_token: Option<String>) -> Self {
        Self { endpoint, x_token }
    }

    pub async fn create_connection(&self) -> AnyResult<GeyserGrpcClient<impl Interceptor>> {
        let builder = GeyserGrpcClient::build_from_shared(self.endpoint.clone())?
            .x_token(self.x_token.clone())?
            // Use the type re-exported by yellowstone_grpc_client
            .tls_config(ClientTlsConfig::new().with_native_roots())?
            // or: .tls_config(ClientTlsConfig::new().with_webpki_roots())?
            .max_decoding_message_size(DEFMAXDECODINGSIZE)
            .connect_timeout(Duration::from_secs(DEFTIMEOUTCONNECT))
            .timeout(Duration::from_secs(DEFTIMEOUTREQUEST));

        Ok(builder.connect().await?)
    }
}