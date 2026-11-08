import { ApiError } from "./client";

/** Reads a field-level validation message from the API's `details` object, if present. */
export function fieldErrorsFrom(error: unknown): Record<string, string> {
  if (!(error instanceof ApiError) || !error.details || typeof error.details !== "object") return {};
  const out: Record<string, string> = {};
  for (const [field, value] of Object.entries(error.details as Record<string, unknown>)) {
    if (Array.isArray(value) && typeof value[0] === "string") out[field] = value[0];
  }
  return out;
}

export function errorMessage(error: unknown, fallback = "Something went wrong. Please try again."): string {
  if (error instanceof ApiError) return error.message;
  if (error instanceof TypeError) return "We could not reach the server. Check your connection.";
  return fallback;
}
