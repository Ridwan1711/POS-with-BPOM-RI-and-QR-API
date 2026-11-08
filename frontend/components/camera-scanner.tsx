"use client";

import { useEffect, useRef, useState } from "react";
import { Dialog } from "@/components/ui/dialog";
import { Alert } from "@/components/ui/alert";

type DetectedBarcode = { rawValue: string };
interface BarcodeDetectorLike {
  detect: (source: CanvasImageSource) => Promise<DetectedBarcode[]>;
}
type BarcodeDetectorCtor = new (options: { formats: string[] }) => BarcodeDetectorLike;

const FORMATS = ["ean_13", "ean_8", "upc_a", "upc_e", "qr_code", "code_128"];

function detectorConstructor(): BarcodeDetectorCtor | null {
  if (typeof window === "undefined") return null;
  const ctor = (window as unknown as { BarcodeDetector?: BarcodeDetectorCtor }).BarcodeDetector;
  return ctor ?? null;
}

/** True when the browser can read barcodes from the camera. Hidden in the UI otherwise. */
export function isCameraScanSupported(): boolean {
  return detectorConstructor() !== null && typeof navigator !== "undefined" && Boolean(navigator.mediaDevices?.getUserMedia);
}

/**
 * Optional camera scanning. It never blocks keyboard scanning, and it degrades to a message
 * when permission is refused or no camera exists.
 */
export function CameraScannerDialog({
  open,
  onClose,
  onDetected,
}: {
  open: boolean;
  onClose: () => void;
  onDetected: (code: string) => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    const Detector = detectorConstructor();
    if (!Detector) {
      setError("This browser cannot read barcodes from the camera. Use a barcode scanner or type the code.");
      return;
    }
    setError(null);
    let stream: MediaStream | null = null;
    let timer: number | undefined;
    let cancelled = false;

    (async () => {
      try {
        stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: "environment" }, audio: false });
        if (cancelled) return;
        const video = videoRef.current;
        if (!video) return;
        video.srcObject = stream;
        await video.play();
        const detector = new Detector({ formats: FORMATS });
        timer = window.setInterval(async () => {
          if (cancelled || video.readyState < 2) return;
          try {
            const codes = await detector.detect(video);
            const value = codes[0]?.rawValue;
            if (value && !cancelled) {
              cancelled = true;
              onDetected(value);
            }
          } catch {
            // Keep scanning. A single failed frame is not an error.
          }
        }, 300);
      } catch {
        if (!cancelled) {
          setError("Camera access was refused or no camera is available. Use a barcode scanner or type the code.");
        }
      }
    })();

    return () => {
      cancelled = true;
      if (timer) window.clearInterval(timer);
      stream?.getTracks().forEach((track) => track.stop());
    };
  }, [open, onDetected]);

  return (
    <Dialog open={open} onClose={onClose} title="Scan with camera" description="Hold the barcode steady inside the frame." size="md">
      {error ? (
        <Alert tone="warning">{error}</Alert>
      ) : (
        <div className="overflow-hidden rounded-lg bg-black">
          <video ref={videoRef} muted playsInline className="aspect-video w-full object-cover" aria-label="Camera preview" />
        </div>
      )}
    </Dialog>
  );
}
