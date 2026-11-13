//! Dados de captura de tela (`FORJA_FIXTURES=1`): uma lista de exemplo e o `SKILL.md` de cada
//! item. Não lê disco nem grava nada; as operações de escrita recusam neste modo.

use crate::model::{Listing, RootInfo, Scope, Skill, FORJA_OWNER};

const PROJECT: &str = "/exemplo/projeto";

/// A lista de exemplo: Forja e dois ajudantes, com um `SKILL.md` quebrado para mostrar o erro.
pub fn listing() -> Listing {
    const FORJA_USER: &str = "/exemplo/.forja/skills";
    const FORJA_PROJECT: &str = "/exemplo/projeto/.forja/skills";
    const CLAUDE_USER: &str = "/exemplo/.claude/skills";
    const CLAUDE_PROJECT: &str = "/exemplo/projeto/.claude/skills";
    const CODEX_USER: &str = "/exemplo/.codex/skills";

    let roots = vec![
        root(FORJA_OWNER, "Forja", Scope::User, FORJA_USER, true),
        root(FORJA_OWNER, "Forja", Scope::Project, FORJA_PROJECT, false),
        root("claude-code", "Claude Code", Scope::User, CLAUDE_USER, true),
        root("claude-code", "Claude Code", Scope::Project, CLAUDE_PROJECT, true),
        root("codex", "Codex", Scope::User, CODEX_USER, false),
    ];
    let skills = vec![
        skill(FORJA_OWNER, "Forja", Scope::User, FORJA_USER, "revisar-pr", Ok("Revisa um pull request e aponta riscos.")),
        skill(FORJA_OWNER, "Forja", Scope::User, FORJA_USER, "traducao-pt", Ok("Traduz textos técnicos para português.")),
        skill(FORJA_OWNER, "Forja", Scope::User, FORJA_USER, "rascunho", Err("skill-error-frontmatter-missing")),
        skill(
            "claude-code",
            "Claude Code",
            Scope::User,
            CLAUDE_USER,
            "gerar-changelog",
            Ok("Escreve o changelog a partir dos commits desde a última tag."),
        ),
        skill(
            "claude-code",
            "Claude Code",
            Scope::Project,
            CLAUDE_PROJECT,
            "deploy-staging",
            Ok("Publica a branch atual no ambiente de homologação."),
        ),
    ];
    Listing {
        roots,
        skills,
        project: Some(PROJECT.to_owned()),
    }
}

/// O `SKILL.md` de exemplo de um item. Qualquer caminho recebe um texto padrão com o nome da
/// pasta.
pub fn body(path: &str) -> String {
    let folder = path.rsplit(['/', '\\']).next().unwrap_or("skill");
    format!(
        "---\nname: {folder}\ndescription: Exemplo para a captura de tela.\n---\n\n# {folder}\n\nPasso 1: leia o contexto do pedido.\n\nPasso 2: aplique as regras da skill e informe o que mudou.\n"
    )
}

fn root(owner: &str, owner_name: &str, scope: Scope, path: &str, exists: bool) -> RootInfo {
    RootInfo {
        owner: owner.to_owned(),
        owner_name: owner_name.to_owned(),
        scope,
        path: path.to_owned(),
        exists,
    }
}

/// Uma skill de exemplo: `Ok` com a descrição, ou `Err` com a chave do erro de front matter.
fn skill(
    owner: &str,
    owner_name: &str,
    scope: Scope,
    root: &str,
    folder: &str,
    outcome: Result<&str, &str>,
) -> Skill {
    let (description, error) = match outcome {
        Ok(description) => (Some(description.to_owned()), None),
        Err(key) => (None, Some(key.to_owned())),
    };
    Skill {
        owner: owner.to_owned(),
        owner_name: owner_name.to_owned(),
        scope,
        folder: folder.to_owned(),
        path: format!("{root}/{folder}"),
        name: description.as_ref().map(|_| folder.to_owned()),
        description,
        error,
    }
}
