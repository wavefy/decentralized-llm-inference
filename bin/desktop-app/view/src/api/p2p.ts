import { P2PStartRequest } from "@/lib/p2p";

export const fetchP2pStatus = async (baseControlUrl: string) => {
  const res = await fetch(`${baseControlUrl}/p2p/v1/status`);

  if (!res.ok) {
    const errorResponse = await res.json();
    const errorMessage = `Connection to vLLM control server failed: ${errorResponse.error} [${res.status} ${res.statusText}]`;
    throw new Error(errorMessage);
  }

  const data = await res.json();
  return data;
};

export const startP2pSession = async (
  baseControlUrl: string,
  req: P2PStartRequest
) => {
  const response = await fetch(baseControlUrl + "/p2p/v1/start", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(req),
  });
  return response.json();
};

export const stopP2pSession = async (baseControlUrl: string) => {
  const response = await fetch(baseControlUrl + "/p2p/v1/stop", {
    method: "POST",
  });
  return response.json();
};

export const suggestP2pLayers = async (
  baseControlUrl: string,
  model: string,
  layers: number,
  maxLayers: number
) => {
  const response = await fetch(
    baseControlUrl +
    `/p2p/v1/suggest_layers?model=${model}&layers=${layers}&max_layers=${maxLayers}`
  );
  return response.json();
};
