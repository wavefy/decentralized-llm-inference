export const fetchSwarmHealth = async (registryUrl: string) => {
  const res = await fetch(`${registryUrl}/api/health`);
  if (!res.ok) {
    throw new Error(
      `Connection to vLLM registry server failed: [${res.status} ${res.statusText}]`
    );
  }

  const data = await res.json();
  return data;
};

export const fetchSupportedModels = async (registryUrl: string) => {
  const res = await fetch(`${registryUrl}/api/models`);
  if (!res.ok) {
    throw new Error(
      `Connection to vLLM registry server failed: [${res.status} ${res.statusText}]`
    );
  }

  const data = await res.json();
  return data;
};
