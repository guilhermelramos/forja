//! Transporte roteirizado para testes: responde na ordem em que as respostas foram
//! empilhadas e guarda cada requisição recebida.

use crate::http::{HttpRequest, HttpResponse, Transport};
use forja_contracts::CounterError;
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Default)]
pub struct FakeTransport {
    responses: Mutex<VecDeque<Result<HttpResponse, CounterError>>>,
    seen: Mutex<Vec<HttpRequest>>,
}

impl FakeTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empilha uma resposta HTTP.
    pub fn reply(self, response: HttpResponse) -> Self {
        self.push(Ok(response));
        self
    }

    /// Empilha uma falha de rede.
    pub fn fail(self, error: CounterError) -> Self {
        self.push(Err(error));
        self
    }

    fn push(&self, item: Result<HttpResponse, CounterError>) {
        if let Ok(mut queue) = self.responses.lock() {
            queue.push_back(item);
        }
    }

    /// Requisições recebidas, na ordem.
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.seen.lock().map(|seen| seen.clone()).unwrap_or_default()
    }

    pub fn last_request(&self) -> Option<HttpRequest> {
        self.requests().pop()
    }
}

impl Transport for FakeTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, CounterError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        let next = self.responses.lock().ok().and_then(|mut queue| queue.pop_front());
        next.unwrap_or_else(|| {
            Err(CounterError::Network {
                message: format!("no scripted response for {} {}", request.method.as_str(), request.url),
            })
        })
    }
}
