import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import { cn } from "@/lib/utils";

type AlertTone = "info" | "success" | "warning" | "danger";

const styles: Record<AlertTone, { box: string; icon: typeof Info }> = {
  info: { box: "border-info/30 bg-info-soft text-info", icon: Info },
  success: { box: "border-success/30 bg-success-soft text-success", icon: CheckCircle2 },
  warning: { box: "border-warning/30 bg-warning-soft text-warning", icon: AlertTriangle },
  danger: { box: "border-danger/30 bg-danger-soft text-danger", icon: XCircle },
};

export function Alert({ tone = "info", title, children, className }: { tone?: AlertTone; title?: string; children?: ReactNode; className?: string }) {
  const { box, icon: Icon } = styles[tone];
  return (
    <div role={tone === "danger" ? "alert" : "status"} className={cn("flex gap-3 rounded-md border px-4 py-3 text-sm", box, className)}>
      <Icon className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
      <div className="min-w-0">
        {title ? <p className="font-medium">{title}</p> : null}
        {children ? <div className={cn(title && "mt-0.5", "text-foreground/90")}>{children}</div> : null}
      </div>
    </div>
  );
}
