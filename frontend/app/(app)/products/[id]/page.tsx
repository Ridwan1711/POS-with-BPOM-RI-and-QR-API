"use client";

import { use, Suspense } from "react";
import { ProductDetailScreen } from "@/features/products/product-detail-screen";
import { Spinner } from "@/components/ui/empty-state";

export default function ProductPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  return (
    <Suspense fallback={<Spinner />}>
      <ProductDetailScreen productId={id} />
    </Suspense>
  );
}
