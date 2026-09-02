/**
 * Cliente sem dependências para integrar um backend Node.js 18+ ao NoteDesk.
 * Mantenha a API key somente no servidor; nunca a publique no JavaScript do navegador.
 */
class NoteDeskIntegrationError extends Error {
  constructor(message, status, response) {
    super(message);
    this.name = "NoteDeskIntegrationError";
    this.status = status;
    this.response = response;
  }
}

class NoteDeskClient {
  constructor({ endpoint, apiKey, sourceSystem, timeoutMs = 10000 }) {
    if (!endpoint) throw new Error("endpoint é obrigatório");
    if (!apiKey) throw new Error("apiKey é obrigatória");
    if (!sourceSystem) throw new Error("sourceSystem é obrigatório");
    this.endpoint = endpoint.replace(/\/$/, "");
    this.apiKey = apiKey;
    this.sourceSystem = sourceSystem;
    this.timeoutMs = timeoutMs;
  }

  async request(path, options = {}) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), this.timeoutMs);
    try {
      const response = await fetch(`${this.endpoint}${path}`, {
        ...options,
        signal: controller.signal,
        headers: {
          "Content-Type": "application/json",
          ...(options.headers || {}),
        },
      });
      const text = await response.text();
      let data = {};
      if (text) {
        try { data = JSON.parse(text); }
        catch { data = { message: text }; }
      }
      if (!response.ok) {
        throw new NoteDeskIntegrationError(
          data.message || `NoteDesk retornou HTTP ${response.status}`,
          response.status,
          data,
        );
      }
      return data;
    } finally {
      clearTimeout(timeout);
    }
  }

  async health() {
    return this.request("/api/v1/health", { method: "GET" });
  }

  async upsertTask(task) {
    if (!task || !task.external_task_id) {
      throw new Error("task.external_task_id é obrigatório");
    }
    if (!task.assigned_user?.id || !task.assigned_user?.name) {
      throw new Error("task.assigned_user.id e task.assigned_user.name são obrigatórios");
    }
    return this.request("/api/v1/tasks/upsert", {
      method: "POST",
      headers: { "X-NoteDesk-Api-Key": this.apiKey },
      body: JSON.stringify({
        ...task,
        source_system: this.sourceSystem,
      }),
    });
  }
}

module.exports = { NoteDeskClient, NoteDeskIntegrationError };

