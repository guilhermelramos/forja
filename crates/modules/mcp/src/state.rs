//! Fila de aprovação das chamadas MCP de escrita.
//!
//! O bridge não decide nada: ele pede `mcp.authorize`, a janela mostra o
//! modal e `mcp.approval` devolve a decisão. Sem resposta até o timeout, a
//! chamada é negada.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Limite de pedidos aguardando decisão.
pub const MAX_PENDING: usize = 5;

/// Pedido aguardando o usuário.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Request {
    /// Correlação usada pelo bridge.
    pub id: String,
    /// Nome do cliente MCP.
    pub client: String,
    /// Método chamado.
    pub tool: String,
    /// Resumo sem valores sensíveis.
    pub summary: String,
    /// `write` ou `destructive`.
    pub effect: String,
    /// Criação em ms.
    pub created_ms: i64,
}

/// Resultado de `mcp.authorize`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authorize {
    /// Já aprovado (allow until close).
    Approved,
    /// Fila cheia.
    QueueFull,
    /// Modal aberto; o bridge deve consultar `mcp.approval`.
    Pending(String),
}

/// Estado de um pedido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Ainda aguardando.
    Pending,
    /// Aprovado.
    Approved,
    /// Recusado.
    Denied,
    /// Sem resposta no prazo.
    TimedOut,
}

impl ApprovalStatus {
    /// Texto estável do RPC.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::TimedOut => "timed_out",
        }
    }
}

/// Estado do módulo.
#[derive(Default)]
pub struct McpState {
    pending: VecDeque<Request>,
    decisions: BTreeMap<String, bool>,
    allow_until_close: BTreeSet<(String, String)>,
    next_id: u64,
}

impl McpState {
    /// Limpa pedidos vencidos antes de aceitar outro.
    pub fn cleanup(&mut self, timeout_s: i64, now_ms: i64) {
        let timeout_ms = timeout_s.max(1) * 1000;
        let expired: Vec<String> = self
            .pending
            .iter()
            .filter(|request| now_ms - request.created_ms >= timeout_ms)
            .map(|request| request.id.clone())
            .collect();
        for id in expired {
            self.pending.retain(|request| request.id != id);
            self.decisions.insert(id, false);
        }
    }

    /// Pede aprovação, a menos que o cliente já tenha liberado a ferramenta
    /// até fechar.
    pub fn authorize(
        &mut self,
        client: &str,
        tool: &str,
        summary: &str,
        effect: &str,
        now_ms: i64,
    ) -> Authorize {
        if self
            .allow_until_close
            .contains(&(client.to_owned(), tool.to_owned()))
        {
            return Authorize::Approved;
        }
        if self.pending.len() >= MAX_PENDING {
            return Authorize::QueueFull;
        }
        self.next_id += 1;
        let id = format!("mcp-approval-{}-{}", std::process::id(), self.next_id);
        self.pending.push_back(Request {
            id: id.clone(),
            client: client.to_owned(),
            tool: tool.to_owned(),
            summary: summary.to_owned(),
            effect: effect.to_owned(),
            created_ms: now_ms,
        });
        Authorize::Pending(id)
    }

    /// Primeiro pedido da fila.
    pub fn front(&self) -> Option<&Request> {
        self.pending.front()
    }

    /// Quantidade pendente.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Decide um pedido. `session_only` libera o par cliente/ferramenta até o
    /// fim do app.
    pub fn decide(
        &mut self,
        id: &str,
        approved: bool,
        session_only: bool,
    ) -> Option<Request> {
        let index = self.pending.iter().position(|request| request.id == id)?;
        let request = self.pending.remove(index)?;
        if approved && session_only {
            self.allow_until_close
                .insert((request.client.clone(), request.tool.clone()));
        }
        self.decisions.insert(id.to_owned(), approved);
        Some(request)
    }

    /// Consulta o estado; aplica o timeout.
    pub fn status(&mut self, id: &str, timeout_s: i64, now_ms: i64) -> ApprovalStatus {
        if let Some(approved) = self.decisions.get(id) {
            return if *approved {
                ApprovalStatus::Approved
            } else {
                ApprovalStatus::Denied
            };
        }
        let Some(request) = self.pending.iter().find(|request| request.id == id) else {
            return ApprovalStatus::Denied;
        };
        if now_ms - request.created_ms >= timeout_s.max(1) * 1000 {
            self.pending.retain(|request| request.id != id);
            self.decisions.insert(id.to_owned(), false);
            return ApprovalStatus::TimedOut;
        }
        ApprovalStatus::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_waits_for_a_decision_and_can_be_allowed_once() {
        let mut state = McpState::default();
        let Authorize::Pending(id) = state.authorize("client", "muster.handoff", "task", "write", 0)
        else {
            panic!("must wait");
        };
        assert_eq!(state.pending(), 1);
        assert_eq!(state.status(&id, 120, 1000), ApprovalStatus::Pending);
        assert!(state.decide(&id, true, false).is_some());
        assert_eq!(state.status(&id, 120, 2000), ApprovalStatus::Approved);
        assert_eq!(state.pending(), 0);
    }

    #[test]
    fn deny_and_timeout_are_not_approval() {
        let mut state = McpState::default();
        let Authorize::Pending(denied) = state.authorize("c", "tool", "", "write", 0) else {
            panic!("must wait");
        };
        state.decide(&denied, false, false);
        assert_eq!(state.status(&denied, 120, 1), ApprovalStatus::Denied);
        let Authorize::Pending(expired) = state.authorize("c", "tool", "", "write", 0) else {
            panic!("must wait");
        };
        assert_eq!(state.status(&expired, 1, 1001), ApprovalStatus::TimedOut);
    }

    #[test]
    fn allow_until_close_skips_following_calls() {
        let mut state = McpState::default();
        let Authorize::Pending(id) = state.authorize("c", "muster.handoff", "", "write", 0) else {
            panic!("must wait");
        };
        state.decide(&id, true, true);
        assert_eq!(
            state.authorize("c", "muster.handoff", "", "write", 1),
            Authorize::Approved
        );
        assert_ne!(
            state.authorize("c", "other.tool", "", "write", 1),
            Authorize::Approved
        );
    }

    #[test]
    fn the_queue_is_capped_at_five() {
        let mut state = McpState::default();
        for _ in 0..MAX_PENDING {
            assert!(matches!(
                state.authorize("c", "tool", "", "write", 0),
                Authorize::Pending(_)
            ));
        }
        assert_eq!(
            state.authorize("c", "tool", "", "write", 0),
            Authorize::QueueFull
        );
    }
}
