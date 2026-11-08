"use client";

import { useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { api, ApiError } from "@/lib/api/client";
import { loginSchema, type LoginValues } from "@/lib/schemas";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { TextField } from "@/components/ui/field";
import { Card } from "@/components/ui/card";
import { useInvalidateSession } from "./session";

/** Only same-site relative paths are accepted as a post-login destination. */
export function safeNextPath(value: string | null): string {
  if (!value || !value.startsWith("/") || value.startsWith("//")) return "/pos";
  return value;
}

export function LoginForm() {
  const router = useRouter();
  const params = useSearchParams();
  const invalidateSession = useInvalidateSession();
  const [serverError, setServerError] = useState<string | null>(null);
  const form = useForm<LoginValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: "", password: "" },
  });

  async function onSubmit(values: LoginValues) {
    setServerError(null);
    try {
      await api.post("/auth/login", values);
      invalidateSession();
      router.replace(safeNextPath(params.get("next")));
      router.refresh();
    } catch (error) {
      setServerError(
        error instanceof ApiError ? error.message : "We could not reach the server. Check your connection and try again.",
      );
    }
  }

  return (
    <Card className="p-6">
      <form onSubmit={form.handleSubmit(onSubmit)} noValidate className="flex flex-col gap-4">
        {serverError ? <Alert tone="danger">{serverError}</Alert> : null}
        <TextField
          label="Email"
          type="email"
          autoComplete="username"
          autoFocus
          {...form.register("email")}
          error={form.formState.errors.email?.message}
        />
        <TextField
          label="Password"
          type="password"
          autoComplete="current-password"
          {...form.register("password")}
          error={form.formState.errors.password?.message}
        />
        <Button type="submit" size="lg" disabled={form.formState.isSubmitting} className="mt-2 w-full">
          {form.formState.isSubmitting ? "Signing in…" : "Sign in"}
        </Button>
      </form>
    </Card>
  );
}
