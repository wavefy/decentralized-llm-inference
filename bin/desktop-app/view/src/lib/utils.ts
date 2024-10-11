import { useEffect, useState } from "react";

import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const basePath = import.meta.env.VITE_VLLM_URL ?? "";
export const controlBasePath = import.meta.env.VITE_VLLM_CONTROLS_URL ?? "";
export const contractAddress = import.meta.env.VITE_VLLM_CONTRACT ?? "";
export const noditApiUrl = import.meta.env.VITE_NODIT_GQL_API ?? "";

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
