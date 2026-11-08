"use client";

import Link from "next/link";
import { use } from "react";
import { Receipt as ReceiptIcon, Printer } from "lucide-react";
import { Receipt } from "@/features/pos/receipt";
import { useSale } from "@/lib/api/queries";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, PageHeader } from "@/components/ui/card";
import { Spinner } from "@/components/ui/empty-state";

export default function TransactionPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const sale = useSale(id);

  if (sale.isPending) return <Spinner label="Loading receipt" />;
  if (sale.isError || !sale.data) return <Alert tone="danger">This transaction could not be found.</Alert>;

  return (
    <div className="flex flex-col gap-6">
      <div className="no-print">
        <PageHeader
          title={`Receipt ${sale.data.receipt_number}`}
          description="Prices, discounts and tax are the values recorded at the time of sale."
          actions={
            <>
              <Button variant="outline" onClick={() => window.print()}>
                <Printer className="size-4" aria-hidden="true" />
                Print
              </Button>
              <Link href="/transactions" className="inline-flex h-10 items-center gap-2 px-3 text-sm font-medium text-primary hover:underline">
                <ReceiptIcon className="size-4" aria-hidden="true" />
                All transactions
              </Link>
            </>
          }
        />
      </div>
      <Card className="p-6 print:border-0 print:shadow-none">
        <Receipt sale={sale.data} />
      </Card>
    </div>
  );
}
