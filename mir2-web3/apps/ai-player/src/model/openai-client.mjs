import { createModelFetch } from "./http-transport.mjs";

const ENDPOINT = "https://api.openai.com/v1/responses";
const MAX_RESPONSE = 64 * 1024;

export class ModelCallError extends Error {
  constructor(code, details = {}) { super("The model request could not produce an executable plan."); this.name = "ModelCallError"; this.code = code;
    if (Number.isInteger(details.httpStatus) && details.httpStatus >= 100 && details.httpStatus <= 599) this.httpStatus = details.httpStatus;
    if (typeof details.requestId === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(details.requestId)) this.requestId = details.requestId;
  }
}

export function validModelName(value) {
  return typeof value === "string" && /^[a-zA-Z0-9][a-zA-Z0-9._:-]{0,99}$/.test(value);
}
export function validApiKey(value) {
  return typeof value === "string" && value.length >= 16 && value.length <= 4096 && !/[\s\p{Cc}\p{Cf}]/u.test(value);
}

/** One bounded Responses request; no implicit retry, endpoint override or remote tool. */
export class OpenAIPlanClient {
  #key;
  #fetch;
  #transport;
  constructor({ apiKey, model, fetchImpl, environment = process.env, timeoutMs = 15_000 }) {
    if (!validApiKey(apiKey) || !validModelName(model) || fetchImpl !== undefined && typeof fetchImpl !== "function" || !Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 30_000) throw new ModelCallError("configuration");
    this.#key = apiKey; this.model = model; this.timeoutMs = timeoutMs;
    if (fetchImpl) this.#fetch = fetchImpl;
    else { this.#transport = createModelFetch({ environment }); this.#fetch = this.#transport.fetch; }
  }
  async plan({ instructions, context, schema, signal }) {
    const body = JSON.stringify({ model: this.model, store: false, max_output_tokens: 1024,
      ...(/^gpt-6(?:[.-]|$)/.test(this.model) ? { reasoning: { effort: "none" } } : {}),
      instructions, input: [{ role: "user", content: JSON.stringify(context) }],
      text: { format: { type: "json_schema", name: "alo_objective", strict: true, schema } } });
    if (Buffer.byteLength(body) > 32 * 1024) throw new ModelCallError("context-too-large");
    const timeout = AbortSignal.timeout(this.timeoutMs);
    const combined = signal ? AbortSignal.any([signal, timeout]) : timeout;
    try {
      const response = await this.#fetch(ENDPOINT, { method: "POST", redirect: "error", signal: combined,
        headers: { "Content-Type": "application/json", Authorization: `Bearer ${this.#key}` }, body });
      if (!response.ok) {
        await response.body?.cancel();
        throw new ModelCallError(response.status === 401 || response.status === 403 ? "authentication" : response.status === 429 ? "rate-limit" : "http-error", { httpStatus: response.status, requestId: response.headers.get("x-request-id") });
      }
      if (Number(response.headers.get("content-length")) > MAX_RESPONSE) { await response.body?.cancel(); throw new ModelCallError("response-too-large"); }
      if (!response.body) throw new ModelCallError("invalid-response");
      const reader = response.body.getReader(), chunks = []; let size = 0;
      try {
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          size += value.byteLength;
          if (size > MAX_RESPONSE) { await reader.cancel(); throw new ModelCallError("response-too-large"); }
          chunks.push(value);
        }
      } finally { reader.releaseLock(); }
      const value = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      if (value.status !== "completed" || !Array.isArray(value.output)) throw new ModelCallError("incomplete");
      const contents = value.output.filter(item => item.type === "message" && item.role === "assistant").flatMap(item => Array.isArray(item.content) ? item.content : []);
      if (contents.some(item => item.type === "refusal")) throw new ModelCallError("refused");
      const texts = contents.filter(item => item.type === "output_text" && typeof item.text === "string");
      if (texts.length !== 1) throw new ModelCallError("invalid-response");
      const plan = JSON.parse(texts[0].text);
      const tokens = field => Number.isSafeInteger(value.usage?.[field]) && value.usage[field] >= 0 ? value.usage[field] : null;
      const requestId = response.headers.get("x-request-id");
      return { plan, call: { model: this.model, status: "completed",
        ...(validModelName(value.model) ? { providerModel: value.model } : {}),
        ...(typeof value.id === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(value.id) ? { responseId: value.id } : {}),
        ...(typeof requestId === "string" && /^[A-Za-z0-9_-]{1,160}$/.test(requestId) ? { requestId } : {}),
        inputTokens: tokens("input_tokens"), outputTokens: tokens("output_tokens") } };
    } catch (error) {
      if (error instanceof ModelCallError) throw error;
      throw new ModelCallError(signal?.aborted ? "cancelled" : timeout.aborted ? "timeout" : error?.code === "MODEL_TRANSPORT_UNAVAILABLE" && error.reason === "tls-trust" ? "tls-trust" : "transport-or-format");
    }
  }
  close() { this.#key = null; this.#transport?.close().catch(() => {}); }
}
