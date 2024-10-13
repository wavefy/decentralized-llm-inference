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
    };
  }[];
}
