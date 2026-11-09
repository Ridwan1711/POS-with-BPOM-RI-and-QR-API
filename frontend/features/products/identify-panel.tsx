"use client";

import Link from "next/link";
import { forwardRef } from "react";
import type { IdentificationStatus, IdentifyResponse, ProviderRecord } from "@/lib/api/types";
import { formatDateTime, formatIdr } from "@/lib/format";
import { Alert } from "@/components/ui/alert";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScanField } from "@/components/scan-field";
import { SUGGESTION_FIELD_LABELS } from "./suggestion-form";

const STATUS_COPY: Record<IdentificationStatus, { tone: "info" | "success" | "warning" | "danger"; title: string }> = {
  existing_product: { tone: "warning", title: "This code is already in your store" },
  identified: { tone: "success", title: "Product found in external data" },
  partially_identified: { tone: "info", title: "Partial information found" },
  not_found: { tone: "info", title: "No external record for this code" },
  provider_unavailable: { tone: "warning", title: "External sources could not be checked" },
  invalid_code: { tone: "danger", title: "This barcode is not valid" },
  unsupported_format: { tone: "warning", title: "This code cannot identify a product" },
};

const CONFIDENCE_TONE: Record<string, Tone> = { low: "warning", medium: "info", high: "success" };

const OUTCOME_LABEL: Record<ProviderRecord["outcome"], { label: string; tone: Tone }> = {
  found: { label: "Found data", tone: "success" },
  not_found: { label: "No record", tone: "neutral" },
  unavailable: { label: "Unavailable", tone: "warning" },
  not_configured: { label: "Not configured", tone: "neutral" },
};

interface IdentifyPanelProps {
  result: IdentifyResponse | null;
  pending: boolean;
  error: string | null;
  onLookup: (code: string) => void;
  onCameraOpen: () => void;
  cameraSupported: boolean;
  onRetry: () => void;
}

/** Step 1 of product creation: get a code, look it up, and show exactly what was found. */
export const IdentifyPanel = forwardRef<HTMLInputElement, IdentifyPanelProps>(function IdentifyPanel(
  { result, pending, error, onLookup, onCameraOpen, cameraSupported, onRetry },
  ref,
) {
  return (
    <div className="flex flex-col gap-5">
      <ScanField
        ref={ref}
        label="Barcode or QR code"
        placeholder="Scan now, or type the number and press Enter"
        onSubmit={onLookup}
        pending={pending}
        autoFocus
      />
      {cameraSupported ? (
        <Button variant="outline" onClick={onCameraOpen} className="w-full">
          Scan with camera
        </Button>
      ) : null}

      {error ? <Alert tone="danger">{error}</Alert> : null}

      {result ? <ResultBlock result={result} onRetry={onRetry} /> : null}

      {!result && !pending ? (
        <p className="text-sm text-muted">
          Scan a product barcode. We check your store first, then external product data. You always review the details before anything is saved.
        </p>
      ) : null}
    </div>
  );
});

function ResultBlock({ result, onRetry }: { result: IdentifyResponse; onRetry: () => void }) {
  const copy = STATUS_COPY[result.status];
  return (
    <div className="flex flex-col gap-4" aria-live="polite">
      <Alert tone={copy.tone} title={copy.title}>
        {result.message}
        {result.from_cache ? (
          <span className="mt-1 block text-xs opacity-80">Saved result from {formatDateTime(result.looked_up_at)}.</span>
        ) : null}
      </Alert>

      {result.status === "existing_product" && result.existing_product ? (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-border bg-surface px-4 py-3">
          <div className="min-w-0">
            <p className="truncate font-medium">{result.existing_product.name}</p>
            <p className="text-sm text-muted">
              {result.existing_product.stock_quantity} {result.existing_product.unit} in stock ·{" "}
              {formatIdr(result.existing_product.selling_price_idr)}
            </p>
          </div>
          <Link
            href={`/products/${result.existing_product.id}`}
            className="inline-flex h-9 items-center rounded-md border border-border-strong px-3 text-sm font-medium hover:bg-surface-muted"
          >
            Open product
          </Link>
        </div>
      ) : null}

      {result.suggestion ? (
        <div className="flex flex-col gap-3 rounded-md border border-border bg-surface p-4">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm font-medium">Filled in from</p>
            {result.suggestion.sources.map((source) => (
              <Badge key={source.code} tone={CONFIDENCE_TONE[source.confidence]}>
                {source.name} · {source.confidence} confidence
              </Badge>
            ))}
          </div>
          <div className="flex flex-wrap gap-1.5" aria-label="Fields supplied by the source">
            {result.suggestion.fields.map((field) => (
              <Badge key={field} tone="primary">
                {SUGGESTION_FIELD_LABELS[field] ?? field}
              </Badge>
            ))}
          </div>
          <p className="text-xs text-muted">
            Check every field before saving. External data can be incomplete or wrong. Prices, stock and SKU are never filled from outside sources.
          </p>
          {result.suggestion.sources.some((source) => source.code === "open_food_facts") ? (
            <p className="text-xs text-muted">
              Data from Open Food Facts, licensed under ODbL. Attribution required. Photos are CC BY-SA.
            </p>
          ) : null}
        </div>
      ) : null}

      {result.status === "provider_unavailable" ? (
        <div className="flex flex-wrap items-center gap-3">
          <Button variant="secondary" size="sm" onClick={onRetry}>
            Try the lookup again
          </Button>
          <span className="text-sm text-muted">Or enter the details yourself below.</span>
        </div>
      ) : null}

      {result.providers.length > 0 ? (
        <details className="rounded-md border border-border bg-surface px-4 py-3 text-sm">
          <summary className="cursor-pointer font-medium">Source details ({result.providers.length})</summary>
          <ul className="mt-3 flex flex-col gap-2">
            {result.providers.map((provider) => {
              const outcome = OUTCOME_LABEL[provider.outcome];
              return (
                <li key={provider.code} className="flex flex-wrap items-center gap-2">
                  <span className="font-medium">{provider.name}</span>
                  <Badge tone={outcome.tone}>{outcome.label}</Badge>
                  {provider.message ? <span className="w-full text-xs text-muted">{provider.message}</span> : null}
                </li>
              );
            })}
          </ul>
        </details>
      ) : null}
    </div>
  );
}
