export class ApiError extends Error {
  constructor(message, { code, status, detail } = {}) {
    super(message);
    this.name = "ApiError";
    this.code = code || null;
    this.status = status || null;
    this.detail = detail;
  }
}

export async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: { "content-type": "application/json", ...(options.headers || {}) },
    ...options,
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });
  const text = await response.text();
  let data = null;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      throw new ApiError(text, { status: response.status });
    }
  }
  if (!response.ok) {
    throw new ApiError(data?.error || text || response.statusText, {
      code: data?.code,
      status: response.status,
      detail: data?.detail,
    });
  }
  return data;
}

export function requestId() {
  if (typeof crypto.randomUUID === "function") return crypto.randomUUID();
  // Tailnet HTTP origins aren't secure contexts; getRandomValues still works.
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export function shouldApply(meta, current) {
  return (
    meta.generation === current.generation &&
    meta.sessionId === current.sessionId &&
    (meta.sessionGeneration === undefined || meta.sessionGeneration === current.sessionGeneration) &&
    (meta.nodeId === undefined || meta.nodeId === current.selectedId) &&
    (meta.jobId === undefined || meta.jobId === current.jobId)
  );
}

export function shouldApplyAnalysis(meta, current) {
  if (!shouldApply(meta, current)) return false;
  if (meta.analysisId !== current.analysisId) return false;
  if (meta.mode !== current.mode) return false;
  if (Number(meta.temperature) !== Number(current.temperature)) return false;
  if ((meta.checkpoint || null) !== (current.checkpoint || null)) return false;
  return true;
}

export function analysisMatchesRequest(analysis, controls) {
  if (!analysis || analysis.historical) return false;
  if (analysis.mode !== controls.mode) return false;
  if (controls.mode === "sample" && Number(analysis.temperature) !== Number(controls.temperature)) return false;
  if (controls.mode === "greedy" && analysis.temperature != null) return false;
  if (controls.checkpoint && analysis.checkpoint_fingerprint && controls.checkpoint !== analysis.checkpoint_fingerprint) {
    return false;
  }
  return true;
}

export function shouldAssignTree(tree, sessionId, sessionGeneration, current) {
  return Boolean(
    tree &&
      tree.session_id === sessionId &&
      current.sessionId === sessionId &&
      current.sessionGeneration === sessionGeneration
  );
}

export function shouldAssignJob(job, sessionId, sessionGeneration, jobId, current, replace = false) {
  if (!job || job.session_id !== sessionId) return false;
  if (current.sessionId !== sessionId || current.sessionGeneration !== sessionGeneration) return false;
  if (jobId && job.id !== jobId) return false;
  if (!replace && current.jobId && jobId && current.jobId !== jobId) return false;
  return true;
}

export function probabilityByNative(source) {
  const out = new Map();
  if (!source || !Array.isArray(source.native_indices)) return out;
  source.native_indices.forEach((native, index) => {
    if (typeof native !== "number") return;
    const base = source.base_probabilities?.[index];
    const adjusted = source.adjusted_probabilities?.[index];
    out.set(native, {
      base: typeof base === "number" && Number.isFinite(base) ? base : null,
      adjusted: typeof adjusted === "number" && Number.isFinite(adjusted) ? adjusted : null,
    });
  });
  return out;
}
