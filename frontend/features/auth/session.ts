"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, setActiveStoreId } from "@/lib/api/client";
import type { Role, SessionResponse } from "@/lib/api/types";

export const SESSION_KEY = ["session"] as const;

/**
 * The signed-in user and their stores. Sets the active store for every API request, so
 * it must resolve before any store-scoped query runs (the app layout waits for it).
 */
export function useSession() {
  return useQuery({
    queryKey: SESSION_KEY,
    queryFn: async () => {
      const session = await api.get<SessionResponse>("/auth/me");
      setActiveStoreId(session.stores[0]?.store_id ?? null);
      return session;
    },
    retry: false,
    staleTime: 5 * 60_000,
  });
}

export interface CurrentStore {
  storeId: string;
  storeName: string;
  role: Role;
}

export function useCurrentStore(): CurrentStore | null {
  const { data } = useSession();
  const store = data?.stores[0];
  return store ? { storeId: store.store_id, storeName: store.name, role: store.role } : null;
}

/** Managers and owners can change products, stock and categories. */
export function canManage(role: Role): boolean {
  return role === "manager" || role === "owner";
}

export function isOwner(role: Role): boolean {
  return role === "owner";
}

export function useInvalidateSession() {
  const queryClient = useQueryClient();
  return () => queryClient.removeQueries({ queryKey: SESSION_KEY });
}
