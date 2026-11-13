//! HTTP client for review providers, reusing the counter transport and vault.
//!
//! The token is the one the matching counter already stores, so connecting a
//! counter is enough to review pull requests. All calls run inside tasks.

use crate::api::{Provider, Target};
use forja_counter_kit::secrets::service_for;
use forja_counter_kit::{
    default_secrets, default_transport, error_for_status, transport_for_policy, HttpRequest,
    HttpResponse, SecretStore, Transport,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

pub struct Client {
    pub target: Target,
    transport: Arc<dyn Transport>,
    token: String,
}

impl Client {
    pub fn connect(target: Target) -> Result<Self, String> {
        Self::with_store(target, default_secrets())
    }

    pub fn connect_with_policy(target: Target, policy: Option<&Value>) -> Result<Self, String> {
        let mut client = Self::connect(target)?;
        client.transport = transport_for_policy(&client.transport, policy);
        Ok(client)
    }

    pub fn with_store(target: Target, store: Arc<dyn SecretStore>) -> Result<Self, String> {
        let service = match target.provider {
            Provider::GitHub => service_for("counter_github"),
            Provider::GitLab => service_for("counter_gitlab"),
        };
        let token = store
            .get(&service, &target.host)
            .map_err(|error| error.to_string())?
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                format!(
                    "connect the {} counter for {} first",
                    target.provider.id(),
                    target.host
                )
            })?;
        Ok(Self {
            target,
            transport: default_transport(),
            token,
        })
    }

    #[cfg(test)]
    pub fn test_client(target: Target, transport: Arc<dyn Transport>) -> Self {
        Self {
            target,
            transport,
            token: "test-token".into(),
        }
    }

    pub fn send(&self, request: HttpRequest) -> Result<HttpResponse, String> {
        let request = self.authorized(request);
        let response = self
            .transport
            .send(&request)
            .map_err(|error| error.to_string())?;
        if let Some(error) = error_for_status(response.status, &response.headers, &response.body) {
            return Err(error.to_string());
        }
        Ok(response)
    }

    pub fn json<T: DeserializeOwned>(&self, request: HttpRequest) -> Result<T, String> {
        let response = self.send(request)?;
        serde_json::from_str(&response.body).map_err(|error| {
            format!(
                "could not read the {} response: {error}",
                self.target.provider.id()
            )
        })
    }

    pub fn value(&self, request: HttpRequest) -> Result<Value, String> {
        self.json(request)
    }

    pub fn text(&self, request: HttpRequest) -> Result<String, String> {
        Ok(self.send(request)?.body)
    }

    fn authorized(&self, request: HttpRequest) -> HttpRequest {
        match self.target.provider {
            Provider::GitHub => request
                .bearer(&self.token)
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .header("User-Agent", "Forja"),
            Provider::GitLab => request
                .header("PRIVATE-TOKEN", &self.token)
                .header("Accept", "application/json")
                .header("User-Agent", "Forja"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_counter_kit::fake::FakeTransport;
    use forja_counter_kit::HttpResponse;

    fn target() -> Target {
        Target {
            provider: Provider::GitHub,
            host: "github.com".into(),
            repo: "acme/app".into(),
            api_base: "https://api.github.com".into(),
        }
    }

    #[test]
    fn authorized_requests_carry_the_counter_token_and_headers() {
        let fake = FakeTransport::new().reply(HttpResponse::new(200, "{}"));
        let client = Client::test_client(target(), Arc::new(fake));
        let _ = client.send(HttpRequest::get("https://api.github.com/user"));
    }

    #[test]
    fn a_missing_token_is_reported_before_any_network_call() {
        let store = Arc::new(forja_counter_kit::MemorySecretStore::new());
        let error = match Client::with_store(target(), store) {
            Ok(_) => panic!("a client was created without a token"),
            Err(error) => error,
        };
        assert!(error.contains("connect the github counter"));
    }
}
