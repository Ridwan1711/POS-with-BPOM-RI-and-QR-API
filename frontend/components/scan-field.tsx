"use client";

import { forwardRef, useState, type FormEvent } from "react";
import { ScanBarcode } from "lucide-react";
import { cn } from "@/lib/utils";

interface ScanFieldProps {
  label: string;
  placeholder?: string;
  onSubmit: (code: string) => void;
  pending?: boolean;
  /** Clears the field after each submit. Use for the POS, where every scan is a new line. */
  clearOnSubmit?: boolean;
  submitLabel?: string;
  className?: string;
  autoFocus?: boolean;
}

/**
 * Accepts input from a keyboard-wedge barcode scanner, which types the digits and sends Enter,
 * as well as from manual typing. A form submit covers both: Enter inside a form submits natively.
 */
export const ScanField = forwardRef<HTMLInputElement, ScanFieldProps>(function ScanField(
  { label, placeholder, onSubmit, pending = false, clearOnSubmit = false, submitLabel = "Look up", className, autoFocus },
  ref,
) {
  const [value, setValue] = useState("");

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const code = value.trim();
    if (!code || pending) return;
    onSubmit(code);
    if (clearOnSubmit) setValue("");
  }

  return (
    <form onSubmit={handleSubmit} role="search" className={cn("flex flex-col gap-2", className)}>
      <label htmlFor="scan-input" className="text-sm font-medium text-foreground">
        {label}
      </label>
      <div className="flex gap-2">
        <div className="relative min-w-0 flex-1">
          <ScanBarcode className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted" aria-hidden="true" />
          <input
            ref={ref}
            id="scan-input"
            name="code"
            type="text"
            inputMode="text"
            autoComplete="off"
            autoCorrect="off"
            spellCheck={false}
            autoFocus={autoFocus}
            placeholder={placeholder}
            value={value}
            onChange={(event) => setValue(event.target.value)}
            className="h-11 w-full rounded-md border border-border-strong bg-surface pl-9 pr-3 font-mono text-base tracking-wide focus:border-primary focus:outline-none focus:ring-2 focus:ring-ring/40"
          />
        </div>
        <button
          type="submit"
          disabled={pending}
          className="h-11 shrink-0 rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground hover:bg-primary-hover disabled:opacity-50"
        >
          {pending ? "Checking…" : submitLabel}
        </button>
      </div>
    </form>
  );
});
