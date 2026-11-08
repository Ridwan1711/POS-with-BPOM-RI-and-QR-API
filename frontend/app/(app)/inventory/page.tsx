import { Suspense } from "react";
import { InventoryScreen } from "@/features/inventory/inventory-screen";
import { Spinner } from "@/components/ui/empty-state";

export const metadata = { title: "Inventory" };

export default function InventoryPage() {
  return (
    <Suspense fallback={<Spinner />}>
      <InventoryScreen />
    </Suspense>
  );
}
