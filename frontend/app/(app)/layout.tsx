"use client";

import type { ReactNode } from "react";
import { AppShell } from "@/components/app-shell";
import { Alert } from "@/components/ui/alert";
import { Spinner } from "@/components/ui/empty-state";
import { useSession } from "@/features/auth/session";

export default function WorkspaceLayout({ children }: { children: ReactNode }) {
  const session = useSession();

  if (session.isPending) return <Spinner label="Loading your workspace" />;
  if (session.isError || !session.data) {
    return (
      <div className="mx-auto max-w-lg p-6">
        <Alert tone="danger" title="We could not load your workspace">
          Refresh the page. If the problem continues, check that the API is running.
        </Alert>
      </div>
    );
  }
  if (session.data.stores.length === 0) {
    return (
      <div className="mx-auto max-w-lg p-6">
        <Alert tone="warning" title="No store access">
          Your account is not a member of any store yet. Ask a store owner to add you.
        </Alert>
      </div>
    );
  }
  return <AppShell session={session.data}>{children}</AppShell>;
}
