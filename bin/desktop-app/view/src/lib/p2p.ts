export interface P2PStatus {
  model?: {
    model: string;
    from_layer: number;
    to_layer: number;
  };
  spending?: number;
  earning?: number;
  balance?: number;
  peers?: number;
  sessions?: number;
  topup_balance?: number;
  status: string;
  address?: string;
}

export interface P2PStartRequest {
  model: string;
  from_layer: number;
  to_layer: number;
  private_key: string;
}

// export interface P2PStopRequest { }

export interface P2PSuggestLayersRes {
  distribution: number[];
  min_layers?: number;
  from_layer?: number;
  to_layer?: number;
}

export function getPrivateKey() {
  return localStorage.getItem("dllm_pk");
}
