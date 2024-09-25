export const OPENAI_ENDPOINT = 'http://localhost:18888';
export const STATUS_ENDPOINT = 'http://localhost:28888';
export const TTS_ENDPOINT = `${OPENAI_ENDPOINT}/v1/audio/speech`;
export const CHAT_COMPLETIONS_ENDPOINT = `${OPENAI_ENDPOINT}/v1/chat/completions`;
export const MODELS_ENDPOINT = `${OPENAI_ENDPOINT}/v1/models`;
export const P2P_STATUS_ENDPOINT = `${STATUS_ENDPOINT}/v1/p2p/status`;
export const P2P_START_ENDPOINT = `${STATUS_ENDPOINT}/v1/p2p/start`;
export const P2P_STOP_ENDPOINT = `${STATUS_ENDPOINT}/v1/p2p/stop`;