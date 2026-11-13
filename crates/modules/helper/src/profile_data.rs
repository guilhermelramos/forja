//! Perfis embutidos. O texto é o mesmo de `content/helpers/*.toml`.

pub const BUILTIN: &[&str] = &[
    include_str!("../../../../content/helpers/aider.toml"),
    include_str!("../../../../content/helpers/amp.toml"),
    include_str!("../../../../content/helpers/antigravity.toml"),
    include_str!("../../../../content/helpers/claude-code.toml"),
    include_str!("../../../../content/helpers/cline.toml"),
    include_str!("../../../../content/helpers/codex.toml"),
    include_str!("../../../../content/helpers/continue.toml"),
    include_str!("../../../../content/helpers/copilot.toml"),
    include_str!("../../../../content/helpers/crush.toml"),
    include_str!("../../../../content/helpers/cursor-agent.toml"),
    include_str!("../../../../content/helpers/gemini.toml"),
    include_str!("../../../../content/helpers/goose.toml"),
    include_str!("../../../../content/helpers/kimi-code.toml"),
    include_str!("../../../../content/helpers/kiro.toml"),
    include_str!("../../../../content/helpers/opencode.toml"),
    include_str!("../../../../content/helpers/qwen-code.toml"),
];
