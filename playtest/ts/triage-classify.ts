import fs from "node:fs";
import { OpenRouter } from "@openrouter/sdk";

interface UsageSummary {
  promptTokens: number;
  completionTokens: number;
  reasoningTokens: number;
  totalTokens: number;
  cost: number | null;
}

const MANDATORY_RELACE_MODEL = "z-ai/glm-5.3-flash";
const MANDATORY_RELACE_PROVIDERS = ["relace/fp4"];

function parseArgs(argv: string[]): { model: string; timeoutMs: number; promptFile: string | null } {
  let model = "";
  let timeoutMs = 90_000;
  let promptFile: string | null = null;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--model" && i + 1 < argv.length) {
      model = argv[++i].trim();
    } else if (arg === "--timeout-ms" && i + 1 < argv.length) {
      timeoutMs = Number(argv[++i]);
    } else if (arg === "--prompt-file" && i + 1 < argv.length) {
      promptFile = argv[++i];
    } else {
      throw new Error(`Unknown or incomplete argument: ${arg}`);
    }
  }

  if (!model) {
    throw new Error("--model is required");
  }
  if (!Number.isFinite(timeoutMs) || timeoutMs < 1) {
    throw new Error(`Invalid --timeout-ms: ${timeoutMs}`);
  }
  return { model, timeoutMs, promptFile };
}

function normalizeOpenRouterModel(model: string): string {
  return model.startsWith("openrouter/") ? model.slice("openrouter/".length) : model;
}

async function readPrompt(promptFile: string | null): Promise<string> {
  if (promptFile) {
    return fs.readFileSync(promptFile, "utf8");
  }
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function formatError(err: unknown): string {
  if (err && typeof err === "object") {
    const record = err as Record<string, unknown>;
    const status = typeof record.statusCode === "number" ? `HTTP ${record.statusCode}` : "";
    const msg = typeof record.message === "string" ? record.message : String(err);
    const body = typeof record.body === "string" && record.body ? ` | body: ${record.body}` : "";
    return [status, msg].filter(Boolean).join(": ") + body;
  }
  return String(err);
}

async function main(): Promise<void> {
  const { model: rawModel, timeoutMs, promptFile } = parseArgs(process.argv.slice(2));
  const apiKey = process.env.OPENROUTER_API_KEY?.trim() ?? "";
  if (!apiKey) {
    throw new Error("OPENROUTER_API_KEY is required");
  }

  const prompt = await readPrompt(promptFile);
  if (!prompt.trim()) {
    throw new Error("Prompt input is empty");
  }

  const model = normalizeOpenRouterModel(rawModel);
  const openrouter = new OpenRouter({
    apiKey,
    timeoutMs,
  });

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  // @openrouter/sdk v1.5.32 maps camelCase `allowFallbacks` to snake_case
  // `allow_fallbacks` in ProviderPreferences$outboundSchema; using snake_case
  // directly would be stripped by Zod before sending.
  const provider =
    model === MANDATORY_RELACE_MODEL
      ? {
          only: MANDATORY_RELACE_PROVIDERS,
          allowFallbacks: false,
        }
      : undefined;

  const messages = [{ role: "user" as const, content: prompt }];

  try {
    const stream = await openrouter.chat.send(
      {
        chatRequest: {
          model,
          messages,
          stream: true,
          ...(provider ? { provider } : {}),
        },
      },
      { signal: controller.signal, timeoutMs }
    );

    let content = "";
    let lastUsage: UsageSummary | null = null;
    let resolvedModel = model;

    for await (const chunk of stream) {
      if (chunk.error) {
        throw new Error(`OpenRouter stream error (${chunk.error.code}): ${chunk.error.message}`);
      }
      if (chunk.model) {
        resolvedModel = chunk.model;
      }
      const delta = chunk.choices?.[0]?.delta?.content;
      if (typeof delta === "string") {
        content += delta;
      }
      if (chunk.usage) {
        lastUsage = {
          promptTokens: chunk.usage.promptTokens ?? 0,
          completionTokens: chunk.usage.completionTokens ?? 0,
          reasoningTokens: chunk.usage.completionTokensDetails?.reasoningTokens ?? 0,
          totalTokens: chunk.usage.totalTokens ?? 0,
          cost: chunk.usage.cost ?? null,
        };
      }
    }

    process.stdout.write(
      JSON.stringify({
        content,
        model: resolvedModel,
        usage: lastUsage ?? {
          promptTokens: 0,
          completionTokens: 0,
          reasoningTokens: 0,
          totalTokens: 0,
          cost: null,
        },
      }) + "\n"
    );
  } finally {
    clearTimeout(timer);
  }
}

main().catch((err) => {
  const isAbort =
    err &&
    typeof err === "object" &&
    (((err as { name?: string }).name === "AbortError") ||
      ((err as { name?: string }).name === "RequestTimeoutError"));
  process.stderr.write(formatError(err) + "\n");
  process.exit(isAbort ? 2 : 1);
});
