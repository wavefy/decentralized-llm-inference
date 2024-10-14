import { useEffect, useState } from "react";

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const basePath = import.meta.env.VITE_VLLM_URL ?? "";
export const controlBasePath = import.meta.env.VITE_VLLM_CONTROLS_URL ?? "";
export const contractAddress = "0xf4289dca4fe79c4e61fe1255d7f47556c38f512b5cf9ddf727f0e44a5c6a6b00";
export const noditApiUrl = import.meta.env.VITE_NODIT_GQL_API ?? "";
export const registryUrl = import.meta.env.VITE_REGISTRY_URL ?? "";
export const appMode: "local" | "cloud" = import.meta.env.VITE_MODE ?? "local";

export function useHasMounted() {
  const [hasMounted, setHasMounted] = useState(false);
  useEffect(() => {
    setHasMounted(true);
  }, []);
  return hasMounted;
}

export function shortenAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}
