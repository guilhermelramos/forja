//! Modelo de mensagens, fila de turnos e parser do composer.

use serde::Serialize;
use std::collections::VecDeque;

/// Papel de uma mensagem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ChatRole {
    /// Pessoa.
    User,
    /// Ajudante.
    Assistant,
    /// Ferramenta executada.
    Tool,
    /// Aviso do sistema.
    System,
}

/// Estado de uma chamada de ferramenta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// Em execução.
    Running,
    /// Terminou bem.
    Ok,
    /// Falhou.
    Error,
}

/// Item de lista de tarefas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoItem {
    /// Texto.
    pub text: String,
    /// `pending`, `in_progress` ou `completed`.
    pub status: String,
}

/// Bloco de uma mensagem.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatBlock {
    /// Texto simples ou markdown leve.
    Text(String),
    /// Bloco de código.
    Code {
        /// Linguagem.
        lang: String,
        /// Conteúdo.
        code: String,
    },
    /// Chamada de ferramenta.
    ToolCall {
        /// Nome.
        name: String,
        /// Entrada em JSON/texto.
        input: String,
        /// Saída, quando houver.
        output: Option<String>,
        /// Estado.
        status: ToolStatus,
    },
    /// Edição de arquivo com diff.
    FileEdit {
        /// Caminho.
        path: String,
        /// Diff unificado.
        diff: String,
    },
    /// Lista de tarefas.
    Todo {
        /// Itens.
        items: Vec<TodoItem>,
    },
    /// Raciocínio do modelo.
    Thinking(String),
}

impl ChatBlock {
    /// Resumo curto para a lista do Acervo.
    pub fn summary(&self) -> String {
        match self {
            Self::Text(text) | Self::Thinking(text) => text.chars().take(120).collect(),
            Self::Code { code, .. } => code.chars().take(80).collect(),
            Self::ToolCall { name, .. } => name.clone(),
            Self::FileEdit { path, .. } => path.clone(),
            Self::Todo { items } => items
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        }
    }
}

/// Uso de tokens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatUsage {
    /// Entrada.
    pub input_tokens: u64,
    /// Saída.
    pub output_tokens: u64,
    /// Total.
    pub total_tokens: u64,
}

impl ChatUsage {
    /// Soma outro uso.
    pub fn add(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.total_tokens += other.total_tokens;
    }
}

/// Uma mensagem pronta para desenhar.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatMessage {
    /// Papel.
    pub role: ChatRole,
    /// Blocos.
    pub blocks: Vec<ChatBlock>,
    /// Timestamp em ms.
    pub timestamp_ms: i64,
    /// Uso, quando houver.
    pub usage: Option<ChatUsage>,
}

impl ChatMessage {
    /// Mensagem de texto simples.
    pub fn text(role: ChatRole, text: impl Into<String>) -> Self {
        Self {
            role,
            blocks: vec![ChatBlock::Text(text.into())],
            timestamp_ms: now_ms(),
            usage: None,
        }
    }

    /// Resumo para o índice e as listas.
    pub fn summary(&self) -> String {
        self.blocks
            .iter()
            .map(ChatBlock::summary)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Mescla uso.
    pub fn merge_usage(&mut self, usage: &ChatUsage) {
        match &mut self.usage {
            Some(existing) => existing.add(usage),
            None => self.usage = Some(*usage),
        }
    }
}

/// Timestamp atual em milissegundos.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

/// Estado de uma sessão de chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    /// Sem turno.
    Idle,
    /// Turno em execução.
    Working,
    /// Cancelado pela pessoa.
    Cancelled,
    /// Falhou.
    Error,
}

/// Fila de prompts de acompanhamento.
#[derive(Clone, Debug, Default)]
pub struct TurnQueue {
    pending: VecDeque<String>,
    running: bool,
}

impl TurnQueue {
    /// Coloca um prompt na fila.
    pub fn enqueue(&mut self, prompt: String) {
        if !prompt.trim().is_empty() {
            self.pending.push_back(prompt);
        }
    }

    /// Inicia o próximo, quando ocioso.
    pub fn start_next(&mut self) -> Option<String> {
        if self.running {
            return None;
        }
        let prompt = self.pending.pop_front()?;
        self.running = true;
        Some(prompt)
    }

    /// Marca o turno atual como concluído.
    pub fn finish(&mut self) {
        self.running = false;
    }

    /// Cancela o turno e esvazia a fila.
    pub fn cancel(&mut self) {
        self.running = false;
        self.pending.clear();
    }

    /// True quando há turno ou itens.
    pub fn is_busy(&self) -> bool {
        self.running || !self.pending.is_empty()
    }

    /// Itens pendentes.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// Menções e comandos reconhecidos no composer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposerTokens {
    /// Arquivos mencionados com `@`.
    pub mentions: Vec<String>,
    /// Comandos iniciados com `/`.
    pub commands: Vec<String>,
}

/// Extrai `@arquivo` e `/comando` do texto.
pub fn composer_tokens(text: &str) -> ComposerTokens {
    let mut tokens = ComposerTokens::default();
    for word in text.split_whitespace() {
        if let Some(mention) = word.strip_prefix('@') {
            if !mention.is_empty() {
                tokens.mentions.push(mention.to_owned());
            }
        } else if let Some(command) = word.strip_prefix('/') {
            if !command.is_empty() {
                tokens.commands.push(command.to_owned());
            }
        }
    }
    tokens
}

/// Filtra nomes de arquivo para o seletor `@`.
pub fn filter_mentions<'a>(files: &'a [String], query: &str, limit: usize) -> Vec<&'a String> {
    let query = query.to_ascii_lowercase();
    files
        .iter()
        .filter(|file| file.to_ascii_lowercase().contains(&query))
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_runs_one_turn_and_keeps_followups() {
        let mut queue = TurnQueue::default();
        queue.enqueue("one".into());
        queue.enqueue("two".into());
        assert_eq!(queue.start_next().as_deref(), Some("one"));
        assert_eq!(queue.start_next(), None);
        queue.finish();
        assert_eq!(queue.start_next().as_deref(), Some("two"));
        queue.cancel();
        assert!(!queue.is_busy());
    }

    #[test]
    fn composer_reads_mentions_and_commands() {
        let tokens = composer_tokens("/review @src/main.rs and @README.md");
        assert_eq!(tokens.commands, vec!["review"]);
        assert_eq!(tokens.mentions, vec!["src/main.rs", "README.md"]);
    }

    #[test]
    fn filter_is_case_insensitive() {
        let files = vec!["src/main.rs".into(), "README.md".into()];
        assert_eq!(filter_mentions(&files, "MAIN", 10), vec!["src/main.rs"]);
    }
}
