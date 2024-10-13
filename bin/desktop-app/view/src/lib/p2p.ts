export interface ModelStatus {
  status: string;
  model: string;
  from_layer: number;
  to_layer: number;
  peers: string[];
  sessions: number;
  wallet: {
    spending: number;
    earning: number;
    balance?: number;
    topup_balance?: number;
    address: string;
  }
}

export interface P2PStatus {
  models: ModelStatus[];
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
