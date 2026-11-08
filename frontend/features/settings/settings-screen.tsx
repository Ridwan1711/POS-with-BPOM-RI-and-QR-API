"use client";

import { useEffect, useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { isOwner, useCurrentStore } from "@/features/auth/session";
import { errorMessage } from "@/lib/api/errors";
import { useSaveSettings, useStoreSettings } from "@/lib/api/queries";
import type { StoreSettings } from "@/lib/api/types";
import { formatPercentBps } from "@/lib/format";
import { settingsSchema, type SettingsValues } from "@/lib/schemas";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { Spinner } from "@/components/ui/empty-state";
import { SelectField, TextAreaField, TextField } from "@/components/ui/field";

const TIME_ZONES = [
  { value: "Asia/Jakarta", label: "Western Indonesia (WIB, UTC+7)" },
  { value: "Asia/Makassar", label: "Central Indonesia (WITA, UTC+8)" },
  { value: "Asia/Jayapura", label: "Eastern Indonesia (WIT, UTC+9)" },
];

function toValues(settings: StoreSettings): SettingsValues {
  return {
    name: settings.name,
    address: settings.address ?? "",
    phone: settings.phone ?? "",
    timezone: settings.timezone,
    taxPercent: String(settings.tax_rate_bps / 100),
    allowNegativeStock: settings.allow_negative_stock,
    defaultLowStockThreshold: String(settings.default_low_stock_threshold),
    receiptFooter: settings.receipt_footer ?? "",
  };
}

export function SettingsScreen() {
  const store = useCurrentStore();
  const owner = store ? isOwner(store.role) : false;
  const settings = useStoreSettings();
  const save = useSaveSettings();
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const form = useForm<SettingsValues>({
    resolver: zodResolver(settingsSchema),
    defaultValues: {
      name: "",
      address: "",
      phone: "",
      timezone: "Asia/Jakarta",
      taxPercent: "0",
      allowNegativeStock: false,
      defaultLowStockThreshold: "5",
      receiptFooter: "",
    },
  });

  useEffect(() => {
    if (settings.data) form.reset(toValues(settings.data));
  }, [settings.data, form]);

  if (settings.isPending) return <Spinner label="Loading settings" />;
  if (settings.isError || !settings.data) return <Alert tone="danger">Settings could not be loaded.</Alert>;

  async function onSubmit(values: SettingsValues) {
    setSaved(false);
    setError(null);
    try {
      await save.mutateAsync({
        name: values.name.trim(),
        address: values.address.trim() || null,
        phone: values.phone.trim() || null,
        timezone: values.timezone,
        tax_rate_bps: Math.round(Number(values.taxPercent) * 100),
        allow_negative_stock: values.allowNegativeStock,
        default_low_stock_threshold: Number(values.defaultLowStockThreshold),
        receipt_footer: values.receiptFooter.trim() || null,
      });
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const disabled = !owner;
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Settings" description="Store details, tax, and stock rules." />
      {!owner ? <Alert tone="info">Only the store owner can change these settings. You can view them here.</Alert> : null}
      {saved ? <Alert tone="success">Settings saved.</Alert> : null}
      {error ? <Alert tone="danger">{error}</Alert> : null}

      <Card>
        <CardHeader title="Store" description="Shown on receipts." />
        <form onSubmit={form.handleSubmit(onSubmit)} noValidate className="grid gap-5 p-5 md:grid-cols-2">
          <TextField containerClassName="md:col-span-2" label="Store name" disabled={disabled} {...form.register("name")} error={form.formState.errors.name?.message} />
          <TextField label="Phone" disabled={disabled} {...form.register("phone")} error={form.formState.errors.phone?.message} />
          <SelectField label="Time zone" hint="Sales dates and reports use this time zone." disabled={disabled} {...form.register("timezone")}>
            {TIME_ZONES.map((tz) => (
              <option key={tz.value} value={tz.value}>
                {tz.label}
              </option>
            ))}
          </SelectField>
          <TextAreaField containerClassName="md:col-span-2" label="Address" disabled={disabled} {...form.register("address")} error={form.formState.errors.address?.message} />

          <div className="md:col-span-2 border-t border-border pt-5">
            <p className="text-sm font-semibold">Sales and stock</p>
          </div>
          <TextField
            label="Tax rate (%)"
            hint={`Currently ${formatPercentBps(settings.data.tax_rate_bps)}. Use 0 when prices already include tax.`}
            inputMode="decimal"
            disabled={disabled}
            {...form.register("taxPercent")}
            error={form.formState.errors.taxPercent?.message}
          />
          <TextField
            label="Default low stock alert"
            hint="Used for products without their own alert level."
            inputMode="numeric"
            disabled={disabled}
            {...form.register("defaultLowStockThreshold")}
            error={form.formState.errors.defaultLowStockThreshold?.message}
          />
          <label className="flex items-start gap-3 md:col-span-2">
            <input type="checkbox" className="mt-1 size-4 accent-primary" disabled={disabled} {...form.register("allowNegativeStock")} />
            <span>
              <span className="block text-sm font-medium">Allow selling when stock is zero</span>
              <span className="block text-xs text-muted">Off by default. Turn on only if you want stock to go negative and fix it later.</span>
            </span>
          </label>

          <div className="md:col-span-2">
            <TextAreaField label="Receipt footer" disabled={disabled} {...form.register("receiptFooter")} error={form.formState.errors.receiptFooter?.message} />
          </div>

          {owner ? (
            <div className="md:col-span-2 flex justify-end border-t border-border pt-5">
              <Button type="submit" disabled={save.isPending} size="lg">
                {save.isPending ? "Saving…" : "Save settings"}
              </Button>
            </div>
          ) : null}
        </form>
      </Card>
    </div>
  );
}
