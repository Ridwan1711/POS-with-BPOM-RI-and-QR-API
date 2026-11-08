import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ScanField } from "./scan-field";

describe("ScanField", () => {
  // Vitest globals are off, so Testing Library cannot clean up on its own.
  afterEach(cleanup);

  it("submits a scanned code when the scanner sends Enter", async () => {
    const onSubmit = vi.fn();
    render(<ScanField label="Barcode" onSubmit={onSubmit} />);
    const input = screen.getByLabelText("Barcode");
    await userEvent.type(input, "  8991002101029{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("8991002101029");
  });

  it("clears the field after submit when clearOnSubmit is set (POS behaviour)", async () => {
    const onSubmit = vi.fn();
    render(<ScanField label="Barcode" onSubmit={onSubmit} clearOnSubmit />);
    const input = screen.getByLabelText("Barcode") as HTMLInputElement;
    // A scanner types the digits, then sends Enter as its own keystroke.
    await userEvent.type(input, "ROTI-001");
    await userEvent.keyboard("{Enter}");
    expect(onSubmit).toHaveBeenCalledWith("ROTI-001");
    expect(input.value).toBe("");
  });

  it("ignores empty input and does not submit while pending", async () => {
    const onSubmit = vi.fn();
    const { rerender } = render(<ScanField label="Barcode" onSubmit={onSubmit} />);
    await userEvent.type(screen.getByLabelText("Barcode"), "{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();

    rerender(<ScanField label="Barcode" onSubmit={onSubmit} pending />);
    await userEvent.type(screen.getByLabelText("Barcode"), "123{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
  });
});
