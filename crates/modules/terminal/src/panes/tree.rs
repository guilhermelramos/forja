//! Árvore de painéis e abas de um terminal de bancada.
//!
//! Uma folha é um painel com abas; uma divisão guarda a orientação e a
//! proporção. Fechar um painel promove o irmão para o lugar do pai, sem
//! rearranjar os demais.

use serde::{Deserialize, Serialize};

/// Identificador estável de terminal.
pub type TerminalId = u64;
/// Identificador estável de painel.
pub type PaneId = u64;

/// Máximo de painéis por bancada. Cada painel pinta e lê o próprio terminal,
/// e o renderizador por software usado nos testes não mantém um quadro abaixo
/// de 10 s acima disso. Refs #72, #74.
pub const MAX_PANES: usize = 8;

/// Orientação de uma divisão.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDir {
    /// Lado a lado; o novo painel fica à direita.
    Columns,
    /// Empilhado; o novo painel fica abaixo.
    Rows,
}

/// Folha com abas ou divisão entre dois painéis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PaneNode {
    /// Painel com uma ou mais abas.
    Leaf {
        /// Id estável do painel.
        pane: PaneId,
        /// Terminais na ordem das abas.
        tabs: Vec<TerminalId>,
        /// Índice da aba ativa.
        active: usize,
    },
    /// Divisão arrastável.
    Split {
        /// Orientação.
        dir: SplitDir,
        /// Fração ocupada pelo primeiro lado.
        ratio: f32,
        /// Primeiro lado.
        first: Box<PaneNode>,
        /// Segundo lado.
        second: Box<PaneNode>,
    },
}

impl PaneNode {
    /// Folha com uma única aba.
    pub fn leaf(pane: PaneId, terminal: TerminalId) -> Self {
        Self::Leaf {
            pane,
            tabs: vec![terminal],
            active: 0,
        }
    }

    /// Transforma esta folha em uma divisão com o novo terminal.
    pub fn split(&mut self, dir: SplitDir, pane: PaneId, terminal: TerminalId) {
        let previous = std::mem::replace(self, Self::leaf(pane, terminal));
        *self = Self::Split {
            dir,
            ratio: 0.5,
            first: Box::new(previous),
            second: Box::new(Self::leaf(pane, terminal)),
        };
    }

    /// Folha com o id pedido, somente leitura.
    pub fn leaf_ref(&self, wanted: PaneId) -> Option<&Self> {
        match self {
            Self::Leaf { pane, .. } => (*pane == wanted).then_some(self),
            Self::Split { first, second, .. } => {
                first.leaf_ref(wanted).or_else(|| second.leaf_ref(wanted))
            }
        }
    }

    /// Folha com o id pedido.
    pub fn leaf_mut(&mut self, wanted: PaneId) -> Option<&mut Self> {
        match self {
            Self::Leaf { pane, .. } => (*pane == wanted).then_some(self),
            Self::Split { first, second, .. } => first
                .leaf_mut(wanted)
                .or_else(|| second.leaf_mut(wanted)),
        }
    }

    /// Ids de todas as folhas, na ordem visual.
    pub fn leaves(&self, out: &mut Vec<PaneId>) {
        match self {
            Self::Leaf { pane, .. } => out.push(*pane),
            Self::Split { first, second, .. } => {
                first.leaves(out);
                second.leaves(out);
            }
        }
    }

    /// Quantas folhas (painéis visíveis) a árvore tem.
    pub fn leaf_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// Mantém as primeiras `max` folhas na ordem visual e remove o resto. Um
    /// layout restaurado com mais painéis que [`MAX_PANES`] não pode deixar
    /// todo quadro pesado demais. Refs #72, #74.
    pub fn pruned(mut self, max: usize) -> Self {
        let mut leaves = Vec::new();
        self.leaves(&mut leaves);
        while leaves.len() > max {
            let Some(last) = leaves.pop() else { break };
            let tree = std::mem::replace(&mut self, Self::leaf(0, 0));
            match tree.remove(last) {
                Some(tree) => self = tree,
                None => break,
            }
        }
        self
    }

    /// Terminais de todas as abas.
    pub fn terminals(&self, out: &mut Vec<TerminalId>) {
        match self {
            Self::Leaf { tabs, .. } => out.extend(tabs),
            Self::Split { first, second, .. } => {
                first.terminals(out);
                second.terminals(out);
            }
        }
    }

    /// Remove uma folha; devolve `None` quando era a única.
    pub fn remove(self, wanted: PaneId) -> Option<Self> {
        match self {
            Self::Leaf { pane, .. } if pane == wanted => None,
            Self::Leaf { .. } => Some(self),
            Self::Split {
                dir,
                ratio,
                first,
                second,
            } => match (first.remove(wanted), second.remove(wanted)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    dir,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (first, second) => first.or(second),
            },
        }
    }

    /// Tamanho mínimo em pixels, somando divisores.
    pub fn minimum_size(&self) -> forja_kernel::ui_contract::egui::Vec2 {
        use forja_kernel::ui_contract::egui::vec2;
        match self {
            Self::Leaf { .. } => vec2(140.0, 120.0),
            Self::Split {
                dir, first, second, ..
            } => {
                let a = first.minimum_size();
                let b = second.minimum_size();
                match dir {
                    SplitDir::Columns => vec2(a.x + b.x + 5.0, a.y.max(b.y)),
                    SplitDir::Rows => vec2(a.x.max(b.x), a.y + b.y + 5.0),
                }
            }
        }
    }
}

/// Id da primeira folha, usado como chave de divisores.
pub fn first_pane(node: &PaneNode) -> PaneId {
    match node {
        PaneNode::Leaf { pane, .. } => *pane,
        PaneNode::Split { first, .. } => first_pane(first),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> PaneNode {
        let mut root = PaneNode::leaf(0, 10);
        root.split(SplitDir::Columns, 1, 11);
        root.leaf_mut(1)
            .expect("pane")
            .split(SplitDir::Rows, 2, 12);
        root
    }

    #[test]
    fn split_and_remove_keep_leaves_consistent() {
        let mut root = tree();
        let mut leaves = Vec::new();
        root.leaves(&mut leaves);
        assert_eq!(leaves, vec![0, 1, 2]);
        let mut terminals = Vec::new();
        root.terminals(&mut terminals);
        assert_eq!(terminals, vec![10, 11, 12]);
        root = root.remove(2).expect("root remains");
        let mut leaves = Vec::new();
        root.leaves(&mut leaves);
        assert_eq!(leaves, vec![0, 1]);
        root = root.remove(0).expect("second leaf remains");
        let mut leaves = Vec::new();
        root.leaves(&mut leaves);
        assert_eq!(leaves, vec![1]);
        assert!(root.remove(1).is_none());
    }

    #[test]
    fn pruning_keeps_the_first_leaves_and_counts_them() {
        let mut root = PaneNode::leaf(0, 10);
        for index in 1..MAX_PANES + 4 {
            let id = index as u64;
            root.split(SplitDir::Rows, id, 10 + id);
        }
        assert_eq!(root.leaf_count(), MAX_PANES + 4);
        root = root.pruned(MAX_PANES);
        assert_eq!(root.leaf_count(), MAX_PANES);
        let mut leaves = Vec::new();
        root.leaves(&mut leaves);
        assert_eq!(leaves.first().copied(), Some(0));
        assert_eq!(leaves.last().copied(), Some(MAX_PANES as u64 - 1));
    }
}
