"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import type { ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { LayoutDashboard, LogOut, Package, Receipt, Settings, ShoppingCart, Tags, Warehouse } from "lucide-react";
import { api, setActiveStoreId } from "@/lib/api/client";
import type { SessionResponse } from "@/lib/api/types";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

const NAV = [
  { href: "/pos", label: "Cashier", icon: ShoppingCart },
  { href: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "/products", label: "Products", icon: Package },
  { href: "/inventory", label: "Inventory", icon: Warehouse },
  { href: "/transactions", label: "Transactions", icon: Receipt },
  { href: "/categories", label: "Categories", icon: Tags },
  { href: "/settings", label: "Settings", icon: Settings },
];

export function AppShell({ session, children }: { session: SessionResponse; children: ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const queryClient = useQueryClient();
  const store = session.stores[0];

  async function signOut() {
    try {
      await api.post("/auth/logout");
    } finally {
      setActiveStoreId(null);
      queryClient.clear();
      router.replace("/login");
    }
  }

  return (
    <div className="flex min-h-dvh flex-col md:flex-row">
      <aside className="no-print hidden w-60 shrink-0 flex-col border-r border-border bg-surface md:flex">
        <div className="border-b border-border px-5 py-4">
          <p className="truncate text-sm font-semibold text-foreground">{store?.name ?? "No store"}</p>
          <p className="mt-0.5 truncate text-xs text-muted">{session.user.full_name}</p>
        </div>
        <nav aria-label="Main" className="flex flex-1 flex-col gap-0.5 p-3">
          {NAV.map((item) => (
            <NavLink key={item.href} href={item.href} active={pathname.startsWith(item.href)} icon={item.icon}>
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="border-t border-border p-3">
          <Button variant="ghost" className="w-full justify-start" onClick={signOut}>
            <LogOut className="size-4" aria-hidden="true" />
            Sign out
          </Button>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="no-print flex items-center justify-between gap-3 border-b border-border bg-surface px-4 py-3 md:hidden">
          <p className="truncate text-sm font-semibold">{store?.name ?? "POS"}</p>
          <Button variant="ghost" size="sm" onClick={signOut} aria-label="Sign out">
            <LogOut className="size-4" aria-hidden="true" />
          </Button>
        </header>
        <nav aria-label="Main mobile" className="no-print flex gap-1 overflow-x-auto border-b border-border bg-surface px-3 py-2 md:hidden">
          {NAV.map((item) => (
            <NavLink key={item.href} href={item.href} active={pathname.startsWith(item.href)} icon={item.icon} compact>
              {item.label}
            </NavLink>
          ))}
        </nav>
        <div className="no-print hidden items-center justify-end gap-3 border-b border-border bg-surface px-6 py-2.5 text-sm md:flex">
          <span className="text-muted">{session.user.email}</span>
          {store ? <Badge tone="primary">{store.role}</Badge> : null}
        </div>
        <main className="mx-auto w-full max-w-7xl flex-1 px-4 py-6 sm:px-6">{children}</main>
      </div>
    </div>
  );
}

function NavLink({
  href,
  active,
  icon: Icon,
  children,
  compact,
}: {
  href: string;
  active: boolean;
  icon: typeof Package;
  children: ReactNode;
  compact?: boolean;
}) {
  return (
    <Link
      href={href}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex shrink-0 items-center gap-2.5 rounded-md text-sm font-medium transition-colors",
        compact ? "px-3 py-1.5" : "px-3 py-2",
        active ? "bg-primary-soft text-primary" : "text-muted hover:bg-surface-muted hover:text-foreground",
      )}
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      {children}
    </Link>
  );
}
