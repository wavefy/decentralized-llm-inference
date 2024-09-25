import React, { useEffect, useState } from 'react';
import { P2pStatus, P2pStatusService } from '../service/P2pStatusService';

const P2pStatusWidget: React.FC = () => {
    const [p2pStatus, setP2pStatus] = useState<P2pStatus | null>(null);

    useEffect(() => {
        P2pStatusService.getP2pStatus().then(setP2pStatus);
        const interval = setInterval(() => {
            P2pStatusService.getP2pStatus().then(setP2pStatus).catch(() => {
                setP2pStatus({
                    balance: 0,
                    earned: 0,
                    spent: 0,
                    peers: 0,
                    sessions: 0,
                    status: "not started"
                });
            });
        }, 1000);
        return () => clearInterval(interval);
    }, []);

    const getStatusColor = (status: string) => {
        switch (status) {
            case 'ready':
                return 'text-green-400';
            case 'incomplete':
                return 'text-yellow-400';
            case 'stopped':
                return 'text-red-400';
            default:
                return 'text-gray-400';
        }
    };

    return (
        <div className="bg-gray-800 p-3 rounded-lg text-sm text-gray-300 border border-gray-700 mb-4">
            <h2 className="font-bold mb-2 text-blue-400">P2P Status</h2>
            <div className="space-y-1">
                <StatusItem label="Balance" value={p2pStatus?.balance ?? 0} />
                <StatusItem label="Earned" value={p2pStatus?.earned ?? 0} />
                <StatusItem label="Spent" value={p2pStatus?.spent ?? 0} />
                <StatusItem label="Peers" value={p2pStatus?.peers ?? 0} />
                <StatusItem label="Sessions" value={p2pStatus?.sessions ?? 0} />
                <StatusItem label="Status" value={p2pStatus?.status ?? "..."} valueClass={getStatusColor(p2pStatus?.status ?? "...")} />
            </div>
        </div>
    );
};

interface StatusItemProps {
    label: string;
    value: string | number;
    valueClass?: string;
}

const StatusItem: React.FC<StatusItemProps> = ({ label, value, valueClass = "" }) => (
    <div className="flex justify-between items-center">
        <span className="text-gray-400">{label}:</span>
        <span className={`font-medium ${valueClass}`}>{value}</span>
    </div>
);

export default P2pStatusWidget;