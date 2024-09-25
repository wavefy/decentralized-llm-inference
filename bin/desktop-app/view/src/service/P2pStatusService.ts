import { P2P_START_ENDPOINT, P2P_STATUS_ENDPOINT, P2P_STOP_ENDPOINT } from "../constants/apiEndpoints";

export interface P2pStartRequest {
    model: string,
    from_layer: number,
    to_layer: number,
    private_key: string,
}

export interface P2pStopRequest { }

export interface P2pConfig {
    model: string,
    from_layer: number,
    to_layer: number,
}

export interface P2pStatus {
    model?: P2pConfig,
    spent: number,
    earned: number,
    balance: number,
    peers: number,
    sessions: number,
    status: string,
}

export class P2pStatusService {
    static async getP2pStatus(): Promise<P2pStatus> {
        const response = await fetch(P2P_STATUS_ENDPOINT);
        return response.json();
    }

    static async startP2p(request: P2pStartRequest): Promise<P2pStatus> {
        const response = await fetch(P2P_START_ENDPOINT, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(request),
        });
        return response.json();
    }

    static async stopP2p(): Promise<P2pStatus> {
        const response = await fetch(P2P_STOP_ENDPOINT, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
        });
        return response.json();
    }
}