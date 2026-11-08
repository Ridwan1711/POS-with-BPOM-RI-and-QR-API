"use client";

import { useState } from "react";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { canManage, useCurrentStore } from "@/features/auth/session";
import { errorMessage } from "@/lib/api/errors";
import { useCategories, useSaveCategory } from "@/lib/api/queries";
import type { CategoryView } from "@/lib/api/types";
import { categorySchema, type CategoryValues } from "@/lib/schemas";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardHeader, PageHeader } from "@/components/ui/card";
import { Dialog } from "@/components/ui/dialog";
import { EmptyState, Spinner } from "@/components/ui/empty-state";
import { TextField } from "@/components/ui/field";
import { formatNumber } from "@/lib/format";

export function CategoriesScreen() {
  const store = useCurrentStore();
  const editable = store ? canManage(store.role) : false;
  const categories = useCategories();
  const save = useSaveCategory();
  const [message, setMessage] = useState<{ tone: "success" | "danger"; text: string } | null>(null);
  const [editing, setEditing] = useState<CategoryView | "new" | null>(null);

  async function toggle(category: CategoryView) {
    setMessage(null);
    try {
      await save.mutateAsync({ id: category.id, isActive: !category.is_active });
    } catch (err) {
      setMessage({ tone: "danger", text: errorMessage(err) });
    }
  }

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        title="Categories"
        description="Group products to find them faster in the cashier and reports."
        actions={
          editable ? (
            <Button onClick={() => setEditing("new")}>Add category</Button>
          ) : null
        }
      />
      {message ? <Alert tone={message.tone === "success" ? "success" : "danger"}>{message.text}</Alert> : null}

      <Card className="overflow-hidden">
        {categories.isPending ? (
          <Spinner />
        ) : categories.data?.length === 0 ? (
          <EmptyState title="No categories yet" description="Add categories such as Food, Drinks or Cleaning." />
        ) : (
          <ul className="divide-y divide-border">
            {categories.data?.map((category) => (
              <li key={category.id} className="flex flex-wrap items-center justify-between gap-3 px-5 py-3.5">
                <div className="min-w-0">
                  <p className="font-medium">{category.name}</p>
                  <p className="text-xs text-muted">{formatNumber(category.product_count)} active products</p>
                </div>
                <div className="flex items-center gap-2">
                  <Badge tone={category.is_active ? "success" : "neutral"}>{category.is_active ? "Active" : "Hidden"}</Badge>
                  {editable ? (
                    <>
                      <Button variant="ghost" size="sm" onClick={() => setEditing(category)}>
                        Rename
                      </Button>
                      <Button variant="outline" size="sm" onClick={() => toggle(category)} disabled={save.isPending}>
                        {category.is_active ? "Hide" : "Show"}
                      </Button>
                    </>
                  ) : null}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {editing ? (
        <CategoryDialog
          key={editing === "new" ? "new" : editing.id}
          category={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            setMessage({ tone: "success", text: "Category saved." });
          }}
        />
      ) : null}
    </div>
  );
}

function CategoryDialog({
  category,
  onClose,
  onSaved,
}: {
  category: CategoryView | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const save = useSaveCategory();
  const [error, setError] = useState<string | null>(null);
  const form = useForm<CategoryValues>({
    resolver: zodResolver(categorySchema),
    defaultValues: { name: category?.name ?? "" },
  });

  async function onSubmit(values: CategoryValues) {
    setError(null);
    try {
      await save.mutateAsync({ id: category?.id, name: values.name.trim() });
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Dialog
      open
      onClose={onClose}
      title={category ? "Rename category" : "New category"}
      size="sm"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" form="category-form" disabled={save.isPending}>
            {save.isPending ? "Saving…" : "Save"}
          </Button>
        </>
      }
    >
      <form id="category-form" onSubmit={form.handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <TextField label="Name" autoFocus {...form.register("name")} error={form.formState.errors.name?.message} />
      </form>
    </Dialog>
  );
}
