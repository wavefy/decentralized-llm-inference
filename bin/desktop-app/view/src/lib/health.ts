export interface SwarmHealth {
  model: string;
  total_layers: number;
  nodes: {
    id: string;
    info: {
      layers: {
        start: number;
        end: number;
      };
      stats: {
        network_in_bytes: number;
        network_out_bytes: number;
        token_in_sum: number;
        token_out_sum: number;
        network_in_bps: number;
        network_out_bps: number;
        token_in_tps: number;
        token_out_tps: number;
      }
    };
  }[];
}
