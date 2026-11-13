---
name: example-custom-skill
description: Template and reference for creating a new agent skill in the open SKILL.md format.
---

# Skill Title: One-Line Summary

Provide a concise overview of what this skill teaches the agent, why it exists, and under what conditions it should be applied.

---

## 1. Prerequisites and Context

- List relevant tools, CLI utilities, or environment configurations required.
- Identify the file formats, database schemas, or APIs being operated upon.

---

## 2. Core Workflow Steps

1. **Step 1: Discovery & Inspection**
   Describe how to query the current system state before making modifications.
   ```sh
   # Example command
   command-name --status
   ```

2. **Step 2: Execution**
   Detail the exact transformations, code patterns, or commands to run.
   Provide concrete snippets demonstrating the desired approach.

3. **Step 3: Verification & Quality Gates**
   State the exact commands to prove correctness and avoid regressions.
   ```sh
   # Example test command
   command-name --test
   ```

---

## 3. Best Practices and Edge Cases

- **Constraint 1:** Describe boundaries the agent must respect (e.g. read-only paths).
- **Edge Case:** Detail how to recover if an external service fails or returns an unexpected error code.
